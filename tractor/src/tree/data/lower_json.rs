//! JSON tree-sitter CST → [`DataTree`] lowering.
//!
//! Pure function. No global state, no in-place mutation. Each
//! tree-sitter kind maps to exactly one DataTree variant (or
//! [`DataTree::Unknown`] if not yet covered).
//!
//! ## Coverage
//!
//! Tree-sitter-json's named-kind universe is small (12 kinds —
//! see `JsonKind`):
//!   - `document` → [`DataTree::Document`]
//!   - `object` → [`DataTree::Mapping`]
//!   - `array` → [`DataTree::Sequence`]
//!   - `pair` → [`DataTree::Pair`]
//!   - `string` → [`DataTree::String`] with parsed text (escapes
//!     resolved) — text bytes inside the quotes are
//!     `string_content` + `escape_sequence` children, which we
//!     reassemble.
//!   - `number` → [`DataTree::Number`] (raw text)
//!   - `true` / `false` → [`DataTree::Bool`]
//!   - `null` → [`DataTree::Null`]
//!   - `comment` → [`DataTree::Comment`] (JSON5 / JSONC)
//!
//! Unhandled kinds (`escape_sequence` / `string_content` outside a
//! `string` parent) fall through to [`DataTree::Unknown`].


use crate::raw::RawNode;

use crate::tree::DataTree;
use crate::tree::lower_helpers::{range_of, span_of, text_borrow};
use crate::tree::types::QuoteStyle;

/// Lower a JSON CST root node to [`DataTree`].
pub fn lower_json_data_root(root: &RawNode, source: &str) -> DataTree {
    lower_node(root, source)
}

fn lower_node(node: &RawNode, source: &str) -> DataTree {
    let range = range_of(node);
    let span = span_of(node);

    match node.kind() {
        "document" => DataTree::Document {
            children: lower_named_children(node, source),
            range,
            span,
        },
        "object" => DataTree::Mapping {
            pairs: lower_named_children(node, source),
            range,
            span,
        },
        "array" => DataTree::Sequence {
            items: lower_named_children(node, source),
            range,
            span,
        },
        "pair" => {
            let mut named = node.named_children();
            let key = named.next();
            let value = named.next();
            match (key, value) {
                (Some(k), Some(v)) => DataTree::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(lower_node(v, source)),
                    range,
                    span,
                },
                _ => DataTree::Unknown {
                    kind: "pair (missing key/value)".to_string(),
                    range,
                    span,
                },
            }
        }
        "string" => {
            let value = decode_json_string(node, source);
            DataTree::String {
                value,
                quote_style: QuoteStyle::Double,
                range,
                span,
            }
        }
        "number" => DataTree::Number {
            text: text_of(node, source),
            range,
            span,
        },
        "true" => DataTree::Bool {
            value: true,
            text: "true".to_string(),
            range,
            span,
        },
        "false" => DataTree::Bool {
            value: false,
            text: "false".to_string(),
            range,
            span,
        },
        "null" => DataTree::Null {
            text: "null".to_string(),
            range,
            span,
        },
        "comment" => {
            // JSON5 / JSONC line comment (`//`) or block comment
            // (`/* */`). Strip the leading delimiter for the `text`
            // field; the original is recoverable via the range.
            let raw = text_of(node, source);
            let text = strip_comment_delimiters(&raw);
            DataTree::Comment {
                text,
                // Comment classification (leading vs trailing) is
                // refined in a post-pass once we know the surrounding
                // structural neighbours. For now, default to leading.
                leading: true,
                trailing: false,
                range,
                span,
            }
        }
        // `string_content` / `escape_sequence` only ever appear
        // *inside* a `string` parent — handled there. If we see
        // them at a top level it's a parse error.
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

fn decode_json_string(node: &RawNode, source: &str) -> String {
    // tree-sitter-json splits the string body into `string_content`
    // and `escape_sequence` children. Reassemble + decode escapes.
    let mut out = String::new();
    for child in node.named_children() {
        match child.kind() {
            "string_content" => {
                out.push_str(text_borrow(child, source));
            }
            "escape_sequence" => {
                let raw = text_borrow(child, source);
                out.push_str(&decode_escape(raw));
            }
            _ => {}
        }
    }
    out
}

fn decode_escape(raw: &str) -> String {
    let mut chars = raw.chars();
    if chars.next() != Some('\\') {
        return raw.to_string();
    }
    match chars.next() {
        Some('"') => "\"".to_string(),
        Some('\\') => "\\".to_string(),
        Some('/') => "/".to_string(),
        Some('b') => "\u{0008}".to_string(),
        Some('f') => "\u{000C}".to_string(),
        Some('n') => "\n".to_string(),
        Some('r') => "\r".to_string(),
        Some('t') => "\t".to_string(),
        Some('u') => {
            let hex: String = chars.take(4).collect();
            u32::from_str_radix(&hex, 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_else(|| raw.to_string())
        }
        _ => raw.to_string(),
    }
}

fn strip_comment_delimiters(raw: &str) -> String {
    if let Some(stripped) = raw.strip_prefix("//") {
        stripped.trim_start().to_string()
    } else if let Some(stripped) = raw.strip_prefix("/*") {
        stripped
            .strip_suffix("*/")
            .unwrap_or(stripped)
            .trim()
            .to_string()
    } else {
        raw.to_string()
    }
}

fn text_of(node: &RawNode, source: &str) -> String {
    text_borrow(node, source).to_string()
}
