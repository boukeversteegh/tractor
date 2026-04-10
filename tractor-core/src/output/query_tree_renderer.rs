use crate::output::xml_renderer::{ansi, render_xml_node, RenderOptions};
use crate::xpath::XmlNode;

pub fn render_query_tree_node(node: &XmlNode, options: &RenderOptions) -> String {
    match node {
        XmlNode::Element { .. } => {
            let entry = build_element_entry(node, options, 0);
            let mut out = String::new();
            emit_entry(&entry, options, &[], true, true, &mut out);
            out
        }
        XmlNode::Text(text) => format!(
            "{}\n",
            render_line(&RenderLine::TextLiteral(text.clone()), options, false)
        ),
        XmlNode::Comment(text) => format!(
            "{}\n",
            render_line(&RenderLine::Other(format!("<!--{}-->", text)), options, false)
        ),
        XmlNode::ProcessingInstruction { target, data } => {
            let mut label = format!("<?{}", target);
            if let Some(data) = data {
                label.push(' ');
                label.push_str(data);
            }
            label.push_str("?>");
            format!("{}\n", render_line(&RenderLine::Other(label), options, false))
        }
        XmlNode::Map { .. }
        | XmlNode::Array { .. }
        | XmlNode::Number(_)
        | XmlNode::Boolean(_)
        | XmlNode::Null => render_xml_node(node, options),
    }
}

#[derive(Debug, Clone)]
struct RenderEntry {
    line: RenderLine,
    children: Vec<RenderEntry>,
}

#[derive(Debug, Clone)]
enum RenderLine {
    Path(String),
    PathValue { path: String, value: String },
    TextLiteral(String),
    Other(String),
    Truncation(usize),
}

#[derive(Debug)]
struct ElementInfo<'a> {
    markers: Vec<&'a XmlNode>,
    attributes: Vec<(&'a str, &'a str)>,
    visible_items: Vec<VisibleChild<'a>>,
    non_marker_elements: Vec<&'a XmlNode>,
}

