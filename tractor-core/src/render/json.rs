//! JSON renderer: render a data-tree xot tree back to JSON source code.
//!
//! This is the inverse of the JSON data transform: given the data tree
//! (where object keys are element names and scalars are text content),
//! produce syntactically valid JSON source.
//!
//! ## Element classification
//!
//! Elements with `field="property"` came from JSON `pair` nodes (key-value pairs).
//! They render as `"key": value`. Elements without this attribute are collection
//! items (array elements) and render as bare values.
//!
//! ## Examples
//!
//! Data tree:
//! ```xml
//! <name field="property">Alice</name>
//! <age field="property">30</age>
//! <db field="property">
//!   <host field="property">localhost</host>
//! </db>
//! ```
//!
//! Renders to:
//! ```json
//! {
//!   "name": "Alice",
//!   "age": 30,
//!   "db": {
//!     "host": "localhost"
//!   }
//! }
//! ```

use super::{RenderOptions, SpanMap};
use crate::xpath::XmlNode;

/// Render a data-tree XmlNode to JSON source code.
///
/// The `node` should be the content root (e.g., File element) whose children
/// are the top-level properties/items.
pub fn render_node(node: &XmlNode, opts: &RenderOptions) -> Result<String, super::RenderError> {
    let mut buf = String::new();
    let mut span_map = SpanMap::new();
    render_value(node, opts, &mut buf, &mut span_map)?;
    buf.push_str(&opts.newline);
    Ok(buf)
}

/// Render a data-tree XmlNode to JSON source code, tracking value spans.
pub fn render_node_tracked(
    node: &XmlNode,
    opts: &RenderOptions,
) -> Result<(String, SpanMap), super::RenderError> {
    let mut buf = String::new();
    let mut span_map = SpanMap::new();
    render_value(node, opts, &mut buf, &mut span_map)?;
    buf.push_str(&opts.newline);
    Ok((buf, span_map))
}

