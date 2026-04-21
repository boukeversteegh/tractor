//! C# code renderer
//!
//! Renders tractor's semantic XML back to C# source code.
//! Supports: class, struct, interface, enum, property, field, method,
//! constructor declarations, with array and base-list type support.

use super::{
    get_child, get_child_text, get_children, has_marker, text_content, RenderError, RenderOptions,
};
use crate::languages::csharp::{ACCESS_MODIFIERS, OTHER_MODIFIERS, semantic::*};
use crate::xpath::XmlNode;

/// Render a single XML node to C# source code
pub fn render_node(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    match node {
        XmlNode::Element { name, .. } => match name.as_str() {
            CLASS => render_class(node, opts),
            STRUCT => render_struct(node, opts),
            INTERFACE => render_interface(node, opts),
            RECORD => render_record(node, opts),
            ENUM => render_enum(node, opts),
            PROPERTY => render_property(node, opts),
            FIELD => render_field(node, opts),
            METHOD => render_method(node, opts),
            CONSTRUCTOR => render_constructor(node, opts),
            UNIT => render_unit(node, opts),
            NAMESPACE => render_namespace(node, opts),
            IMPORT => render_import(node, opts),
            COMMENT => render_comment(node, opts),
            _ => Err(RenderError::UnsupportedNode(name.clone())),
        },
        XmlNode::Text(t) => Ok(t.clone()),
        _ => Ok(String::new()),
    }
}

// ---------------------------------------------------------------------------
// Modifiers (consts imported from languages::csharp)
// ---------------------------------------------------------------------------

/// Collect all modifier markers from a node, in canonical C# order
fn collect_modifiers(node: &XmlNode) -> Vec<&'static str> {
    let mut mods = Vec::new();

    // Access modifiers first
    for &m in ACCESS_MODIFIERS {
        if has_marker(node, m) {
            mods.push(m);
        }
    }
    // Then other modifiers in canonical order
    for &m in OTHER_MODIFIERS {
        if has_marker(node, m) {
            mods.push(m);
        }
    }

    mods
}

fn modifiers_str(node: &XmlNode) -> String {
    let mods = collect_modifiers(node);
    if mods.is_empty() {
        String::new()
    } else {
        format!("{} ", mods.join(" "))
    }
}

// ---------------------------------------------------------------------------
// Type rendering
// ---------------------------------------------------------------------------

