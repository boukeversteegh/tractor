//! Python code renderer
//!
//! Renders tractor's semantic XML back to Python source code.
//! Supports: module, class (with base list), field (annotated attribute),
//! method, function, parameter, import, comment, plus `optional` and `list`
//! type markers.

use super::{
    get_child, get_child_text, get_children, has_marker, text_content, RenderError, RenderOptions,
};
use crate::languages::python::semantic::*;
use crate::xpath::XmlNode;

pub fn render_node(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    match node {
        XmlNode::Element { name, .. } => match name.as_str() {
            MODULE => render_module(node, opts),
            CLASS => render_class(node, opts),
            FUNCTION | METHOD => render_function(node, opts),
            FIELD => render_field(node, opts),
            IMPORT => render_import(node, opts),
            "from" => render_from_import(node, opts),
            "decorated" => render_decorated(node, opts),
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

fn render_type(node: &XmlNode) -> Result<String, RenderError> {
    match node {
        XmlNode::Element { name, children, .. } if name == TYPE => {
            // Parser shape: <type><generic_type><name>list</name><type_parameter>[<type>str</type>]</type_parameter></generic_type></type>
            if let Some(g) = get_child(node, "generic_type") {
                return render_generic_type(g);
            }

            let has_optional = has_marker(node, OPTIONAL);
            let has_list = has_marker(node, LIST);

            // Collect inner text / nested <type> content.
            let inner: String = children
                .iter()
                .filter_map(|c| match c {
                    XmlNode::Text(t) => Some(t.trim().to_string()),
                    XmlNode::Element { name, children: ch, .. }
                        if (name == OPTIONAL || name == LIST) && ch.is_empty() =>
                    {
                        None
                    }
                    XmlNode::Element { name: n, .. } if n == TYPE => render_type(c).ok(),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");

            let mut out = inner;
            if has_list {
                out = format!("list[{}]", out);
            }
            if has_optional {
                out = format!("{} | None", out);
            }
            Ok(out)
        }
        _ => text_content(node).ok_or_else(|| RenderError::MissingChild {
            parent: TYPE.into(),
            child: "text".into(),
        }),
    }
}

/// Render `<generic_type>` — parser form of `Name[T]` or `Name[K, V]` such as
/// `list[str]`, `dict[str, int]`, `Optional[str]`. The name is read from the
/// child `<name>` element, the arguments from the `<type_parameter>` wrapper
/// which contains `[`, comma-separated `<type>` elements, and `]`.
fn render_generic_type(node: &XmlNode) -> Result<String, RenderError> {
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let args = get_child(node, "type_parameter")
        .map(|p| {
            get_children(p, TYPE)
                .iter()
                .filter_map(|t| render_type(t).ok())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    Ok(format!("{}[{}]", name, args))
}

fn render_type_slot(node: &XmlNode) -> Option<String> {
    get_child(node, TYPE).and_then(|t| render_type(t).ok())
}

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
        format!("({})", refs.join(", "))
    }
}

fn render_decorators(node: &XmlNode, opts: &RenderOptions) -> String {
    let decorators_node = match get_child(node, DECORATORS) {
        Some(d) => d,
        None => return String::new(),
    };
    let indent = opts.current_indent();
    get_children(decorators_node, DECORATOR)
        .iter()
        .filter_map(|d| text_content(d))
        .map(|t| {
            let raw = t.trim();
            let body = raw.strip_prefix('@').unwrap_or(raw);
            format!("{}@{}{}", indent, body, opts.newline)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Field (annotated attribute)
// ---------------------------------------------------------------------------

fn render_field(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let name = get_child_text(node, NAME)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| RenderError::MissingChild {
            parent: FIELD.into(),
            child: NAME.into(),
        })?;
    let type_str = render_type_slot(node);
    let default = get_child(node, DEFAULT).and_then(render_value_slot);

    let line = match (type_str, default) {
        (Some(t), Some(v)) => format!("{}{}: {} = {}", indent, name, t, v),
        (Some(t), None) => format!("{}{}: {}", indent, name, t),
        (None, Some(v)) => format!("{}{} = {}", indent, name, v),
        (None, None) => format!("{}{}", indent, name),
    };
    Ok(line)
}

/// Render a value slot (`<default>`, `<value>`, etc.) by extracting its first
/// literal-shaped child. Handles Python literals: int, float, string, bool
/// (`<true/>`/`<false/>`), and `<none>None</none>`. Strings and booleans are
/// returned verbatim (quotes preserved from source via `text_content`).
fn render_value_slot(node: &XmlNode) -> Option<String> {
    let XmlNode::Element { children, .. } = node else {
        return None;
    };
    children.iter().find_map(render_python_literal)
}

fn render_python_literal(node: &XmlNode) -> Option<String> {
    let XmlNode::Element { name, children, .. } = node else {
        return None;
    };
    match name.as_str() {
        "int" | "float" | "true" | "false" | "none" => {
            text_content(node).map(|t| t.trim().to_string())
        }
        // Python strings come in as
        //   <string><string_start>"</string_start>
        //           <string_content>…</string_content>?
        //           <string_end>"</string_end></string>
        // Reassemble quote + content + quote so embedded whitespace in the
        // content survives even though pretty-printed XML adds whitespace
        // around the child elements.
        "string" => {
            let get_part = |part: &str| -> String {
                children
                    .iter()
                    .find_map(|c| match c {
                        XmlNode::Element { name: n, .. } if n == part => text_content(c),
                        _ => None,
                    })
                    .unwrap_or_default()
            };
            let start = get_part("string_start");
            let end = get_part("string_end");
            let content = get_part("string_content");
            let quote = if !start.is_empty() { start } else { "\"".to_string() };
            let closer = if !end.is_empty() { end } else { quote.clone() };
            Some(format!("{}{}{}", quote, content, closer))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

fn render_parameter(node: &XmlNode) -> Option<String> {
    let name = get_child_text(node, NAME)?;
    let ty = render_type_slot(node);
    let default = get_child_text(node, DEFAULT);
    let annotated = match ty {
        Some(t) => format!("{}: {}", name, t),
        None => name,
    };
    Some(match default {
        Some(d) => format!("{} = {}", annotated, d),
        None => annotated,
    })
}

fn render_parameters(node: &XmlNode) -> String {
    let params_node = match get_child(node, PARAMETERS) {
        Some(p) => p,
        None => return "()".to_string(),
    };
    let parts: Vec<String> = get_children(params_node, PARAMETER)
        .iter()
        .filter_map(|p| render_parameter(p))
        .collect();
    format!("({})", parts.join(", "))
}

// ---------------------------------------------------------------------------
// Function / Method
// ---------------------------------------------------------------------------

fn render_function(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let decorators = render_decorators(node, opts);
    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: FUNCTION.into(),
        child: NAME.into(),
    })?;
    let params = render_parameters(node);
    let returns = get_child(node, RETURNS)
        .and_then(|r| render_type_slot(r).or_else(|| text_content(r)))
        .map(|t| format!(" -> {}", t))
        .unwrap_or_default();

    let body_opts = opts.indented();
    let body_indent = body_opts.current_indent();
    let body = render_function_body(node, &body_opts)
        .unwrap_or_else(|| format!("{}pass", body_indent));

    Ok(format!(
        "{}{}def {}{}{}:{}{}",
        decorators, indent, name, params, returns, opts.newline, body
    ))
}

/// Render a function/method body.
/// Accepts a <body> wrapper or treats the function's direct children as statements.
/// Currently supports <pass/>, <return> with text, <comment>, and raw text statements.
fn render_function_body(node: &XmlNode, opts: &RenderOptions) -> Option<String> {
    let body = get_child(node, BODY)?;
    let XmlNode::Element { children, .. } = body else {
        return None;
    };
    let indent = opts.current_indent();
    let mut lines: Vec<String> = Vec::new();
    for child in children {
        match child {
            XmlNode::Element { name, .. } if name == "pass" => {
                lines.push(format!("{}pass", indent));
            }
            XmlNode::Element { name, .. } if name == COMMENT => {
                if let Ok(s) = render_comment(child, opts) {
                    lines.push(s);
                }
            }
            XmlNode::Element { name, .. } if name == "raw" => {
                if let Some(t) = text_content(child) {
                    lines.push(format!("{}{}", indent, t.trim()));
                }
            }
            _ => {}
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(&opts.newline))
    }
}

// ---------------------------------------------------------------------------
// Class
// ---------------------------------------------------------------------------

fn render_class(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let indent = opts.current_indent();
    let decorators = render_decorators(node, opts);
    let name = get_child_text(node, NAME).ok_or_else(|| RenderError::MissingChild {
        parent: CLASS.into(),
        child: NAME.into(),
    })?;
    let base = render_base_list(node);

    let body_opts = opts.indented();
    let body_indent = body_opts.current_indent();
    let members = collect_body_members(node, &body_opts)?;

    let body = if members.is_empty() {
        format!("{}pass", body_indent)
    } else {
        let mut rendered = Vec::new();
        let mut prev_kind: Option<&str> = None;
        for (kind, text) in &members {
            // Blank line between methods/functions and between a block of fields
            // and a following method.
            if let Some(pk) = prev_kind {
                if is_callable(kind) || is_callable(pk) {
                    rendered.push(String::new());
                }
            }
            rendered.push(text.clone());
            prev_kind = Some(kind);
        }
        rendered.join(&opts.newline)
    };

    Ok(format!(
        "{}{}class {}{}:{}{}",
        decorators, indent, name, base, opts.newline, body
    ))
}

fn is_callable(kind: &str) -> bool {
    matches!(kind, FUNCTION | METHOD)
}

/// Collect body members as (kind, rendered text) pairs.
fn collect_body_members<'a>(
    node: &'a XmlNode,
    opts: &RenderOptions,
) -> Result<Vec<(&'a str, String)>, RenderError> {
    let container = get_child(node, BODY).unwrap_or(node);
    let XmlNode::Element { children, .. } = container else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                FIELD | FUNCTION | METHOD | COMMENT | CLASS => {
                    out.push((name.as_str(), render_node(child, opts)?));
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Import / Comment / Module
// ---------------------------------------------------------------------------

/// Render `<import>import<name><dotted_name>…</dotted_name></name></import>` as
/// `import os.path`. Falls back to flat text content when the parser shape
/// isn't recognised.
fn render_import(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let module = find_import_module(node).unwrap_or_else(|| {
        text_content(node)
            .unwrap_or_default()
            .trim()
            .strip_prefix("import")
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    Ok(format!("{}import {}", opts.current_indent(), module))
}

/// Render `<from>from<dotted_name>M</dotted_name>import<name>A</name>,<name>B</name></from>`
/// as `from M import A, B`.
fn render_from_import(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Ok(String::new());
    };

    // The module is the FIRST <dotted_name> — it's the source of the import.
    // Subsequent imported names live in <name> wrappers.
    let module = children
        .iter()
        .find_map(|c| match c {
            XmlNode::Element { name, .. } if name == "dotted_name" => dotted_name_text(c),
            _ => None,
        })
        .unwrap_or_default();

    let imports: Vec<String> = children
        .iter()
        .filter_map(|c| match c {
            XmlNode::Element { name, .. } if name == NAME => {
                // The <name> wrapper can itself contain <dotted_name> (the
                // Python parser wraps imported names that way).
                get_child(c, "dotted_name")
                    .and_then(dotted_name_text)
                    .or_else(|| text_content(c).map(|t| t.trim().to_string()))
            }
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect();

    let imports_str = imports.join(", ");
    Ok(format!(
        "{}from {} import {}",
        opts.current_indent(),
        module,
        imports_str
    ))
}

fn find_import_module(node: &XmlNode) -> Option<String> {
    let name_child = get_child(node, NAME)?;
    get_child(name_child, "dotted_name")
        .and_then(dotted_name_text)
        .or_else(|| text_content(name_child).map(|t| t.trim().to_string()))
}

/// Join the `<name>` children of a `<dotted_name>` with dots —
/// `<dotted_name><name>os</name>.<name>path</name></dotted_name>` → `os.path`.
fn dotted_name_text(node: &XmlNode) -> Option<String> {
    let XmlNode::Element { children, .. } = node else {
        return None;
    };
    let parts: Vec<String> = children
        .iter()
        .filter_map(|c| match c {
            XmlNode::Element { name, .. } if name == NAME => {
                text_content(c).map(|t| t.trim().to_string())
            }
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

/// Render `<decorated><decorator>@X</decorator>...<class|function>…</class|function></decorated>`
/// by prepending the decorator to the wrapped declaration.
fn render_decorated(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Ok(String::new());
    };
    let indent = opts.current_indent();
    let mut decorators = Vec::new();
    let mut inner: Option<String> = None;

    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                "decorator" => {
                    let text = text_content(child).unwrap_or_default();
                    let body = text
                        .trim()
                        .strip_prefix('@')
                        .map(|s| s.trim())
                        .unwrap_or_else(|| text.trim());
                    decorators.push(format!("{}@{}", indent, body));
                }
                CLASS | FUNCTION | METHOD => {
                    inner = Some(render_node(child, opts)?);
                }
                _ => {}
            }
        }
    }

    let mut parts = decorators;
    if let Some(body) = inner {
        parts.push(body);
    }
    Ok(parts.join(&opts.newline))
}

fn render_comment(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let text = text_content(node).unwrap_or_default();
    // Preserve the comment verbatim (indentation trimmed). A bare `#` stays a
    // bare `#`, `# foo` stays `# foo`; we don't reflow the body because round
    // trips need byte equality.
    let trimmed = text.trim_start_matches([' ', '\t']).trim_end_matches(['\r', '\n']);
    let body = if trimmed.is_empty() { "#" } else { trimmed };
    Ok(format!("{}{}", opts.current_indent(), body))
}

fn render_module(node: &XmlNode, opts: &RenderOptions) -> Result<String, RenderError> {
    let XmlNode::Element { children, .. } = node else {
        return Ok(String::new());
    };

    let mut parts: Vec<(&str, String)> = Vec::new();
    for child in children {
        if let XmlNode::Element { name, .. } = child {
            match name.as_str() {
                IMPORT | "from" | CLASS | FUNCTION | "decorated" | COMMENT | FIELD => {
                    parts.push((name.as_str(), render_node(child, opts)?));
                }
                _ => {}
            }
        }
    }

    // Separation rules:
    //   * Consecutive items of the same lightweight kind (imports, from
    //     imports, comments) stay tight.
    //   * Two blank lines around top-level class/function/decorated (PEP 8).
    //   * Any other kind transition (e.g. comment → import, import → class)
    //     gets a single blank line.
    let is_decl = |k: &str| matches!(k, CLASS | FUNCTION | "decorated");
    let is_import_like = |k: &str| matches!(k, IMPORT | "from");
    let mut result = String::new();
    for (i, (kind, text)) in parts.iter().enumerate() {
        if i > 0 {
            let prev = parts[i - 1].0;
            // Consecutive imports (any mix of `import X` / `from X import Y`)
            // and consecutive comment lines stay tight.
            let tight = (is_import_like(kind) && is_import_like(prev))
                || (prev == *kind && *kind == COMMENT);
            let blank = if tight {
                String::new()
            } else if is_decl(kind) || is_decl(prev) {
                format!("{}{}", opts.newline, opts.newline)
            } else {
                // Different kind-on-kind transition (e.g. comment → import,
                // import → from, from → import) gets one blank line.
                opts.newline.clone()
            };
            result.push_str(&opts.newline);
            result.push_str(&blank);
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

    fn render(xml: &str) -> String {
        let node = parse_xml(xml).unwrap();
        render_node(&node, &RenderOptions::default()).unwrap()
    }

    #[test]
    fn field_annotated() {
        assert_eq!(
            render(r#"<field><name>count</name><type>int</type></field>"#),
            "count: int"
        );
    }

    #[test]
    fn field_optional() {
        assert_eq!(
            render(r#"<field><name>age</name><type><optional/>int</type></field>"#),
            "age: int | None"
        );
    }

    #[test]
    fn field_list() {
        assert_eq!(
            render(r#"<field><name>tags</name><type><list/>str</type></field>"#),
            "tags: list[str]"
        );
    }

    #[test]
    fn class_with_fields() {
        let xml = r#"<class><name>User</name><body><field><name>name</name><type>str</type></field><field><name>id</name><type>int</type></field></body></class>"#;
        let expected = "class User:\n    name: str\n    id: int";
        assert_eq!(render(xml), expected);
    }

    #[test]
    fn class_empty_is_pass() {
        assert_eq!(render(r#"<class><name>Empty</name></class>"#), "class Empty:\n    pass");
    }

    #[test]
    fn class_with_base() {
        let xml = r#"<class><name>Dog</name><base><ref>Animal</ref></base><body><field><name>legs</name><type>int</type></field></body></class>"#;
        assert_eq!(render(xml), "class Dog(Animal):\n    legs: int");
    }

    #[test]
    fn function_signature() {
        let xml = r#"<function><name>save</name><parameters><parameter><name>self</name></parameter><parameter><name>id</name><type>int</type></parameter></parameters><returns><type>None</type></returns></function>"#;
        assert_eq!(render(xml), "def save(self, id: int) -> None:\n    pass");
    }

    #[test]
    fn class_field_then_method_blank_line() {
        let xml = r#"<class><name>User</name><body><field><name>id</name><type>int</type></field><method><name>save</name><parameters><parameter><name>self</name></parameter></parameters><returns><type>None</type></returns></method></body></class>"#;
        let expected = "class User:\n    id: int\n\n    def save(self) -> None:\n        pass";
        assert_eq!(render(xml), expected);
    }
}
