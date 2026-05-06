//! Ruby tree-sitter CST → IR lowering.
//!
//! **Status: scaffold.** Most CST kinds fall through to `Ir::Unknown`.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{ByteRange, Ir, Modifiers, Span};

pub fn lower_ruby_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => Ir::Module {
            element_name: "program",
            children: lower_children(root, source),
            range,
            span,
        },
        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

pub fn lower_ruby_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        "identifier" | "constant" | "global_variable" | "instance_variable"
        | "class_variable" | "self" | "method_identifier" => Ir::Name { range, span },
        "integer" => Ir::Int { range, span },
        "float" => Ir::Float { range, span },
        "string" | "string_content" | "simple_symbol" | "regex"
        | "character" => Ir::String { range, span },
        "true" => Ir::True { range, span },
        "false" => Ir::False { range, span },
        "nil" => Ir::Null { range, span },
        "comment" => Ir::Comment { leading: false, trailing: false, range, span },
        other => Ir::Unknown { kind: other.to_string(), range, span },
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
