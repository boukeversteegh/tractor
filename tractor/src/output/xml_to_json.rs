//! XML node → JSON tree conversion with field-based property lifting.
//!
//! Converts an `XmlNode` tree directly to a serde_json::Value tree.
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
//!   `<body field="body">...</body>` → parent gets `"body": { "$children": [...] }`
//! - Non-field text-only leaves keep compact form: `<accessor>get;</accessor>` → `{"accessor": "get;"}`
//! - Non-field structural nodes get `$type`: `{"$type": "method", "$children": [...]}`
//!
//! Sigil-prefixed keys (`$type`, `$children`, `$truncated`) are reserved
//! for serializer artifacts and never collide with element names — an
//! element literally named `<children>` would render as a bare `children`
//! key, distinct from `$children`.
//!
//! Attributes in the raw fragment (location metadata like `start`, `end`, etc.) are
//! silently ignored — except `field` which drives the singleton detection.
//! Whitespace-only text nodes are also dropped.

use serde_json::{json, Map, Value};
use crate::xpath::XmlNode;
use crate::output::query_tree_renderer::count_descendant_elements;

const KEY_TYPE: &str = "$type";
const KEY_TEXT: &str = "text";
/// Anonymous-overflow array for same-name siblings that don't all
/// fit a singleton key or `list=`-tagged array. Sigil-prefixed so an
/// element literally named `<children>` renders as a bare `children`
/// key, distinct from this serializer artifact.
const KEY_CHILDREN: &str = "$children";
/// When a subtree is elided at `--depth`, the parent object carries
/// this key with the count of descendant elements that were dropped —
/// mirroring the text renderer's `... (N children)` marker so readers
/// know `{}` means "truncated" rather than "empty".
const KEY_TRUNCATED: &str = "$truncated";

/// Convert an XmlNode tree directly to a JSON tree value.
///
/// `max_depth` limits how deep the tree is expanded (None = unlimited).
pub fn xml_node_to_json(node: &XmlNode, max_depth: Option<usize>) -> Value {
    xml_node_to_json_inner(node, max_depth, 0)
}

