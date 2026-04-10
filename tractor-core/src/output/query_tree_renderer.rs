use crate::languages::get_syntax_category;
use crate::output::syntax_highlight::{extract_syntax_spans_from_xml_node, highlight_lines};
use crate::output::xml_renderer::{ansi, render_xml_node, RenderOptions};
use crate::source_utils::get_source_lines;
use crate::xpath::XmlNode;

pub fn render_query_tree_node(node: &XmlNode, options: &RenderOptions) -> String {
    let lines = render_query_tree_lines(node, options);
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

pub fn render_query_tree_with_source(
    node: &XmlNode,
    source: &str,
    options: &RenderOptions,
) -> Option<String> {
    let tree_lines = render_query_tree_lines(node, options);
    if tree_lines.is_empty() {
        return None;
    }

    let (start_line, end_line) = find_source_anchor_span(node)?;
    let source_lines = get_source_lines(source, start_line, end_line);
    if source_lines.is_empty() {
        return None;
    }

    let source_lines = maybe_highlight_source_lines(node, &source_lines, start_line, end_line, options);
    Some(render_side_by_side(&tree_lines, &source_lines))
}

fn render_query_tree_lines(node: &XmlNode, options: &RenderOptions) -> Vec<String> {
    let output = match node {
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
    };

    output
        .lines()
        .map(|line| line.to_string())
        .collect()
}

fn render_side_by_side(tree_lines: &[String], source_lines: &[String]) -> String {
    let left_width = tree_lines
        .iter()
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    let line_count = tree_lines.len().max(source_lines.len());
    for idx in 0..line_count {
        let left = tree_lines.get(idx).map(|s| s.as_str()).unwrap_or("");
        let right = source_lines.get(idx).map(|s| s.as_str()).unwrap_or("");
        let padding = left_width.saturating_sub(visible_width(left));

        out.push_str(left);
        out.push_str(&" ".repeat(padding));
        out.push_str(" | ");
        out.push_str(right);
        out.push('\n');
    }

    out
}

fn maybe_highlight_source_lines(
    node: &XmlNode,
    source_lines: &[String],
    start_line: u32,
    end_line: u32,
    options: &RenderOptions,
) -> Vec<String> {
    if !options.use_color {
        return source_lines.to_vec();
    }

    let category_fn = get_syntax_category(options.language.as_deref().unwrap_or(""));
    let spans = extract_syntax_spans_from_xml_node(node, category_fn);
    if spans.is_empty() {
        return source_lines.to_vec();
    }

    highlight_lines(source_lines, &spans, start_line, end_line)
        .split('\n')
        .map(|line| line.to_string())
        .collect()
}

fn find_source_anchor_span(node: &XmlNode) -> Option<(u32, u32)> {
    let anchor = find_outermost_positioned_element(node)?;
    let (start_line, _, end_line, _) = extract_position_span(anchor)?;
    Some((start_line, end_line))
}

fn find_outermost_positioned_element<'a>(node: &'a XmlNode) -> Option<&'a XmlNode> {
    if extract_position_span(node).is_some() {
        return Some(node);
    }

    let XmlNode::Element { children, .. } = node else {
        return None;
    };

    let element_children: Vec<&XmlNode> = children
        .iter()
        .filter(|child| matches!(child, XmlNode::Element { .. }))
        .collect();

    if element_children.len() == 1 {
        return find_outermost_positioned_element(element_children[0]);
    }

    element_children
        .into_iter()
        .find_map(find_outermost_positioned_element)
}

fn extract_position_span(node: &XmlNode) -> Option<(u32, u32, u32, u32)> {
    let XmlNode::Element { attributes, .. } = node else {
        return None;
    };

    let mut start_line = None;
    let mut start_column = None;
    let mut end_line = None;
    let mut end_column = None;

    for (name, value) in attributes {
        match name.as_str() {
            "line" => start_line = value.parse().ok(),
            "column" => start_column = value.parse().ok(),
            "end_line" => end_line = value.parse().ok(),
            "end_column" => end_column = value.parse().ok(),
            _ => {}
        }
    }

    Some((start_line?, start_column?, end_line?, end_column?))
}

fn visible_width(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut width = 0usize;
    let mut idx = 0usize;

    while idx < bytes.len() {
        if bytes[idx] == 0x1b && idx + 1 < bytes.len() && bytes[idx + 1] == b'[' {
            idx += 2;
            while idx < bytes.len() && bytes[idx] != b'm' {
                idx += 1;
            }
            if idx < bytes.len() {
                idx += 1;
            }
            continue;
        }

        let ch = text[idx..].chars().next().unwrap();
        width += 1;
        idx += ch.len_utf8();
    }

    width
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
            let mut predicates = Vec::new();
            for marker in info.markers {
                if let XmlNode::Element { name, .. } = marker {
                    predicates.push(name.clone());
                }
            }
            for (attr_name, attr_value) in info.attributes {
                predicates.push(format!("@{}={}", attr_name, quote_literal(attr_value)));
            }
            if !predicates.is_empty() {
                segment.push('[');
                segment.push_str(&predicates.join(" and "));
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

    fn elem_with_attrs(name: &str, attributes: &[(&str, &str)], children: Vec<XmlNode>) -> XmlNode {
        XmlNode::Element {
            name: name.to_string(),
            attributes: attributes
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children,
        }
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

    #[test]
    fn combines_multiple_predicates_into_one_block() {
        let node = XmlNode::Element {
            name: "method".to_string(),
            attributes: vec![
                ("kind".to_string(), "method_declaration".to_string()),
                ("line".to_string(), "19".to_string()),
            ],
            children: vec![
                elem("public", vec![]),
                elem("static", vec![]),
                elem("parameters", vec![text("()")]),
                elem("body", vec![text("{ }")]),
            ],
        };

        let rendered = render_query_tree_node(
            &node,
            &RenderOptions::new().with_meta(true),
        );

        assert_eq!(
            rendered,
            concat!(
                "method[public and static and @kind=\"method_declaration\" and @line=\"19\"]/\n",
                "  ├─ parameters = \"()\"\n",
                "  └─ body = \"{ }\"\n"
            )
        );
    }

    #[test]
    fn renders_tree_with_source_from_outermost_positioned_child() {
        let node = elem(
            "Files",
            vec![elem(
                "File",
                vec![elem_with_attrs(
                    "unit",
                    &[("line", "2"), ("column", "1"), ("end_line", "2"), ("end_column", "10")],
                    vec![elem_with_attrs(
                        "class",
                        &[("line", "2"), ("column", "1"), ("end_line", "2"), ("end_column", "10")],
                        vec![
                            text("class"),
                            elem_with_attrs(
                                "name",
                                &[("line", "2"), ("column", "7"), ("end_line", "2"), ("end_column", "10")],
                                vec![text("Foo")],
                            ),
                        ],
                    )],
                )],
            )],
        );

        let rendered = render_query_tree_with_source(
            &node,
            "ignore me\nclass Foo\n",
            &RenderOptions::new().with_meta(false),
        )
        .unwrap();

        assert_eq!(
            rendered,
            concat!(
                "Files/File/unit/class/ | class Foo\n",
                "  \u{251c}\u{2500} \"class\"           | \n",
                "  \u{2514}\u{2500} name = \"Foo\"      | \n",
            )
        );
    }
}
