//! JSON tree-sitter CST → [`DataIr`] lowering.
//!
//! Pure function. No global state, no in-place mutation. Each
//! tree-sitter kind maps to exactly one DataIr variant (or
//! [`DataIr::Unknown`] if not yet covered).
//!
//! ## Coverage
//!
//! Tree-sitter-json's named-kind universe is small (12 kinds —
//! see `JsonKind`):
//!   - `document` → [`DataIr::Document`]
//!   - `object` → [`DataIr::Mapping`]
//!   - `array` → [`DataIr::Sequence`]
//!   - `pair` → [`DataIr::Pair`]
//!   - `string` → [`DataIr::String`] with parsed text (escapes
//!     resolved) — text bytes inside the quotes are
//!     `string_content` + `escape_sequence` children, which we
//!     reassemble.
//!   - `number` → [`DataIr::Number`] (raw text)
//!   - `true` / `false` → [`DataIr::Bool`]
//!   - `null` → [`DataIr::Null`]
//!   - `comment` → [`DataIr::Comment`] (JSON5 / JSONC)
//!
//! Unhandled kinds (`escape_sequence` / `string_content` outside a
//! `string` parent) fall through to [`DataIr::Unknown`].

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::data::DataIr;
use super::lower_helpers::{range_of, span_of, text_borrow};

/// Lower a JSON CST root node to [`DataIr`].
pub fn lower_json_data_root(root: TsNode<'_>, source: &str) -> DataIr {
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
        "object" => DataIr::Mapping {
            pairs: lower_named_children(node, source),
            range,
            span,
        },
        "array" => DataIr::Sequence {
            items: lower_named_children(node, source),
            range,
            span,
        },
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
        "string" => {
            let value = decode_json_string(node, source);
            DataIr::String { value, range, span }
        }
        "number" => DataIr::Number {
            text: text_of(node, source),
            range,
            span,
        },
        "true" => DataIr::Bool { value: true, range, span },
        "false" => DataIr::Bool { value: false, range, span },
        "null" => DataIr::Null { range, span },
        "comment" => {
            // JSON5 / JSONC line comment (`//`) or block comment
            // (`/* */`). Strip the leading delimiter for the `text`
            // field; the original is recoverable via the range.
            let raw = text_of(node, source);
            let text = strip_comment_delimiters(&raw);
            DataIr::Comment {
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

fn decode_json_string(node: TsNode<'_>, source: &str) -> String {
    // tree-sitter-json splits the string body into `string_content`
    // and `escape_sequence` children. Reassemble + decode escapes.
    let mut out = String::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
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

fn text_of(node: TsNode<'_>, source: &str) -> String {
    text_borrow(node, source).to_string()
}
