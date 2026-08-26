//! TypeScript code renderer
//!
//! Renders tractor's semantic XML back to TypeScript source. Scope is
//! data-structure constructs only — interfaces, type aliases, enums, and
//! classes with typed fields — matching the supported-fixture round-trip
//! test in `tractor/tests/render_roundtrip.rs`.

use super::{
    get_child, get_child_text, get_children, has_marker, text_content, RenderError, RenderOptions,
};
use crate::xpath::XmlNode;

const PROGRAM: &str = "program";
const INTERFACE: &str = "interface";
const CLASS: &str = "class";
const ENUM: &str = "enum";
const TYPEALIAS: &str = "typealias";
const FIELD: &str = "field";
const NAME: &str = "name";
const TYPE: &str = "type";
const ARRAY: &str = "array";
const BODY: &str = "body";
const VALUE: &str = "value";
const OPTIONAL: &str = "optional";
const IMPORT: &str = "import";
const EXPORT: &str = "export";
const COMMENT: &str = "comment";

pub fn render_node(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    match node {
        XmlNode::Element { name, .. } => match name.as_str() {
            PROGRAM => render_program(node, opts),
            INTERFACE => render_interface(node, opts),
            CLASS => render_class(node, opts),
            ENUM => render_enum(node, opts),
            TYPEALIAS => render_typealias(node, opts),
            FIELD => render_field(node, opts),
            IMPORT | EXPORT => render_passthrough(node, opts),
            COMMENT => render_comment(node, opts),
            _ => Err(RenderError::UnsupportedNode(name.clone())),
        },
        XmlNode::Text(t) => Ok(t.clone()),
        _ => Ok(String::new()),
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Render a TypeScript type: `<type>string</type>` → `string`,
/// `<array><type>string</type></array>` → `string[]`. When the node is a
/// `<type>` containing a nested `<array>` we descend; generic types are out
/// of scope for this batch.
fn render_type(node: &XmlNode) -> Result<String, RenderError> {
    let XmlNode::Element { name, children, .. } = node else {
        return text_content(node).ok_or_else(|| RenderError::MissingChild {
            parent: TYPE.into(),
            child: "text".into(),
        });
    };

    if name == ARRAY {
        let inner = children
            .iter()
            .find_map(|c| match c {
                XmlNode::Element { name: n, .. } if n == TYPE || n == ARRAY => render_type(c).ok(),
                _ => None,
            })
            .unwrap_or_default();
        return Ok(format!("{}[]", inner));
    }

    // <type> — may wrap another <array> for `string[]`, or hold plain text.
    if let Some(a) = get_child(node, ARRAY) {
        return render_type(a);
    }
    Ok(text_content(node)
        .unwrap_or_default()
        .trim()
        .to_string())
}

fn render_type_slot(node: &XmlNode) -> Option<String> {
    if let Some(t) = get_child(node, TYPE) {
        return render_type(t).ok();
    }
    if let Some(a) = get_child(node, ARRAY) {
        return render_type(a).ok();
    }
    None
}

// ---------------------------------------------------------------------------
// Field — shared shape with Python/C#: <field><required|optional/><name>…</name><type>…</type></field>
// ---------------------------------------------------------------------------

fn render_field(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: FIELD.into(),
            child: NAME.into(),
        })?;
    let optional = if has_marker(node, OPTIONAL) { "?" } else { "" };
    let ty = render_type_slot(node).unwrap_or_default();
    if ty.is_empty() {
        Ok(format!("{}{}{};", indent, name, optional))
    } else {
        Ok(format!("{}{}{}: {};", indent, name, optional, ty))
    }
}

// ---------------------------------------------------------------------------
// Interface / Class
// ---------------------------------------------------------------------------

fn render_interface(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    render_type_declaration(node, "interface", INTERFACE, opts)
}

fn render_class(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    render_type_declaration(node, "class", CLASS, opts)
}

fn render_type_declaration(
    node: &XmlNode,
    keyword: &str,
    parent: &str,
    opts: &RenderOptions,
) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let name = declaration_name(node, parent)?;
    let body_opts = opts.indented();
    let members = collect_body_members(node, &body_opts)?;
    let mut result = format!("{}{} {} {{", indent, keyword, name);
    result.push_str(&opts.newline);
    for member in &members {
        result.push_str(member);
        result.push_str(&opts.newline);
    }
    result.push_str(&format!("{}}}", indent));
    Ok(result)
}

fn declaration_name(node: &XmlNode, parent: &str) -> Result<String, RenderError> {
    get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: parent.into(),
            child: NAME.into(),
        })
}

fn collect_body_members(
    node: &XmlNode,
    opts: &RenderOptions,
) -> Result<Vec<String>, RenderError> {
    let container = get_child(node, BODY).unwrap_or(node);
    let XmlNode::Element { children, .. } = container else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                FIELD | INTERFACE | CLASS | ENUM | TYPEALIAS | COMMENT => {
                    out.push(render_node(child, opts)?);
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Enum
// ---------------------------------------------------------------------------

fn render_enum(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let name = declaration_name(node, ENUM)?;
    let member_indent = opts.indented().current_indent();

    let container = get_child(node, BODY).unwrap_or(node);
    let members: Vec<String> = get_children(container, NAME)
        .iter()
        .filter_map(|m| text_content(m).map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .map(|m| format!("{}{}", member_indent, m))
        .collect();

    let mut result = format!("{}enum {} {{", indent, name);
    result.push_str(&opts.newline);
    result.push_str(&members.join(&format!(",{}", opts.newline)));
    if !members.is_empty() {
        result.push_str(&opts.newline);
    }
    result.push_str(&format!("{}}}", indent));
    Ok(result)
}

// ---------------------------------------------------------------------------
// Type alias: `type X = T;`
// ---------------------------------------------------------------------------

fn render_typealias(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let name = declaration_name(node, TYPEALIAS)?;
    let value = get_child(node, VALUE)
        .and_then(|v| render_type_slot(v).or_else(|| text_content(v).map(|t| t.trim().to_string())))
        .unwrap_or_default();
    Ok(format!("{}type {} = {};", indent, name, value))
}

// ---------------------------------------------------------------------------
// Pass-through for leaves like import/export rendered from raw text
// ---------------------------------------------------------------------------

fn render_passthrough(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let text = text_content(node).unwrap_or_default();
    Ok(format!("{}{}", opts.current_indent(), text.trim()))
}

fn render_comment(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let text = text_content(node).unwrap_or_default();
    Ok(format!("{}{}", opts.current_indent(), text.trim()))
}

// ---------------------------------------------------------------------------
// Program (top-level)
// ---------------------------------------------------------------------------

fn render_program(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Ok(String::new());
    };
    let mut parts: Vec<(&str, String)> = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                IMPORT | EXPORT | INTERFACE | CLASS | ENUM | TYPEALIAS | COMMENT => {
                    parts.push((name.as_str(), render_node(child, opts)?));
                }
                _ => {}
            }
        }
    }

    // Consecutive imports/exports and consecutive comment lines stay tight.
    // Any other transition (comment → decl, decl → decl, import → decl) gets
    // a single blank line between them, which is the idiomatic TS style.
    let mut result = String::new();
    for (i, (kind, text)) in parts.iter().enumerate() {
        if i > 0 {
            let prev = parts[i - 1].0;
            let tight = prev == *kind && matches!(*kind, IMPORT | EXPORT | COMMENT);
            result.push_str(&opts.newline);
            if !tight {
                result.push_str(&opts.newline);
            }
        }
        result.push_str(text);
    }
    Ok(result)
}
