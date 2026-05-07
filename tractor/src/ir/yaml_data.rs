//! YAML tree-sitter CST → [`DataIr`] lowering.
//!
//! YAML's tree-sitter grammar is more verbose than JSON's:
//! `block_node` / `flow_node` are transparent wrappers around the
//! actual scalar/sequence/mapping; `block_sequence_item` wraps each
//! list element; mappings come in `block_mapping` and `flow_mapping`
//! flavours that lower to the same `DataIr::Mapping`.
//!
//! Lowered shape mirrors the JSON pipeline so the same renderer
//! (`render_data_to_xot_json`) produces matching XML.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::data::DataIr;
use super::lower_helpers::{range_of, span_of, text_of};
use super::types::ByteRange;

/// Lower a YAML CST root node (`stream`) to [`DataIr`].
pub fn lower_yaml_data_root(root: TsNode<'_>, source: &str) -> DataIr {
    lower_node(root, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> DataIr {
    let range = range_of(node);
    let span = span_of(node);

    match node.kind() {
        // `stream` is the YAML top level (one or more documents).
        // Render as `<document>` — matches the imperative pipeline
        // which renamed `stream` → `document` and flattened the
        // inner per-document wrapper.
        "stream" | "document" => DataIr::Document {
            children: lower_named_children(node, source),
            range,
            span,
        },

        // Block / flow nodes are transparent wrappers around their
        // inner scalar / sequence / mapping. Promote the inner.
        "block_node" | "flow_node" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(c) => lower_node(c, source),
                None => DataIr::Unknown {
                    kind: "empty block_node".to_string(),
                    range,
                    span,
                },
            }
        }

        // Mappings (block + flow form) → DataIr::Mapping with Pair
        // children.
        "block_mapping" | "flow_mapping" => DataIr::Mapping {
            pairs: lower_named_children(node, source),
            range,
            span,
        },

        // Pairs.
        "block_mapping_pair" | "flow_pair" => {
            let key = node.child_by_field_name("key");
            let value = node.child_by_field_name("value");
            match (key, value) {
                (Some(k), Some(v)) => DataIr::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(lower_node(v, source)),
                    range,
                    span,
                },
                (Some(k), None) => DataIr::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(DataIr::Null { range: ByteRange::empty_at(range.end), span }),
                    range,
                    span,
                },
                _ => DataIr::Unknown {
                    kind: "pair (missing key)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Sequences (block + flow form) → DataIr::Sequence.
        "block_sequence" | "flow_sequence" => DataIr::Sequence {
            items: lower_named_children(node, source),
            range,
            span,
        },

        // `block_sequence_item` wraps each list element. Promote.
        "block_sequence_item" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(c) => lower_node(c, source),
                None => DataIr::Null { range, span },
            }
        }

        // Scalars.
        "plain_scalar" | "string_scalar" | "single_quote_scalar"
        | "double_quote_scalar" | "block_scalar" => {
            // Heuristic: try to detect numeric / bool / null scalars
            // that the grammar didn't tag (plain_scalar can be any).
            let raw = text_of(node, source);
            let trimmed = raw.trim();
            if let Some(b) = parse_bool(trimmed) {
                return DataIr::Bool { value: b, range, span };
            }
            if is_null_literal(trimmed) {
                return DataIr::Null { range, span };
            }
            if is_numeric_literal(trimmed) {
                return DataIr::Number { text: trimmed.to_string(), range, span };
            }
            // Otherwise, string. Strip surrounding quotes if any.
            let value = strip_yaml_quotes(&raw);
            DataIr::String { value, range, span }
        }
        "integer_scalar" | "float_scalar" => DataIr::Number {
            text: text_of(node, source),
            range,
            span,
        },
        "boolean_scalar" => {
            let raw = text_of(node, source);
            let value = parse_bool(raw.trim()).unwrap_or(false);
            DataIr::Bool { value, range, span }
        }
        "null_scalar" => DataIr::Null { range, span },

        "comment" => {
            let raw = text_of(node, source);
            let text = strip_comment_prefix(&raw);
            DataIr::Comment { text, leading: true, trailing: false, range, span }
        }

        // YAML directives. `%YAML 1.2`, `%TAG !! …` —
        // tree-sitter-yaml exposes these as separate kinds.
        "yaml_directive" => {
            // Children: `yaml_version`. Wrap in `<version>` text
            // leaf so XPath `[version='1.2']` works.
            let mut children: Vec<DataIr> = Vec::new();
            let mut cursor = node.walk();
            for c in node.named_children(&mut cursor) {
                if c.kind() == "yaml_version" {
                    children.push(DataIr::Pair {
                        key: Box::new(DataIr::String {
                            value: "version".to_string(),
                            range: range_of(c),
                            span: span_of(c),
                        }),
                        value: Box::new(DataIr::String {
                            value: text_of(c, source).trim().to_string(),
                            range: range_of(c),
                            span: span_of(c),
                        }),
                        range: range_of(c),
                        span: span_of(c),
                    });
                }
            }
            DataIr::Directive { flavor: "yaml", children, range, span }
        }
        "tag_directive" => {
            // Children: `tag_handle` + `tag_prefix`. Wrap each in
            // its own pair so XPath sees `<handle>` / `<prefix>`
            // text-children of `<directive[tag]>`.
            let mut children: Vec<DataIr> = Vec::new();
            let mut cursor = node.walk();
            for c in node.named_children(&mut cursor) {
                let key = match c.kind() {
                    "tag_handle" => "handle",
                    "tag_prefix" => "prefix",
                    _ => continue,
                };
                children.push(DataIr::Pair {
                    key: Box::new(DataIr::String {
                        value: key.to_string(),
                        range: range_of(c),
                        span: span_of(c),
                    }),
                    value: Box::new(DataIr::String {
                        value: text_of(c, source).trim().to_string(),
                        range: range_of(c),
                        span: span_of(c),
                    }),
                    range: range_of(c),
                    span: span_of(c),
                });
            }
            DataIr::Directive { flavor: "tag", children, range, span }
        }
        "reserved_directive" => {
            DataIr::Directive {
                flavor: "reserved",
                children: lower_named_children(node, source),
                range,
                span,
            }
        }

        // Unhandled — alias / anchor / tag / directive etc. Pass
        // through to Unknown so XPath text-recovery still holds.
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

fn parse_bool(s: &str) -> Option<bool> {
    match s {
        "true" | "True" | "TRUE" | "yes" | "Yes" | "YES" | "on" | "On" | "ON" => Some(true),
        "false" | "False" | "FALSE" | "no" | "No" | "NO" | "off" | "Off" | "OFF" => Some(false),
        _ => None,
    }
}

fn is_null_literal(s: &str) -> bool {
    matches!(s, "null" | "Null" | "NULL" | "~" | "")
}

fn is_numeric_literal(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    // Integer or float — let str::parse handle the gnarly cases.
    s.parse::<f64>().is_ok()
}

fn strip_yaml_quotes(raw: &str) -> String {
    let trimmed = raw.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        raw.to_string()
    }
}

fn strip_comment_prefix(raw: &str) -> String {
    raw.strip_prefix('#')
        .map(|s| s.trim_start().to_string())
        .unwrap_or_else(|| raw.to_string())
}

