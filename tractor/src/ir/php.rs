//! PHP tree-sitter CST → IR lowering.
//!
//! **Status: scaffold.** Most CST kinds fall through to `Ir::Unknown`.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{ByteRange, Ir, Modifiers, Span};

pub fn lower_php_root(root: TsNode<'_>, source: &str) -> Ir {
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

pub fn lower_php_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        "name" | "variable_name" | "namespace_name"
        | "qualified_name" | "var_modifier" => Ir::Name { range, span },
        "integer" => Ir::Int { range, span },
        "float" => Ir::Float { range, span },
        "string" | "encapsed_string" | "string_value" | "heredoc"
        | "nowdoc" | "shell_command_expression" => Ir::String { range, span },
        "boolean" => {
            // text is "true"/"false" — emit a `<bool>` leaf.
            Ir::SimpleStatement {
                element_name: "bool",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: Vec::new(),
                range,
                span,
            }
        }
        "null" => Ir::Null { range, span },
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
