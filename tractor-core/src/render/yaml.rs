//! YAML renderer: render a data-tree back to YAML source code.
//!
//! This is the inverse of the YAML data transform: given the data tree
//! (where mapping keys are element names and scalars are text content),
//! produce syntactically valid YAML source.
//!
//! Uses the same `field` attribute convention as the JSON renderer to
//! distinguish mapping properties from sequence items.

use super::RenderOptions;
use crate::xpath::XmlNode;

/// Render a data-tree XmlNode to YAML source code.
pub fn render_node(node: &XmlNode, opts: &RenderOptions) -> Result<String, super::RenderError> {
    let mut buf = String::new();
    render_top(node, opts, &mut buf)?;
    // Ensure trailing newline
    if !buf.ends_with('\n') {
        buf.push_str(&opts.newline);
    }
    Ok(buf)
}

/// Render the top-level node (File or document).
fn render_top(
    node: &XmlNode,
    opts: &RenderOptions,
    buf: &mut String,
) -> Result<(), super::RenderError> {
    match node {
        XmlNode::Element { children, name, .. } => {
            let element_kids = element_children(children);

            let has_documents = element_kids.iter().any(|c| {
                matches!(c, XmlNode::Element { name, .. } if name == "document")
            });

            if has_documents {
                // All children must be documents — reject mixed trees
                let all_documents = element_kids.iter().all(|c| {
                    matches!(c, XmlNode::Element { name, .. } if name == "document")
                });
                if !all_documents {
                    let bad_names: Vec<_> = element_kids.iter()
                        .filter_map(|c| match c {
                            XmlNode::Element { name, .. } if name != "document" => Some(name.as_str()),
                            _ => None,
                        })
                        .collect();
                    return Err(super::RenderError::UnsupportedNode(
                        format!("File contains <document> mixed with non-document children: {:?}. \
                                 Insert XPath must resolve to an existing parent inside the document.", bad_names)
                    ));
                }
                for (i, child) in element_kids.iter().enumerate() {
                    if i > 0 {
                        buf.push_str("---");
                        buf.push_str(&opts.newline);
                    }
                    render_value(child, opts, buf, true)?;
                }
            } else if name == "document" {
                // Single document node passed directly
                render_mapping(&element_kids, opts, buf)?;
            } else {
                // File node with direct properties (single-doc, no document wrapper)
                render_mapping(&element_kids, opts, buf)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Render a node as a YAML value (mapping, sequence, or scalar).
///
/// `at_top` indicates we're rendering the body of a document (no extra indent).
fn render_value(
    node: &XmlNode,
    opts: &RenderOptions,
    buf: &mut String,
    at_top: bool,
) -> Result<(), super::RenderError> {
    match node {
        XmlNode::Element {
            attributes,
            children,
            ..
        } => {
            let element_kids = element_children(children);
            let text = text_content(children);

            if element_kids.is_empty() {
                // Leaf node — render as scalar value
                if let Some(text) = &text {
                    let kind = get_attr(attributes, "kind");
                    render_scalar(text, kind.as_deref(), buf);
                } else {
                    // Empty element with no text — render as empty mapping
                    buf.push_str("{}");
                }
            } else {
                // Has element children — determine if mapping or sequence
                let all_properties = element_kids.iter().all(|c| is_property_element(c));

                if all_properties {
                    if at_top {
                        render_mapping(&element_kids, opts, buf)?;
                    } else {
                        buf.push_str(&opts.newline);
                        render_mapping(&element_kids, &opts.indented(), buf)?;
                    }
                } else {
                    if at_top {
                        render_sequence(&element_kids, opts, buf)?;
                    } else {
                        buf.push_str(&opts.newline);
                        render_sequence(&element_kids, &opts.indented(), buf)?;
                    }
                }
            }
            Ok(())
        }
        XmlNode::Text(text) => {
            render_scalar(text, None, buf);
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Render a mapping: `key: value` pairs, each on its own line.
fn render_mapping(
    properties: &[&XmlNode],
    opts: &RenderOptions,
    buf: &mut String,
) -> Result<(), super::RenderError> {
    let indent = opts.current_indent();

    for (i, prop) in properties.iter().enumerate() {
        if i > 0 {
            // No extra blank lines between properties
        }
        if let XmlNode::Element {
            name,
            attributes,
            children,
            ..
        } = prop
        {
            buf.push_str(&indent);

            // Use original key from `key` attribute if present (sanitized names)
            let key = get_attr(attributes, "key").unwrap_or_else(|| name.clone());
            let quoted_key = yaml_quote_key(&key);
            buf.push_str(&quoted_key);
            buf.push(':');

            let element_kids = element_children(children);
            let text = text_content(children);

            if element_kids.is_empty() {
                // Scalar value
                buf.push(' ');
                if let Some(text) = &text {
                    let kind = get_attr(attributes, "kind");
                    render_scalar(text, kind.as_deref(), buf);
                } else {
                    buf.push_str("{}");
                }
                buf.push_str(&opts.newline);
            } else {
                // Nested mapping or sequence
                let all_props = element_kids.iter().all(|c| is_property_element(c));
                if all_props {
                    buf.push_str(&opts.newline);
                    render_mapping(&element_kids, &opts.indented(), buf)?;
                } else {
                    buf.push_str(&opts.newline);
                    render_sequence(&element_kids, &opts.indented(), buf)?;
                }
            }
        }
    }
    Ok(())
}

/// Render a sequence: `- value` items, each on its own line.
fn render_sequence(
    items: &[&XmlNode],
    opts: &RenderOptions,
    buf: &mut String,
) -> Result<(), super::RenderError> {
    let indent = opts.current_indent();

    for item in items {
        buf.push_str(&indent);
        buf.push_str("- ");
        render_value(item, opts, buf, false)?;
        // render_value for scalars doesn't add newline, so add one
        if is_scalar(item) {
            buf.push_str(&opts.newline);
        }
    }
    Ok(())
}

/// Render a scalar value.
///
/// YAML scalars are plain by default. Values that could be misinterpreted
/// (booleans, nulls, or strings containing special characters) are quoted.
fn render_scalar(text: &str, scalar_type: Option<&str>, buf: &mut String) {
    match scalar_type {
        Some("string") => {
            // Explicitly typed as string — quote if it could be misinterpreted
            if needs_yaml_quoting(text) {
                yaml_quote_string(text, buf);
            } else {
                buf.push_str(text);
            }
        }
        Some("number") | Some("boolean") | Some("null") => {
            // Bare literal
            buf.push_str(text);
        }
        _ => {
            // No type info — use heuristic: quote if ambiguous
            if needs_yaml_quoting(text) {
                yaml_quote_string(text, buf);
            } else {
                buf.push_str(text);
            }
        }
    }
}

/// Check if a string needs quoting in YAML.
fn needs_yaml_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }

    // Values that look like booleans, null, or numbers
    match s {
        "true" | "false" | "True" | "False" | "TRUE" | "FALSE"
        | "yes" | "no" | "Yes" | "No" | "YES" | "NO"
        | "on" | "off" | "On" | "Off" | "ON" | "OFF"
        | "null" | "Null" | "NULL" | "~" => return true,
        _ => {}
    }

    // Contains characters that need quoting
    if s.contains(':') || s.contains('#') || s.contains('\n')
        || s.contains('"') || s.contains('\'')
        || s.starts_with('&') || s.starts_with('*')
        || s.starts_with('!') || s.starts_with('|')
        || s.starts_with('>') || s.starts_with('%')
        || s.starts_with('@') || s.starts_with('`')
        || s.starts_with('{') || s.starts_with('}')
        || s.starts_with('[') || s.starts_with(']')
        || s.starts_with(',') || s.starts_with('?')
        || s.starts_with('-') || s.starts_with(' ')
        || s.ends_with(' ')
    {
        return true;
    }

    false
}

/// Quote a string for YAML using double quotes.
fn yaml_quote_string(s: &str, buf: &mut String) {
    buf.push('"');
    for c in s.chars() {
        match c {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                buf.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => buf.push(c),
        }
    }
    buf.push('"');
}

/// Quote a mapping key if needed.
fn yaml_quote_key(key: &str) -> String {
    if needs_yaml_quoting(key) {
        let mut buf = String::new();
        yaml_quote_string(key, &mut buf);
        buf
    } else {
        key.to_string()
    }
}

/// Check if an XmlNode element has a `field` attribute (marks it as a property).
fn is_property_element(node: &XmlNode) -> bool {
    matches!(node, XmlNode::Element { attributes, .. } if get_attr(attributes, "field").is_some())
}

/// Check if a node is a scalar (leaf with text, no element children).
fn is_scalar(node: &XmlNode) -> bool {
    match node {
        XmlNode::Element { children, .. } => {
            element_children(children).is_empty()
        }
        XmlNode::Text(_) => true,
        _ => true,
    }
}

/// Get an attribute value.
fn get_attr(attributes: &[(String, String)], name: &str) -> Option<String> {
    attributes
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
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
    fn simple_mapping() {
        let root = make_container(
            "File",
            vec![
                make_prop("name", "Alice"),
                make_prop("age", "30"),
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        assert_eq!(result, "name: Alice\nage: 30\n");
    }

    #[test]
    fn nested_mapping() {
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
        assert_eq!(
            result,
            "name: myapp\ndb:\n  host: localhost\n  port: 5432\n"
        );
    }

    #[test]
    fn sequence() {
        let root = make_container(
            "File",
            vec![
                make_prop_obj(
                    "tags",
                    vec![
                        make_container("tags", vec![XmlNode::Text("admin".to_string())]),
                        make_container("tags", vec![XmlNode::Text("user".to_string())]),
                    ],
                ),
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        assert_eq!(result, "tags:\n  - admin\n  - user\n");
    }

    #[test]
    fn document_wrapper() {
        let root = make_container(
            "File",
            vec![make_container(
                "document",
                vec![
                    make_prop("name", "Alice"),
                    make_prop("age", "30"),
                ],
            )],
        );
        let result = render_node(&root, &opts()).unwrap();
        assert_eq!(result, "name: Alice\nage: 30\n");
    }

    #[test]
    fn multi_document() {
        let root = make_container(
            "File",
            vec![
                make_container(
                    "document",
                    vec![make_prop("name", "Alice")],
                ),
                make_container(
                    "document",
                    vec![make_prop("name", "Bob")],
                ),
            ],
        );
        let result = render_node(&root, &opts()).unwrap();
        assert_eq!(result, "name: Alice\n---\nname: Bob\n");
    }

    #[test]
    fn string_needing_quoting() {
        let root = make_container(
            "File",
            vec![make_prop("value", "true")],
        );
        let result = render_node(&root, &opts()).unwrap();
        // "true" as a string value needs quoting to avoid being parsed as boolean
        assert_eq!(result, "value: \"true\"\n");
    }

    #[test]
    fn typed_string_no_quoting() {
        // kind="string" but value doesn't need quoting
        let root = make_container(
            "File",
            vec![XmlNode::Element {
                name: "host".to_string(),
                attributes: vec![
                    ("field".to_string(), "host".to_string()),
                    ("kind".to_string(), "string".to_string()),
                ],
                children: vec![XmlNode::Text("localhost".to_string())],
            }],
        );
        let result = render_node(&root, &opts()).unwrap();
        assert_eq!(result, "host: localhost\n");
    }

    #[test]
    fn typed_number() {
        let root = make_container(
            "File",
            vec![XmlNode::Element {
                name: "port".to_string(),
                attributes: vec![
                    ("field".to_string(), "port".to_string()),
                    ("kind".to_string(), "number".to_string()),
                ],
                children: vec![XmlNode::Text("5432".to_string())],
            }],
        );
        let result = render_node(&root, &opts()).unwrap();
        assert_eq!(result, "port: 5432\n");
    }

    #[test]
    fn sanitized_key_uses_key_attr() {
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
        assert!(result.contains("my-key:"), "should use original key from key attr");
    }
}
