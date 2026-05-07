//! Markdown tree-sitter CST → [`DataIr`] lowering.
//!
//! Markdown is structurally distinct from JSON / YAML / TOML / INI:
//! it's hierarchical *block-level text* rather than key-value
//! data. The lowering uses [`DataIr::Element`] (a generic element
//! variant with a fixed name + optional empty-marker children) for
//! the structural shapes (`<heading[h1]>`, `<list[ordered]>`,
//! `<codeblock>`, …) and the standard scalar variants for inline
//! text content.
//!
//! Mapping (focused on what the transform tests assert):
//!
//!   - `document` → [`DataIr::Document`]
//!   - `atx_heading` → `Element { name: "heading", markers: ["h{N}"], … }`
//!   - `setext_heading` → `Element { name: "heading", markers: ["h1"|"h2"], … }`
//!   - `list` → `Element { name: "list", markers: ["ordered"|"unordered"], children: <items> }`
//!   - `list_item` → `Element { name: "item", … }`
//!   - `block_quote` → `Element { name: "blockquote", … }`
//!   - `fenced_code_block` → `Element { name: "codeblock", children: [Element("language", text)?, Element("code", text)] }`
//!   - `indented_code_block` → `Element { name: "codeblock", children: [Element("code", text)] }`
//!   - `thematic_break` → `Element { name: "hr" }`

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::data::DataIr;
use super::lower_helpers::{range_of, span_of, text_of};
use super::types::{ByteRange, Span};