/// Render a <type> element to C# type syntax
fn render_type(node: &XmlNode) -> Result<String, RenderError> {
    match node {
        XmlNode::Element { name, children, .. } if name == TYPE => {
            if has_marker(node, GENERIC) {
                return render_generic_type(node);
            }
            let has_nullable = has_marker(node, NULLABLE);

            let type_name: String = children
                .iter()
                .filter_map(|c| match c {
                    XmlNode::Text(t) => Some(t.trim().to_string()),
                    XmlNode::Element {
                        name, children: ch, ..
                    } if (name == NULLABLE || name == GENERIC) && ch.is_empty() => None,
                    XmlNode::Element { name: n, .. } if n == TYPE => render_type(c).ok(),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");

            if has_nullable {
                Ok(format!("{}?", type_name))
            } else {
                Ok(type_name)
            }
        }
        XmlNode::Element { name, .. } if name == ARRAY => {
            let inner = get_child(node, TYPE).ok_or_else(|| RenderError::MissingChild {
                parent: ARRAY.into(),
                child: TYPE.into(),
            })?;
            Ok(format!("{}[]", render_type(inner)?))
        }
        _ => text_content(node).ok_or_else(|| RenderError::MissingChild {
            parent: TYPE.into(),
            child: "text".into(),
        }),
    }
}

/// Render a type-position child — looks first for direct <type>/<array>,
/// then falls back to the <variable> wrapper emitted by the C# parser for
/// field declarations (`<field><variable><type>…</type><declarator>…</declarator></variable></field>`).
fn render_type_slot(node: &XmlNode, parent_label: &str) -> Result<String, RenderError> {
    if let Some(t) = get_child(node, TYPE) {
        return render_type(t);
    }
    if let Some(a) = get_child(node, ARRAY) {
        return render_type(a);
    }
    if let Some(v) = get_child(node, VARIABLE) {
        if let Some(t) = get_child(v, TYPE) {
            return render_type(t);
        }
        if let Some(a) = get_child(v, ARRAY) {
            return render_type(a);
        }
    }
    Err(RenderError::MissingChild {
        parent: parent_label.into(),
        child: TYPE.into(),
    })
}

/// Render a `<bases>` list: `: Foo, IBar` or `: byte` (enum underlying type).
/// Accepts either `<ref>` children (inheritance) or `<type>` children
/// (enum underlying type).
fn render_base_list(node: &XmlNode) -> String {
    let base = match get_child(node, BASES) {
        Some(b) => b,
        None => return String::new(),
    };
    let XmlNode::Element { children, .. } = base else {
        return String::new();
    };
    let items: Vec<String> = children
        .iter()
        .filter_map(|c| match c {
            XmlNode::Element { name, .. } if name == REF => text_content(c),
            XmlNode::Element { name, .. } if name == TYPE => render_type(c).ok(),
            _ => None,
        })
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if items.is_empty() {
        String::new()
    } else {
        format!(" : {}", items.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Literals (for field/property initializers, enum values, attribute args)
// ---------------------------------------------------------------------------

/// Render a typed literal element (`<int>`, `<float>`, `<bool>`, `<null>`,
/// `<string>`) back to C# source form. Returns `None` when the node isn't a
/// recognised literal.
///
/// Strings are reassembled from the parser's
/// `<string>"<string_literal_content>…</string_literal_content>"</string>`
/// shape so internal whitespace is preserved verbatim while the
/// pretty-printer's surrounding whitespace is discarded.
fn render_literal(node: &XmlNode) -> Option<String> {
    let XmlNode::Element { name, children, .. } = node else {
        return None;
    };
    match name.as_str() {
        "int" | "float" | "bool" | "null" => text_content(node).map(|t| t.trim().to_string()),
        "string" => {
            let inner = children
                .iter()
                .find_map(|c| match c {
                    XmlNode::Element { name: n, .. } if n == "string_literal_content" => {
                        text_content(c)
                    }
                    _ => None,
                })
                .unwrap_or_default();
            Some(format!("\"{}\"", inner))
        }
        _ => None,
    }
}

/// Find the first literal-valued child of `node` and render it.
/// Accepts either a direct literal child (fields: `<declarator>…<int>5</int></declarator>`)
/// or a `<value>` wrapper (properties, enum members: `<value><int>2</int></value>`).
fn render_literal_slot(node: &XmlNode) -> Option<String> {
    let XmlNode::Element { name, children, .. } = node else {
        return None;
    };
    if name == VALUE {
        // Inside <value>, look for the first literal element.
        return children.iter().find_map(render_literal);
    }
    children.iter().find_map(render_literal)
}

/// Render a generic type like <type><generic/>List<arguments>...</arguments></type>
fn render_generic_type(node: &XmlNode) -> Result<String, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Err(RenderError::UnsupportedNode("expected element".into()));
    };

    // Get the type name (text node after <generic/> marker)
    let type_name: String = children
        .iter()
        .filter_map(|c| match c {
            XmlNode::Text(t) => {
                let trimmed = t.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    // Get type arguments
    let args_node = get_child(node, ARGUMENTS);
    let type_args = if let Some(args) = args_node {
        let arg_types: Vec<String> = get_children(args, TYPE)
            .iter()
            .filter_map(|t| render_type(t).ok())
            .collect();
        if arg_types.is_empty() {
            String::new()
        } else {
            format!("<{}>", arg_types.join(", "))
        }
    } else {
        String::new()
    };

    Ok(format!("{}{}", type_name, type_args))
}

// ---------------------------------------------------------------------------
// Attributes
// ---------------------------------------------------------------------------

/// Render <attributes> block
fn render_attributes(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let attrs = get_children(node, ATTRIBUTES);
    if attrs.is_empty() {
        return Ok(String::new());
    }

    let indent = opts.current_indent();
    let mut result = String::new();

    for attr_list in attrs {
        let items: Vec<String> = get_children(attr_list, ATTRIBUTE)
            .iter()
            .filter_map(|a| render_single_attribute(a).ok())
            .collect();

        if !items.is_empty() {
            result.push_str(&format!("{}[{}]{}", indent, items.join(", "), opts.newline));
        }
    }

    Ok(result)
}

fn render_single_attribute(node: &XmlNode) -> Result<String, RenderError> {
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: ATTRIBUTE.into(),
            child: NAME.into(),
        })?;

    let args = get_child(node, ARGUMENTS);
    if let Some(args_node) = args {
        let arg_values: Vec<String> = get_children(args_node, ARGUMENT)
            .iter()
            .filter_map(|a| text_content(a).map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();
        if !arg_values.is_empty() {
            return Ok(format!("{}({})", name, arg_values.join(", ")));
        }
    }

    Ok(name)
}

// ---------------------------------------------------------------------------
// Accessors
// ---------------------------------------------------------------------------

fn render_accessors(node: &XmlNode) -> Result<String, RenderError> {
    let accessors_node = match get_child(node, ACCESSORS) {
        Some(n) => n,
        None => return Ok(String::new()),
    };

    let accessors: Vec<String> = get_children(accessors_node, ACCESSOR)
        .iter()
        .filter_map(|a| {
            let text = text_content(a)?;
            let trimmed = text.trim().trim_end_matches(';').trim().to_string();
            Some(format!("{};", trimmed))
        })
        .collect();

    if accessors.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{{ {} }}", accessors.join(" ")))
    }
}

// ---------------------------------------------------------------------------
// Property
// ---------------------------------------------------------------------------

fn render_property(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();

    // Attributes (e.g., [Required])
    let attrs = render_attributes(node, opts)?;

    // Modifiers
    let mods = modifiers_str(node);

    // Type (may be <type> or <array>)
    let type_str = render_type_slot(node, PROPERTY)?;

    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: PROPERTY.into(),
        child: NAME.into(),
    })?;

    // Accessors
    let accessors = render_accessors(node)?;

    // Optional initializer: `<value>…</value>` after the accessors renders as
    // `= <literal>;`. Property initializers always end with `;`, unlike the
    // bare accessor form.
    let initializer = get_child(node, VALUE).and_then(render_literal_slot);

    let decl = match initializer {
        Some(init) => format!("{}{}{} {} {} = {};", indent, mods, type_str, name, accessors, init),
        None => format!("{}{}{} {} {}", indent, mods, type_str, name, accessors),
    };

    Ok(format!("{}{}", attrs, decl))
}

// ---------------------------------------------------------------------------
// Field
// ---------------------------------------------------------------------------

fn render_field(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();

    let attrs = render_attributes(node, opts)?;
    let mods = modifiers_str(node);

    // Type — fields may have type as a child element or as a variable/declarator structure
    let type_str = render_type_slot(node, FIELD)?;

    // Locate the declarator that carries name + optional initializer.
    let declarator = get_child(node, VARIABLE)
        .and_then(|v| get_child(v, DECLARATOR))
        .or_else(|| get_child(node, DECLARATOR));

    let name = declarator
        .and_then(|d| get_child_text(d, NAME))
        .or_else(|| get_child_text(node, NAME))
        .ok_or_else(|| RenderError::MissingChild {
            parent: FIELD.into(),
            child: NAME.into(),
        })?;

    // Optional field initializer: a literal directly inside the declarator
    // (the parser places it after the `=` text node).
    let initializer = declarator.and_then(render_literal_slot);

    let decl = match initializer {
        Some(init) => format!("{}{}{} {} = {};", indent, mods, type_str, name, init),
        None => format!("{}{}{} {};", indent, mods, type_str, name),
    };
    Ok(format!("{}{}", attrs, decl))
}

// ---------------------------------------------------------------------------
// Class / Struct
// ---------------------------------------------------------------------------

fn render_class(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    render_type_declaration(node, CLASS, opts)
}

fn render_struct(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    render_type_declaration(node, STRUCT, opts)
}

fn render_interface(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    render_type_declaration(node, INTERFACE, opts)
}

/// Render a record declaration. Supports the positional form
/// (`public record User(string Name);`) and the body form
/// (`public record User(string Name) { ... }`). Presence of a primary
/// parameter list is optional.
fn render_record(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let attrs = render_attributes(node, opts)?;
    let mods = modifiers_str(node);
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: RECORD.into(),
            child: NAME.into(),
        })?;

    let params = if get_child(node, PARAMETERS).is_some() {
        render_parameters(node)
    } else {
        String::new()
    };
    let base = render_base_list(node);

    let header = format!("{}{}{} {}{}{}", indent, mods, RECORD, name, params, base);

    // If a body exists, render members inside Allman braces; otherwise emit the
    // positional-only statement form with a trailing semicolon.
    if get_child(node, BODY).is_some() {
        let body_opts = opts.indented();
        let members = collect_body_members(node, &body_opts)?;
        let mut result = String::new();
        result.push_str(&attrs);
        result.push_str(&header);
        result.push_str(&format!("{}{}{{{}", opts.newline, indent, opts.newline));
        for (i, member) in members.iter().enumerate() {
            result.push_str(member);
            result.push_str(&opts.newline);
            if i < members.len() - 1 {
                result.push_str(&opts.newline);
            }
        }
        result.push_str(&format!("{}}}", indent));
        Ok(result)
    } else {
        Ok(format!("{}{};", attrs, header))
    }
}

