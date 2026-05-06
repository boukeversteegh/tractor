//! T-SQL tree-sitter CST → IR lowering.
//!
//! T-SQL has 552 distinct CST kinds, most of which are reserved-
//! keyword leaves (`keyword_*`). Those are detached uniformly
//! (no semantic), leaving only ~80 structural kinds to type.
//!
//! **Status: under construction.** Production parser does NOT yet
//! route T-SQL through this lowering.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{ByteRange, Ir, Modifiers, Span};

pub fn lower_tsql_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => Ir::Module {
            element_name: "file",
            children: lower_children(root, source),
            range, span,
        },
        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

pub fn lower_tsql_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let kind = node.kind();

    // All `keyword_*` and the `op_*` operator leaves are detached —
    // their text is already in the surrounding source. We emit them
    // as Inline (no element, no children) so the gap-text mechanism
    // preserves the bytes.
    if kind.starts_with("keyword_") || kind.starts_with("op_") {
        return Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        };
    }

    match kind {
        // ----- Atoms ---------------------------------------------------
        "identifier" | "object_reference" | "column_reference" | "field" => {
            Ir::Name { range, span }
        }
        "int" => Ir::Int { range, span },
        "literal" => simple_statement(node, "literal", source),
        "string" | "national_string" => Ir::String { range, span },
        "comment" | "line_comment" | "block_comment" => {
            Ir::Comment { leading: false, trailing: false, range, span }
        }

        // ----- Statements ----------------------------------------------
        "statement" => simple_statement(node, "statement", source),
        "go_statement" => simple_statement(node, "go", source),
        "execute_statement" => simple_statement(node, "exec", source),
        "set_statement" => simple_statement(node, "set", source),
        "select_expression" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "select" => simple_statement(node, "select", source),
        "from" => simple_statement(node, "from", source),
        "where" => simple_statement(node, "where", source),
        "group_by" => simple_statement(node, "group", source),
        "having" => simple_statement(node, "having", source),
        "order_by" => simple_statement(node, "order", source),
        "order_target" => simple_statement(node, "target", source),
        "partition_by" => simple_statement(node, "partition", source),
        "join" => simple_statement(node, "join", source),
        "insert" => simple_statement(node, "insert", source),
        "update" => simple_statement(node, "update", source),
        "delete" => simple_statement(node, "delete", source),
        "transaction" => simple_statement(node, "transaction", source),
        "subquery" => simple_statement(node, "subquery", source),
        "set_operation" => simple_statement(node, "union", source),
        "cte" => simple_statement(node, "cte", source),

        // ----- DDL -----------------------------------------------------
        "create_table" => simple_statement(node, "create", source),
        "create_index" => simple_statement(node, "create", source),
        "create_function" => simple_statement(node, "function", source),
        "alter_table" => simple_statement(node, "alter", source),
        "add_column" => simple_statement(node, "column", source),
        "column_definition" => simple_statement(node, "definition", source),
        "column_definitions" => simple_statement(node, "columns", source),
        "column" => simple_statement(node, "column", source),
        "index_fields" => simple_statement(node, "columns", source),
        "function_body" => simple_statement(node, "body", source),

        // ----- Expressions ---------------------------------------------
        "binary_expression" => simple_statement(node, "compare", source),
        "unary_expression" => simple_statement(node, "unary", source),
        "assignment" => simple_statement(node, "assign", source),
        "between_expression" => simple_statement(node, "between", source),
        "exists" => simple_statement(node, "exists", source),
        "case" => simple_statement(node, "case", source),
        "when_clause" => simple_statement(node, "when", source),
        "cast" => simple_statement(node, "cast", source),
        "invocation" => simple_statement(node, "call", source),
        "function_argument" => simple_statement(node, "arg", source),
        "function_arguments" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "all_fields" => simple_statement(node, "star", source),
        "list" => simple_statement(node, "list", source),
        "term" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "relation" => simple_statement(node, "relation", source),
        "direction" => simple_statement(node, "direction", source),
        "window_function" => simple_statement(node, "window", source),
        "window_specification" => simple_statement(node, "over", source),

        // ----- Type keywords (datatypes) -------------------------------
        "varchar" => simple_statement(node, "varchar", source),
        "nvarchar" => simple_statement(node, "nvarchar", source),

        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let mut cursor = node.walk();
    let children: Vec<Ir> = node.named_children(&mut cursor).map(|c| lower_node(c, source)).collect();
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

fn lower_children(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).map(|c| lower_node(c, source)).collect()
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