fn xml_node_to_json_inner(node: &XmlNode, max_depth: Option<usize>, depth: usize) -> Value {
    match node {
        XmlNode::Element { name, attributes: _, children } => {
            // Whether element children at depth+1 should be skipped.
            let skip_element_children = max_depth.map_or(false, |max| depth + 1 >= max);

            // Collect flags (self-closing elements), text, and structural children.
            // Text children and self-closing elements are always collected (they don't
            // increase depth). Only element children with their own children are
            // subject to the depth check.
            let mut flags: Vec<String> = Vec::new();
            let mut text_fragments: Vec<String> = Vec::new();
            let mut content_children: Vec<ChildEntry> = Vec::new();
            let mut truncated_descendants: usize = 0;

            for child in children {
                match child {
                    XmlNode::Element { name: child_name, children: child_children, attributes: child_attrs } => {
                        if skip_element_children {
                            // At depth limit: skip all element children, but
                            // count the dropped descendants so the marker
                            // below reports the same number the text renderer
                            // would show.
                            truncated_descendants += 1 + count_descendant_elements(child);
                        } else if child_children.is_empty() {
                            // Self-closing → boolean flag.
                            flags.push(child_name.clone());
                        } else {
                            // `list="X"` is the renderer signal for "I am one
                            // item in a list named X" (Principle #12). When
                            // present, the child is always emitted into an
                            // array under JSON key X. When absent, the child
                            // is keyed by its element name (singleton property).
                            let list_name = child_attrs.iter()
                                .find(|(k, _)| k == "list")
                                .map(|(_, v)| v.clone());
                            let val = xml_node_to_json_inner(child, max_depth, depth + 1);
                            content_children.push(ChildEntry {
                                element_name: child_name.clone(),
                                list_name,
                                value: val,
                            });
                        }
                    }
                    XmlNode::Text(text) => {
                        // Text is always collected (not depth-limited, matching
                        // the old SAX-based behavior where text events are never skipped)
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            text_fragments.push(trimmed.to_string());
                        }
                    }
                    _ => {}
                }
            }

            // Build the JSON value
            // After iter 139, content_children only holds elements with
            // their own children — text-only-leaf children short-circuit
            // to a scalar string before reaching this list. So if there
            // are no `content_children`, the element is a text-only leaf.
            let is_text_only = content_children.is_empty();
            let has_text = !text_fragments.is_empty();
            let combined_text = if has_text { text_fragments.join(" ") } else { String::new() };
            let children_truncated = truncated_descendants > 0;

            // Pure text-only leaf — return scalar string. Parent will key
            // by the element name or list name; this just supplies the value.
            if is_text_only && flags.is_empty() && has_text && !children_truncated {
                return Value::String(combined_text);
            }

            let mut obj = Map::new();
            obj.insert(KEY_TYPE.into(), Value::String(name.clone()));

            for flag in flags {
                obj.insert(flag, Value::Bool(true));
            }

            // Slot children into the JSON object:
            //   `list="X"` present → property `X`, value is always an array
            //                        (multiple siblings append).
            //   `list=` absent     → property keyed by element name; collisions
            //                        promote to anonymous `children` array
            //                        (transform-bug fallback per Principle #19).
            //
            // Strip `$type` from a child when its value is redundant with the
            // parent's chosen JSON key:
            //   - singleton (no list): key = child element-name; $type repeats it.
            //   - list with list-name = element-name: key = element-name; $type repeats.
            // Children that go into the anonymous `children` array keep `$type`
            // (no key context). Roots also keep `$type` (no parent at all).
            // The render path (`render::parse_json`) tolerates both: it uses the
            // property key for keyed children and falls back to `$type` for
            // anonymous / list-with-different-name items.
            let mut array_children: Vec<Value> = Vec::new();
            for entry in content_children {
                let ChildEntry { element_name, list_name, value } = entry;
                if let Some(list) = list_name {
                    // List entries' `$type` is redundant when the list= name
                    // is the (plural) form of the element name — every entry
                    // in `methods: [...]` is a `<method>` so `$type: method`
                    // just repeats the array key. Iter 231 made list= names
                    // plural English nouns, so the equality check against
                    // the element name is no longer enough; use the
                    // pluralize helper.
                    let value = if list == element_name
                        || list == crate::transform::helpers::pluralize_list_name(&element_name)
                    {
                        strip_top_level_type(value)
                    } else {
                        value
                    };
                    match obj.remove(&list) {
                        Some(Value::Array(mut arr)) => {
                            arr.push(value);
                            obj.insert(list, Value::Array(arr));
                        }
                        Some(existing) => {
                            obj.insert(list, Value::Array(vec![existing, value]));
                        }
                        None => {
                            obj.insert(list, Value::Array(vec![value]));
                        }
                    }
                } else {
                    match obj.remove(&element_name) {
                        None => {
                            obj.insert(element_name, strip_top_level_type(value));
                        }
                        Some(existing) => {
                            // Same-element-name collision without `list=` —
                            // shouldn't happen for role-mixed shapes after
                            // Principle #19. Fall back to anonymous children;
                            // restore the singleton (existing) without `$type`
                            // and push the conflicting child with `$type` kept
                            // (anonymous-array context).
                            obj.insert(element_name, existing);
                            array_children.push(value);
                        }
                    }
                }
            }

            if children_truncated && is_text_only {
                // truncated — drop surviving text
            } else if is_text_only && has_text {
                obj.insert(KEY_TEXT.into(), Value::String(combined_text));
            } else if !array_children.is_empty() {
                obj.insert(KEY_CHILDREN.into(), Value::Array(array_children));
            }

            if children_truncated {
                obj.insert(KEY_TRUNCATED.into(), json!(truncated_descendants));
            }

            Value::Object(obj)
        }
        XmlNode::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Value::Null
            } else {
                Value::String(trimmed.to_string())
            }
        }
        XmlNode::Map { entries } => {
            let mut obj = Map::new();
            for (key, val) in entries {
                obj.insert(key.clone(), xml_node_to_json_inner(val, max_depth, depth + 1));
            }
            Value::Object(obj)
        }
        XmlNode::Array { items } => {
            Value::Array(items.iter()
                .map(|item| xml_node_to_json_inner(item, max_depth, depth + 1))
                .collect())
        }
        XmlNode::Number(n) => json!(*n),
        XmlNode::Boolean(b) => Value::Bool(*b),
        XmlNode::Null => Value::Null,
        _ => Value::Null,
    }
}

/// Strip `$type` from the top-level of a JSON object value. Used at
/// child-insertion time when the parent's chosen JSON key already
/// equals the child's element name (so `$type` would just repeat
/// the key).
fn strip_top_level_type(value: Value) -> Value {
    match value {
        Value::Object(mut obj) => {
            obj.remove(KEY_TYPE);
            Value::Object(obj)
        }
        other => other,
    }
}

struct ChildEntry {
    /// The XML element name (used as JSON key when `list_name` is absent).
    element_name: String,
    /// Optional `list="X"` attribute value (Principle #12). When set, this
    /// child is always emitted into a JSON array under key X, regardless
    /// of cardinality.
    list_name: Option<String>,
    value: Value,
}