fn render_type_declaration(
    node: &XmlNode,
    keyword: &str,
    opts: &RenderOptions,
) -> Result<String, RenderError> {
    let indent = opts.current_indent();

    let attrs = render_attributes(node, opts)?;
    let mods = modifiers_str(node);

    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: keyword.into(),
        child: NAME.into(),
    })?;

    let base = render_base_list(node);

    // Collect body members
    let body_opts = opts.indented();
    let members = collect_body_members(node, &body_opts)?;

    let mut result = String::new();
    result.push_str(&attrs);
    result.push_str(&format!("{}{}{} {}{}", indent, mods, keyword, name, base));
    result.push_str(&format!("{}{}{{{}", opts.newline, indent, opts.newline));

    for (i, member) in members.iter().enumerate() {
        result.push_str(member);
        result.push_str(&opts.newline);
        // Add blank line between members (except after last)
        if i < members.len() - 1 {
            result.push_str(&opts.newline);
        }
    }

    result.push_str(&format!("{}}}", indent));

    Ok(result)
}

// ---------------------------------------------------------------------------
// Enum
// ---------------------------------------------------------------------------

fn render_enum(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let attrs = render_attributes(node, opts)?;
    let mods = modifiers_str(node);
    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: ENUM.into(),
        child: NAME.into(),
    })?;

    let member_indent = opts.indented().current_indent();
    let members: Vec<String> = collect_enum_members(node)
        .iter()
        .map(|(n, v)| match v {
            Some(val) => format!("{}{} = {}", member_indent, n, val),
            None => format!("{}{}", member_indent, n),
        })
        .collect();

    let base = render_base_list(node);

    let mut result = String::new();
    result.push_str(&attrs);
    result.push_str(&format!("{}{}{} {}{}", indent, mods, ENUM, name, base));
    result.push_str(&format!("{}{}{{{}", opts.newline, indent, opts.newline));
    result.push_str(&members.join(&format!(",{}", opts.newline)));
    if !members.is_empty() {
        result.push_str(&opts.newline);
    }
    result.push_str(&format!("{}}}", indent));
    Ok(result)
}

