//! XML fragment → JSON tree conversion with field-based property lifting.
//!
//! Converts a raw tractor XML fragment directly to a serde_json::Value tree,
//! without going through a rendered string representation.
//!
//! ## Output structure
//!
//! Elements with a `field` attribute are **singletons** (one per parent) and get
//! lifted to direct JSON properties on their parent object. Elements without
//! `field` go into a `children` array.
//!
//! - Self-closing children (modifiers/flags) become boolean properties: `"public": true`
//! - Field-backed text-only leaves collapse to plain strings:
//!   `<name field="name">Foo</name>` → parent gets `"name": "Foo"`
//! - Field-backed structural nodes become objects WITHOUT `$type`:
//!   `<body field="body">...</body>` → parent gets `"body": { "children": [...] }`
//! - Non-field text-only leaves keep compact form: `<accessor>get;</accessor>` → `{"accessor": "get;"}`
//! - Non-field structural nodes get `$type`: `{"$type": "method", "children": [...]}`
//!
//! Attributes in the raw fragment (location metadata like `start`, `end`, etc.) are
//! silently ignored — except `field` which drives the singleton detection.
//! Whitespace-only text nodes are also dropped.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value};

const KEY_TYPE: &str = "$type";
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
    stack.push(JsonNode::new("__root__", None));

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
                    if let Some(parent) = stack.last_mut() {
                        parent.children_truncated = true;
                    }
                    continue;
                }
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();

                // Extract `field` attribute if present
                let field = e.attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.as_ref() == b"field")
                    .and_then(|a| String::from_utf8(a.value.to_vec()).ok());

                stack.push(JsonNode::new(&name, field));
            }
            Ok(Event::End(_)) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                if stack.len() > 1 {
                    let node = stack.pop().unwrap();
                    let field = node.field.clone();
                    let val = node.into_value();
                    if let Some(parent) = stack.last_mut() {
                        parent.content_children.push(ChildEntry { field, value: val });
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if skip_depth > 0 {
                    continue;
                }
                let content_depth = stack.len() - 1;
                if max_depth.map_or(false, |max| content_depth >= max) {
                    if let Some(parent) = stack.last_mut() {
                        parent.children_truncated = true;
                    }
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
                        if let Some(top) = stack.last_mut() {
                            top.text_fragments.push(trimmed.to_string());
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
                        if let Some(top) = stack.last_mut() {
                            top.text_fragments.push(trimmed.to_string());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    // Unwrap the sentinel root
    let root = stack.pop().unwrap_or_else(|| JsonNode::new("__root__", None));
    let non_text_children: Vec<&ChildEntry> = root.content_children.iter()
        .filter(|c| !is_anon_text_entry(c))
        .collect();
    if non_text_children.len() == 1 && root.flags.is_empty() {
        // Single top-level element — unwrap the sentinel
        let entry = root.content_children.into_iter()
            .find(|c| !is_anon_text_entry(c))
            .unwrap();
        entry.value
    } else if non_text_children.is_empty() && root.flags.is_empty() {
        Value::Null
    } else {
        // Multiple top-level items — return flat list
        let mut result = Vec::new();
        for flag in root.flags {
            let mut obj = Map::new();
            obj.insert(KEY_TYPE.into(), Value::String(flag));
            result.push(Value::Object(obj));
        }
        for entry in root.content_children {
            if !is_anon_text_entry(&entry) {
                result.push(entry.value);
            }
        }
        Value::Array(result)
    }
}

/// A child entry that tracks whether it came from a field-backed element.
struct ChildEntry {
    field: Option<String>,
    value: Value,
}

/// Check if a ChildEntry is an anonymous text token (no field, text-only).
fn is_anon_text_entry(entry: &ChildEntry) -> bool {
    entry.field.is_none() && entry.value.as_object().map_or(false, |o| {
        o.len() == 1 && o.contains_key(KEY_TEXT)
    })
}

struct JsonNode {
    name: String,
    /// The `field` attribute value, if this element is field-backed.
    field: Option<String>,
    /// Self-closing child elements become boolean flags on this node
    flags: Vec<String>,
    /// Text fragments directly inside this element (not inside child elements)
    text_fragments: Vec<String>,
    /// Child entries (element children with optional field info)
    content_children: Vec<ChildEntry>,
    /// True when at least one element child was skipped due to max_depth.
    children_truncated: bool,
}

impl JsonNode {
    fn new(name: &str, field: Option<String>) -> Self {
        JsonNode {
            name: name.to_string(),
            field,
            flags: Vec::new(),
            text_fragments: Vec::new(),
            content_children: Vec::new(),
            children_truncated: false,
        }
    }

    /// True if this node has only text content (no element children).
    fn is_text_only(&self) -> bool {
        self.content_children.iter().all(|c| is_anon_text_entry(c))
    }

    /// Combine all text fragments into a single string.
    fn combined_text(&self) -> String {
        // Include both direct text fragments and text from anonymous child entries
        let mut parts: Vec<&str> = Vec::new();
        // We interleave: text_fragments are direct text, anon text entries are
        // text children that were pushed as ChildEntry. But since we now store
        // text directly in text_fragments, just use those.
        for frag in &self.text_fragments {
            parts.push(frag);
        }
        parts.join(" ")
    }

    fn into_value(self) -> Value {
        let is_text_only = self.is_text_only();
        let has_text = !self.text_fragments.is_empty();
        let combined_text = if has_text { self.combined_text() } else { String::new() };

        // Pure text-only leaf (no flags, no element children): eligible for collapsing.
        // Guard: if children were truncated by max_depth, keep structural form.
        if is_text_only && self.flags.is_empty() && has_text && !self.children_truncated {
            if self.field.is_some() {
                // Field-backed text-only leaf: just return the string value.
                // The parent will lift it as a property using the field name.
                return Value::String(combined_text);
            } else {
                // Non-field text-only leaf: compact form {elementName: text}
                let mut obj = Map::new();
                obj.insert(self.name, Value::String(combined_text));
                return Value::Object(obj);
            }
        }

        // Build the JSON object for this node.
        // Field-backed nodes omit $type (parent lifts by field name).
        // Non-field nodes include $type for identification in children array.
        let mut obj = Map::new();

        if self.field.is_none() {
            obj.insert(KEY_TYPE.into(), Value::String(self.name));
        }

        // Boolean flags become direct properties
        for flag in self.flags {
            obj.insert(flag, Value::Bool(true));
        }

        // Partition children: field-backed → direct properties, rest → children array
        let mut array_children: Vec<Value> = Vec::new();

        for entry in self.content_children {
            if is_anon_text_entry(&entry) {
                // Anonymous text token — drop in structural content (syntactic noise)
                continue;
            }
            if let Some(field_name) = entry.field {
                // Field-backed child → lift to direct property
                obj.insert(field_name, entry.value);
            } else {
                // Non-field child → children array
                array_children.push(entry.value);
            }
        }

        if self.children_truncated && is_text_only {
            // Children were truncated, surviving text is syntactic noise — drop
        } else if is_text_only && has_text {
            // Text leaf that also has flags: keep {$type, text, flag: true, ...}
            obj.insert(KEY_TEXT.into(), Value::String(combined_text));
        } else if !array_children.is_empty() {
            obj.insert(KEY_CHILDREN.into(), Value::Array(array_children));
        }

        Value::Object(obj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_backed_text_leaf_lifted() {
        // <parent><name field="name">Foo</name></parent>
        let xml = r#"<parent><name field="name">Foo</name></parent>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("name").unwrap(), "Foo");
        assert!(!obj.contains_key(KEY_CHILDREN));
    }

    #[test]
    fn test_field_backed_structural_node_lifted() {
        // <parent><body field="body"><block>{ }</block></body></parent>
        let xml = r#"<parent><body field="body"><block>{ }</block></body></parent>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        let body = obj.get("body").unwrap().as_object().unwrap();
        assert!(body.contains_key(KEY_CHILDREN));
        assert!(!body.contains_key(KEY_TYPE));
    }

    #[test]
    fn test_non_field_child_gets_type() {
        // <parent><method>text</method></parent>
        // method has no field attr → goes to children array with $type
        let xml = r#"<parent><method></method></parent>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        let children = obj.get(KEY_CHILDREN).unwrap().as_array().unwrap();
        assert_eq!(children.len(), 1);
        let child = children[0].as_object().unwrap();
        assert_eq!(child.get(KEY_TYPE).unwrap(), "method");
    }

    #[test]
    fn test_mixed_field_and_non_field() {
        let xml = r#"<class><name field="name">Foo</name><method></method></class>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        // name is lifted as property
        assert_eq!(obj.get("name").unwrap(), "Foo");
        // method goes to children
        let children = obj.get(KEY_CHILDREN).unwrap().as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].get(KEY_TYPE).unwrap(), "method");
    }

    #[test]
    fn test_flags_still_boolean() {
        let xml = r#"<class><public/><name field="name">Foo</name></class>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("public").unwrap(), true);
        assert_eq!(obj.get("name").unwrap(), "Foo");
    }

    #[test]
    fn test_non_field_text_leaf_compact() {
        // Non-field text-only leaf keeps compact form {name: text}
        let xml = r#"<parent><accessor>get;</accessor></parent>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        let children = obj.get(KEY_CHILDREN).unwrap().as_array().unwrap();
        assert_eq!(children.len(), 1);
        let child = children[0].as_object().unwrap();
        assert_eq!(child.get("accessor").unwrap(), "get;");
    }

    #[test]
    fn test_type_key_is_dollar_type() {
        let xml = r#"<method></method>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("$type"));
        assert!(!obj.contains_key("type"));
    }

    #[test]
    fn test_depth_limiting() {
        // At depth 2, <b> is included but <c> is truncated
        let xml = r#"<a><b field="b"><c>deep</c></b></a>"#;
        let result = xml_fragment_to_json(xml, Some(2));
        let obj = result.as_object().unwrap();
        // b is field-backed but its children are truncated
        let b = obj.get("b").unwrap().as_object().unwrap();
        assert!(!b.contains_key(KEY_TYPE)); // field-backed, no $type

        // At depth 1, b is skipped entirely (truncated at parent level)
        let result1 = xml_fragment_to_json(xml, Some(1));
        let obj1 = result1.as_object().unwrap();
        assert!(!obj1.contains_key("b")); // truncated
        assert_eq!(obj1.get(KEY_TYPE).unwrap(), "a");
    }
}