pub fn lower_markdown_data_root(root: TsNode<'_>, source: &str) -> DataIr {
    lower_node(root, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> DataIr {
    let range = range_of(node);
    let span = span_of(node);

    match node.kind() {
        "document" => DataIr::Document {
            children: lower_named_children(node, source),
            range,
            span,
        },

        // Headings
        "atx_heading" => {
            let level = atx_heading_level(node);
            DataIr::Element {
                name: "heading",
                markers: vec![level_marker(level)],
                children: lower_heading_content(node, source),
                range,
                span,
            }
        }
        "setext_heading" => {
            let level = setext_heading_level(node);
            DataIr::Element {
                name: "heading",
                markers: vec![level_marker(level)],
                children: lower_heading_content(node, source),
                range,
                span,
            }
        }

        // Lists. The ordered/unordered distinction comes from the
        // first item's marker kind (list_marker_dot/minus/star/plus
        // → unordered; list_marker_dot/parenthesis with digits →
        // ordered).
        "list" => {
            let marker = list_marker(node);
            DataIr::Element {
                name: "list",
                markers: vec![marker],
                children: lower_named_children(node, source),
                range,
                span,
            }
        }
        "list_item" => DataIr::Element {
            name: "item",
            markers: vec![],
            children: lower_list_item_content(node, source),
            range,
            span,
        },

        // Block quote
        "block_quote" => DataIr::Element {
            name: "blockquote",
            markers: vec![],
            children: lower_named_children(node, source),
            range,
            span,
        },

        // Code blocks. Fenced exposes optional `info_string` →
        // `language` text; indented has none.
        "fenced_code_block" => {
            let mut children: Vec<DataIr> = Vec::new();
            let mut cursor = node.walk();
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "info_string" => {
                        // Take just the language name (strip leading
                        // attribute classes like `python attr=…`).
                        let lang = text_of(c, source).split_whitespace().next().unwrap_or("").to_string();
                        if !lang.is_empty() {
                            children.push(DataIr::Element {
                                name: "language",
                                markers: vec![],
                                children: vec![DataIr::String {
                                    value: lang,
                                    range: range_of(c),
                                    span: span_of(c),
                                }],
                                range: range_of(c),
                                span: span_of(c),
                            });
                        }
                    }
                    "code_fence_content" => {
                        children.push(DataIr::Element {
                            name: "code",
                            markers: vec![],
                            children: vec![DataIr::String {
                                value: text_of(c, source),
                                range: range_of(c),
                                span: span_of(c),
                            }],
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                    _ => {}
                }
            }
            DataIr::Element {
                name: "codeblock",
                markers: vec![],
                children,
                range,
                span,
            }
        }
        "indented_code_block" => DataIr::Element {
            name: "codeblock",
            markers: vec![],
            children: vec![DataIr::Element {
                name: "code",
                markers: vec![],
                children: vec![DataIr::String {
                    value: text_of(node, source),
                    range,
                    span,
                }],
                range,
                span,
            }],
            range,
            span,
        },

        // Horizontal rule
        "thematic_break" => DataIr::Element {
            name: "hr",
            markers: vec![],
            children: vec![],
            range,
            span,
        },

        // HTML block (raw `<!-- ... -->`, `<div>...</div>`, etc.) and
        // inline HTML — rendered as `<html>` so XPath queries like
        // `//html[contains(., 'TODO')]` can find embedded comments.
        "html_block" | "html_tag" | "html_atx_open_tag" | "html_atx_close_tag" => {
            DataIr::Element {
                name: "html",
                markers: vec![],
                children: vec![DataIr::String {
                    value: text_of(node, source),
                    range,
                    span,
                }],
                range,
                span,
            }
        }

        // Paragraph — flatten its inline children rather than
        // wrapping in `<paragraph>`. The transform tests don't
        // assert paragraph structure; flattening keeps queries
        // like `//heading` simple at the cost of losing paragraph
        // boundaries.
        "paragraph" => DataIr::Element {
            name: "paragraph",
            markers: vec![],
            children: lower_named_children(node, source),
            range,
            span,
        },

        // Inline content (bold/italic/text/link/...) — just lower
        // children. Inline-level shape is intentionally not pinned
        // by these tests.
        "inline" | "section" => DataIr::Document {
            children: lower_named_children(node, source),
            range,
            span,
        },

        // Other kinds — fallthrough.
        other => DataIr::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

fn lower_named_children(node: TsNode<'_>, source: &str) -> Vec<DataIr> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect()
}

/// Heading content excludes the `atx_h{N}_marker` child (which
/// only carries the `#` markers, not text).
fn lower_heading_content(node: TsNode<'_>, source: &str) -> Vec<DataIr> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| !matches!(
            c.kind(),
            "atx_h1_marker" | "atx_h2_marker" | "atx_h3_marker"
            | "atx_h4_marker" | "atx_h5_marker" | "atx_h6_marker"
            | "setext_h1_underline" | "setext_h2_underline"
        ))
        .map(|c| lower_node(c, source))
        .collect()
}

/// List item content excludes list-marker children + task-list
/// markers (those become Element markers on the list, not content).
fn lower_list_item_content(node: TsNode<'_>, source: &str) -> Vec<DataIr> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| !matches!(
            c.kind(),
            "list_marker_dot" | "list_marker_minus" | "list_marker_plus"
            | "list_marker_star" | "list_marker_parenthesis"
            | "task_list_marker_checked" | "task_list_marker_unchecked"
            | "block_continuation"
        ))
        .map(|c| lower_node(c, source))
        .collect()
}

fn atx_heading_level(node: TsNode<'_>) -> u8 {
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "atx_h1_marker" => return 1,
            "atx_h2_marker" => return 2,
            "atx_h3_marker" => return 3,
            "atx_h4_marker" => return 4,
            "atx_h5_marker" => return 5,
            "atx_h6_marker" => return 6,
            _ => {}
        }
    }
    1
}

fn setext_heading_level(node: TsNode<'_>) -> u8 {
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "setext_h1_underline" => return 1,
            "setext_h2_underline" => return 2,
            _ => {}
        }
    }
    1
}

fn level_marker(level: u8) -> &'static str {
    match level {
        1 => "h1",
        2 => "h2",
        3 => "h3",
        4 => "h4",
        5 => "h5",
        6 => "h6",
        _ => "h1",
    }
}

/// Inspect the first list item's marker to classify the list.
fn list_marker(node: TsNode<'_>) -> &'static str {
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if c.kind() == "list_item" {
            let mut cc = c.walk();
            for inner in c.named_children(&mut cc) {
                match inner.kind() {
                    "list_marker_dot" | "list_marker_parenthesis" => return "ordered",
                    "list_marker_minus" | "list_marker_plus" | "list_marker_star" => {
                        return "unordered";
                    }
                    _ => {}
                }
            }
        }
    }
    "unordered"
}

// Re-export to silence "unused" warnings on lower_helpers in this file.
#[allow(dead_code)]
fn _ranges(_: ByteRange, _: Span) {}
