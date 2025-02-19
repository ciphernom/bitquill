use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};  // For SHA-256
use serde_wasm_bindgen;
use wasm_bindgen_futures;
use wasm_bindgen::JsValue;
use serde_json::Value;  // Needed for validate_attributes
use std::collections::HashMap;  // Needed for validate_attributes

// Re-export modules
pub mod timestamps;
pub use timestamps::{OpenTimestamps, Timestamp, TimestampError};

pub mod delta; // Import the delta module
pub use delta::{Delta, Operation}; // Re-export for convenience

pub mod edit_analyzer;
pub use edit_analyzer::EditAnalyzer;

// Import the composeDeltas function from the JS module.
// (Webpack will bundle www/delta_composer.js correctly.)
#[wasm_bindgen(module = "/www/delta_composer.js")]
extern "C" {
    #[wasm_bindgen(js_name = composeDeltas)]
    fn compose_deltas(deltas: &JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EditStats {
    pub total_edits: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_interval: Option<f64>,
    #[serde(default)]
    pub geometric_mean_interval: f64,
    #[serde(default)]
    pub chars_per_minute: f64,
    #[serde(default)]
    pub total_chars: u32,
}

impl Default for EditStats {
    fn default() -> Self {
        EditStats {
            total_edits: 0,
            average_interval: None,
            geometric_mean_interval: 0.0,
            chars_per_minute: 0.0,
            total_chars: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PowResult {
    pub nonce: u64,
    pub hash: String,
    pub duration: f64,
    pub difficulty: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct MerkleNode {
    hash: String,
    delta: Option<Delta>,
    metadata: Option<NodeMetadata>,
    #[serde(skip)]
    #[serde(default)]
    left: Option<Box<MerkleNode>>,
    #[serde(skip)]
    #[serde(default)]
    right: Option<Box<MerkleNode>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetadata {
    pub timestamp: f64,
    pub edit_stats: Option<EditStats>,
    pub pow_result: Option<PowResult>,
    pub is_genesis: Option<bool>,
    pub ots_timestamp: Option<Timestamp>,
}

#[wasm_bindgen]
pub struct MerkleTree {
    leaves: Vec<MerkleNode>,
    root: Option<MerkleNode>,
    document_state: Delta,
    levels: Vec<Vec<MerkleNode>>,
}

const CHECKPOINT_INTERVAL: usize = 100;

#[wasm_bindgen]
impl MerkleTree {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        MerkleTree {
            leaves: Vec::new(),
            root: None,
            document_state: Delta { ops: Vec::new() },
            levels: Vec::new(),
        }
    }

    // Expose compute_hash_js so JavaScript can compute SHA-256 using Rust.
    #[wasm_bindgen]
    pub fn compute_hash_js(&self, data: &str) -> String {
        self.compute_hash(data)
    }

    async fn handle_checkpoint(&mut self) -> Result<(), JsError> {
        web_sys::console::log_1(&"Starting handle_checkpoint function".into());
        
        if self.leaves.len() % CHECKPOINT_INTERVAL == 0 { 
            web_sys::console::log_1(&format!("Creating checkpoint at leaf {}", self.leaves.len()).into());
            web_sys::console::log_1(&format!("Root exists: {}", self.root.is_some()).into());
            let checkpoint_data = if let Some(root) = &self.root {
                web_sys::console::log_1(&"Extracting data from root node".into());
                web_sys::console::log_1(&format!("Root hash: {}", root.hash).into());
                if let Some(ref metadata) = root.metadata {
                    web_sys::console::log_1(&format!("Current metadata - timestamp: {}", metadata.timestamp).into());
                    web_sys::console::log_1(&format!("Current metadata - ots_timestamp exists: {}", metadata.ots_timestamp.is_some()).into());
                } else {
                    web_sys::console::log_1(&"No existing metadata".into());
                }
                Some((
                    root.hash.clone(),
                    root.metadata.as_ref().map(|m| m.timestamp),
                    root.metadata.as_ref().and_then(|m| m.edit_stats.clone()),
                    root.metadata.as_ref().and_then(|m| m.pow_result.clone()),
                    root.metadata.as_ref().and_then(|m| m.is_genesis),
                ))
            } else {
                web_sys::console::log_1(&"No root node found".into());
                None
            };

            if let Some((root_hash, timestamp, edit_stats, pow_result, is_genesis)) = checkpoint_data {
                web_sys::console::log_1(&"Checkpoint data extracted successfully".into());
                web_sys::console::log_1(&format!("About to create OpenTimestamps for hash: {}", root_hash).into());
                let ots = OpenTimestamps::default();
                web_sys::console::log_1(&"Created OpenTimestamps instance".into());
                web_sys::console::log_1(&"About to call stamp()".into());
                match ots.stamp(&root_hash).await {
                    Ok(timestamp_result) => {
                        web_sys::console::log_1(&"Successfully created timestamp".into());
                        web_sys::console::log_1(&format!("Timestamp result - digest: {}", timestamp_result.digest).into());
                        web_sys::console::log_1(&format!("Timestamp result - timestamp: {}", timestamp_result.timestamp).into());
                        web_sys::console::log_1(&"Creating new metadata".into());
                        let new_metadata = NodeMetadata {
                            timestamp: timestamp.unwrap_or(0.0),
                            edit_stats,
                            pow_result,
                            is_genesis,
                            ots_timestamp: Some(timestamp_result),
                        };
                        web_sys::console::log_1(&"Creating new root node".into());
                        let new_root = MerkleNode {
                            hash: root_hash.clone(),
                            delta: None,
                            metadata: Some(new_metadata),
                            left: None,
                            right: None,
                        };
                        web_sys::console::log_1(&"About to replace root node".into());
                        self.root = Some(new_root);
                        web_sys::console::log_1(&"About to rebuild tree".into());
                        match self.rebuild_tree() {
                            Ok(_) => web_sys::console::log_1(&"Tree rebuilt successfully".into()),
                            Err(e) => web_sys::console::warn_1(&format!("Error rebuilding tree: {:?}", e).into()),
                        }
                        web_sys::console::log_1(&format!("Created checkpoint timestamp for root hash: {}", root_hash).into());
                    }
                    Err(e) => {
                        web_sys::console::warn_1(&format!("Failed to create timestamp: {:?}", e).into());
                    }
                }
            }
        } else {
            web_sys::console::log_1(&format!("Skipping checkpoint - leaf count {} not at interval {}", self.leaves.len(), CHECKPOINT_INTERVAL).into());
        }
        
        web_sys::console::log_1(&"Finished handle_checkpoint function".into());
        Ok(())
    }
    
    #[wasm_bindgen]
    pub async fn verify_timestamps(&self) -> Result<JsValue, JsError> {
        let mut results = Vec::new();
        let ots = OpenTimestamps::default();

        for (i, leaf) in self.leaves.iter().enumerate() {
            if let Some(metadata) = &leaf.metadata {
                if let Some(timestamp) = &metadata.ots_timestamp {
                    match ots.verify(timestamp).await {
                        Ok(verified) => {
                            results.push(serde_json::json!({
                                "index": i,
                                "hash": leaf.hash,
                                "timestamp": timestamp.timestamp,
                                "verified": verified,
                            }));
                        }
                        Err(e) => {
                            web_sys::console::warn_1(&format!("Failed to verify timestamp at index {}: {}", i, e).into());
                        }
                    }
                }
            }
        }
        Ok(serde_wasm_bindgen::to_value(&results)?)
    }
    
         fn validate_attributes(&self, attrs: &HashMap<String, Value>, op_index: usize) -> Result<(), JsError> {
            for (key, value) in attrs {
                // Validate attribute key
                if key.is_empty() {
                    return Err(JsError::new(&format!(
                        "Empty attribute key at operation {}", op_index
                    )));
                }

                // Validate common Quill formatting attributes
                match key.as_str() {
                    "bold" | "italic" | "underline" | "strike" => {
                        if !value.is_boolean() {
                            return Err(JsError::new(&format!(
                                "Invalid value for {} at operation {}", key, op_index
                            )));
                        }
                    }
                    "color" | "background" => {
                        if !value.is_string() {
                            return Err(JsError::new(&format!(
                                "Invalid value for {} at operation {}", key, op_index
                            )));
                        }
                    }
                    "header" => {
                        if !value.is_number() {
                            return Err(JsError::new(&format!(
                                "Invalid value for header at operation {}", op_index
                            )));
                        }
                    }
                    // Add other Quill attribute validations as needed
                    _ => () // Allow custom attributes
                }
            }
            Ok(())
        }
    
    
    #[wasm_bindgen]
    pub async fn manual_timestamp(&mut self) -> Result<JsValue, JsError> {
        if let Some(root) = &mut self.root {
            let ots = OpenTimestamps::default();
            let timestamp = ots.stamp(&root.hash).await.map_err(|e| JsError::new(&format!("Timestamp error: {}", e)))?;
            if let Some(metadata) = &mut root.metadata {
                metadata.ots_timestamp = Some(timestamp.clone());
                return Ok(serde_wasm_bindgen::to_value(&timestamp)?);
            }
        }
        Ok(JsValue::NULL)
    }
    

        fn validate_delta(&self, delta: &Delta) -> Result<(), JsError> {
        // Check basic structure
        if delta.ops.is_empty() {
            return Err(JsError::new("Delta contains no operations"));
        }

        // Validate each operation
        for (i, op) in delta.ops.iter().enumerate() {
            // Check operation type validity
            let has_operation = op.insert.is_some() || op.delete.is_some() || op.retain.is_some();
            if !has_operation {
                return Err(JsError::new(&format!(
                    "Operation at index {} missing required properties", i
                )));
            }

            // Validate attributes if present
            if let Some(ref attrs) = op.attributes {
                self.validate_attributes(attrs, i)?;
            }

            // Validate insert content
            if let Some(ref insert) = op.insert {
                match insert {
                    Value::String(s) if s.is_empty() => {
                        return Err(JsError::new(&format!(
                            "Empty string insert at index {}", i
                        )));
                    }
                    Value::Object(obj) if obj.is_empty() => {
                        return Err(JsError::new(&format!(
                            "Empty embed object at index {}", i
                        )));
                    }
                    Value::String(_) | Value::Object(_) => (), // Valid cases
                    _ => {
                        return Err(JsError::new(&format!(
                            "Invalid insert value type at index {}", i
                        )));
                    }
                }
            }

            // Validate delete/retain values
            if let Some(val) = op.delete.or(op.retain) {
                if val == 0 {
                    return Err(JsError::new(&format!(
                        "Zero-length delete/retain at index {}", i
                    )));
                }
            }
        }

        Ok(())
    }
    
    
    #[wasm_bindgen]
    pub async fn add_leaf(&mut self, delta_str: &str, metadata_str: &str) -> Result<JsValue, JsError> {
        // Parse and validate delta
        let delta: Delta = serde_json::from_str(delta_str)
            .map_err(|e| JsError::new(&format!("Delta parse error: {}", e)))?;
        
        // Validate delta structure
        self.validate_delta(&delta)?;
        
        // Parse and validate metadata
        let mut metadata: NodeMetadata = serde_json::from_str(metadata_str)
            .map_err(|e| JsError::new(&format!("Metadata parse error: {}", e)))?;
        
        // Set timestamp if not present
        if metadata.timestamp == 0.0 {
            metadata.timestamp = js_sys::Date::now();
        }
        
        // Create leaf content with formatting preserved
        let leaf_content = serde_json::json!({
            "delta": delta,
            "metadata": metadata
        });
        
        // Generate leaf hash
        let leaf_hash = self.compute_hash(&serde_json::to_string(&leaf_content)?);
        
        // Create new leaf node
        let new_leaf = MerkleNode {
            hash: leaf_hash.clone(),
            delta: Some(delta.clone()),
            metadata: Some(metadata),
            left: None,
            right: None,
        };
        
        // Store previous root for comparison
        let prev_root = self.root.clone();
        
        // Add leaf and update tree
        self.leaves.push(new_leaf);
        self.apply_delta(&delta);
        self.rebuild_tree()?;
        
        // Verify tree consistency
        if !self.verify_tree_consistency()? {
            web_sys::console::error_1(&"Tree consistency check failed after adding leaf".into());
        }
        
        // Handle checkpoint if needed
        self.handle_checkpoint().await?;
        
        // Generate proof
        let proof = if self.leaves.len() > 1 {
            self.generate_proof_from_levels(&self.levels, self.leaves.len() - 1)?
        } else {
            serde_json::json!({
                "proof": [],
                "rootHash": self.root.as_ref().map(|r| r.hash.clone())
            })
        };
        
        // Log operation
        web_sys::console::log_1(&format!(
            "Added leaf: hash={}, total_leaves={}, root_hash={:?}",
            leaf_hash,
            self.leaves.len(),
            self.root.as_ref().map(|r| r.hash.clone())
        ).into());
        
        // Return result
        Ok(serde_wasm_bindgen::to_value(&serde_json::json!({
            "leaf": {
                "hash": leaf_hash,
                "content": leaf_content
            },
            "proof": proof,
            "rootHash": self.root.as_ref().map(|r| r.hash.clone()),
            "previousRoot": prev_root.map(|r| r.hash)
        }))?)
    }
    
    #[wasm_bindgen]
    pub fn get_checkpoint_status(&self) -> Result<JsValue, JsError> {
        let next_checkpoint = {
            let current_leaves = self.leaves.len();
            let next = (current_leaves / CHECKPOINT_INTERVAL + 1) * CHECKPOINT_INTERVAL;
            next
        };

        let latest_checkpoint = {
            let current_leaves = self.leaves.len();
            let latest = (current_leaves / CHECKPOINT_INTERVAL) * CHECKPOINT_INTERVAL;
            if latest == 0 { None } else { Some(latest) }
        };

        let status = serde_json::json!({
            "total_leaves": self.leaves.len(),
            "checkpoint_interval": CHECKPOINT_INTERVAL,
            "latest_checkpoint": latest_checkpoint,
            "next_checkpoint": next_checkpoint,
            "timestamped_nodes": self.leaves.iter().filter(|leaf| {
                leaf.metadata.as_ref().and_then(|m| m.ots_timestamp.as_ref()).is_some()
            }).count()
        });

        Ok(serde_wasm_bindgen::to_value(&status)?)
    }
    
    fn compute_hash(&self, data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    fn apply_delta(&mut self, delta: &Delta) {
        self.document_state = self.document_state.compose(delta);
    }

    #[wasm_bindgen]
    pub async fn perform_pow(&self, content: &str, difficulty: u32) -> Result<JsValue, JsError> {
        let target = "0".repeat(difficulty as usize);
        let mut nonce = 0u64;
        let start_time = web_sys::window().unwrap().performance().unwrap().now();

        loop {
            for _ in 0..1000 {
                let combined = format!("{}{}", content, nonce);
                let hash = self.compute_hash(&combined);
                if hash.starts_with(&target) {
                    let duration = web_sys::window().unwrap().performance().unwrap().now() - start_time;
                    let result = PowResult { nonce, hash, duration, difficulty };
                    return Ok(serde_wasm_bindgen::to_value(&result)?);
                }
                nonce += 1;
            }
            let promise = js_sys::Promise::new(&mut |resolve, _reject| {
                web_sys::window().unwrap().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 10).unwrap();
            });
            wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(|err| JsError::new(&err.as_string().unwrap_or_else(|| "Unknown error".into())))?;
        }
    }

    pub fn verify_proof(&self, index: usize) -> Result<JsValue, JsError> {
        if index >= self.leaves.len() {
            return Err(JsError::new("Invalid leaf index"));
        }
        let leaf = &self.leaves[index];
        let leaf_content = serde_json::json!({
            "delta": leaf.delta,
            "metadata": leaf.metadata
        });
        let leaf_str = serde_json::to_string(&leaf_content)?;
        let computed_leaf_hash = self.compute_hash(&leaf_str);
        
        if index == 0 {
            return Ok(serde_wasm_bindgen::to_value(&serde_json::json!({
                "valid": computed_leaf_hash == leaf.hash,
                "verification_type": "genesis",
                "computed_hash": computed_leaf_hash,
                "stored_hash": leaf.hash,
                "leaf_content": leaf_str,
                "timestamp": leaf.metadata.as_ref().map(|m| m.timestamp)
            }))?);
        }

        let proof_value = self.generate_proof_from_levels(&self.levels, index)?;
        
        if proof_value.get("proof").map_or(true, |p| p.as_array().map_or(true, |a| a.is_empty())) {
            return Ok(serde_wasm_bindgen::to_value(&serde_json::json!({
                "valid": false,
                "error": "Missing proof for non-genesis block",
                "computed_hash": computed_leaf_hash,
                "stored_hash": leaf.hash,
                "leaf_content": leaf_str,
                "timestamp": leaf.metadata.as_ref().map(|m| m.timestamp)
            }))?);
        }

        let mut current_hash = computed_leaf_hash.clone();
        for proof_item in proof_value.get("proof").unwrap().as_array().unwrap() {
            let sibling_hash = proof_item.get("hash").unwrap().as_str().unwrap();
            let position = proof_item.get("position").unwrap().as_str().unwrap();
            let combined = if position == "left" {
                serde_json::to_string(&serde_json::json!({
                    "left": sibling_hash,
                    "right": current_hash,
                }))?
            } else {
                serde_json::to_string(&serde_json::json!({
                    "left": current_hash,
                    "right": sibling_hash,
                }))?
            };
            current_hash = self.compute_hash(&combined);
        }
        let root_hash = self.root.as_ref().map(|r| r.hash.clone());
        let valid = Some(current_hash.clone()) == root_hash;
        Ok(serde_wasm_bindgen::to_value(&serde_json::json!({
            "valid": valid,
            "verification_type": "regular",
            "computed_hash": current_hash,
            "root_hash": root_hash,
            "timestamp": leaf.metadata.as_ref().map(|m| m.timestamp)
        }))?)
    }

    fn verify_tree_consistency(&self) -> Result<bool, JsError> {
        if self.leaves.is_empty() {
            return Ok(true);
        }
        let mut verification_levels = Vec::new();
        let mut current_level = self.leaves.clone();
        verification_levels.push(current_level.clone());

        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                let right = if chunk.len() > 1 { &chunk[1] } else { left };
                let internal_node_content = serde_json::json!({
                    "left": left.hash,
                    "right": right.hash
                });
                let combined_hash = self.compute_hash(&serde_json::to_string(&internal_node_content)?);
                let parent = MerkleNode {
                    hash: combined_hash,
                    delta: None,
                    metadata: Some(NodeMetadata {
                        timestamp: js_sys::Date::now(),
                        edit_stats: None,
                        pow_result: None,
                        is_genesis: None,
                        ots_timestamp: None,
                    }),
                    left: Some(Box::new(left.clone())),
                    right: Some(Box::new(right.clone())),
                };
                next_level.push(parent);
            }
            verification_levels.push(next_level.clone());
            current_level = next_level;
        }
        let expected_root_hash = if current_level.is_empty() {
            self.leaves[0].hash.clone()
        } else {
            current_level[0].hash.clone()
        };
        let actual_root_hash = self.root.as_ref().map(|r| r.hash.clone());
        let is_valid = Some(expected_root_hash.clone()) == actual_root_hash;
        if !is_valid {
            web_sys::console::warn_1(&format!("Tree consistency check failed: expected_root={}, actual_root={:?}", expected_root_hash, actual_root_hash).into());
        }
        Ok(is_valid)
    }

    fn rebuild_tree(&mut self) -> Result<(), JsError> {
        if self.leaves.is_empty() {
            self.root = None;
            self.levels.clear();
            return Ok(());
        }
        let mut current_level = self.leaves.clone();
        let mut levels = vec![current_level.clone()];
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                let right = if chunk.len() > 1 { &chunk[1] } else { left };
                let internal_node_content = serde_json::json!({
                    "left": left.hash,
                    "right": right.hash
                });
                let combined_hash = self.compute_hash(&serde_json::to_string(&internal_node_content)?);
                let parent = MerkleNode {
                    hash: combined_hash,
                    delta: None,
                    metadata: Some(NodeMetadata {
                        timestamp: js_sys::Date::now(),
                        edit_stats: None,
                        pow_result: None,
                        is_genesis: None,
                        ots_timestamp: None,
                    }),
                    left: Some(Box::new(left.clone())),
                    right: Some(Box::new(right.clone())),
                };
                next_level.push(parent);
            }
            levels.push(next_level.clone());
            current_level = next_level;
        }
        self.levels = levels;
        self.root = if current_level.is_empty() {
            Some(self.leaves[0].clone())
        } else {
            Some(current_level[0].clone())
        };
        web_sys::console::log_1(&format!("Tree rebuilt: leaves={}, levels={}, root_hash={}", self.leaves.len(), self.levels.len(), self.root.as_ref().map_or("None".to_string(), |r| r.hash.clone())).into());
        Ok(())
    }
    
    fn generate_proof_from_levels(&self, levels: &[Vec<MerkleNode>], index: usize) -> Result<serde_json::Value, JsError> {
        let mut proof = Vec::new();
        let mut current_index = index;
        for level in levels.iter() {
            if level.len() <= 1 {
                break;
            }
            let pair_start = (current_index / 2) * 2;
            let sibling_index = if current_index % 2 == 0 { pair_start + 1 } else { pair_start };
            let sibling_hash = if sibling_index < level.len() { level[sibling_index].hash.clone() } else { level[pair_start].hash.clone() };
            let position = if current_index % 2 == 0 { "right" } else { "left" };
            proof.push(serde_json::json!({
                "hash": sibling_hash,
                "position": position
            }));
            current_index /= 2;
        }
        Ok(serde_json::json!({
            "proof": proof,
            "rootHash": self.root.as_ref().map(|r| r.hash.clone())
        }))
    }
    
    #[wasm_bindgen]
    pub fn get_proof(&self, index: usize) -> Result<String, JsError> {
        if index >= self.leaves.len() {
            return Err(JsError::new("Invalid leaf index"));
        }
        if index == 0 {
            let proof = serde_json::json!({
                "proof": [],
                "rootHash": self.root.as_ref().map(|r| r.hash.clone())
            });
            return serde_json::to_string_pretty(&proof).map_err(|e| JsError::new(&e.to_string()));
        }
        let proof = self.generate_proof_from_levels(&self.levels, index)?;
        serde_json::to_string_pretty(&proof).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Reconstructs the document by composing all leaf deltas using Quill's Delta compose.
    /// (This function is used only to update the UI; Quill will handle deserialization.)
    #[wasm_bindgen]
    pub fn get_current_content(&self) -> Result<JsValue, JsError> {
        // Start with empty delta
        let mut composed = Delta { ops: Vec::new() };
        
        // Log for debugging
        web_sys::console::log_1(&format!("Starting composition with {} leaves", self.leaves.len()).into());
        
        // Compose all deltas while preserving attributes
        for (i, leaf) in self.leaves.iter().enumerate() {
            if let Some(delta) = &leaf.delta {
                // Log each delta's attributes for debugging
                if let Some(ops) = &delta.ops.iter().find(|op| op.attributes.is_some()) {
                    web_sys::console::log_1(&format!("Leaf {} has formatting: {:?}", i, ops.attributes).into());
                }
                
                // Compose while preserving attributes
                composed = composed.compose(delta);
            }
        }
        
        // Log final composed delta
        web_sys::console::log_1(&format!("Final composed delta: {:?}", composed).into());
        
        // Convert to JsValue
        Ok(serde_wasm_bindgen::to_value(&composed)?)
    }

    pub fn get_history(&self) -> Result<JsValue, JsError> {
        let history: Vec<_> = self.leaves.iter().map(|leaf| {
            serde_json::json!({
                "delta": leaf.delta,
                "metadata": leaf.metadata,
                "hash": leaf.hash,
                "timestamp": leaf.metadata.as_ref().map(|m| m.timestamp)
            })
        }).collect();
        Ok(serde_wasm_bindgen::to_value(&history)?)
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<String, JsError> {
        let serialized = serde_json::json!({
            "leaves": self.leaves,
            "documentState": self.document_state,
            "levels": self.levels,
            "root": self.root
        });
        web_sys::console::log_1(&format!("Serializing content: {}", serde_json::to_string_pretty(&serialized).unwrap_or_default()).into());
        let json_str = serde_json::to_string_pretty(&serialized).map_err(|e| JsError::new(&e.to_string()))?;
        web_sys::console::log_1(&format!("Serializing content: {}", json_str).into());
        Ok(json_str)
    }

    pub fn deserialize(&mut self, data_str: &str) -> Result<bool, JsError> {
        let data: serde_json::Value = serde_json::from_str(data_str)?;
        if let Some(leaves) = data.get("leaves").and_then(|v| v.as_array()) {
            self.leaves = leaves.iter().map(|leaf| serde_json::from_value(leaf.clone())).collect::<Result<Vec<_>, _>>()?;
        }
        if let Some(doc_state) = data.get("documentState") {
            self.document_state = serde_json::from_value(doc_state.clone())?;
        }
        self.levels = if let Some(levels) = data.get("levels").and_then(|v| v.as_array()) {
            levels.iter().map(|level| serde_json::from_value(level.clone())).collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        self.rebuild_tree()?;
        Ok(true)
    }

    pub fn clear(&mut self) {
        self.leaves.clear();
        self.root = None;
        self.document_state = Delta { ops: Vec::new() };
        self.levels.clear();
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    
    web_sys::console::log_1(&"WASM start function beginning...".into());
    
    #[cfg(feature = "console_log")]
    {
        use log::Level;
        console_log::init_with_level(Level::Debug)
            .map_err(|e| JsValue::from_str(&format!("Failed to initialize logger: {}", e)))?;
    }
    
    web_sys::console::log_1(&"WASM start function completed successfully".into());
    Ok(())
}