/// Return enum members as (name, optional value) pairs.
/// Accepts either a flat list of <enum_member> children or a <body> wrapper.
/// The name is extracted from the `<name>` wrapper (parser shape wraps it in
/// `<ref>…</ref>`, but `text_content` handles that). The value goes through
/// `render_literal_slot` so typed literals like `<value><int>2</int></value>`
/// render correctly.
fn collect_enum_members(node: &XmlNode) -> Vec<(String, Option<String>)> {
    let container = get_child(node, BODY).unwrap_or(node);
    get_children(container, ENUM_MEMBER)
        .iter()
        .filter_map(|m| {
            let n = get_child_text(m, NAME).map(|s| s.trim().to_string())?;
            let v = get_child(m, VALUE).and_then(render_literal_slot);
            Some((n, v))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Method / Constructor
// ---------------------------------------------------------------------------

fn render_parameters(node: &XmlNode) -> String {
    let params_node = match get_child(node, PARAMETERS) {
        Some(p) => p,
        None => return "()".to_string(),
    };
    let parts: Vec<String> = get_children(params_node, PARAMETER)
        .iter()
        .filter_map(|p| {
            let ty = render_type_slot(p, PARAMETER).ok()?;
            let name = get_child_text(p, NAME)?;
            Some(format!("{} {}", ty, name))
        })
        .collect();
    format!("({})", parts.join(", "))
}

fn render_method(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let attrs = render_attributes(node, opts)?;
    let mods = modifiers_str(node);

    let return_type = if let Some(returns) = get_child(node, RETURNS) {
        render_type_slot(returns, RETURNS)?
    } else {
        render_type_slot(node, METHOD).unwrap_or_else(|_| "void".into())
    };

    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: METHOD.into(),
        child: NAME.into(),
    })?;

    let params = render_parameters(node);

    // If a <body> is present, render `{ }`. Otherwise treat as a signature (`;`).
    let tail = if get_child(node, BODY).is_some() {
        format!("{}{}{{{}{}}}", opts.newline, indent, opts.newline, indent)
    } else {
        ";".to_string()
    };

    let decl = format!("{}{}{} {}{}{}", indent, mods, return_type, name, params, tail);
    Ok(format!("{}{}", attrs, decl))
}

fn render_constructor(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let attrs = render_attributes(node, opts)?;
    let mods = modifiers_str(node);
    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: CONSTRUCTOR.into(),
        child: NAME.into(),
    })?;
    let params = render_parameters(node);
    let tail = if get_child(node, BODY).is_some() {
        format!("{}{}{{{}{}}}", opts.newline, indent, opts.newline, indent)
    } else {
        ";".to_string()
    };
    let decl = format!("{}{}{}{}{}", indent, mods, name, params, tail);
    Ok(format!("{}{}", attrs, decl))
}