#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::Reader;
    use quick_xml::events::Event;

    // -----------------------------------------------------------------------
    // String-based XML → JSON converter (kept for test reference/comparison)
    // -----------------------------------------------------------------------

    fn xml_fragment_to_json(xml: &str, max_depth: Option<usize>) -> Value {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(false);

        let mut stack: Vec<JsonNode> = Vec::new();
        stack.push(JsonNode::new("__root__", None));
        let mut skip_depth: usize = 0;

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    if skip_depth > 0 {
                        skip_depth += 1;
                        continue;
                    }
                    let content_depth = stack.len() - 1;
                    if max_depth.map_or(false, |max| content_depth >= max) {
                        skip_depth = 1;
                        if let Some(parent) = stack.last_mut() {
                            parent.children_truncated = true;
                        }
                        continue;
                    }
                    let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
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
                            parent.content_children.push(TestChildEntry { field, value: val });
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

        let root = stack.pop().unwrap_or_else(|| JsonNode::new("__root__", None));
        let non_text_children: Vec<&TestChildEntry> = root.content_children.iter()
            .filter(|c| !is_test_anon_text_entry(c))
            .collect();
        if non_text_children.len() == 1 && root.flags.is_empty() {
            let entry = root.content_children.into_iter()
                .find(|c| !is_test_anon_text_entry(c))
                .unwrap();
            entry.value
        } else if non_text_children.is_empty() && root.flags.is_empty() {
            Value::Null
        } else {
            let mut result = Vec::new();
            for flag in root.flags {
                let mut obj = Map::new();
                obj.insert(KEY_TYPE.into(), Value::String(flag));
                result.push(Value::Object(obj));
            }
            for entry in root.content_children {
                if !is_test_anon_text_entry(&entry) {
                    result.push(entry.value);
                }
            }
            Value::Array(result)
        }
    }

    struct TestChildEntry {
        field: Option<String>,
        value: Value,
    }

    fn is_test_anon_text_entry(entry: &TestChildEntry) -> bool {
        entry.field.is_none() && entry.value.as_object().map_or(false, |o| {
            o.len() == 1 && o.contains_key(KEY_TEXT)
        })
    }

    struct JsonNode {
        name: String,
        field: Option<String>,
        flags: Vec<String>,
        text_fragments: Vec<String>,
        content_children: Vec<TestChildEntry>,
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

        fn is_text_only(&self) -> bool {
            self.content_children.iter().all(|c| is_test_anon_text_entry(c))
        }

        fn combined_text(&self) -> String {
            let mut parts: Vec<&str> = Vec::new();
            for frag in &self.text_fragments {
                parts.push(frag);
            }
            parts.join(" ")
        }

        fn into_value(self) -> Value {
            let is_text_only = self.is_text_only();
            let has_text = !self.text_fragments.is_empty();
            let combined_text = if has_text { self.combined_text() } else { String::new() };

            if is_text_only && self.flags.is_empty() && has_text && !self.children_truncated {
                if self.field.is_some() {
                    return Value::String(combined_text);
                } else {
                    let mut obj = Map::new();
                    obj.insert(self.name, Value::String(combined_text));
                    return Value::Object(obj);
                }
            }

            let mut obj = Map::new();
            if self.field.is_none() {
                obj.insert(KEY_TYPE.into(), Value::String(self.name));
            }

            for flag in self.flags {
                obj.insert(flag, Value::Bool(true));
            }

            let mut array_children: Vec<Value> = Vec::new();
            for entry in self.content_children {
                if is_test_anon_text_entry(&entry) {
                    continue;
                }
                if let Some(field_name) = entry.field {
                    match obj.remove(&field_name) {
                        Some(Value::Array(mut arr)) => {
                            arr.push(entry.value);
                            obj.insert(field_name, Value::Array(arr));
                        }
                        Some(existing) => {
                            obj.insert(field_name, Value::Array(vec![existing, entry.value]));
                        }
                        None => {
                            obj.insert(field_name, entry.value);
                        }
                    }
                } else {
                    array_children.push(entry.value);
                }
            }

            if self.children_truncated && is_text_only {
                // truncated — drop surviving text
            } else if is_text_only && has_text {
                obj.insert(KEY_TEXT.into(), Value::String(combined_text));
            } else if !array_children.is_empty() {
                obj.insert(KEY_CHILDREN.into(), Value::Array(array_children));
            }

            Value::Object(obj)
        }
    }

    #[test]
    fn test_field_backed_text_leaf_lifted() {
        let xml = r#"<parent><name field="name">Foo</name></parent>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("name").unwrap(), "Foo");
        assert!(!obj.contains_key(KEY_CHILDREN));
    }

    #[test]
    fn test_field_backed_structural_node_lifted() {
        let xml = r#"<parent><body field="body"><block>{ }</block></body></parent>"#;
        let result = xml_fragment_to_json(xml, None);
        let obj = result.as_object().unwrap();
        let body = obj.get("body").unwrap().as_object().unwrap();
        assert!(body.contains_key(KEY_CHILDREN));
        assert!(!body.contains_key(KEY_TYPE));
    }

    #[test]
    fn test_non_field_child_gets_type() {
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
        assert_eq!(obj.get("name").unwrap(), "Foo");
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
        let xml = r#"<a><b field="b"><c>deep</c></b></a>"#;
        let result = xml_fragment_to_json(xml, Some(2));
        let obj = result.as_object().unwrap();
        let b = obj.get("b").unwrap().as_object().unwrap();
        assert!(!b.contains_key(KEY_TYPE));

        let result1 = xml_fragment_to_json(xml, Some(1));
        let obj1 = result1.as_object().unwrap();
        assert!(!obj1.contains_key("b"));
        assert_eq!(obj1.get(KEY_TYPE).unwrap(), "a");
    }
}