#[derive(Debug)]
enum VisibleChild<'a> {
    Element(&'a XmlNode),
    Text(&'a str),
    Comment(&'a str),
    ProcessingInstruction { target: &'a str, data: Option<&'a str> },
}

fn build_element_entry(node: &XmlNode, options: &RenderOptions, depth: usize) -> RenderEntry {
    let mut current = node;
    let mut current_depth = depth;
    let mut segments = vec![segment_for_element(node, options)];

    loop {
        if options.max_depth.is_some_and(|max| current_depth >= max) {
            break;
        }

        let info = analyze_element(current, options);
        if info.non_marker_elements.len() != 1 || info.visible_items.len() != 1 {
            break;
        }

        let child = info.non_marker_elements[0];
        segments.push(segment_for_element(child, options));
        current = child;
        current_depth += 1;
    }

    let path = segments.join("/");
    let info = analyze_element(current, options);

    if let Some(value) = direct_leaf_value(&info) {
        return RenderEntry {
            line: RenderLine::PathValue {
                path,
                value: value.to_string(),
            },
            children: Vec::new(),
        };
    }

    let children = if options.max_depth.is_some_and(|max| current_depth >= max)
        && !info.visible_items.is_empty()
    {
        vec![RenderEntry {
            line: RenderLine::Truncation(count_descendant_elements(current)),
            children: Vec::new(),
        }]
    } else {
        info.visible_items
            .iter()
            .map(|child| build_child_entry(child, options, current_depth + 1))
            .collect()
    };

    RenderEntry {
        line: RenderLine::Path(path),
        children,
    }
}

fn build_child_entry(child: &VisibleChild<'_>, options: &RenderOptions, depth: usize) -> RenderEntry {
    match child {
        VisibleChild::Element(node) => build_element_entry(node, options, depth),
        VisibleChild::Text(text) => RenderEntry {
            line: RenderLine::TextLiteral((*text).to_string()),
            children: Vec::new(),
        },
        VisibleChild::Comment(text) => RenderEntry {
            line: RenderLine::Other(format!("<!--{}-->", text)),
            children: Vec::new(),
        },
        VisibleChild::ProcessingInstruction { target, data } => {
            let mut label = format!("<?{}", target);
            if let Some(data) = data {
                label.push(' ');
                label.push_str(data);
            }
            label.push_str("?>");
            RenderEntry {
                line: RenderLine::Other(label),
                children: Vec::new(),
            }
        }
    }
}

fn emit_entry(
    entry: &RenderEntry,
    options: &RenderOptions,
    ancestor_has_more_siblings: &[bool],
    is_last: bool,
    is_root: bool,
    out: &mut String,
) {
    if !is_root {
        out.push_str(&render_prefix(options, ancestor_has_more_siblings));
        out.push_str(&render_branch(options, is_last));
    }

    out.push_str(&render_line(&entry.line, options, !entry.children.is_empty()));
    out.push('\n');

    let mut child_ancestors = ancestor_has_more_siblings.to_vec();
    if !is_root {
        child_ancestors.push(!is_last);
    }

    for (idx, child) in entry.children.iter().enumerate() {
        emit_entry(
            child,
            options,
            &child_ancestors,
            idx + 1 == entry.children.len(),
            false,
            out,
        );
    }
}

fn render_prefix(options: &RenderOptions, ancestor_has_more_siblings: &[bool]) -> String {
    let mut prefix = paint(options, ansi::DIM, "  ");
    for &has_more_siblings in ancestor_has_more_siblings {
        let segment = if has_more_siblings { "│   " } else { "    " };
        prefix.push_str(&paint(options, ansi::DIM, segment));
    }
    prefix
}

fn render_branch(options: &RenderOptions, is_last: bool) -> String {
    let branch = if is_last { "└─ " } else { "├─ " };
    paint(options, ansi::DIM, branch)
}

fn render_line(line: &RenderLine, options: &RenderOptions, has_children: bool) -> String {
    match line {
        RenderLine::Path(path) => {
            let mut out = paint(options, ansi::BLUE, path);
            if has_children {
                out.push_str(&paint(options, ansi::DIM, "/"));
            }
            out
        }
        RenderLine::PathValue { path, value } => format!(
            "{}{}{}",
            paint(options, ansi::BLUE, path),
            paint(options, ansi::DIM, " = "),
            paint(options, ansi::YELLOW, &quote_literal(value))
        ),
        RenderLine::TextLiteral(text) => paint(options, ansi::YELLOW, &quote_literal(text)),
        RenderLine::Other(label) => paint(options, ansi::DIM, label),
        RenderLine::Truncation(count) => {
            paint(options, ansi::DIM, &format!("... ({} children)", count))
        }
    }
}

fn paint(options: &RenderOptions, color: &str, text: &str) -> String {
    if options.use_color {
        format!("{}{}{}", color, text, ansi::RESET)
    } else {
        text.to_string()
    }
}

fn segment_for_element(node: &XmlNode, options: &RenderOptions) -> String {
    match node {
        XmlNode::Element { name, .. } => {
            let info = analyze_element(node, options);
            let mut segment = name.clone();
            for marker in info.markers {
                if let XmlNode::Element { name, .. } = marker {
                    segment.push('[');
                    segment.push_str(name);
                    segment.push(']');
                }
            }
            for (attr_name, attr_value) in info.attributes {
                segment.push_str("[@");
                segment.push_str(attr_name);
                segment.push('=');
                segment.push_str(&quote_literal(attr_value));
                segment.push(']');
            }
            segment
        }
        _ => String::new(),
    }
}

fn analyze_element<'a>(node: &'a XmlNode, options: &RenderOptions) -> ElementInfo<'a> {
    let XmlNode::Element {
        attributes,
        children,
        ..
    } = node
    else {
        return ElementInfo {
            markers: Vec::new(),
            attributes: Vec::new(),
            visible_items: Vec::new(),
            non_marker_elements: Vec::new(),
        };
    };

    let visible_attributes = attributes
        .iter()
        .filter(|(name, _)| options.include_meta || !is_hidden_meta_attr(name))
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<Vec<_>>();

    let mut markers = Vec::new();
    let mut visible_items = Vec::new();
    let mut non_marker_elements = Vec::new();

    for child in children {
        match child {
            XmlNode::Element { .. } if is_marker_element(child, options) => markers.push(child),
            XmlNode::Element { .. } => {
                non_marker_elements.push(child);
                visible_items.push(VisibleChild::Element(child));
            }
            XmlNode::Text(text) if !text.trim().is_empty() => {
                visible_items.push(VisibleChild::Text(text.trim()));
            }
            XmlNode::Comment(text) => visible_items.push(VisibleChild::Comment(text)),
            XmlNode::ProcessingInstruction { target, data } => {
                visible_items.push(VisibleChild::ProcessingInstruction {
                    target,
                    data: data.as_deref(),
                });
            }
            _ => {}
        }
    }

    ElementInfo {
        markers,
        attributes: visible_attributes,
        visible_items,
        non_marker_elements,
    }
}

fn direct_leaf_value<'a>(info: &'a ElementInfo<'a>) -> Option<&'a str> {
    if info.non_marker_elements.is_empty() && info.visible_items.len() == 1 {
        if let VisibleChild::Text(text) = info.visible_items[0] {
            return Some(text);
        }
    }
    None
}

