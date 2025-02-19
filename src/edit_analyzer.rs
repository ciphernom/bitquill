use crate::EditStats;

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::f64;

use crate::Delta;

#[derive(Serialize, Deserialize, Clone)]
struct EditMetrics {
    timestamp: f64,
    delta: Delta,
    change_size: u32,
    time_since_last_edit: Option<f64>,
}

#[derive(Serialize, Deserialize)]
struct EditThresholds {
    base_typing_interval: f64,
    thinking_time: f64,
    word_boundary_pause: f64,
    fast_burst_threshold: f64,
    burst_variance: f64,
    consistent_pattern_window: u32,
    max_consistent_count: u32,
    max_word_length: u32,
    window_size: u32,
    min_sample_size: u32,
}

#[wasm_bindgen]
pub struct EditAnalyzer {
    edit_history: Vec<EditMetrics>,
    thresholds: EditThresholds,
    interval_history: Vec<f64>,
    pattern_buffer: Vec<f64>,
}

#[wasm_bindgen]
impl EditAnalyzer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        EditAnalyzer {
            edit_history: Vec::new(),
            thresholds: EditThresholds {
                             // Basic human typing parameters - Minimal restrictions
                base_typing_interval: 1.0,  
                thinking_time: 10.0,         
                word_boundary_pause: 10.0,  
                fast_burst_threshold: 1.0,  
                burst_variance: 1000.0,         
                consistent_pattern_window: 100,
                max_consistent_count: 1000,
                max_word_length: 10000,
                window_size: 5,
                min_sample_size: 2,
            },
            interval_history: Vec::new(),
            pattern_buffer: Vec::new(),
        }
    }

    fn calculate_delta_size(&self, delta: &Delta) -> u32 {
        let mut size = 0;
        let mut cursor_pos = 0;
        
        for op in &delta.ops {
            if let Some(retain) = &op.retain {
                cursor_pos += *retain;
            } else if let Some(insert) = &op.insert {
                if let Some(s) = insert.as_str() {
                    size += s.chars().count() as u32;
                } else {
                    size += 1; // For non-string inserts (embeds)
                }
            } else if let Some(delete) = &op.delete {
                size += delete;
            }
        }
        size
    }


    fn analyze_typing_pattern(&mut self, new_interval: f64) -> bool {
        // Store interval
        self.interval_history.push(new_interval);
        if self.interval_history.len() > self.thresholds.window_size as usize {
            self.interval_history.remove(0);
        }

        // Need minimum samples for analysis
        if self.interval_history.len() < self.thresholds.min_sample_size as usize {
            return true;
        }

        // Calculate geometric mean
        let log_sum: f64 = self.interval_history.iter()
            .map(|&x| safe_ln(f64::max(x, 1.0)))
            .sum();
        let geometric_mean = f64::exp(log_sum / self.interval_history.len() as f64);

        // Check patterns
        let too_fast = geometric_mean < self.thresholds.fast_burst_threshold;
        let too_consistent = self.check_consistency();
        let no_natural_pauses = self.check_pause_pattern();

        !(too_fast || too_consistent || no_natural_pauses)
    }

    fn check_consistency(&self) -> bool {
        let mut consistent_count = 1;
        let recent_intervals = &self.interval_history[f64::max(
            0.0,
            self.interval_history.len() as f64 - self.thresholds.consistent_pattern_window as f64
        ) as usize..];

        for window in recent_intervals.windows(2) {
            if f64::abs(window[0] - window[1]) < self.thresholds.burst_variance {
                consistent_count += 1;
                if consistent_count > self.thresholds.max_consistent_count {
                    return true;
                }
            } else {
                consistent_count = 1;
            }
        }
        false
    }

    fn check_pause_pattern(&self) -> bool {
        let recent_intervals = &self.interval_history[f64::max(
            0.0,
            self.interval_history.len() as f64 - self.thresholds.window_size as f64
        ) as usize..];

        let pause_count = recent_intervals.iter()
            .filter(|&&interval| interval > self.thresholds.word_boundary_pause)
            .count();

        pause_count < (recent_intervals.len() / self.thresholds.max_word_length as usize)
    }

    fn analyze_edit(&self, metrics: &EditMetrics) -> serde_json::Value {
        let mut suspicious_patterns = Vec::new();
        let mut is_valid = true;
        let mut cursor_jumps = 0;
        let mut last_cursor_pos = 0;

        // Basic validation checks
        if metrics.change_size == 0 {
            return json!({
                "isValid": true,
                "patterns": ["Empty edit"]
            });
        }

        // Analyze cursor movement and content changes
        for op in &metrics.delta.ops {
            if let Some(retain) = &op.retain {
                let jump_size = (*retain as i64 - last_cursor_pos as i64).abs();
                if jump_size > 20 { // Threshold for suspicious cursor jumps
                    cursor_jumps += 1;
                }
                last_cursor_pos = *retain;
            }
        }

        // Check for large cursor jumps combined with rapid edits
        if cursor_jumps > 3 && metrics.time_since_last_edit.map_or(false, |t| t < self.thresholds.base_typing_interval) {
            suspicious_patterns.push("Suspicious cursor movement pattern");
            is_valid = false;
        }

        // Check for large content changes
        if metrics.change_size > self.thresholds.max_word_length {
            suspicious_patterns.push("Large content change detected");
            is_valid = false;
        }

        // Check timing if we have previous edit timing
        if let Some(time_since_last) = metrics.time_since_last_edit {
            if metrics.change_size > 1 && time_since_last < self.thresholds.base_typing_interval {
                // Allow fast edits only if they're near the previous cursor position
                if cursor_jumps > 0 {
                    suspicious_patterns.push("Content changed too quickly with cursor movement");
                    is_valid = false;
                }
            }
        }

        // If no suspicious patterns were detected, add a default success pattern
        if suspicious_patterns.is_empty() {
            suspicious_patterns.push("Normal edit pattern");
        }

        json!({
            "isValid": is_valid,
            "patterns": suspicious_patterns,
            "cursorJumps": cursor_jumps
        })
    }


    #[wasm_bindgen]
    pub fn record_edit(&mut self, delta_str: &str, prev_delta_str: &str, timestamp: f64) -> Result<JsValue, JsError> {
        let delta: Delta = serde_json::from_str(delta_str)?;
        let _prev_delta: Delta = serde_json::from_str(prev_delta_str)?;
        
        let time_since_last_edit = if !self.edit_history.is_empty() {
            Some(timestamp - self.edit_history.last().unwrap().timestamp)
        } else {
            None
        };

        let metrics = EditMetrics {
            timestamp,
            delta: delta.clone(),
            change_size: self.calculate_delta_size(&delta),
            time_since_last_edit,
        };

        if let Some(interval) = time_since_last_edit {
            if !self.analyze_typing_pattern(interval) {
                return Ok(serde_wasm_bindgen::to_value(&json!({
                    "isValid": false,
                    "patterns": ["Unnatural typing pattern detected"]
                }))?);
            }
        }

        self.edit_history.push(metrics.clone());
        Ok(serde_wasm_bindgen::to_value(&self.analyze_edit(&metrics))?)
    }

    #[wasm_bindgen]
    pub fn get_edit_stats(&self) -> Result<JsValue, JsError> {
        if self.edit_history.is_empty() {
            let default_stats = EditStats::default();
            return Ok(serde_wasm_bindgen::to_value(&default_stats)?);
        }

        let total_edits = self.edit_history.len() as u32;
        let total_time = self.edit_history.last().unwrap().timestamp - self.edit_history[0].timestamp;
        let total_chars: u32 = self.edit_history.iter()
            .map(|edit| edit.change_size)
            .sum();

        // Calculate geometric mean of intervals
        let intervals: Vec<f64> = self.interval_history.iter()
            .filter(|&&x| x > 0.0)
            .copied()
            .collect();

        let geometric_mean = if !intervals.is_empty() {
            let log_sum: f64 = intervals.iter().map(|&x| safe_ln(x)).sum();
            f64::exp(log_sum / intervals.len() as f64)
        } else {
            0.0
        };

        let stats = EditStats {
            total_edits,
            average_interval: if total_edits > 1 { 
                Some(total_time / (total_edits - 1) as f64) 
            } else { 
                None 
            },
            geometric_mean_interval: geometric_mean,
            chars_per_minute: if total_time > 0.0 { 
                (total_chars as f64 / total_time) * 60000.0 
            } else { 
                0.0 
            },
            total_chars,
        };

        Ok(serde_wasm_bindgen::to_value(&stats)?)
    }


    pub fn clear(&mut self) {
        self.edit_history.clear();
        self.interval_history.clear();
        self.pattern_buffer.clear();
    }
}

// Helper function to calculate logarithm safely
fn safe_ln(x: f64) -> f64 {
    if x <= 0.0 {
        f64::NEG_INFINITY
    } else {
        f64::ln(x)
    }
}
