//! Rust tree-sitter CST → IR lowering.
//!
//! Mirrors the C#/Java/TypeScript IR pipeline patterns. Each per-kind
//! arm recursively lowers children; unhandled kinds fall through to
//! `Ir::Unknown`. The renderer in `crate::ir::render` is shared with
//! the other IR languages.
//!
//! **Status: scaffold.** Most CST kinds still fall through to
//! `Ir::Unknown`. The production parser does NOT yet route Rust
//! through this lowering — `parse_with_ir_pipeline`'s allowlist
//! must add `"rust"` once round-trip + XPath text recovery hold and
//! the major construct shapes are typed. Diagnostic via
//! `tests/coverage_report.rs`.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{ByteRange, Ir, Modifiers, Span};

/// Lower a Rust tree-sitter root node to [`Ir`].
pub fn lower_rust_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "source_file" => Ir::Module {
            element_name: "program",
            children: lower_children(root, source),
            range,
            span,
        },
        other => Ir::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// Public entry for lowering a single Rust CST node — useful for tests.
pub fn lower_rust_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "identifier" | "type_identifier" | "field_identifier"
        | "shorthand_field_identifier" | "primitive_type" | "scoped_identifier"
        | "scoped_type_identifier" | "self" | "super"
        | "metavariable" | "lifetime" | "label" => Ir::Name { range, span },

        "integer_literal" => Ir::Int { range, span },
        "float_literal" => Ir::Float { range, span },
        "string_literal" | "raw_string_literal" | "char_literal"
        | "byte_literal" | "string_content" => Ir::String { range, span },
        "boolean_literal" => {
            // Tree-sitter boolean_literal — text is "true" or "false".
            // Use simple_statement with an "true"/"false" element name; the
            // existing IR doesn't have True/False variants for Rust so we
            // emit `<bool>true|false</bool>` as a leaf.
            Ir::SimpleStatement {
                element_name: "bool",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: Vec::new(),
                range,
                span,
            }
        }

        // ----- Comments ------------------------------------------------
        "line_comment" | "block_comment" | "doc_comment" => Ir::Comment {
            leading: false,
            trailing: false,
            range,
            span,
        },

        // ----- Source-file children fall through to Inline -------------
        // Most top-level items remain as Unknown until each is typed.
        // The fallback below preserves source bytes via gap text.

        other => Ir::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

fn lower_children(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect()
}

fn range_of(node: TsNode<'_>) -> ByteRange {
    ByteRange::new(node.start_byte() as u32, node.end_byte() as u32)
}

fn span_of(node: TsNode<'_>) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    Span {
        line: start.row as u32 + 1,
        column: start.column as u32 + 1,
        end_line: end.row as u32 + 1,
        end_column: end.column as u32 + 1,
    }
}
