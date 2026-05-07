//! INI tree-sitter CST → [`DataIr`] lowering.
//!
//! INI is the simplest data language: section headers (`[name]`)
//! containing flat `key = value` settings + comments.
//!
//! Mapping:
//!   - `document` → [`DataIr::Document`]
//!   - `section` → [`DataIr::Section`] (name from `section_name`)
//!   - `setting` → [`DataIr::Pair`] (key from `setting_name`,
//!     value as String from `setting_value`)
//!   - `comment` → [`DataIr::Comment`]

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::data::DataIr;
use super::types::{ByteRange, Span};

pub fn lower_ini_data_root(root: TsNode<'_>, source: &str) -> DataIr {
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
        "section" => {
            // First child is `section_name`; rest are settings /
            // comments.
            let mut cursor = node.walk();
            let mut named = node.named_children(&mut cursor);
            let header = named.next();
            let name_ir = match header {
                Some(h) if h.kind() == "section_name" => DataIr::String {
                    value: extract_text_inside(h, source),
                    range: range_of(h),
                    span: span_of(h),
                },
                _ => DataIr::String {
                    value: String::new(),
                    range,
                    span,
                },
            };
            let mut children: Vec<DataIr> = Vec::new();
            let mut c2 = node.walk();
            for child in node.named_children(&mut c2) {
                if child.kind() == "section_name" {
                    continue;
                }
                children.push(lower_node(child, source));
            }
            DataIr::Section {
                name: Box::new(name_ir),
                children,
                range,
                span,
            }
        }
        "setting" => {
            let mut name = String::new();
            let mut value = String::new();
            let mut name_node: Option<TsNode> = None;
            let mut value_node: Option<TsNode> = None;
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "setting_name" => {
                        name = text_of(child, source).trim().to_string();
                        name_node = Some(child);
                    }
                    "setting_value" => {
                        value = text_of(child, source).trim().to_string();
                        value_node = Some(child);
                    }
                    _ => {}
                }
            }
            let key_range = name_node.map(range_of).unwrap_or(range);
            let key_span = name_node.map(span_of).unwrap_or(span);
            let value_range = value_node.map(range_of).unwrap_or(range);
            let value_span = value_node.map(span_of).unwrap_or(span);
            DataIr::Pair {
                key: Box::new(DataIr::String {
                    value: name,
                    range: key_range,
                    span: key_span,
                }),
                value: Box::new(DataIr::String {
                    value,
                    range: value_range,
                    span: value_span,
                }),
                range,
                span,
            }
        }
        "comment" => {
            let raw = text_of(node, source);
            let text = strip_ini_comment_prefix(&raw);
            DataIr::Comment {
                text,
                leading: true,
                trailing: false,
                range,
                span,
            }
        }
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

/// `section_name` wraps the name in `[` `]` brackets with a `text`
/// child carrying the actual name. Pull the text content.
fn extract_text_inside(node: TsNode<'_>, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "text" {
            return text_of(child, source).trim().to_string();
        }
    }
    text_of(node, source).trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim()
        .to_string()
}

fn strip_ini_comment_prefix(raw: &str) -> String {
    raw.strip_prefix(|c: char| c == '#' || c == ';')
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| raw.trim().to_string())
}

fn text_of(node: TsNode<'_>, source: &str) -> String {
    let r = node.byte_range();
    source[r].to_string()
}

fn range_of(node: TsNode<'_>) -> ByteRange {
    let r = node.byte_range();
    ByteRange::new(r.start as u32, r.end as u32)
}

fn span_of(node: TsNode<'_>) -> Span {
    let s = node.start_position();
    let e = node.end_position();
    Span {
        line: s.row as u32 + 1,
        column: s.column as u32 + 1,
        end_line: e.row as u32 + 1,
        end_column: e.column as u32 + 1,
    }
}
