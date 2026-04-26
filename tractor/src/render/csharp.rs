//! C# code renderer
//!
//! Renders tractor's semantic XML back to C# source code.
//! Supports: class, property, field declarations.

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
            PROPERTY => render_property(node, opts),
            FIELD => render_field(node, opts),
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
        _ => text_content(node).ok_or_else(|| RenderError::MissingChild {
            parent: TYPE.into(),
            child: "text".into(),
        }),
    }
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
    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: ATTRIBUTE.into(),
        child: NAME.into(),
    })?;

    let args = get_child(node, ARGUMENTS);
    if let Some(args_node) = args {
        let arg_values: Vec<String> = get_children(args_node, ARGUMENT)
            .iter()
            .filter_map(|a| text_content(a))
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
    let direct_accessors = match node {
        XmlNode::Element { children, .. } => children
            .iter()
            .filter(|child| {
                matches!(
                    child,
                    XmlNode::Element { name, .. }
                        if matches!(name.as_str(), GET | SET | INIT | ADD | REMOVE)
                )
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    let accessors: Vec<String> = if direct_accessors.is_empty() {
        // Backward-compatible support for older hand-written renderer
        // fixtures that still use <accessors><accessor>get;</accessor>...
        get_child(node, ACCESSORS)
            .map(|accessors_node| {
                get_children(accessors_node, ACCESSOR)
                    .iter()
                    .filter_map(|a| {
                        let text = text_content(a)?;
                        let trimmed = text.trim().trim_end_matches(';').trim().to_string();
                        Some(format!("{};", trimmed))
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        direct_accessors
            .iter()
            .filter_map(|a| match a {
                XmlNode::Element { name, .. } => Some(format!("{};", name)),
                _ => None,
            })
            .collect()
    };

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

    // Type
    let type_node = get_child(node, TYPE).ok_or_else(|| RenderError::MissingChild {
        parent: PROPERTY.into(),
        child: TYPE.into(),
    })?;
    let type_str = render_type(type_node)?;

    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: PROPERTY.into(),
        child: NAME.into(),
    })?;

    // Accessors
    let accessors = render_accessors(node)?;

    let decl = format!("{}{}{} {} {}", indent, mods, type_str, name, accessors);

    Ok(format!("{}{}", attrs, decl))
}

// ---------------------------------------------------------------------------
// Field
// ---------------------------------------------------------------------------

fn render_field(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();

    let attrs = render_attributes(node, opts)?;
    let mods = modifiers_str(node);

    // Type: transformed fields expose <type> directly.
    let type_str = if let Some(type_node) = get_child(node, TYPE) {
        render_type(type_node)?
    } else {
        return Err(RenderError::MissingChild {
            parent: FIELD.into(),
            child: TYPE.into(),
        });
    };

    // Name: current field shape uses <declarator><name>; keep the
    // legacy <variable> fallback for older serialized trees.
    let name = get_child_text(node, NAME)
        .or_else(|| {
            get_child(node, VARIABLE)
                .and_then(|v| get_child(v, DECLARATOR))
                .and_then(|d| get_child_text(d, NAME))
        })
        .or_else(|| get_child(node, DECLARATOR).and_then(|d| get_child_text(d, NAME)))
        .ok_or_else(|| RenderError::MissingChild {
            parent: FIELD.into(),
            child: NAME.into(),
        })?;

    let decl = format!("{}{}{} {};", indent, mods, type_str, name);
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

    // Collect body members
    let body_opts = opts.indented();
    let members = collect_body_members(node, &body_opts)?;

    let mut result = String::new();
    result.push_str(&attrs);
    result.push_str(&format!("{}{}{} {}", indent, mods, keyword, name));
    result.push_str(&format!("{}{{{}", opts.newline, opts.newline));

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
                PROPERTY | FIELD | CLASS | STRUCT | COMMENT => {
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
    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: NAMESPACE.into(),
        child: NAME.into(),
    })?;

    let has_body = get_child(node, BODY).is_some();

    if has_body {
        let body_opts = opts.indented();
        let members = collect_namespace_members(node, &body_opts)?;
        let mut result = format!("namespace {}{{{}", name, opts.newline);
        for member in &members {
            result.push_str(member);
            result.push_str(&opts.newline);
        }
        result.push('}');
        Ok(result)
    } else {
        // File-scoped: just the declaration + members at same indent level
        let members = collect_namespace_members(node, opts)?;
        let mut result = format!("namespace {};{}{}", name, opts.newline, opts.newline);
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
    let XmlNode::Element { children, .. } = node else {
        return Ok(Vec::new());
    };

    let mut members = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                CLASS | STRUCT | IMPORT | COMMENT => {
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
    let text = text_content(node).unwrap_or_default();
    // The text content should contain "using X;" or just the namespace
    let trimmed = text.trim();
    if trimmed.starts_with("using") {
        Ok(format!("{}{}", opts.current_indent(), trimmed))
    } else {
        Ok(format!("{}using {};", opts.current_indent(), trimmed))
    }
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

    let mut parts = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                IMPORT | NAMESPACE | CLASS | STRUCT | COMMENT => {
                    parts.push(render_node(child, opts)?);
                }
                _ => {}
            }
        }
    }

    Ok(parts.join(&format!("{}", opts.newline)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::parse_xml;

    #[test]
    fn test_render_simple_property() {
        let xml = r#"<property><public/><type>string</type><name>Name</name><get>get;</get><set>set;</set></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public string Name { get; set; }");
    }

    #[test]
    fn test_render_nullable_property() {
        let xml = r#"<property><public/><type>Guid<nullable/></type><name>UserId</name><get>get;</get><set>set;</set></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public Guid? UserId { get; set; }");
    }

    #[test]
    fn test_render_property_with_attribute() {
        let xml = r#"<property><attributes><attribute><name>Required</name></attribute></attributes><public/><type>string</type><name>Name</name><get>get;</get><set>set;</set></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "[Required]\npublic string Name { get; set; }");
    }

    #[test]
    fn test_render_field() {
        let xml = r#"<field><private/><readonly/><type>int</type><name>_count</name></field>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "private readonly int _count;");
    }

    #[test]
    fn test_render_static_field() {
        let xml =
            r#"<field><private/><static/><type>string</type><name>DefaultName</name></field>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "private static string DefaultName;");
    }

    #[test]
    fn test_render_class_with_members() {
        let xml = r#"<class><public/><name>User</name><body><property><public/><type>string</type><name>Name</name><get>get;</get><set>set;</set></property><field><private/><readonly/><type>int</type><name>_id</name></field></body></class>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        let expected = "public class User\n{\n    public string Name { get; set; }\n\n    private readonly int _id;\n}";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_render_generic_type_property() {
        let xml = r#"<property><public/><type><generic/>List<arguments><type>string</type></arguments></type><name>Items</name><get>get;</get><set>set;</set></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public List<string> Items { get; set; }");
    }

    #[test]
    fn test_render_nested_generic_type() {
        let xml = r#"<property><public/><type><generic/>Dictionary<arguments><type>string</type><type>int</type></arguments></type><name>Map</name><get>get;</get><set>set;</set></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public Dictionary<string, int> Map { get; set; }");
    }

    #[test]
    fn test_render_property_with_indentation() {
        let xml = r#"<property><public/><type>string</type><name>Name</name><get>get;</get><set>set;</set></property>"#;
        let node = parse_xml(xml).unwrap();
        let opts = RenderOptions {
            indent_level: 1,
            ..Default::default()
        };
        let result = render_node(&node, &opts).unwrap();
        assert_eq!(result, "    public string Name { get; set; }");
    }

    #[test]
    fn test_render_empty_class() {
        let xml = r#"<class><public/><name>Empty</name></class>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public class Empty\n{\n}");
    }
}
