//! Code synthesis: render tractor XML back to source code
//!
//! This module provides the inverse of the parse+transform pipeline:
//! given tractor's semantic XML (the same format output by `tractor -x ...`),
//! render it back to syntactically valid source code.
//!
//! Each language implements its own renderer that knows the syntax rules
//! for its constructs. The renderer operates on `XmlNode` trees.

pub mod csharp;

use crate::xpath::XmlNode;

/// Errors that can occur during rendering
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("unsupported node type: {0}")]
    UnsupportedNode(String),

    #[error("missing required child '{child}' in <{parent}>")]
    MissingChild { parent: String, child: String },

    #[error("parse error: {0}")]
    ParseError(String),
}

/// Options for controlling rendered output
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Indentation string (default: 4 spaces)
    pub indent: String,
    /// Current indentation level
    pub indent_level: usize,
    /// Newline string
    pub newline: String,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            indent: "    ".to_string(),
            indent_level: 0,
            newline: "\n".to_string(),
        }
    }
}

impl RenderOptions {
    fn indented(&self) -> Self {
        Self {
            indent_level: self.indent_level + 1,
            ..self.clone()
        }
    }

    fn current_indent(&self) -> String {
        self.indent.repeat(self.indent_level)
    }
}

/// Parse an XML string into an XmlNode tree
pub fn parse_xml(input: &str) -> Result<XmlNode, RenderError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(input);
    let mut stack: Vec<(String, Vec<(String, String)>, Vec<XmlNode>)> = Vec::new();
    let mut root: Option<XmlNode> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&a.value).to_string();
                        (key, val)
                    })
                    .collect();
                stack.push((name, attrs, Vec::new()));
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&a.value).to_string();
                        (key, val)
                    })
                    .collect();
                let node = XmlNode::Element {
                    name,
                    attributes: attrs,
                    children: Vec::new(),
                };
                if let Some(parent) = stack.last_mut() {
                    parent.2.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(Event::End(_)) => {
                if let Some((name, attrs, children)) = stack.pop() {
                    let node = XmlNode::Element {
                        name,
                        attributes: attrs,
                        children,
                    };
                    if let Some(parent) = stack.last_mut() {
                        parent.2.push(node);
                    } else {
                        root = Some(node);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if !text.is_empty() {
                    if let Some(parent) = stack.last_mut() {
                        parent.2.push(XmlNode::Text(text));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(RenderError::ParseError(format!("{}", e))),
            _ => {}
        }
    }

    root.ok_or_else(|| RenderError::ParseError("empty input".to_string()))
}

/// Parse a JSON value (tractor's `-f json` output format) into an XmlNode tree.
///
/// Reverses the lifting rules from `xml_to_json`:
/// - `"key": true`           → `<key/>`          (self-closing marker)
/// - `"key": "text"`         → `<key>text</key>`  (text element)
/// - `"key": { ... }`        → `<key>...</key>`   (structural element, key as name)
/// - `"$type": "name"`       → element name
/// - `"children": [...]`     → ordered child elements
/// - `{ "name": "text" }`    → `<name>text</name>` (compact leaf in children array)
pub fn parse_json(input: &str) -> Result<XmlNode, RenderError> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| RenderError::ParseError(format!("invalid JSON: {}", e)))?;
    json_value_to_xmlnode(&value)
}

fn json_value_to_xmlnode(value: &serde_json::Value) -> Result<XmlNode, RenderError> {
    match value {
        serde_json::Value::Object(obj) => {
            // Determine element name: from $type key, or from single-key compact form
            let name = if let Some(serde_json::Value::String(t)) = obj.get("$type") {
                t.clone()
            } else if obj.len() == 1 {
                // Compact leaf: { "accessor": "get;" } or { "type": "string" }
                let (key, val) = obj.iter().next().unwrap();
                match val {
                    serde_json::Value::String(s) => {
                        return Ok(XmlNode::Element {
                            name: key.clone(),
                            attributes: Vec::new(),
                            children: vec![XmlNode::Text(s.clone())],
                        });
                    }
                    serde_json::Value::Bool(true) => {
                        return Ok(XmlNode::Element {
                            name: key.clone(),
                            attributes: Vec::new(),
                            children: Vec::new(),
                        });
                    }
                    serde_json::Value::Object(_) => {
                        // Single-key structural: { "body": { ... } }
                        let inner = json_object_to_children(val.as_object().unwrap())?;
                        return Ok(XmlNode::Element {
                            name: key.clone(),
                            attributes: Vec::new(),
                            children: inner,
                        });
                    }
                    _ => {
                        return Err(RenderError::ParseError(
                            format!("unexpected single-key value type for '{}'", key),
                        ));
                    }
                }
            } else {
                // Multi-key object without $type — could be a lifted field node
                // This shouldn't happen at the top level, but handle gracefully
                return Err(RenderError::ParseError(
                    "object without $type and multiple keys".to_string(),
                ));
            };

            let children = json_object_to_children(obj)?;
            Ok(XmlNode::Element {
                name,
                attributes: Vec::new(),
                children,
            })
        }
        serde_json::Value::String(s) => Ok(XmlNode::Text(s.clone())),
        _ => Err(RenderError::ParseError(format!("unexpected JSON value type at root"))),
    }
}

/// Convert a JSON object's properties into XmlNode children.
/// Handles booleans (markers), strings (text elements), objects (structural),
/// and the special "children" array.
fn json_object_to_children(obj: &serde_json::Map<String, serde_json::Value>) -> Result<Vec<XmlNode>, RenderError> {
    let mut children = Vec::new();

    // First pass: collect lifted properties (everything except $type and children)
    for (key, value) in obj {
        if key == "$type" || key == "children" {
            continue;
        }
        match value {
            serde_json::Value::Bool(true) => {
                // Marker: <public/>
                children.push(XmlNode::Element {
                    name: key.clone(),
                    attributes: Vec::new(),
                    children: Vec::new(),
                });
            }
            serde_json::Value::String(s) => {
                // Text element: <name>Foo</name>
                children.push(XmlNode::Element {
                    name: key.clone(),
                    attributes: Vec::new(),
                    children: vec![XmlNode::Text(s.clone())],
                });
            }
            serde_json::Value::Object(inner) => {
                // Lifted structural node: the key is the element name
                let inner_children = json_object_to_children(inner)?;
                children.push(XmlNode::Element {
                    name: key.clone(),
                    attributes: Vec::new(),
                    children: inner_children,
                });
            }
            _ => {
                // Skip unexpected types (false, numbers, null, arrays as properties)
            }
        }
    }

    // Second pass: append children array elements
    if let Some(serde_json::Value::Array(arr)) = obj.get("children") {
        for item in arr {
            children.push(json_value_to_xmlnode(item)?);
        }
    }

    Ok(children)
}

/// Parse input as either XML or JSON, auto-detecting the format.
/// Tries XML first (starts with '<'), falls back to JSON.
pub fn parse_input(input: &str) -> Result<XmlNode, RenderError> {
    let trimmed = input.trim_start();
    if trimmed.starts_with('<') {
        parse_xml(input)
    } else if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_json(input)
    } else {
        Err(RenderError::ParseError(
            "input doesn't look like XML or JSON".to_string(),
        ))
    }
}