/// Render a node as a JSON value (object, array, or scalar).
fn render_value(
    node: &XmlNode,
    opts: &RenderOptions,
    buf: &mut String,
    span_map: &mut SpanMap,
) -> Result<(), super::RenderError> {
    match node {
        XmlNode::Element {
            name: _,
            attributes,
            children,
        } => {
            let element_children = element_children(children);
            let text = text_content(children);

            // Track the value span for elements with a `start` attribute
            let start_pos = buf.len();

            if element_children.is_empty() {
                // Leaf node — render as scalar value
                if let Some(text) = &text {
                    let kind = get_attr(attributes, "kind");
                    render_scalar(text, kind.as_deref(), buf);
                } else {
                    // Empty element with no text — render as empty object
                    buf.push_str("{}");
                }
            } else {
                // Has element children — determine if object or array
                let all_properties = element_children
                    .iter()
                    .all(|c| is_property_element(c));

                if all_properties {
                    render_object(&element_children, opts, buf, span_map)?;
                } else {
                    // Mixed or no properties — render as array
                    render_array(&element_children, opts, buf, span_map)?;
                }
            }

            record_span(attributes, start_pos, buf.len(), span_map);
            Ok(())
        }
        XmlNode::Text(text) => {
            render_scalar(text, None, buf);
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Render an object: `{ "key": value, ... }`
fn render_object(
    properties: &[&XmlNode],
    opts: &RenderOptions,
    buf: &mut String,
    span_map: &mut SpanMap,
) -> Result<(), super::RenderError> {
    if properties.is_empty() {
        buf.push_str("{}");
        return Ok(());
    }

    let inner_opts = opts.indented();
    let indent = inner_opts.current_indent();
    let outer_indent = opts.current_indent();

    buf.push('{');

    for (i, prop) in properties.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        buf.push_str(&opts.newline);
        buf.push_str(&indent);

        if let XmlNode::Element {
            name,
            attributes,
            children,
            ..
        } = prop
        {
            // Use original key from `key` attribute if present (sanitized names)
            let key = get_attr(attributes, "key").unwrap_or_else(|| name.clone());
            buf.push('"');
            buf.push_str(&escape_json_string(&key));
            buf.push_str("\": ");

            let element_kids = element_children(children);
            let text = text_content(children);

            // Track value span (starts after "key": )
            let value_start = buf.len();

            if element_kids.is_empty() {
                if let Some(text) = &text {
                    let kind = get_attr(attributes, "kind");
                    render_scalar(text, kind.as_deref(), buf);
                } else {
                    buf.push_str("{}");
                }
            } else {
                let all_props = element_kids.iter().all(|c| is_property_element(c));
                if all_props {
                    render_object(&element_kids, &inner_opts, buf, span_map)?;
                } else {
                    render_array(&element_kids, &inner_opts, buf, span_map)?;
                }
            }

            record_span(attributes, value_start, buf.len(), span_map);
        }
    }

    buf.push_str(&opts.newline);
    buf.push_str(&outer_indent);
    buf.push('}');
    Ok(())
}

/// Render an array: `[ value, ... ]`
fn render_array(
    items: &[&XmlNode],
    opts: &RenderOptions,
    buf: &mut String,
    span_map: &mut SpanMap,
) -> Result<(), super::RenderError> {
    if items.is_empty() {
        buf.push_str("[]");
        return Ok(());
    }

    let inner_opts = opts.indented();
    let indent = inner_opts.current_indent();
    let outer_indent = opts.current_indent();

    buf.push('[');

    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        buf.push_str(&opts.newline);
        buf.push_str(&indent);
        render_value(item, &inner_opts, buf, span_map)?;
    }

    buf.push_str(&opts.newline);
    buf.push_str(&outer_indent);
    buf.push(']');
    Ok(())
}

/// Record the byte span of a node's value in the span map, keyed by (line, column).
fn record_span(
    attributes: &[(String, String)],
    start: usize,
    end: usize,
    span_map: &mut SpanMap,
) {
    if let (Some(line), Some(col)) = (
        get_attr(attributes, "line").and_then(|v| v.parse::<u32>().ok()),
        get_attr(attributes, "column").and_then(|v| v.parse::<u32>().ok()),
    ) {
        span_map.insert((line, col), (start, end));
    }
}

/// Render a scalar text value using the `type` attribute when available,
/// falling back to heuristic auto-detection.
///
/// With `type` attribute:
/// - `"string"` → JSON string
/// - `"number"` → JSON number (bare)
/// - `"true"` / `"false"` → JSON boolean
/// - `"null"` → JSON null
///
/// Without `type` (fallback heuristic):
/// - `"true"` / `"false"` / `"null"` → literal
/// - Parseable as number → JSON number
/// - Otherwise → JSON string
fn render_scalar(text: &str, scalar_type: Option<&str>, buf: &mut String) {
    match scalar_type {
        Some("string") => {
            buf.push('"');
            buf.push_str(&escape_json_string(text));
            buf.push('"');
        }
        Some("number") => {
            buf.push_str(text);
        }
        Some("true") | Some("false") | Some("null") => {
            buf.push_str(text);
        }
        _ => {
            // Fallback: heuristic auto-detection for trees without type info
            match text {
                "true" | "false" | "null" => buf.push_str(text),
                _ => {
                    if text.parse::<f64>().is_ok() && !text.is_empty() {
                        buf.push_str(text);
                    } else {
                        buf.push('"');
                        buf.push_str(&escape_json_string(text));
                        buf.push('"');
                    }
                }
            }
        }
    }
}

/// Escape a string for JSON output.
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// Get an attribute value.
fn get_attr(attributes: &[(String, String)], name: &str) -> Option<String> {
    attributes
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
}

/// Check if an XmlNode element has a `field` attribute (marks it as a property).
fn is_property_element(node: &XmlNode) -> bool {
    matches!(node, XmlNode::Element { attributes, .. } if get_attr(attributes, "field").is_some())
}

/// Get all element children from a list of XmlNode children.
fn element_children(children: &[XmlNode]) -> Vec<&XmlNode> {
    children
        .iter()
        .filter(|c| matches!(c, XmlNode::Element { .. }))
        .collect()
}

/// Get concatenated text content from children.
fn text_content(children: &[XmlNode]) -> Option<String> {
    let mut result = String::new();
    for child in children {
        if let XmlNode::Text(t) = child {
            result.push_str(t);
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_prop(name: &str, text: &str) -> XmlNode {
        XmlNode::Element {
            name: name.to_string(),
            attributes: vec![("field".to_string(), name.to_string())],
            children: vec![XmlNode::Text(text.to_string())],
        }
    }

    fn make_prop_obj(name: &str, children: Vec<XmlNode>) -> XmlNode {
        XmlNode::Element {
            name: name.to_string(),
            attributes: vec![("field".to_string(), name.to_string())],
            children,
        }
    }

    fn make_container(name: &str, children: Vec<XmlNode>) -> XmlNode {
        XmlNode::Element {
            name: name.to_string(),
            attributes: vec![],
            children,
        }
    }

    fn opts() -> RenderOptions {
        RenderOptions {
            indent: "  ".to_string(),
            indent_level: 0,
            newline: "\n".to_string(),
        }
    }

    #[test]
    fn simple_object() {
        let root = make_container(
            "File",
            vec![
                make_prop("name", "Alice"),
                make_prop("age", "30"),
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn nested_object() {
        let root = make_container(
            "File",
            vec![
                make_prop("name", "myapp"),
                make_prop_obj(
                    "db",
                    vec![
                        make_prop("host", "localhost"),
                        make_prop("port", "5432"),
                    ],
                ),
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "myapp");
        assert_eq!(parsed["db"]["host"], "localhost");
        assert_eq!(parsed["db"]["port"], 5432);
    }

    #[test]
    fn boolean_and_null_values() {
        let root = make_container(
            "File",
            vec![
                make_prop("active", "true"),
                make_prop("deleted", "false"),
                make_prop("notes", "null"),
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["active"], true);
        assert_eq!(parsed["deleted"], false);
        assert!(parsed["notes"].is_null());
    }

    #[test]
    fn string_value_with_escaping() {
        let root = make_container(
            "File",
            vec![make_prop("msg", "hello \"world\"\nnewline")],
        );
        let result = render_node(&root, &opts()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["msg"], "hello \"world\"\nnewline");
    }

    #[test]
    fn roundtrip_renders_valid_json() {
        let root = make_container(
            "File",
            vec![
                make_prop("a", "1"),
                make_prop("b", "hello"),
                make_prop_obj(
                    "c",
                    vec![
                        make_prop("d", "true"),
                        make_prop("e", "null"),
                    ],
                ),
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        // Must be valid JSON
        let _: serde_json::Value = serde_json::from_str(&result)
            .unwrap_or_else(|e| panic!("invalid JSON: {}\n{}", e, result));
    }

    fn make_typed_prop(name: &str, text: &str, kind: &str) -> XmlNode {
        XmlNode::Element {
            name: name.to_string(),
            attributes: vec![
                ("field".to_string(), name.to_string()),
                ("kind".to_string(), kind.to_string()),
            ],
            children: vec![XmlNode::Text(text.to_string())],
        }
    }

    #[test]
    fn type_attribute_overrides_heuristic() {
        // Without type attr, "true" would render as bare `true` (boolean).
        // With type="string", it must render as `"true"` (string).
        let root = make_container(
            "File",
            vec![
                make_typed_prop("flag", "true", "string"),      // string "true", not boolean
                make_typed_prop("count", "42", "string"),        // string "42", not number
                make_typed_prop("nothing", "null", "string"),    // string "null", not null
                make_typed_prop("active", "true", "true"),       // boolean true
                make_typed_prop("pi", "3.14", "number"),         // number
                make_typed_prop("empty", "null", "null"),        // null
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        // type="string" forces quoting even for "true", "42", "null"
        assert_eq!(parsed["flag"], "true");
        assert_eq!(parsed["count"], "42");
        assert_eq!(parsed["nothing"], "null");
        // type="true"/"number"/"null" renders as bare values
        assert_eq!(parsed["active"], true);
        #[allow(clippy::approx_constant)] // test value, not meant as PI
        let pi_value = 3.14;
        assert_eq!(parsed["pi"], pi_value);
        assert!(parsed["empty"].is_null());
    }

    #[test]
    fn sanitized_key_uses_key_attr() {
        // When a JSON key is sanitized (e.g., "my-key" → "my_key"), the original
        // key is stored in a `key` attribute.
        let root = make_container(
            "File",
            vec![XmlNode::Element {
                name: "my_key".to_string(),
                attributes: vec![
                    ("field".to_string(), "my_key".to_string()),
                    ("key".to_string(), "my-key".to_string()),
                ],
                children: vec![XmlNode::Text("value".to_string())],
            }],
        );
        let result = render_node(&root, &opts()).unwrap();
        assert!(result.contains("\"my-key\""), "should use original key from key attr");
    }

    #[test]
    fn tracked_render_records_spans() {
        let root = make_container(
            "File",
            vec![
                XmlNode::Element {
                    name: "name".to_string(),
                    attributes: vec![
                        ("field".to_string(), "name".to_string()),
                        ("line".to_string(), "1".to_string()),
                        ("column".to_string(), "10".to_string()),
                    ],
                    children: vec![XmlNode::Text("Alice".to_string())],
                },
            ],
        );
        let (rendered, spans) = render_node_tracked(&root, &opts()).unwrap();
        let (start, end) = spans[&(1, 10)];
        // The value span should cover the rendered scalar "Alice"
        assert_eq!(&rendered[start..end], "\"Alice\"");
    }
}
