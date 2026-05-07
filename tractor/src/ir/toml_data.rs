//! TOML tree-sitter CST → [`DataIr`] lowering.
//!
//! TOML's natural projection is the data branch (key-as-element-
//! name): `[database]\nhost = "localhost"` becomes
//! `<database><host>localhost</host></database>`. The renderer
//! `render_data_to_xot_keyed` provides that shape; this lowering
//! produces a structural `DataIr` tree that maps cleanly onto it.
//!
//! Mapping:
//!   - `document` → [`DataIr::Document`] containing pairs +
//!     section children
//!   - `table` → [`DataIr::Section`] (one per `[name]` header)
//!   - `pair` → [`DataIr::Pair`] (key = bare/dotted/quoted key
//!     string; value = scalar / array / inline_table)
//!   - scalar kinds → [`DataIr::String`] / [`DataIr::Number`] /
//!     [`DataIr::Bool`]
//!   - `array` → [`DataIr::Sequence`]
//!   - `inline_table` → [`DataIr::Mapping`]

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::data::DataIr;
use super::types::{ByteRange, Span};

pub fn lower_toml_data_root(root: TsNode<'_>, source: &str) -> DataIr {
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
        "table" | "table_array_element" => {
            // First named child is the bare/dotted/quoted key.
            // Remaining children are pairs.
            let mut cursor = node.walk();
            let mut named = node.named_children(&mut cursor);
            let header = named.next();
            match header {
                Some(h) => {
                    let name_ir = lower_node(h, source);
                    let mut children: Vec<DataIr> = Vec::new();
                    let mut c2 = node.walk();
                    for child in node.named_children(&mut c2).skip(1) {
                        children.push(lower_node(child, source));
                    }
                    DataIr::Section {
                        name: Box::new(name_ir),
                        children,
                        range,
                        span,
                    }
                }
                None => DataIr::Unknown {
                    kind: "empty table".to_string(),
                    range,
                    span,
                },
            }
        }
        "pair" => {
            let mut cursor = node.walk();
            let mut named = node.named_children(&mut cursor);
            let key = named.next();
            let value = named.next();
            match (key, value) {
                (Some(k), Some(v)) => DataIr::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(lower_node(v, source)),
                    range,
                    span,
                },
                _ => DataIr::Unknown {
                    kind: "pair (missing key/value)".to_string(),
                    range,
                    span,
                },
            }
        }
        "bare_key" | "quoted_key" => {
            // Key text used as an element name in the keyed
            // renderer. Strip surrounding quotes for quoted_key.
            let raw = text_of(node, source);
            let value = if node.kind() == "quoted_key" {
                strip_quotes(&raw)
            } else {
                raw
            };
            DataIr::String { value, range, span }
        }
        "dotted_key" => {
            // Dotted key like `foo.bar.baz` — flatten to a single
            // string for now (the keyed renderer can split if
            // needed later).
            DataIr::String {
                value: text_of(node, source),
                range,
                span,
            }
        }
        "string" => DataIr::String {
            value: strip_quotes(&text_of(node, source)),
            range,
            span,
        },
        "integer" | "float" => DataIr::Number {
            text: text_of(node, source),
            range,
            span,
        },
        "boolean" => {
            let value = text_of(node, source).trim() == "true";
            DataIr::Bool { value, range, span }
        }
        "local_date" | "local_time" | "local_date_time" | "offset_date_time" => {
            // Treat dates / times as strings for now — they're
            // valid TOML scalars but tree-sitter tags them
            // separately. Future work could give them a dedicated
            // variant.
            DataIr::String {
                value: text_of(node, source),
                range,
                span,
            }
        }
        "array" => DataIr::Sequence {
            items: lower_named_children(node, source),
            range,
            span,
        },
        "inline_table" => DataIr::Mapping {
            pairs: lower_named_children(node, source),
            range,
            span,
        },
        "comment" => {
            let raw = text_of(node, source);
            let text = raw.strip_prefix('#').map(|s| s.trim_start().to_string())
                .unwrap_or(raw);
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

fn strip_quotes(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        raw.to_string()
    }
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
