//! Java code renderer
//!
//! Renders tractor's semantic XML back to Java source. Scope is the
//! data-structure subset: package and import declarations, classes,
//! interfaces (with method signatures), records, enums (with constants),
//! fields (with modifiers, primitive/reference/generic/array types, and
//! literal initializers). Imperative code is out of scope.

use super::{
    get_child, get_child_text, get_children, has_marker, text_content, RenderError, RenderOptions,
};
use crate::languages::java::{ACCESS_MODIFIERS, OTHER_MODIFIERS};
use crate::xpath::XmlNode;

const PROGRAM: &str = "program";
const PACKAGE: &str = "package";
const IMPORT: &str = "import";
const CLASS: &str = "class";
const INTERFACE: &str = "interface";
const ENUM: &str = "enum";
const RECORD: &str = "record";
const FIELD: &str = "field";
const METHOD: &str = "method";
const NAME: &str = "name";
const TYPE: &str = "type";
const ARRAY: &str = "array";
const GENERIC: &str = "generic";
const TYPE_ARGS: &str = "type_arguments";
const BODY: &str = "body";
const PARAMETERS: &str = "parameters";
const PARAMS: &str = "params";
const PARAM: &str = "param";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const VALUE: &str = "value";
const ENUM_MEMBER: &str = "enum_member";
const COMMENT: &str = "comment";
const SCOPED_ID: &str = "scoped_identifier";

pub fn render_node(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    match node {
        XmlNode::Element { name, .. } => match name.as_str() {
            PROGRAM => render_program(node, opts),
            PACKAGE => render_package(node, opts),
            IMPORT => render_import(node, opts),
            CLASS => render_type_declaration(node, "class", CLASS, opts),
            INTERFACE => render_type_declaration(node, "interface", INTERFACE, opts),
            RECORD => render_record(node, opts),
            ENUM => render_enum(node, opts),
            FIELD => render_field(node, opts),
            METHOD => render_method(node, opts),
            COMMENT => render_comment(node, opts),
            _ => Err(RenderError::UnsupportedNode(name.clone())),
        },
        XmlNode::Text(t) => Ok(t.clone()),
        _ => Ok(String::new()),
    }
}

// ---------------------------------------------------------------------------
// Modifiers
// ---------------------------------------------------------------------------

