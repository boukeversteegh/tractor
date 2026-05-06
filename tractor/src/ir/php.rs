//! PHP tree-sitter CST → IR lowering.
//!
//! **Status: under construction.** Production parser does NOT yet
//! route PHP through this lowering.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{AccessSegment, ByteRange, Ir, Modifiers, Span};

pub fn lower_php_root(root: TsNode<'_>, source: &str) -> Ir {
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

pub fn lower_php_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "name" => Ir::Name { range, span },
        "variable_name" => simple_statement(node, "variable", source),
        "namespace_name" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "qualified_name" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "integer" => Ir::Int { range, span },
        "float" => Ir::Float { range, span },
        "string" => Ir::String { range, span },
        "encapsed_string" => simple_statement(node, "string", source),
        "heredoc" => simple_statement_marked(node, "string", &["heredoc"], source),
        "nowdoc" | "nowdoc_string" => simple_statement_marked(node, "string", &["nowdoc"], source),
        "boolean" => Ir::SimpleStatement {
            element_name: "bool",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range, span,
        },
        "null" => Ir::Null { range, span },
        "comment" => Ir::Comment { leading: false, trailing: false, range, span },

        // ----- PHP tag -------------------------------------------------
        "php_tag" => simple_statement_marked(node, "tag", &["open"], source),
        "text" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Top-level structures ------------------------------------
        "namespace_definition" => simple_statement(node, "namespace", source),
        "namespace_use_declaration" => simple_statement(node, "use", source),
        "use_declaration" => simple_statement(node, "use", source),
        "namespace_use_clause" | "namespace_use_group"
        | "use_as_clause" | "use_instead_of_clause" | "use_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "declare_statement" => simple_statement(node, "declare", source),
        "declare_directive" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Class / interface / trait / enum -----------------------
        "class_declaration" => simple_statement(node, "class", source),
        "interface_declaration" => simple_statement(node, "interface", source),
        "trait_declaration" => simple_statement(node, "trait", source),
        "enum_declaration" => simple_statement(node, "enum", source),
        "anonymous_class" => simple_statement_marked(node, "class", &["anonymous"], source),
        "base_clause" => simple_statement(node, "extends", source),
        "class_interface_clause" => simple_statement(node, "implements", source),
        "method_declaration" => simple_statement(node, "method", source),
        "property_declaration" => simple_statement(node, "field", source),
        "property_element" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "property_hook" => simple_statement(node, "method", source),
        "property_hook_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "const_declaration" => simple_statement(node, "const", source),
        "const_element" => simple_statement(node, "constant", source),
        "enum_case" => simple_statement(node, "constant", source),
        "enum_declaration_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "declaration_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Modifiers -----------------------------------------------
        "abstract_modifier" | "final_modifier" | "readonly_modifier"
        | "static_modifier" | "visibility_modifier" | "var_modifier" => {
            // Modifiers — produce as marker by extracting raw text and
            // mapping into a SimpleStatement element. Without modifier
            // semantics the simplest is to emit them as Inline (they
            // appear in source order).
            Ir::Inline {
                children: Vec::new(),
                list_name: None,
                range, span,
            }
        }
        "reference_modifier" | "by_ref" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Functions / parameters ---------------------------------
        "function_definition" => simple_statement(node, "function", source),
        "anonymous_function" => simple_statement_marked(node, "function", &["anonymous"], source),
        "arrow_function" => simple_statement(node, "arrow", source),
        "anonymous_function_use_clause" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "formal_parameters" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("parameters"),
            range, span,
        },
        "simple_parameter" => simple_statement(node, "parameter", source),
        "variadic_parameter" => simple_statement_marked(node, "parameter", &["variadic"], source),
        "property_promotion_parameter" => simple_statement_marked(node, "parameter", &["promoted"], source),

        // ----- Types ---------------------------------------------------
        "named_type" => simple_statement(node, "type", source),
        "primitive_type" => simple_statement(node, "type", source),
        "optional_type" => simple_statement_marked(node, "type", &["optional"], source),
        "union_type" => simple_statement_marked(node, "type", &["union"], source),
        "intersection_type" => simple_statement_marked(node, "type", &["intersection"], source),
        "bottom_type" => simple_statement_marked(node, "type", &["bottom"], source),
        "disjunctive_normal_form_type" => simple_statement_marked(node, "type", &["disjunctive"], source),
        "type_list" => simple_statement(node, "types", source),
        "cast_type" => simple_statement(node, "type", source),

        // ----- Control flow --------------------------------------------
        "if_statement" => simple_statement(node, "if", source),
        "else_clause" => simple_statement(node, "else", source),
        "else_if_clause" => simple_statement(node, "else_if", source),
        "switch_statement" => simple_statement(node, "switch", source),
        "case_statement" => simple_statement(node, "case", source),
        "default_statement" => simple_statement(node, "default", source),
        "match_expression" => simple_statement(node, "match", source),
        "match_conditional_expression" => simple_statement(node, "arm", source),
        "match_default_expression" => simple_statement_marked(node, "arm", &["default"], source),
        "for_statement" => simple_statement(node, "for", source),
        "foreach_statement" => simple_statement(node, "for", source),
        "while_statement" => simple_statement(node, "while", source),
        "do_statement" => simple_statement(node, "do", source),
        "try_statement" => simple_statement(node, "try", source),
        "catch_clause" => simple_statement(node, "catch", source),
        "finally_clause" => simple_statement(node, "finally", source),
        "throw_expression" => simple_statement(node, "throw", source),
        "return_statement" => simple_statement(node, "return", source),
        "break_statement" => simple_statement(node, "break", source),
        "continue_statement" => simple_statement(node, "continue", source),
        "goto_statement" => simple_statement(node, "goto", source),
        "named_label_statement" => simple_statement(node, "label", source),
        "echo_statement" => simple_statement(node, "echo", source),
        "exit_statement" => simple_statement(node, "exit", source),
        "yield_expression" => simple_statement(node, "yield", source),
        "expression_statement" => simple_statement(node, "expression", source),
        "compound_statement" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "colon_block" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "switch_block" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "match_block" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "match_condition_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "empty_statement" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Expressions ---------------------------------------------
        "binary_expression" => simple_statement(node, "binary", source),
        "unary_op_expression" => simple_statement(node, "unary", source),
        "error_suppression_expression" => simple_statement(node, "unary", source),
        "update_expression" => simple_statement(node, "unary", source),
        "assignment_expression" => simple_statement(node, "assign", source),
        "augmented_assignment_expression" => simple_statement(node, "assign", source),
        "reference_assignment_expression" => simple_statement(node, "assign", source),
        "conditional_expression" => simple_statement(node, "ternary", source),
        "function_call_expression" => {
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
                None => Ir::Unknown { kind: "function_call_expression(missing)".to_string(), range, span },
            }
        }
        "scoped_call_expression" => simple_statement_marked(node, "call", &["static"], source),
        "member_call_expression" => simple_statement(node, "call", source),
        "nullsafe_member_call_expression" => simple_statement_marked(node, "call", &["nullsafe"], source),
        "member_access_expression" => {
            let object_node = node.child_by_field_name("object");
            let name_node = node.child_by_field_name("name");
            match (object_node, name_node) {
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
                _ => simple_statement(node, "member", source),
            }
        }
        "scoped_property_access_expression" => simple_statement_marked(node, "member", &["static"], source),
        "nullsafe_member_access_expression" => simple_statement_marked(node, "member", &["nullsafe"], source),
        "class_constant_access_expression" => simple_statement(node, "member", source),
        "subscript_expression" => simple_statement(node, "index", source),
        "object_creation_expression" => simple_statement(node, "new", source),
        "cast_expression" => simple_statement(node, "cast", source),
        "clone_expression" => simple_statement(node, "clone", source),
        "unset_statement" => simple_statement(node, "unset", source),
        "include_expression" | "include_once_expression"
        | "require_expression" | "require_once_expression" => simple_statement(node, "require", source),
        "print_intrinsic" => simple_statement(node, "print", source),
        "shell_command_expression" => simple_statement(node, "shell", source),
        "array_creation_expression" => simple_statement(node, "array", source),
        "array_element_initializer" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "list_literal" => simple_statement(node, "array", source),
        "pair" => simple_statement(node, "pair", source),
        "argument" => simple_statement(node, "argument", source),
        "arguments" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },
        "variadic_unpacking" => simple_statement(node, "spread", source),
        "variadic_placeholder" => simple_statement_marked(node, "argument", &["variadic"], source),
        "parenthesized_expression" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "sequence_expression" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "primary_expression" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Variables -----------------------------------------------
        "function_static_declaration" => simple_statement_marked(node, "variable", &["static"], source),
        "static_variable_declaration" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "global_declaration" => simple_statement_marked(node, "variable", &["global"], source),
        "dynamic_variable_name" => simple_statement_marked(node, "variable", &["dynamic"], source),

        // ----- Strings / interpolation ---------------------------------
        "text_interpolation" => simple_statement(node, "interpolation", source),
        "string_value" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "string_content" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "escape_sequence" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "heredoc_body" | "heredoc_start" | "heredoc_end" | "nowdoc_body" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Attributes ----------------------------------------------
        "attribute" => simple_statement(node, "attribute", source),
        "attribute_group" | "attribute_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Misc ----------------------------------------------------
        "relative_scope" => simple_statement(node, "scope", source),
        // Supertype-style structural kinds.
        "expression" | "literal" | "operation" | "statement" | "type" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

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
