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

/// Render a type-position child — looks first for <type>, then <array>.
fn render_type_slot(node: &XmlNode, parent_label: &str) -> Result<String, RenderError> {
    if let Some(t) = get_child(node, TYPE) {
        return render_type(t);
    }
    if let Some(a) = get_child(node, ARRAY) {
        return render_type(a);
    }
    Err(RenderError::MissingChild {
        parent: parent_label.into(),
        child: TYPE.into(),
    })
}

/// Render a <base> list: `: Foo, IBar`
fn render_base_list(node: &XmlNode) -> String {
    let base = match get_child(node, BASE) {
        Some(b) => b,
        None => return String::new(),
    };
    let refs: Vec<String> = get_children(base, REF)
        .iter()
        .filter_map(|r| text_content(r))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if refs.is_empty() {
        String::new()
    } else {
        format!(" : {}", refs.join(", "))
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

    // Type — fields may have type as a child element or as a variable/declarator structure
    let type_str = render_type_slot(node, FIELD)?;

    // Name — may be in <name> directly or in a <variable><declarator><name>
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

fn render_interface(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    render_type_declaration(node, INTERFACE, opts)
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

    let mut result = String::new();
    result.push_str(&attrs);
    result.push_str(&format!("{}{}{} {}", indent, mods, ENUM, name));
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
fn collect_enum_members(node: &XmlNode) -> Vec<(String, Option<String>)> {
    let container = get_child(node, BODY).unwrap_or(node);
    get_children(container, ENUM_MEMBER)
        .iter()
        .filter_map(|m| {
            let n = get_child_text(m, NAME)?;
            let v = get_child_text(m, VALUE);
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
                | CLASS | STRUCT | INTERFACE | ENUM | COMMENT => {
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
                CLASS | STRUCT | INTERFACE | ENUM | IMPORT | COMMENT => {
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
                IMPORT | NAMESPACE | CLASS | STRUCT | INTERFACE | ENUM | COMMENT => {
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
        let xml = r#"<property><public/><type>string</type><name>Name</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public string Name { get; set; }");
    }

    #[test]
    fn test_render_nullable_property() {
        let xml = r#"<property><public/><type>Guid<nullable/></type><name>UserId</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public Guid? UserId { get; set; }");
    }

    #[test]
    fn test_render_property_with_attribute() {
        let xml = r#"<property><attributes><attribute><name>Required</name></attribute></attributes><public/><type>string</type><name>Name</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
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
        let xml = r#"<class><public/><name>User</name><body><property><public/><type>string</type><name>Name</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property><field><private/><readonly/><type>int</type><name>_id</name></field></body></class>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        let expected = "public class User\n{\n    public string Name { get; set; }\n\n    private readonly int _id;\n}";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_render_generic_type_property() {
        let xml = r#"<property><public/><type><generic/>List<arguments><type>string</type></arguments></type><name>Items</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public List<string> Items { get; set; }");
    }

    #[test]
    fn test_render_nested_generic_type() {
        let xml = r#"<property><public/><type><generic/>Dictionary<arguments><type>string</type><type>int</type></arguments></type><name>Map</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public Dictionary<string, int> Map { get; set; }");
    }

    #[test]
    fn test_render_property_with_indentation() {
        let xml = r#"<property><public/><type>string</type><name>Name</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
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

    #[test]
    fn test_render_interface_with_signature_method() {
        let xml = r#"<interface><public/><name>IUser</name><body><method><returns><type>void</type></returns><name>Save</name><parameters><parameter><type>int</type><name>id</name></parameter></parameters></method></body></interface>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(
            result,
            "public interface IUser\n{\n    void Save(int id);\n}"
        );
    }

    #[test]
    fn test_render_enum_with_values() {
        let xml = r#"<enum><public/><name>Color</name><body><enum_member><name>Red</name></enum_member><enum_member><name>Green</name><value>2</value></enum_member><enum_member><name>Blue</name></enum_member></body></enum>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(
            result,
            "public enum Color\n{\n    Red,\n    Green = 2,\n    Blue\n}"
        );
    }

    #[test]
    fn test_render_array_property() {
        let xml = r#"<property><public/><array><type>int</type></array><name>Ids</name><accessors><accessor>get;</accessor><accessor>set;</accessor></accessors></property>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public int[] Ids { get; set; }");
    }

    #[test]
    fn test_render_class_with_base_list() {
        let xml = r#"<class><public/><name>Dog</name><base><ref>Animal</ref><ref>IBarker</ref></base></class>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public class Dog : Animal, IBarker\n{\n}");
    }

    #[test]
    fn test_render_constructor_with_body() {
        let xml = r#"<constructor><public/><name>Dog</name><parameters><parameter><type>string</type><name>breed</name></parameter></parameters><body/></constructor>"#;
        let node = parse_xml(xml).unwrap();
        let result = render_node(&node, &RenderOptions::default()).unwrap();
        assert_eq!(result, "public Dog(string breed)\n{\n}");
    }
}