fn is_marker_element(node: &XmlNode, options: &RenderOptions) -> bool {
    match node {
        XmlNode::Element {
            attributes,
            children,
            ..
        } => {
            let has_visible_attributes = attributes
                .iter()
                .any(|(name, _)| options.include_meta || !is_hidden_meta_attr(name));

            if has_visible_attributes {
                return false;
            }

            children.iter().all(|child| match child {
                XmlNode::Text(text) => text.trim().is_empty(),
                _ => false,
            })
        }
        _ => false,
    }
}

fn is_hidden_meta_attr(name: &str) -> bool {
    matches!(
        name,
        "line" | "column" | "end_line" | "end_column" | "kind" | "field" | "path"
    )
}

fn count_descendant_elements(node: &XmlNode) -> usize {
    match node {
        XmlNode::Element { children, .. } => children
            .iter()
            .map(|child| match child {
                XmlNode::Element { .. } => 1 + count_descendant_elements(child),
                _ => 0,
            })
            .sum(),
        _ => 0,
    }
}

fn quote_literal(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| format!("\"{}\"", text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elem(name: &str, children: Vec<XmlNode>) -> XmlNode {
        XmlNode::Element {
            name: name.to_string(),
            attributes: Vec::new(),
            children,
        }
    }

    fn text(value: &str) -> XmlNode {
        XmlNode::Text(value.to_string())
    }

    #[test]
    fn collapses_marker_child() {
        let node = elem("variable", vec![elem("const", vec![])]);
        assert_eq!(render_query_tree_node(&node, &RenderOptions::new()), "variable[const]\n");
    }

    #[test]
    fn collapses_leaf_value() {
        let node = elem("type", vec![text("API_URL")]);
        assert_eq!(render_query_tree_node(&node, &RenderOptions::new()), "type = \"API_URL\"\n");
    }

    #[test]
    fn collapses_linear_chain() {
        let node = elem("name", vec![elem("type", vec![text("fs")])]);
        assert_eq!(render_query_tree_node(&node, &RenderOptions::new()), "name/type = \"fs\"\n");
    }

    #[test]
    fn collapses_marker_and_value() {
        let node = elem("function", vec![elem("ref", vec![]), text("require")]);
        assert_eq!(
            render_query_tree_node(&node, &RenderOptions::new()),
            "function[ref] = \"require\"\n"
        );
    }

    #[test]
    fn collapses_structured_child_value() {
        let node = elem("value", vec![elem("call", vec![elem("ref", vec![]), text("x")])]);
        assert_eq!(
            render_query_tree_node(&node, &RenderOptions::new()),
            "value/call[ref] = \"x\"\n"
        );
    }

    #[test]
    fn renders_mixed_string_contents() {
        let node = elem(
            "string",
            vec![text("\""), elem("string_fragment", vec![text("x")]), text("\"")],
        );
        assert_eq!(
            render_query_tree_node(&node, &RenderOptions::new()),
            concat!(
                "string/\n",
                "  ├─ \"\\\"\"\n",
                "  ├─ string_fragment = \"x\"\n",
                "  └─ \"\\\"\"\n"
            )
        );
    }

    #[test]
    fn preserves_sibling_order() {
        let node = elem(
            "root",
            vec![
                elem("first", vec![text("1")]),
                elem("second", vec![text("2")]),
                elem("third", vec![text("3")]),
            ],
        );

        assert_eq!(
            render_query_tree_node(&node, &RenderOptions::new()),
            concat!(
                "root/\n",
                "  ├─ first = \"1\"\n",
                "  ├─ second = \"2\"\n",
                "  └─ third = \"3\"\n"
            )
        );
    }

    #[test]
    fn keeps_vertical_guides_for_truncated_nested_siblings() {
        let node = elem(
            "class",
            vec![
                elem("public", vec![]),
                text("class"),
                elem(
                    "body",
                    vec![
                        text("{"),
                        elem("property", vec![elem("details", vec![text("x")])]),
                        elem("method", vec![elem("details", vec![text("y")])]),
                        text("}"),
                    ],
                ),
            ],
        );

        let options = RenderOptions::new().with_max_depth(Some(2));
        assert_eq!(
            render_query_tree_node(&node, &options),
            concat!(
                "class[public]/\n",
                "  ├─ \"class\"\n",
                "  └─ body/\n",
                "      ├─ \"{\"\n",
                "      ├─ property/\n",
                "      │   └─ ... (1 children)\n",
                "      ├─ method/\n",
                "      │   └─ ... (1 children)\n",
                "      └─ \"}\"\n"
            )
        );
    }
}
