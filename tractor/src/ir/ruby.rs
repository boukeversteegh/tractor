//! Ruby tree-sitter CST → IR lowering.
//!
//! **Status: under construction.** Production parser does NOT yet
//! route Ruby through this lowering.

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
            range, span,
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
        // ----- Atoms ---------------------------------------------------
        "identifier" | "constant" | "global_variable" | "instance_variable"
        | "class_variable" | "self" | "method_identifier"
        | "encoding" | "file" | "line" | "setter" | "subshell"
        | "super" | "uninterpreted" => Ir::Name { range, span },

        "integer" => Ir::Int { range, span },
        "float" | "complex" | "rational" => Ir::Float { range, span },
        "string" => Ir::String { range, span },
        "character" => Ir::String { range, span },
        "regex" => simple_statement(node, "regex", source),
        "true" => Ir::True { range, span },
        "false" => Ir::False { range, span },
        "nil" => Ir::Null { range, span },
        "comment" => Ir::Comment { leading: false, trailing: false, range, span },

        // ----- Symbols -------------------------------------------------
        "simple_symbol" | "hash_key_symbol" => simple_statement(node, "symbol", source),
        "delimited_symbol" => simple_statement_marked(node, "symbol", &["delimited"], source),

        // ----- Module / class / method ---------------------------------
        "module" => simple_statement(node, "module", source),
        "class" => simple_statement(node, "class", source),
        "singleton_class" => simple_statement_marked(node, "class", &["singleton"], source),
        "method" => simple_statement(node, "method", source),
        "singleton_method" => simple_statement_marked(node, "method", &["singleton"], source),
        "lambda" => simple_statement(node, "lambda", source),

        // ----- Parameters ----------------------------------------------
        "method_parameters" | "block_parameters" | "lambda_parameters" => {
            Ir::Inline {
                children: lower_children(node, source),
                list_name: Some("parameters"),
                range, span,
            }
        }
        "block_parameter" => simple_statement_marked(node, "parameter", &["block"], source),
        "splat_parameter" => simple_statement_marked(node, "parameter", &["splat"], source),
        "hash_splat_parameter" => simple_statement_marked(node, "parameter", &["kwsplat"], source),
        "keyword_parameter" => simple_statement_marked(node, "parameter", &["keyword"], source),
        "optional_parameter" => simple_statement_marked(node, "parameter", &["default"], source),
        "forward_parameter" => simple_statement_marked(node, "parameter", &["forward"], source),
        "destructured_parameter" => simple_statement_marked(node, "parameter", &["destructured"], source),

        // ----- Control flow --------------------------------------------
        "if" | "if_modifier" => simple_statement(node, "if", source),
        "unless" | "unless_modifier" => simple_statement(node, "unless", source),
        "elsif" => simple_statement(node, "else_if", source),
        "else" => simple_statement(node, "else", source),
        "for" => simple_statement(node, "for", source),
        "while" | "while_modifier" => simple_statement(node, "while", source),
        "until" | "until_modifier" => simple_statement(node, "until", source),
        "case" => simple_statement(node, "case", source),
        "when" => simple_statement(node, "when", source),
        "case_match" => simple_statement(node, "match", source),
        "in_clause" => simple_statement(node, "in", source),
        "if_guard" => simple_statement(node, "if", source),
        "unless_guard" => simple_statement(node, "unless", source),
        "begin" => simple_statement(node, "begin", source),
        "begin_block" => simple_statement_marked(node, "block", &["begin"], source),
        "end_block" => simple_statement_marked(node, "block", &["end"], source),
        "do_block" => simple_statement_marked(node, "block", &["do"], source),
        "rescue" => simple_statement(node, "rescue", source),
        "rescue_modifier" => simple_statement(node, "rescue", source),
        "ensure" => simple_statement(node, "ensure", source),
        "exception_variable" => simple_statement(node, "variable", source),
        "exceptions" => simple_statement(node, "exceptions", source),
        "return" => simple_statement(node, "return", source),
        "break" => simple_statement(node, "break", source),
        "next" => simple_statement(node, "next", source),
        "redo" => simple_statement(node, "redo", source),
        "retry" => simple_statement(node, "retry", source),
        "yield" => simple_statement(node, "yield", source),
        "block" => simple_statement(node, "block", source),
        "do" => simple_statement(node, "do", source),
        "then" => simple_statement(node, "then", source),

        // ----- Expressions ---------------------------------------------
        "assignment" => simple_statement(node, "assign", source),
        "operator_assignment" => simple_statement(node, "assign", source),
        "binary" => simple_statement(node, "binary", source),
        "unary" => simple_statement(node, "unary", source),
        "call" => simple_statement(node, "call", source),
        "conditional" => simple_statement(node, "ternary", source),
        "range" => simple_statement(node, "range", source),
        "array" => simple_statement(node, "array", source),
        "hash" => simple_statement(node, "hash", source),
        "pair" => simple_statement(node, "pair", source),
        "string_array" => simple_statement_marked(node, "array", &["string"], source),
        "symbol_array" => simple_statement_marked(node, "array", &["symbol"], source),
        "chained_string" => simple_statement_marked(node, "string", &["concatenated"], source),
        "interpolation" => simple_statement(node, "interpolation", source),
        "element_reference" => simple_statement(node, "index", source),
        "scope_resolution" => simple_statement_marked(node, "member", &["static"], source),

        // ----- Patterns ------------------------------------------------
        "pattern" => simple_statement(node, "pattern", source),
        "alternative_pattern" => simple_statement_marked(node, "pattern", &["alternative"], source),
        "array_pattern" => simple_statement_marked(node, "pattern", &["array"], source),
        "as_pattern" => simple_statement_marked(node, "pattern", &["as"], source),
        "expression_reference_pattern" => simple_statement_marked(node, "pattern", &["expression"], source),
        "find_pattern" => simple_statement_marked(node, "pattern", &["find"], source),
        "hash_pattern" => simple_statement_marked(node, "pattern", &["hash"], source),
        "keyword_pattern" => simple_statement_marked(node, "pattern", &["keyword"], source),
        "match_pattern" => simple_statement_marked(node, "pattern", &["match"], source),
        "test_pattern" => simple_statement_marked(node, "pattern", &["test"], source),
        "variable_reference_pattern" => simple_statement_marked(node, "pattern", &["variable"], source),
        "parenthesized_pattern" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Arguments / spread --------------------------------------
        "argument_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },
        "splat_argument" => simple_statement_marked(node, "spread", &["list"], source),
        "hash_splat_argument" => simple_statement_marked(node, "spread", &["dict"], source),
        "block_argument" => simple_statement_marked(node, "argument", &["block"], source),
        "forward_argument" => simple_statement_marked(node, "argument", &["forward"], source),
        "hash_splat_nil" => simple_statement_marked(node, "spread", &["nil"], source),

        // ----- Structural wrappers (flatten) ---------------------------
        "body_statement" | "block_body" | "parenthesized_statements"
        | "string_content" | "escape_sequence" | "bare_string" | "bare_symbol"
        | "heredoc_beginning" | "heredoc_body" | "heredoc_content" | "heredoc_end"
        | "in" | "left_assignment_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "empty_statement" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Inheritance ---------------------------------------------
        "superclass" => simple_statement(node, "extends", source),

        // ----- Aliasing ------------------------------------------------
        "alias" => simple_statement(node, "alias", source),
        "undef" => simple_statement(node, "undef", source),

        // ----- Misc ----------------------------------------------------
        "rest_assignment" => simple_statement(node, "spread", source),
        "right_assignment_list" => simple_statement(node, "right", source),
        "destructured_left_assignment" => simple_statement_marked(node, "left", &["destructured"], source),
        "operator" => simple_statement(node, "operator", source),

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

fn simple_statement_marked(
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers: &'static [&'static str],
    source: &str,
) -> Ir {
    let mut cursor = node.walk();
    let children: Vec<Ir> = node.named_children(&mut cursor).map(|c| lower_node(c, source)).collect();
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
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
