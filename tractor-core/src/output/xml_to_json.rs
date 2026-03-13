//! XML fragment → JSON tree conversion.
//!
//! Converts a raw tractor XML fragment directly to a serde_json::Value tree,
//! without going through a rendered string representation.
//!
//! ## Output structure
//!
//! - Self-closing children (modifiers/flags) become boolean properties: `"public": true`
//! - Text-only elements: `{"type": "name", "text": "QueryHelpers"}`
//! - Elements with content: `{"type": "class", "public": true, "children": [...]}`
//!   where `children` contains only element nodes (anonymous text tokens are dropped).
//!
//! Attributes in the raw fragment (location metadata like `start`, `end`, etc.) are
//! silently ignored — they are internal parser annotations, not semantic content.
//! Whitespace-only text nodes are also dropped.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value};

const KEY_TYPE: &str = "type";
const KEY_TEXT: &str = "text";
const KEY_CHILDREN: &str = "children";

/// Convert a raw XML fragment string to a JSON tree value.
///
/// `max_depth` limits how deep the tree is expanded (None = unlimited).
///
/// The fragment may contain a single root element or multiple siblings.
/// Single element returns a JSON object; multiple return a JSON array.
pub fn xml_fragment_to_json(xml: &str, max_depth: Option<usize>) -> Value {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut stack: Vec<JsonNode> = Vec::new();
    // Sentinel root to collect top-level siblings (depth 0)
    stack.push(JsonNode::new("__root__"));

    // skip_depth > 0 means we are inside a subtree that exceeds max_depth
    let mut skip_depth: usize = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if skip_depth > 0 {
                    skip_depth += 1;
                    continue;
                }
                // stack.len() - 1 = current content depth (sentinel = 0, first real element = 1)
                let content_depth = stack.len() - 1;
                if max_depth.map_or(false, |max| content_depth >= max) {
                    skip_depth = 1;
                    continue;
                }
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                stack.push(JsonNode::new(&name));
            }
            Ok(Event::End(_)) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                if stack.len() > 1 {
                    let node = stack.pop().unwrap();
                    let val = node.into_value();
                    if let Some(parent) = stack.last_mut() {
                        parent.add_content_child(val);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if skip_depth > 0 {
                    continue;
                }
                let content_depth = stack.len() - 1;
                if max_depth.map_or(false, |max| content_depth >= max) {
                    continue;
                }
                // Self-closing element — becomes a boolean flag on the parent
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if let Some(top) = stack.last_mut() {
                    top.flags.push(name);
                }
            }
            Ok(Event::Text(e)) => {
                if skip_depth > 0 {
                    continue;
                }
                if let Ok(text) = e.unescape() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let mut obj = Map::new();
                        obj.insert(KEY_TEXT.into(), Value::String(trimmed.to_string()));
                        if let Some(top) = stack.last_mut() {
                            top.add_content_child(Value::Object(obj));
                        }
                    }
                }
            }
            Ok(Event::CData(e)) => {
                if skip_depth > 0 {
                    continue;
                }
                if let Ok(text) = std::str::from_utf8(e.as_ref()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let mut obj = Map::new();
                        obj.insert(KEY_TEXT.into(), Value::String(trimmed.to_string()));
                        if let Some(top) = stack.last_mut() {
                            top.add_content_child(Value::Object(obj));
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    // Unwrap the sentinel root
    let root = stack.pop().unwrap_or_else(|| JsonNode::new("__root__"));
    if root.content_children.len() == 1 && root.flags.is_empty() {
        root.content_children.into_iter().next().unwrap()
    } else if root.content_children.is_empty() && root.flags.is_empty() {
        Value::Null
    } else {
        // Multiple top-level items — return flat list
        let mut result = Vec::new();
        for flag in root.flags {
            let mut obj = Map::new();
            obj.insert(KEY_TYPE.into(), Value::String(flag));
            result.push(Value::Object(obj));
        }
        result.extend(root.content_children);
        Value::Array(result)
    }
}

struct JsonNode {
    name: String,
    /// Self-closing child elements become boolean flags on this node
    flags: Vec<String>,
    /// Non-flag child content (element objects + text nodes)
    content_children: Vec<Value>,
}

impl JsonNode {
    fn new(name: &str) -> Self {
        JsonNode { name: name.to_string(), flags: Vec::new(), content_children: Vec::new() }
    }

    fn add_content_child(&mut self, child: Value) {
        self.content_children.push(child);
    }

    fn into_value(self) -> Value {
        let only_text = self.content_children.iter().all(|c| {
            c.as_object().map_or(false, |o| o.contains_key(KEY_TEXT) && o.len() == 1)
        });
        let has_element_children = self.content_children.iter().any(|c| {
            c.as_object().map_or(false, |o| o.contains_key(KEY_TYPE) || o.contains_key(KEY_CHILDREN))
        });

        let mut obj = Map::new();
        obj.insert(KEY_TYPE.into(), Value::String(self.name));

        // Boolean flags become direct properties
        for flag in self.flags {
            obj.insert(flag, Value::Bool(true));
        }

        if self.content_children.is_empty() {
            // No content — type and flags only
        } else if only_text && !has_element_children {
            // Collapse all text children into a single "text" string
            let combined: Vec<&str> = self.content_children.iter()
                .filter_map(|c| c.get(KEY_TEXT).and_then(|v| v.as_str()))
                .collect();
            obj.insert(KEY_TEXT.into(), Value::String(combined.join(" ")));
        } else {
            // Mixed content: drop anonymous text tokens (syntactic noise like "class", "{", "<")
            // Only structural element children are meaningful here
            let element_children: Vec<Value> = self.content_children.into_iter()
                .filter(|c| c.as_object().map_or(false, |o| {
                    o.contains_key(KEY_TYPE) || o.contains_key(KEY_CHILDREN)
                }))
                .collect();
            if !element_children.is_empty() {
                obj.insert(KEY_CHILDREN.into(), Value::Array(element_children));
            }
        }

        Value::Object(obj)
    }
}
