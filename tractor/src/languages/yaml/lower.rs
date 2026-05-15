//! YAML tree-sitter CST → [`DataTree`] lowering.
//!
//! YAML's tree-sitter grammar is more verbose than JSON's:
//! `block_node` / `flow_node` are transparent wrappers around the
//! actual scalar/sequence/mapping; `block_sequence_item` wraps each
//! list element; mappings come in `block_mapping` and `flow_mapping`
//! flavours that lower to the same `DataTree::Mapping`.
//!
//! Lowered shape mirrors the JSON pipeline so the same renderer
//! (`render_data_to_xot_json`) produces matching XML.


use crate::raw::RawNode;

use crate::tree::DataTree;
use crate::tree::lower_helpers::{range_of, span_of, text_of};
use crate::tree::types::{ByteRange, QuoteStyle};

/// Lower a YAML CST root node (`stream`) to [`DataTree`].
pub fn lower_yaml_data_root(root: &RawNode, source: &str) -> DataTree {
    lower_node(root, source)
}

fn lower_node(node: &RawNode, source: &str) -> DataTree {
    let range = range_of(node);
    let span = span_of(node);

    match node.kind() {
        // `stream` is the YAML top level (one or more documents).
        // Render as `<document>` — matches the imperative pipeline
        // which renamed `stream` → `document` and flattened the
        // inner per-document wrapper.
        "stream" | "document" => DataTree::Document {
            children: lower_named_children(node, source),
            range,
            span,
        },

        // Block / flow nodes are transparent wrappers around their
        // inner scalar / sequence / mapping. Promote the inner.
        "block_node" | "flow_node" => {
            let inner = node.named_children().next();
            match inner {
                Some(c) => lower_node(c, source),
                None => DataTree::Unknown {
                    kind: "empty block_node".to_string(),
                    range,
                    span,
                },
            }
        }

        // Mappings (block + flow form) → DataTree::Mapping with Pair
        // children.
        "block_mapping" | "flow_mapping" => DataTree::Mapping {
            pairs: lower_named_children(node, source),
            range,
            span,
        },

        // Pairs.
        "block_mapping_pair" | "flow_pair" => {
            let key = node.child_by_field_name("key");
            let value = node.child_by_field_name("value");
            match (key, value) {
                (Some(k), Some(v)) => DataTree::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(lower_node(v, source)),
                    range,
                    span,
                },
                (Some(k), None) => DataTree::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(DataTree::Null {
                        text: String::new(),
                        range: ByteRange::empty_at(range.end),
                        span,
                    }),
                    range,
                    span,
                },
                _ => DataTree::Unknown {
                    kind: "pair (missing key)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Sequences (block + flow form) → DataTree::Sequence.
        "block_sequence" | "flow_sequence" => DataTree::Sequence {
            items: lower_named_children(node, source),
            range,
            span,
        },

        // `block_sequence_item` wraps each list element. Promote.
        "block_sequence_item" => {
            let inner = node.named_children().next();
            match inner {
                Some(c) => lower_node(c, source),
                None => DataTree::Null {
                    text: String::new(),
                    range,
                    span,
                },
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
                return DataTree::Bool {
                    value: b,
                    text: raw,
                    range,
                    span,
                };
            }
            if is_null_literal(trimmed) {
                return DataTree::Null { text: raw, range, span };
            }
            if is_numeric_literal(trimmed) {
                return DataTree::Number { text: trimmed.to_string(), range, span };
            }
            // Otherwise, string. Strip surrounding quotes if any and
            // tag the quote style from the CST kind.
            let value = strip_yaml_quotes(&raw);
            let quote_style = yaml_quote_style(node.kind(), &raw);
            DataTree::String { value, quote_style, range, span }
        }
        "integer_scalar" | "float_scalar" => DataTree::Number {
            text: text_of(node, source),
            range,
            span,
        },
        "boolean_scalar" => {
            let raw = text_of(node, source);
            let value = parse_bool(raw.trim()).unwrap_or(false);
            DataTree::Bool { value, text: raw, range, span }
        }
        "null_scalar" => {
            let raw = text_of(node, source);
            DataTree::Null { text: raw, range, span }
        }

        "comment" => {
            let raw = text_of(node, source);
            let text = strip_comment_prefix(&raw);
            DataTree::Comment { text, leading: true, trailing: false, range, span }
        }

        // YAML directives. `%YAML 1.2`, `%TAG !! …` —
        // tree-sitter-yaml exposes these as separate kinds.
        "yaml_directive" => {
            // Children: `yaml_version`. Wrap in `<version>` text
            // leaf so XPath `[version='1.2']` works.
            let mut children: Vec<DataTree> = Vec::new();
            for c in node.named_children() {
                if c.kind() == "yaml_version" {
                    children.push(DataTree::Pair {
                        key: Box::new(DataTree::String {
                            value: "version".to_string(),
                            quote_style: QuoteStyle::Plain,
                            range: range_of(c),
                            span: span_of(c),
                        }),
                        value: Box::new(DataTree::String {
                            value: text_of(c, source).trim().to_string(),
                            quote_style: QuoteStyle::Plain,
                            range: range_of(c),
                            span: span_of(c),
                        }),
                        range: range_of(c),
                        span: span_of(c),
                    });
                }
            }
            DataTree::Directive { flavor: "yaml", children, range, span }
        }
        "tag_directive" => {
            // Children: `tag_handle` + `tag_prefix`. Wrap each in
            // its own pair so XPath sees `<handle>` / `<prefix>`
            // text-children of `<directive[tag]>`.
            let mut children: Vec<DataTree> = Vec::new();
            for c in node.named_children() {
                let key = match c.kind() {
                    "tag_handle" => "handle",
                    "tag_prefix" => "prefix",
                    _ => continue,
                };
                children.push(DataTree::Pair {
                    key: Box::new(DataTree::String {
                        value: key.to_string(),
                        quote_style: QuoteStyle::Plain,
                        range: range_of(c),
                        span: span_of(c),
                    }),
                    value: Box::new(DataTree::String {
                        value: text_of(c, source).trim().to_string(),
                        quote_style: QuoteStyle::Plain,
                        range: range_of(c),
                        span: span_of(c),
                    }),
                    range: range_of(c),
                    span: span_of(c),
                });
            }
            DataTree::Directive { flavor: "tag", children, range, span }
        }
        "reserved_directive" => {
            DataTree::Directive {
                flavor: "reserved",
                children: lower_named_children(node, source),
                range,
                span,
            }
        }

        // Unhandled — alias / anchor / tag / directive etc. Pass
        // through to Unknown so XPath text-recovery still holds.
        other => DataTree::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

fn lower_named_children(node: &RawNode, source: &str) -> Vec<DataTree> {
    node.named_children()
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

/// Map a YAML scalar CST kind + raw text to a [`QuoteStyle`]. Plain
/// scalars use [`QuoteStyle::Plain`]; single / double quoted use the
/// matching variant; block scalars (`|` / `>`) become
/// [`QuoteStyle::Block`] with `folded` set from the leading marker.
/// Unrecognised kinds fall back to `Plain`.
fn yaml_quote_style(kind: &str, raw: &str) -> QuoteStyle {
    match kind {
        "single_quote_scalar" => QuoteStyle::Single,
        "double_quote_scalar" => QuoteStyle::Double,
        "block_scalar" => {
            // First non-whitespace char distinguishes literal vs folded.
            let folded = raw.trim_start().starts_with('>');
            // Chomp indicator follows the `|`/`>` marker.
            let chomp = raw
                .chars()
                .skip_while(|c| !matches!(c, '|' | '>'))
                .nth(1)
                .filter(|c| matches!(c, '-' | '+'))
                .unwrap_or(' ');
            QuoteStyle::Block { folded, chomp }
        }
        _ => QuoteStyle::Plain,
    }
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