/// Collect renderable body members from a class/struct node
fn collect_body_members(node: &XmlNode, opts: &RenderOptions) -> Result<Vec<String>, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Ok(Vec::new());
    };

    let mut members = Vec::new();

    // Look for members directly in children (after flatten transform, body members
    // may be direct children or inside a <body> element)
    let body_children = if let Some(body) = get_child(node, BODY) {
        if let XmlNode::Element { children: bc, .. } = body {
            bc.as_slice()
        } else {
            children.as_slice()
        }
    } else {
        children.as_slice()
    };

    for child in body_children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                PROPERTY | FIELD | METHOD | CONSTRUCTOR
                | CLASS | STRUCT | INTERFACE | ENUM | RECORD | COMMENT => {
                    members.push(render_node(child, opts)?);
                }
                // Skip non-renderable elements (body wrapper text like { })
                _ => {}
            }
        }
    }

    Ok(members)
}

// ---------------------------------------------------------------------------
// Namespace
// ---------------------------------------------------------------------------

fn render_namespace(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: NAMESPACE.into(),
            child: NAME.into(),
        })?;

    let has_body = get_child(node, BODY).is_some();

    if has_body {
        let body_opts = opts.indented();
        let members = collect_namespace_members(node, &body_opts)?;
        let mut result = format!(
            "{}namespace {}{}{}{{{}",
            indent, name, opts.newline, indent, opts.newline
        );
        for (i, member) in members.iter().enumerate() {
            result.push_str(member);
            result.push_str(&opts.newline);
            if i < members.len() - 1 {
                result.push_str(&opts.newline);
            }
        }
        result.push_str(&format!("{}}}", indent));
        Ok(result)
    } else {
        // File-scoped: just the declaration + members at same indent level
        let members = collect_namespace_members(node, opts)?;
        let mut result = format!("{}namespace {};{}{}", indent, name, opts.newline, opts.newline);
        for member in &members {
            result.push_str(member);
            result.push_str(&opts.newline);
        }
        Ok(result)
    }
}

fn collect_namespace_members(
    node: &XmlNode,
    opts: &RenderOptions,
) -> Result<Vec<String>, RenderError> {
    // Parser form wraps members in <body>; hand-authored shapes may put them
    // directly under the namespace.
    let container = get_child(node, BODY).unwrap_or(node);
    let XmlNode::Element { children, .. } = container else {
        return Ok(Vec::new());
    };

    let mut members = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                CLASS | STRUCT | INTERFACE | ENUM | RECORD | IMPORT | COMMENT => {
                    members.push(render_node(child, opts)?);
                }
                _ => {}
            }
        }
    }
    Ok(members)
}

