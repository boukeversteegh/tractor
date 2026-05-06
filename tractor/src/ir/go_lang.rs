//! Go tree-sitter CST → IR lowering.
//!
//! Mirrors the Rust IR pipeline pattern. Each per-kind arm
//! recursively lowers children; unhandled kinds fall through to
//! `Ir::Unknown`. The renderer in `crate::ir::render` is shared.
//!
//! **Status: under construction.** Many kinds typed but NOT yet
//! production-routed. `parse_with_ir_pipeline`'s allowlist is
//! unchanged for Go.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{AccessSegment, ByteRange, Ir, Modifiers, Span};

pub fn lower_go_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "source_file" => Ir::Module {
            element_name: "file",
            children: lower_children(root, source),
            range,
            span,
        },
        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

pub fn lower_go_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "identifier" | "type_identifier" | "field_identifier"
        | "package_identifier" | "blank_identifier" | "label_name"
        | "iota" | "dot" => Ir::Name { range, span },

        "int_literal" => Ir::Int { range, span },
        "float_literal" | "imaginary_literal" => Ir::Float { range, span },
        "interpreted_string_literal" | "raw_string_literal" => Ir::String { range, span },
        "rune_literal" => simple_statement(node, "char", source),
        "true" => Ir::True { range, span },
        "false" => Ir::False { range, span },
        "nil" => Ir::Null { range, span },
        "comment" => Ir::Comment { leading: false, trailing: false, range, span },

        // ----- Top-level structure -------------------------------------
        "package_clause" => simple_statement(node, "package", source),
        "import_declaration" => simple_statement(node, "import", source),
        "import_spec" => simple_statement(node, "spec", source),
        "import_spec_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Type declarations ---------------------------------------
        "type_declaration" => simple_statement(node, "type", source),
        "type_spec" => simple_statement(node, "spec", source),
        "type_alias" => simple_statement_marked(node, "alias", &[], source),
        "type_parameter_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("generics"),
            range, span,
        },
        "type_parameter_declaration" => simple_statement(node, "generic", source),
        "type_constraint" | "type_elem" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "type_arguments" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },

        // ----- Type-shape grammar --------------------------------------
        "function_type" => simple_statement_marked(node, "type", &["function"], source),
        "generic_type" => simple_statement_marked(node, "type", &["generic"], source),
        "negated_type" => simple_statement_marked(node, "type", &["approximation"], source),
        "array_type" => simple_statement(node, "array", source),
        "implicit_length_array_type" => simple_statement_marked(node, "array", &["implicit"], source),
        "slice_type" => simple_statement(node, "slice", source),
        "map_type" => simple_statement(node, "map", source),
        "channel_type" => simple_statement(node, "chan", source),
        "pointer_type" => simple_statement(node, "pointer", source),
        "struct_type" => simple_statement(node, "struct", source),
        "interface_type" => simple_statement(node, "interface", source),
        "qualified_type" => simple_statement(node, "type", source),
        "parenthesized_type" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Const / Var declarations --------------------------------
        "const_declaration" => simple_statement(node, "const", source),
        "const_spec" => simple_statement(node, "const", source),
        "var_declaration" => simple_statement(node, "var", source),
        "var_spec" => simple_statement(node, "var", source),
        "var_spec_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "short_var_declaration" => simple_statement(node, "variable", source),

        // ----- Functions / methods -------------------------------------
        "function_declaration" => simple_statement(node, "function", source),
        "method_declaration" => simple_statement(node, "method", source),
        "func_literal" => simple_statement(node, "closure", source),
        "method_elem" => simple_statement(node, "method", source),
        "field_declaration" => simple_statement(node, "field", source),
        "field_declaration_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "parameter_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("parameters"),
            range, span,
        },
        "parameter_declaration" => simple_statement(node, "parameter", source),
        "variadic_parameter_declaration" => simple_statement_marked(node, "parameter", &["variadic"], source),

        // ----- Control flow --------------------------------------------
        "if_statement" => simple_statement(node, "if", source),
        "for_statement" => simple_statement(node, "for", source),
        "for_clause" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "range_clause" => simple_statement(node, "range", source),
        "expression_switch_statement" => simple_statement(node, "switch", source),
        "type_switch_statement" => simple_statement_marked(node, "switch", &["type"], source),
        "expression_case" | "type_case" | "communication_case" => simple_statement(node, "case", source),
        "default_case" => simple_statement(node, "default", source),
        "select_statement" => simple_statement(node, "select", source),
        "send_statement" => simple_statement(node, "send", source),
        "receive_statement" => simple_statement(node, "receive", source),
        "return_statement" => simple_statement(node, "return", source),
        "break_statement" => simple_statement(node, "break", source),
        "continue_statement" => simple_statement(node, "continue", source),
        "goto_statement" => simple_statement(node, "goto", source),
        "go_statement" => simple_statement(node, "go", source),
        "defer_statement" => simple_statement(node, "defer", source),
        "labeled_statement" => simple_statement(node, "labeled", source),
        "fallthrough_statement" => simple_statement(node, "fallthrough", source),
        "expression_statement" => simple_statement(node, "expression", source),
        "empty_statement" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "block" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Expressions ---------------------------------------------
        "binary_expression" => simple_statement(node, "binary", source),
        "unary_expression" => simple_statement(node, "unary", source),
        "assignment_statement" => simple_statement(node, "assign", source),
        "inc_statement" => simple_statement(node, "unary", source),
        "dec_statement" => simple_statement(node, "unary", source),
        // Chain inversion for Go: selector_expression (`obj.field`) +
        // call_expression — fold into Ir::Access mirroring TS/Rust.
        "selector_expression" => {
            let operand_node = node.child_by_field_name("operand");
            let field_node = node.child_by_field_name("field");
            match (operand_node, field_node) {
                (Some(obj), Some(prop)) => {
                    let object_ir = lower_node(obj, source);
                    let property_range = range_of(prop);
                    let property_span = span_of(prop);
                    let segment_range = ByteRange::new(object_ir.range().end, property_range.end);
                    let segment = AccessSegment::Member {
                        property_range,
                        property_span,
                        optional: false,
                        range: segment_range,
                        span,
                    };
                    match object_ir {
                        Ir::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            Ir::Access { receiver, segments, range, span }
                        }
                        other => Ir::Access {
                            receiver: Box::new(other),
                            segments: vec![segment],
                            range, span,
                        },
                    }
                }
                _ => Ir::Unknown { kind: "selector_expression(missing)".to_string(), range, span },
            }
        }
        "call_expression" => {
            let function_node = node.child_by_field_name("function");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<Ir> = match args_node {
                Some(a) => {
                    let mut ac = a.walk();
                    a.named_children(&mut ac).map(|c| lower_node(c, source)).collect()
                }
                None => Vec::new(),
            };
            match function_node {
                Some(f) => {
                    let callee = lower_node(f, source);
                    let callee_range = callee.range();
                    if let Ir::Access { receiver, mut segments, .. } = callee {
                        let last_member = if let Some(AccessSegment::Member {
                            property_range, property_span, ..
                        }) = segments.last() {
                            Some((*property_range, *property_span))
                        } else { None };
                        let call_segment = if let Some((pr, ps)) = last_member {
                            segments.pop();
                            AccessSegment::Call {
                                name: Some(pr), name_span: Some(ps),
                                arguments,
                                range: ByteRange::new(pr.start, range.end),
                                span,
                            }
                        } else {
                            AccessSegment::Call {
                                name: None, name_span: None,
                                arguments,
                                range: ByteRange::new(callee_range.end, range.end),
                                span,
                            }
                        };
                        segments.push(call_segment);
                        return Ir::Access { receiver, segments, range, span };
                    }
                    Ir::Call { callee: Box::new(callee), arguments, range, span }
                }
                None => Ir::Unknown { kind: "call_expression(missing)".to_string(), range, span },
            }
        }
        "type_conversion_expression" => simple_statement_marked(node, "call", &["type"], source),
        "type_instantiation_expression" => simple_statement_marked(node, "type", &["generic"], source),
        "index_expression" => simple_statement(node, "index", source),
        "slice_expression" => simple_statement_marked(node, "index", &["slice"], source),
        "type_assertion_expression" => simple_statement(node, "assert", source),
        "composite_literal" => simple_statement(node, "literal", source),
        "literal_value" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "literal_element" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "keyed_element" => simple_statement(node, "pair", source),
        "argument_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },
        "variadic_argument" => simple_statement(node, "spread", source),
        "expression_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "parenthesized_expression" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- String / escape -----------------------------------------
        "interpreted_string_literal_content" | "raw_string_literal_content"
        | "escape_sequence" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let mut cursor = node.walk();
    let children: Vec<Ir> = node
        .named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
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
    let children: Vec<Ir> = node
        .named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
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