/// Render an XmlNode tree to source code for the given language
pub fn render(node: &XmlNode, lang: &str, opts: &RenderOptions) -> Result<String, RenderError> {
    match lang {
        "csharp" => csharp::render_node(node, opts),
        _ => Err(RenderError::UnsupportedLanguage(lang.to_string())),
    }
}

// --- Shared helpers for renderers ---

/// Get a named child element from an XmlNode
pub fn get_child<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    if let XmlNode::Element { children, .. } = node {
        children
            .iter()
            .find(|c| matches!(c, XmlNode::Element { name: n, .. } if n == name))
    } else {
        None
    }
}

/// Get all child elements with a given name
pub fn get_children<'a>(node: &'a XmlNode, name: &str) -> Vec<&'a XmlNode> {
    if let XmlNode::Element { children, .. } = node {
        children
            .iter()
            .filter(|c| matches!(c, XmlNode::Element { name: n, .. } if n == name))
            .collect()
    } else {
        Vec::new()
    }
}

/// Check if a node has an empty-element child (marker like <public/>)
pub fn has_marker(node: &XmlNode, name: &str) -> bool {
    if let XmlNode::Element { children, .. } = node {
        children.iter().any(|c| matches!(c, XmlNode::Element { name: n, children: ch, .. } if n == name && ch.is_empty()))
    } else {
        false
    }
}

/// Get the text content of a child element (e.g., <name>Foo</name> → "Foo")
pub fn get_child_text(node: &XmlNode, child_name: &str) -> Option<String> {
    get_child(node, child_name).and_then(|c| text_content(c))
}

/// Get the text content of a node (concatenated text children)
pub fn text_content(node: &XmlNode) -> Option<String> {
    match node {
        XmlNode::Text(t) => Some(t.clone()),
        XmlNode::Element { children, .. } => {
            let mut result = String::new();
            for child in children {
                if let Some(t) = text_content(child) {
                    result.push_str(&t);
                }
            }
            if result.is_empty() {
                None
            } else {
                Some(result)
            }
        }
        _ => None,
    }
}