fn modifiers_str(node: &XmlNode) -> String {
    let mut mods = Vec::new();
    for &m in ACCESS_MODIFIERS {
        // `package-private` is the canonical absent-modifier marker — emit
        // nothing for it so round-trips preserve the source's lack of a
        // keyword.
        if m == "package-private" {
            continue;
        }
        if has_marker(node, m) {
            mods.push(m);
        }
    }
    for &m in OTHER_MODIFIERS {
        if has_marker(node, m) {
            mods.push(m);
        }
    }
    if mods.is_empty() {
        String::new()
    } else {
        format!("{} ", mods.join(" "))
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Render a Java type: primitive / reference / generic / array.
fn render_type(node: &XmlNode) -> Result<String, RenderError> {
    let XmlNode::Element { name, children, .. } = node else {
        return text_content(node).ok_or_else(|| RenderError::MissingChild {
            parent: TYPE.into(),
            child: "text".into(),
        });
    };

    match name.as_str() {
        GENERIC => {
            // <generic><type>List</type><type_arguments><type>String</type>...</type_arguments></generic>
            let base = children
                .iter()
                .find_map(|c| match c {
                    XmlNode::Element { name: n, .. } if n == TYPE => render_type(c).ok(),
                    _ => None,
                })
                .unwrap_or_default();
            let args = get_child(node, TYPE_ARGS)
                .map(|a| {
                    get_children(a, TYPE)
                        .iter()
                        .filter_map(|t| render_type(t).ok())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            Ok(format!("{}<{}>", base, args))
        }
        ARRAY => {
            let inner = children
                .iter()
                .find_map(|c| match c {
                    XmlNode::Element { name: n, .. } if n == TYPE || n == ARRAY || n == GENERIC => {
                        render_type(c).ok()
                    }
                    _ => None,
                })
                .unwrap_or_default();
            Ok(format!("{}[]", inner))
        }
        TYPE => {
            // Plain <type>String</type> or wrapper containing another type.
            if let Some(g) = get_child(node, GENERIC) {
                return render_type(g);
            }
            if let Some(a) = get_child(node, ARRAY) {
                return render_type(a);
            }
            Ok(text_content(node)
                .unwrap_or_default()
                .trim()
                .to_string())
        }
        _ => Ok(text_content(node).unwrap_or_default().trim().to_string()),
    }
}

fn render_type_slot(node: &XmlNode) -> Option<String> {
    for candidate in [TYPE, GENERIC, ARRAY] {
        if let Some(c) = get_child(node, candidate) {
            return render_type(c).ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Literals (field initializers, enum constant values, default values)
// ---------------------------------------------------------------------------

/// Extract a typed literal (`<int>`, `<float>`, `<string>`, `<true/>`,
/// `<false/>`, `<null>`) from a `<value>` slot, reassembling Java string
/// literals from `<string_fragment>` content.
fn render_value(node: &XmlNode) -> Option<String> {
    let XmlNode::Element { children, .. } = node else {
        return None;
    };
    children.iter().find_map(render_literal)
}

fn render_literal(node: &XmlNode) -> Option<String> {
    let XmlNode::Element { name, children, .. } = node else {
        return None;
    };
    match name.as_str() {
        "int" | "float" | "true" | "false" | "null" => {
            text_content(node).map(|t| t.trim().to_string())
        }
        "string" => {
            let content = children
                .iter()
                .find_map(|c| match c {
                    XmlNode::Element { name: n, .. } if n == "string_fragment" => text_content(c),
                    _ => None,
                })
                .unwrap_or_default();
            Some(format!("\"{}\"", content))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Package / Import — Java uses dotted `scoped_identifier` for qualified names
// ---------------------------------------------------------------------------

fn dotted_text(node: &XmlNode) -> String {
    // Recurse through nested <scoped_identifier> and pull <name> text children
    // in order, joining with dots.
    fn walk(node: &XmlNode, out: &mut Vec<String>) {
        if let XmlNode::Element { name, children, .. } = node {
            match name.as_str() {
                SCOPED_ID => {
                    for child in children {
                        walk(child, out);
                    }
                }
                NAME => {
                    if let Some(t) = text_content(node) {
                        let t = t.trim().to_string();
                        if !t.is_empty() {
                            out.push(t);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut parts = Vec::new();
    walk(node, &mut parts);
    parts.join(".")
}

fn render_package(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let dotted = get_child(node, SCOPED_ID)
        .map(dotted_text)
        .or_else(|| get_child_text(node, NAME).map(|s| s.trim().to_string()))
        .unwrap_or_default();
    Ok(format!("{}package {};", opts.current_indent(), dotted))
}

fn render_import(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let dotted = get_child(node, SCOPED_ID)
        .map(dotted_text)
        .or_else(|| get_child_text(node, NAME).map(|s| s.trim().to_string()))
        .unwrap_or_default();
    Ok(format!("{}import {};", opts.current_indent(), dotted))
}

// ---------------------------------------------------------------------------
// Field / Method
// ---------------------------------------------------------------------------

fn render_field(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let mods = modifiers_str(node);
    let ty = render_type_slot(node).ok_or_else(|| RenderError::MissingChild {
        parent: FIELD.into(),
        child: TYPE.into(),
    })?;
    let declarator = get_child(node, VARIABLE_DECLARATOR).ok_or_else(|| RenderError::MissingChild {
        parent: FIELD.into(),
        child: VARIABLE_DECLARATOR.into(),
    })?;
    let name = get_child_text(declarator, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: VARIABLE_DECLARATOR.into(),
            child: NAME.into(),
        })?;
    let init = get_child(declarator, VALUE).and_then(render_value);

    let decl = match init {
        Some(v) => format!("{}{}{} {} = {};", indent, mods, ty, name, v),
        None => format!("{}{}{} {};", indent, mods, ty, name),
    };
    Ok(decl)
}

fn render_method(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let mods = modifiers_str(node);
    let ty = render_type_slot(node).unwrap_or_else(|| "void".into());
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: METHOD.into(),
            child: NAME.into(),
        })?;
    let params = render_parameters(node);
    // Interface / abstract methods have no body and end with `;`.
    // Concrete methods end with an empty body `{ }` — imperative statements
    // are out of scope for this batch.
    let tail = if get_child(node, BODY).is_some() {
        format!(" {{{}{}{}}}", opts.newline, indent, opts.newline.clone() + &indent)
    } else {
        ";".to_string()
    };
    let tail = if tail.contains('\n') {
        // Simplified body: "{\n    <indent>}" — keep it empty on one line instead.
        " {}".to_string()
    } else {
        tail
    };
    Ok(format!("{}{}{} {}{}{}", indent, mods, ty, name, params, tail))
}

fn render_parameters(node: &XmlNode) -> String {
    // Parser shape: <parameters><params>(<param>...</param>,...)</params></parameters>
    let list = get_child(node, PARAMETERS)
        .and_then(|p| get_child(p, PARAMS).or(Some(p)))
        .or_else(|| get_child(node, PARAMS));
    let list = match list {
        Some(l) => l,
        None => return "()".to_string(),
    };
    let parts: Vec<String> = get_children(list, PARAM)
        .iter()
        .filter_map(|p| {
            let ty = render_type_slot(p)?;
            let name = get_child_text(p, NAME).map(|s| s.trim().to_string())?;
            Some(format!("{} {}", ty, name))
        })
        .collect();
    format!("({})", parts.join(", "))
}

// ---------------------------------------------------------------------------
// Class / Interface
// ---------------------------------------------------------------------------

fn render_type_declaration(
    node: &XmlNode,
    keyword: &str,
    parent: &str,
    opts: &RenderOptions,
) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let mods = modifiers_str(node);
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: parent.into(),
            child: NAME.into(),
        })?;

    let body_opts = opts.indented();
    let members = collect_body_members(node, &body_opts)?;

    let mut result = format!("{}{}{} {} {{", indent, mods, keyword, name);
    result.push_str(&opts.newline);
    for (i, member) in members.iter().enumerate() {
        result.push_str(member);
        result.push_str(&opts.newline);
        if i < members.len() - 1 {
            result.push_str(&opts.newline);
        }
    }
    result.push_str(&format!("{}}}", indent));
    Ok(result)
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
                FIELD | METHOD | CLASS | INTERFACE | ENUM | RECORD | COMMENT => {
                    out.push(render_node(child, opts)?);
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Record
// ---------------------------------------------------------------------------

fn render_record(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let mods = modifiers_str(node);
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: RECORD.into(),
            child: NAME.into(),
        })?;
    let params = render_parameters(node);

    // Java records always require a (possibly empty) body block after the
    // parameter list. Empty body renders as `{}` inline; non-empty bodies
    // would open a block — deferred to a later batch along with imperative
    // content.
    let body_members: Vec<String> = collect_body_members(node, &opts.indented())?;
    if body_members.is_empty() {
        Ok(format!("{}{}record {}{} {{}}", indent, mods, name, params))
    } else {
        let mut result = format!("{}{}record {}{} {{", indent, mods, name, params);
        result.push_str(&opts.newline);
        for (i, m) in body_members.iter().enumerate() {
            result.push_str(m);
            result.push_str(&opts.newline);
            if i < body_members.len() - 1 {
                result.push_str(&opts.newline);
            }
        }
        result.push_str(&format!("{}}}", indent));
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Enum
// ---------------------------------------------------------------------------

fn render_enum(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let mods = modifiers_str(node);
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: ENUM.into(),
            child: NAME.into(),
        })?;
    let member_indent = opts.indented().current_indent();

    let container = get_child(node, BODY).unwrap_or(node);
    let members: Vec<String> = get_children(container, ENUM_MEMBER)
        .iter()
        .filter_map(|m| get_child_text(m, NAME).map(|s| s.trim().to_string()))
        .filter(|t| !t.is_empty())
        .map(|m| format!("{}{}", member_indent, m))
        .collect();

    let mut result = format!("{}{}enum {} {{", indent, mods, name);
    result.push_str(&opts.newline);
    result.push_str(&members.join(&format!(",{}", opts.newline)));
    if !members.is_empty() {
        result.push_str(&opts.newline);
    }
    result.push_str(&format!("{}}}", indent));
    Ok(result)
}

// ---------------------------------------------------------------------------
// Comment / Program
// ---------------------------------------------------------------------------

fn render_comment(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let text = text_content(node).unwrap_or_default();
    Ok(format!("{}{}", opts.current_indent(), text.trim()))
}

fn render_program(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Ok(String::new());
    };
    let mut parts: Vec<(&str, String)> = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                PACKAGE | IMPORT | CLASS | INTERFACE | ENUM | RECORD | COMMENT => {
                    parts.push((name.as_str(), render_node(child, opts)?));
                }
                _ => {}
            }
        }
    }

    let mut result = String::new();
    for (i, (kind, text)) in parts.iter().enumerate() {
        if i > 0 {
            let prev = parts[i - 1].0;
            let tight = prev == *kind && matches!(*kind, IMPORT | COMMENT);
            result.push_str(&opts.newline);
            if !tight {
                result.push_str(&opts.newline);
            }
        }
        result.push_str(text);
    }
    Ok(result)
}