// ---------------------------------------------------------------------------
// Import (using directive)
// ---------------------------------------------------------------------------

fn render_import(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    // Parser shapes:
    //   <import>using<ref>System</ref>;</import>
    //   <import>using<qualified_name>..dotted..</qualified_name>;</import>
    // Both reduce to "using <namespace>;" — extract the namespace from whichever
    // child is present.
    let ns = extract_qualified_name(node)
        .or_else(|| text_content(node).map(|t| strip_import_keywords(&t)))
        .unwrap_or_default();
    let ns = ns.trim();
    if ns.is_empty() {
        return Ok(format!("{}using ;", opts.current_indent()));
    }
    Ok(format!("{}using {};", opts.current_indent(), ns))
}

/// Collect a dotted namespace from a `<qualified_name>` or `<ref>` child.
/// Pretty-printed XML puts whitespace between the dots and the refs, so we
/// strip whitespace from the raw text content — `System.Collections.Generic`
/// is whitespace-free regardless of formatting.
fn extract_qualified_name(node: &XmlNode) -> Option<String> {
    if let Some(q) = get_child(node, "qualified_name") {
        return Some(strip_whitespace(&text_content(q)?));
    }
    if let Some(r) = get_child(node, REF) {
        return Some(strip_whitespace(&text_content(r)?));
    }
    None
}

fn strip_import_keywords(text: &str) -> String {
    let cleaned = strip_whitespace(text);
    cleaned
        .strip_prefix("using")
        .unwrap_or(&cleaned)
        .trim_end_matches(';')
        .to_string()
}

fn strip_whitespace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

// ---------------------------------------------------------------------------
// Comment
// ---------------------------------------------------------------------------

fn render_comment(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let text = text_content(node).unwrap_or_default();
    Ok(format!("{}{}", opts.current_indent(), text.trim()))
}

// ---------------------------------------------------------------------------
// Unit (compilation_unit — top-level file)
// ---------------------------------------------------------------------------

fn render_unit(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Ok(String::new());
    };

    let mut parts: Vec<(&str, String)> = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                IMPORT | NAMESPACE | CLASS | STRUCT | INTERFACE | ENUM | RECORD | COMMENT => {
                    parts.push((name.as_str(), render_node(child, opts)?));
                }
                _ => {}
            }
        }
    }

    // Join with a blank line between different kinds (e.g. imports → types) and
    // between top-level declarations; consecutive imports stay tight.
    let mut result = String::new();
    for (i, (kind, text)) in parts.iter().enumerate() {
        if i > 0 {
            let prev_kind = parts[i - 1].0;
            let tight = prev_kind == IMPORT && *kind == IMPORT;
            result.push_str(&opts.newline);
            if !tight {
                result.push_str(&opts.newline);
            }
        }
        result.push_str(text);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::parse_xml;

    // Round-trip coverage for the C# renderer lives in the fixture-based
    // integration test `tractor/tests/render_roundtrip.rs`, which reads
    // `tests/integration/render/csharp/supported.cs` as its single snapshot.
    // The tests below anchor individual renderer behaviors on hand-authored
    // XML, independent of the parser, and document the input contract.

    fn render_xml(xml: &str) -> String {
        let node = parse_xml(xml).unwrap();
        render_node(&node, &RenderOptions::default()).unwrap()
    }

    #[test]
    fn property_from_flat_shape() {
        let xml = r#"<property><public/><type>string</type><name>Name</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
        assert_eq!(render_xml(xml), "public string Name { get; set; }");
    }

    #[test]
    fn field_from_variable_wrapper() {
        // The parser emits fields as <field><variable><type>..</type><declarator><name>..</name></declarator></variable></field>
        let xml = r#"<field><private/><readonly/><variable><type>int</type><declarator><name>_count</name></declarator></variable></field>"#;
        assert_eq!(render_xml(xml), "private readonly int _count;");
    }

    #[test]
    fn property_indented_by_option() {
        let xml = r#"<property><public/><type>string</type><name>Name</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
        let opts = RenderOptions {
            indent_level: 1,
            ..Default::default()
        };
        let node = parse_xml(xml).unwrap();
        assert_eq!(
            render_node(&node, &opts).unwrap(),
            "    public string Name { get; set; }"
        );
    }
}
