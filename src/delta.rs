use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::collections::HashMap;

/// An operation in a Delta.
/// Exactly one of `insert`, `delete`, or `retain` is set.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Operation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert: Option<Value>, // can be a string or an embed object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, Value>>,
}

impl Operation {
    pub fn length(&self) -> u32 {
        if let Some(insert) = &self.insert {
            if let Some(s) = insert.as_str() {
                s.chars().count() as u32
            } else {
                1
            }
        } else if let Some(retain) = self.retain {
            retain
        } else if let Some(delete) = self.delete {
            delete
        } else {
            0
        }
    }

    pub fn is_insert(&self) -> bool {
        self.insert.is_some()
    }

    pub fn is_delete(&self) -> bool {
        self.delete.is_some()
    }

    pub fn is_retain(&self) -> bool {
        self.retain.is_some()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Delta {
    pub ops: Vec<Operation>,
}

impl Delta {
    pub fn compose(&self, other: &Delta) -> Delta {
        let mut result_ops = Vec::new();
        let mut iter_a = DeltaIterator::new(&self.ops);
        let mut pos: u32 = 0;
        let mut current_attributes: Option<HashMap<String, Value>> = None;

        for op_b in &other.ops {
            if let Some(ref insert_val) = op_b.insert {
                // Handle inserts
                if let Some(retain_pos) = op_b.retain {
                    // Move up to the retain position
                    while pos < retain_pos && iter_a.has_next() {
                        let op = iter_a.next(iter_a.peek_length());
                        if !op.is_retain() || op.length() > 0 {
                            // Apply current formatting to existing content
                            let merged_attrs = merge_attributes(
                                op.attributes.clone(),
                                current_attributes.clone(),
                            );
                            let mut new_op = op.clone();
                            new_op.attributes = merged_attrs;
                            result_ops.push(new_op);
                        }
                        pos += op.length();
                    }
                    
                    // Insert with combined attributes
                    let insert_attrs = merge_attributes(
                        op_b.attributes.clone(),
                        current_attributes.clone(),
                    );
                    result_ops.push(Operation {
                        insert: Some(insert_val.clone()),
                        delete: None,
                        retain: None,
                        attributes: insert_attrs,
                    });
                } else {
                    // Direct insert with combined attributes
                    let insert_attrs = merge_attributes(
                        op_b.attributes.clone(),
                        current_attributes.clone(),
                    );
                    result_ops.push(Operation {
                        insert: Some(insert_val.clone()),
                        delete: None,
                        retain: None,
                        attributes: insert_attrs,
                    });
                }
                let added = if let Some(s) = insert_val.as_str() {
                    s.chars().count() as u32
                } else {
                    1 // For embeds
                };
                pos += added;
            } else if let Some(delete_len) = op_b.delete {
                // Handle deletes (same as before)
                if let Some(retain_pos) = op_b.retain {
                    while pos < retain_pos && iter_a.has_next() {
                        let op = iter_a.next(iter_a.peek_length());
                        if !op.is_retain() || op.length() > 0 {
                            result_ops.push(op.clone());
                        }
                        pos += op.length();
                    }
                }
                iter_a.consume(delete_len);
                pos += delete_len;
                if delete_len > 0 {
                    result_ops.push(Operation {
                        insert: None,
                        delete: Some(delete_len),
                        retain: None,
                        attributes: None,
                    });
                }
            } else if let Some(retain_len) = op_b.retain {
                // Update current formatting if this retain has attributes
                if op_b.attributes.is_some() {
                    current_attributes = Some(merge_attributes(
                        current_attributes,
                        op_b.attributes.clone(),
                    ).unwrap_or_default());
                }

                let mut remaining = retain_len;
                
                // Move to target position if specified
                if let Some(pos_target) = op_b.retain {
                    while pos < pos_target && iter_a.has_next() {
                        let op = iter_a.next(iter_a.peek_length());
                        if !op.is_retain() || op.length() > 0 {
                            // Apply current formatting to content being moved over
                            let merged_attrs = merge_attributes(
                                op.attributes.clone(),
                                current_attributes.clone(),
                            );
                            let mut new_op = op.clone();
                            new_op.attributes = merged_attrs;
                            result_ops.push(new_op);
                        }
                        pos += op.length();
                    }
                }

                // Process the retain operation
                while remaining > 0 && iter_a.has_next() {
                    let a_op = iter_a.next(remaining);
                    let part_len = a_op.length();
                    
                    // Merge attributes from both operations
                    let merged_attrs = merge_attributes(
                        a_op.attributes.clone(),
                        current_attributes.clone(),
                    );
                    
                    if a_op.is_insert() {
                        result_ops.push(Operation {
                            insert: a_op.insert.clone(),
                            delete: None,
                            retain: None,
                            attributes: merged_attrs,
                        });
                    } else if a_op.is_retain() && part_len > 0 {
                        result_ops.push(Operation {
                            insert: None,
                            delete: None,
                            retain: Some(part_len),
                            attributes: merged_attrs,
                        });
                    }
                    pos += part_len;
                    remaining = remaining.saturating_sub(part_len);
                }
            }
        }

        // Append remaining operations with current formatting
        while iter_a.has_next() {
            let op = iter_a.next(iter_a.peek_length());
            if !op.is_retain() || op.length() > 0 {
                let merged_attrs = merge_attributes(
                    op.attributes.clone(),
                    current_attributes.clone(),
                );
                let mut new_op = op.clone();
                new_op.attributes = merged_attrs;
                result_ops.push(new_op);
            }
            pos += op.length();
        }

        // Remove trailing newline if present (Quill convention)
        if let Some(last) = result_ops.last() {
            if last.insert.as_ref().and_then(|s| s.as_str()) == Some("\n") {
                result_ops.pop();
            }
        }

        Delta { ops: result_ops }
    }
}

pub fn merge_attributes(
    base: Option<HashMap<String, Value>>,
    modifier: Option<HashMap<String, Value>>
) -> Option<HashMap<String, Value>> {
    match (base, modifier) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(m)) => Some(m),
        (Some(mut b), Some(m)) => {
            for (key, val) in m {
                if val.is_null() {
                    b.remove(&key);
                } else {
                    b.insert(key, val);
                }
            }
            Some(b)
        }
    }
}

enum OpType {
    Insert,
    Delete,
    Retain,
}

struct DeltaIterator<'a> {
    ops: &'a [Operation],
    index: usize,
    offset: u32,
}

impl<'a> DeltaIterator<'a> {
    fn new(ops: &'a [Operation]) -> Self {
        DeltaIterator { ops, index: 0, offset: 0 }
    }

    fn has_next(&self) -> bool {
        self.index < self.ops.len()
    }

    fn peek_length(&self) -> u32 {
        if self.index >= self.ops.len() {
            0
        } else {
            self.ops[self.index].length() - self.offset
        }
    }

    fn peek_type(&self) -> Option<OpType> {
        if self.index >= self.ops.len() {
            return None;
        }
        let op = &self.ops[self.index];
        if op.insert.is_some() {
            Some(OpType::Insert)
        } else if op.delete.is_some() {
            Some(OpType::Delete)
        } else if op.retain.is_some() {
            Some(OpType::Retain)
        } else {
            None
        }
    }

    fn peek(&self) -> &'a Operation {
        if self.index >= self.ops.len() {
            panic!("No more operations to peek");
        }
        &self.ops[self.index]
    }

    fn peek_attributes(&self) -> Option<HashMap<String, Value>> {
        if self.index >= self.ops.len() {
            return None;
        }
        self.ops[self.index].attributes.clone()
    }

    fn next(&mut self, length: u32) -> Operation {
        if self.index >= self.ops.len() {
            panic!("No more operations to consume");
        }
        let op = &self.ops[self.index];
        let op_remaining = op.length() - self.offset;
        if length >= op_remaining {
            self.index += 1;
            self.offset = 0;
            op.clone()
        } else {
            let part = split_op(op, self.offset, length);
            self.offset += length;
            part
        }
    }

    fn consume(&mut self, length: u32) {
        let remaining = self.peek_length();
        if length >= remaining {
            self.index += 1;
            self.offset = 0;
        } else {
            self.offset += length;
        }
    }
}

fn split_op(op: &Operation, offset: u32, length: u32) -> Operation {
    if let Some(insert) = &op.insert {
        if let Some(s) = insert.as_str() {
            let mut char_indices = s.char_indices();
            let start = char_indices.nth(offset as usize).map(|(idx, _)| idx).unwrap_or(s.len());
            let mut end = s.len();
            for (i, (idx, _)) in s.char_indices().enumerate() {
                if i == (offset + length) as usize {
                    end = idx;
                    break;
                }
            }
            let sliced = s[start..end].to_string();
            Operation {
                insert: Some(Value::String(sliced)),
                delete: None,
                retain: None,
                attributes: op.attributes.clone(),
            }
        } else {
            op.clone()
        }
    } else if let Some(_retain) = op.retain {
        Operation {
            insert: None,
            delete: None,
            retain: Some(length),
            attributes: op.attributes.clone(),
        }
    } else if let Some(_delete) = op.delete {
        Operation {
            insert: None,
            delete: Some(length),
            retain: None,
            attributes: None,
        }
    } else {
        panic!("Operation must be insert, delete, or retain")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_attributes() {
        let mut base = HashMap::new();
        base.insert("bold".to_string(), json!(true));
        let mut modifier = HashMap::new();
        modifier.insert("italic".to_string(), json!(true));
        modifier.insert("bold".to_string(), json!(null));
        let merged = merge_attributes(Some(base), Some(modifier)).unwrap();
        assert!(merged.get("bold").is_none());
        assert_eq!(merged.get("italic").unwrap(), &json!(true));
    }

    #[test]
    fn test_delta_compose_inserts() {
        let delta1 = Delta {
            ops: vec![
                Operation { insert: Some(json!("Hello")), delete: None, retain: None, attributes: None },
            ],
        };
        let delta2 = Delta {
            ops: vec![
                Operation { insert: Some(json!(" World")), delete: None, retain: None, attributes: None },
            ],
        };
        let composed = delta1.compose(&delta2);
        assert_eq!(composed.ops.len(), 1);
        assert_eq!(composed.ops[0].insert.as_ref().unwrap().as_str().unwrap(), "Hello World");
    }

    #[test]
    fn test_delta_compose_with_retain_and_attributes() {
        let delta1 = Delta {
            ops: vec![
                Operation {
                    insert: Some(json!("Hello World")),
                    delete: None,
                    retain: None,
                    attributes: Some(HashMap::from([("color".to_string(), json!("red"))])),
                }
            ],
        };
        let delta2 = Delta {
            ops: vec![
                Operation {
                    retain: Some(5),
                    insert: None,
                    delete: None,
                    attributes: Some(HashMap::from([("color".to_string(), json!("blue"))])),
                }
            ],
        };
        let composed = delta1.compose(&delta2);
        assert!(composed.ops.len() >= 1);
        let first_op = &composed.ops[0];
        assert_eq!(first_op.insert.as_ref().unwrap().as_str().unwrap(), "Hello");
        let attrs = first_op.attributes.as_ref().unwrap();
        assert_eq!(attrs.get("color").unwrap(), &json!("blue"));
    }
}
