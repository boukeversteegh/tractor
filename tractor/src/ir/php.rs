//! PHP tree-sitter CST → IR lowering.
//!
//! **Status: under construction.** Production parser does NOT yet
//! route PHP through this lowering.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{Access, AccessSegment, ByteRange, Ir, Modifiers, Span};

pub fn lower_php_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => Ir::Module {
            element_name: "program",
            children: merge_php_line_comments(lower_children(root, source), source),
            range, span,
        },
        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

/// Classify PHP comments (line `//`, line `#`, block `/* */`).
/// Adjacent line comments separated by a single newline merge into one
/// `<comment>` block. Pattern lifted from rust_lang/ruby IR.
fn merge_php_line_comments(children: Vec<Ir>, source: &str) -> Vec<Ir> {
    let mut out: Vec<Ir> = Vec::with_capacity(children.len());
    for child in children {
        if let Ir::Comment { leading, trailing, range, span } = child {
            let prev_non_comment = out.iter().rev().find(|c| !matches!(c, Ir::Comment { .. }));
            let curr_is_trailing = prev_non_comment.map_or(false, |prev| {
                let prev_end = prev.range().end as usize;
                let between = &source[prev_end..range.start as usize];
                !between.contains('\n')
            });
            if let Some(Ir::Comment { range: prev_range, .. }) = out.last() {
                let gap = &source[prev_range.end as usize..range.start as usize];
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_text = &source[prev_range.start as usize..prev_range.end as usize];
                let curr_text = &source[range.start as usize..range.end as usize];
                let prev_is_line_comment = prev_text.trim_start().starts_with("//")
                    || prev_text.trim_start().starts_with('#');
                let curr_is_line_comment = curr_text.trim_start().starts_with("//")
                    || curr_text.trim_start().starts_with('#');
                let prev_was_trailing = matches!(out.last(), Some(Ir::Comment { trailing: true, .. }));
                if only_one_newline
                    && prev_is_line_comment
                    && curr_is_line_comment
                    && !prev_was_trailing
                    && !curr_is_trailing
                {
                    if let Some(Ir::Comment { range: r, span: s, .. }) = out.last_mut() {
                        r.end = range.end;
                        s.end_line = span.end_line;
                        s.end_column = span.end_column;
                    }
                    continue;
                }
            }
            let trailing = trailing || curr_is_trailing;
            out.push(Ir::Comment { leading, trailing, range, span });
        } else {
            out.push(child);
        }
    }
    let n = out.len();
    for i in 0..n {
        if let Ir::Comment { trailing, range, .. } = &out[i] {
            if *trailing { continue; }
            let comment_end = range.end as usize;
            let next = out.iter().skip(i + 1).find(|c| !matches!(c, Ir::Comment { .. }));
            if let Some(next_ir) = next {
                let next_start = next_ir.range().start as usize;
                let between = &source[comment_end..next_start];
                let newlines = between.chars().filter(|&c| c == '\n').count();
                if newlines <= 1 && between.chars().all(|c| c.is_whitespace()) {
                    if let Ir::Comment { leading, .. } = &mut out[i] {
                        *leading = true;
                    }
                }
            }
        }
    }
    out
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
        "class_declaration" => php_class_like(node, source, "class"),
        "interface_declaration" => php_class_like(node, source, "interface"),
        "trait_declaration" => php_class_like(node, source, "trait"),
        "enum_declaration" => php_class_like(node, source, "enum"),
        "anonymous_class" => simple_statement_marked(node, "class", &["anonymous"], source),
        "base_clause" => simple_statement(node, "extends", source),
        "class_interface_clause" => simple_statement(node, "implements", source),
        "method_declaration" => php_method_declaration(node, source),
        "property_declaration" => php_property_declaration(node, source),
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
        "function_definition" => php_function_definition(node, source, "function", &[]),
        "anonymous_function" => php_function_definition(node, source, "function", &["anonymous"]),
        "arrow_function" => php_arrow_function(node, source),
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
        "if_statement" => php_if_statement(node, source),
        "else_clause" => php_else_clause(node, source),
        "else_if_clause" => php_else_if_clause(node, source),
        "switch_statement" => php_switch_statement(node, source),
        "case_statement" => simple_statement(node, "case", source),
        "default_statement" => simple_statement(node, "default", source),
        "match_expression" => php_match_expression(node, source),
        "match_conditional_expression" => simple_statement(node, "arm", source),
        "match_default_expression" => simple_statement_marked(node, "arm", &["default"], source),
        "for_statement" => php_for_statement(node, source),
        "foreach_statement" => php_foreach_statement(node, source),
        "while_statement" => php_while_statement(node, source),
        "do_statement" => php_do_statement(node, source),
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
        "binary_expression" => php_binary_expression(node, source),
        "unary_op_expression" => php_unary_op_expression(node, source),
        "error_suppression_expression" => php_error_suppression(node, source),
        "update_expression" => php_update_expression(node, source),
        "assignment_expression" => php_assignment(node, source),
        "augmented_assignment_expression" => php_assignment(node, source),
        "reference_assignment_expression" => php_assignment(node, source),
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
        // `$obj->method(args)` — fold into Ir::Access with a Call
        // segment carrying the method name. tree-sitter PHP fields:
        // object, name, arguments.
        "member_call_expression" | "nullsafe_member_call_expression" => {
            let object_node = node.child_by_field_name("object");
            let name_node = node.child_by_field_name("name");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<Ir> = match args_node {
                Some(a) => {
                    let mut ac = a.walk();
                    a.named_children(&mut ac).map(|c| lower_node(c, source)).collect()
                }
                None => Vec::new(),
            };
            match (object_node, name_node) {
                (Some(obj), Some(name)) => {
                    let object_ir = lower_node(obj, source);
                    let name_range = range_of(name);
                    let name_span = span_of(name);
                    let segment = AccessSegment::Call {
                        name: Some(name_range),
                        name_span: Some(name_span),
                        arguments,
                        range: ByteRange::new(object_ir.range().end, range.end),
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
                _ => simple_statement(node, "call", source),
            }
        }
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

fn text_of(node: TsNode<'_>, source: &str) -> String {
    source[node.start_byte()..node.end_byte()].to_string()
}

/// Wrap a single Ir in `<condition><expression>...</expression></condition>`
/// for control-flow shapes that demand the condition slot host.
fn wrap_condition(inner: Ir, range: ByteRange, span: Span) -> Ir {
    Ir::SimpleStatement {
        element_name: "condition",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children: vec![Ir::SimpleStatement {
            element_name: "expression",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![inner],
            range, span,
        }],
        range, span,
    }
}

/// Wrap the named children of a tree-sitter block-like node into a
/// `<body>` simple-statement. Used by `php_method_declaration`,
/// `php_function_definition`, and `php_*_statement`.
fn body_of(block: TsNode<'_>, source: &str) -> Ir {
    let mut bc = block.walk();
    let body_children: Vec<Ir> = block
        .named_children(&mut bc)
        .map(|s| lower_node(s, source))
        .collect();
    Ir::SimpleStatement {
        element_name: "body",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children: body_children,
        range: range_of(block),
        span: span_of(block),
    }
}

/// Lower `binary_expression`: extract op marker into `<op>`.
fn php_binary_expression(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
    let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
    let op_node = node.child_by_field_name("operator");
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
    let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    match (left, right, php_op_marker(&op_text)) {
        (Some(l), Some(r), Some(marker)) => Ir::Binary {
            element_name: if matches!(op_text.as_str(), "&&" | "||" | "and" | "or" | "xor") { "logical" } else { "binary" },
            op_text,
            op_marker: marker,
            op_range,
            left: Box::new(l),
            right: Box::new(r),
            range, span,
        },
        _ => simple_statement(node, "binary", source),
    }
}

fn php_op_marker(op: &str) -> Option<&'static str> {
    Some(match op {
        "+" => "plus",
        "-" => "minus",
        "*" => "multiply",
        "/" => "divide",
        "%" => "modulo",
        "**" => "power",
        "." => "concat",
        "==" => "equal",
        "!=" | "<>" => "not_equal",
        "===" => "identical",
        "!==" => "not_identical",
        "<" => "less",
        "<=" => "less_or_equal",
        ">" => "greater",
        ">=" => "greater_or_equal",
        "<=>" => "spaceship",
        "&&" | "and" => "and",
        "||" | "or" => "or",
        "xor" => "xor",
        "&" => "bitwise_and",
        "|" => "bitwise_or",
        "^" => "bitwise_xor",
        "<<" => "shift_left",
        ">>" => "shift_right",
        "??" => "null_coalesce",
        "instanceof" => "instanceof",
        _ => return None,
    })
}

/// Compound-assignment op marker (`+=`, `-=`, etc.). `=` itself maps
/// to `assign`. Returns `"assign"` for unknown forms (matches Go).
fn php_assign_op_marker(op: &str) -> &'static str {
    match op {
        "=" => "assign",
        "+=" => "plus",
        "-=" => "minus",
        "*=" => "multiply",
        "/=" => "divide",
        "%=" => "modulo",
        "**=" => "power",
        ".=" => "concat",
        "&=" => "bitwise_and",
        "|=" => "bitwise_or",
        "^=" => "bitwise_xor",
        "<<=" => "shift_left",
        ">>=" => "shift_right",
        "??=" => "null_coalesce",
        _ => "assign",
    }
}

/// Lower `unary_op_expression`: `+x`, `-x`, `!x`, `~x`.
fn php_unary_op_expression(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let op_node = node.child_by_field_name("operator");
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
    let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    let mut cursor = node.walk();
    let operand = node.named_children(&mut cursor).next();
    let marker = match op_text.as_str() {
        "+" => "plus",
        "-" => "minus",
        "!" => "not",
        "~" => "bitwise_not",
        _ => "",
    };
    match operand {
        Some(o) if !marker.is_empty() => Ir::Unary {
            op_text,
            op_marker: marker,
            op_range: op_byte_range,
            operand: Box::new(lower_node(o, source)),
            extra_markers: &[],
            range, span,
        },
        _ => simple_statement(node, "unary", source),
    }
}

/// Lower `error_suppression_expression`: `@expr`.
fn php_error_suppression(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let operand = node.named_children(&mut cursor).next();
    let mut tcursor = node.walk();
    let op_node = node.children(&mut tcursor).find(|c| !c.is_named());
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_else(|| "@".to_string());
    let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    match operand {
        Some(o) => Ir::Unary {
            op_text,
            op_marker: "suppress",
            op_range: op_byte_range,
            operand: Box::new(lower_node(o, source)),
            extra_markers: &[],
            range, span,
        },
        None => simple_statement(node, "unary", source),
    }
}

/// Lower `update_expression`: `++$x` / `$x++` / `--$x` / `$x--`.
/// Detect prefix vs postfix from token order.
fn php_update_expression(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut tcursor = node.walk();
    let all_children: Vec<TsNode<'_>> = node.children(&mut tcursor).collect();
    let op_node = all_children.iter().copied().find(|c| !c.is_named());
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
    let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    let was_prefix = match (all_children.first(), op_node) {
        (Some(first), Some(op)) => first.id() == op.id() && !first.is_named(),
        _ => false,
    };
    let mut cursor = node.walk();
    let operand = node.named_children(&mut cursor).next();
    let marker = match op_text.as_str() {
        "++" => "increment",
        "--" => "decrement",
        _ => "",
    };
    let extra_markers: &'static [&'static str] = if was_prefix { &["prefix"] } else { &[] };
    match operand {
        Some(o) if !marker.is_empty() => Ir::Unary {
            op_text,
            op_marker: marker,
            op_range: op_byte_range,
            operand: Box::new(lower_node(o, source)),
            extra_markers,
            range, span,
        },
        _ => simple_statement(node, "unary", source),
    }
}

/// Lower assignment / augmented_assignment / reference_assignment.
/// Extracts `<op>` marker for compound forms; emits `<assign>` with
/// `<left>`/`<right>` slot wrapping.
fn php_assignment(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let left_node = node.child_by_field_name("left");
    let right_node = node.child_by_field_name("right");
    let op_node = node.child_by_field_name("operator");
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_else(|| "=".to_string());
    let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    let op_marker_text = php_assign_op_marker(&op_text);
    match (left_node, right_node) {
        (Some(l), Some(r)) => Ir::Assign {
            targets: vec![lower_node(l, source)],
            type_annotation: None,
            op_text,
            op_range,
            op_markers: vec![op_marker_text],
            values: vec![lower_node(r, source)],
            range, span,
        },
        _ => simple_statement(node, "assign", source),
    }
}

/// Lower `if_statement` with condition/then slot wrapping and
/// flattened else-if chain.
fn php_if_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let alternative_node = node.child_by_field_name("alternative");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        let body = body_of(b, source);
        children.push(Ir::SimpleStatement {
            element_name: "then",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![body],
            range: range_of(b),
            span: span_of(b),
        });
    }
    if let Some(a) = alternative_node {
        // PHP wraps alternative as `else_clause` or `else_if_clause`.
        // It can also be a sequence (chain) of else_if_clauses + final else_clause.
        // child_by_field_name("alternative") returns one node; when chained
        // tree-sitter PHP emits multiple alternatives via *iterating* fields,
        // so we walk all named children of the if_statement past the body.
        // Simpler: use named_children index. We'll re-collect alternatives.
        let mut walk_cursor = node.walk();
        let mut seen_body = false;
        for c in node.named_children(&mut walk_cursor) {
            if Some(c.id()) == body_node.map(|b| b.id()) { seen_body = true; continue; }
            if !seen_body { continue; }
            // c is an alternative (else_clause / else_if_clause).
            children.push(lower_node(c, source));
        }
        let _ = a;
    }
    Ir::SimpleStatement {
        element_name: "if",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `else_clause` — wrap body in `<else><body>...`.
fn php_else_clause(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    // The else_clause body is its single named child (compound_statement).
    let mut cursor = node.walk();
    let child = node.named_children(&mut cursor).next();
    let inner: Vec<Ir> = match child {
        Some(c) if matches!(c.kind(), "compound_statement" | "colon_block") => {
            let mut bc = c.walk();
            c.named_children(&mut bc).map(|s| lower_node(s, source)).collect()
        }
        Some(c) => vec![lower_node(c, source)],
        None => Vec::new(),
    };
    Ir::SimpleStatement {
        element_name: "else",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children: vec![Ir::SimpleStatement {
            element_name: "body",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: inner,
            range, span,
        }],
        range, span,
    }
}

/// Lower `else_if_clause` — wrap as `<else_if><condition>...<body>...`.
fn php_else_if_clause(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        let body = body_of(b, source);
        children.push(body);
    }
    Ir::SimpleStatement {
        element_name: "else_if",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `while_statement` — `<while><condition>...<body>...`.
fn php_while_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        children.push(body_of(b, source));
    }
    Ir::SimpleStatement {
        element_name: "while",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `do_statement` — `<do><body>...<condition>...`.
fn php_do_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(b) = body_node {
        children.push(body_of(b, source));
    }
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    Ir::SimpleStatement {
        element_name: "do",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `for_statement`. Tree-sitter PHP exposes initialize/condition/
/// update fields and a body. Wrap the condition in
/// `<condition><expression>...` and the body in `<body>`. Init and
/// update stay bare.
fn php_for_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let condition_node = node.child_by_field_name("condition");
    let mut children: Vec<Ir> = Vec::new();
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if Some(c.id()) == body_node.map(|b| b.id()) {
            children.push(body_of(c, source));
            continue;
        }
        if Some(c.id()) == condition_node.map(|cn| cn.id()) {
            let inner = lower_node(c, source);
            children.push(Ir::SimpleStatement {
                element_name: "condition",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![Ir::SimpleStatement {
                    element_name: "expression",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![inner],
                    range: range_of(c),
                    span: span_of(c),
                }],
                range: range_of(c),
                span: span_of(c),
            });
            continue;
        }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
        element_name: "for",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `foreach_statement`. tree-sitter PHP exposes the iterable
/// and the binding as positional children (no field names) — first
/// expression is the iterable, last (before body) is the binding.
/// Wrap the iterable in `<right><expression>...</expression></right>`,
/// the binding in `<left><expression>...</expression></left>`, and
/// the body in `<body>`.
fn php_foreach_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    // Collect non-body named children — should be 2 (iterable + binding).
    let mut cursor = node.walk();
    let kids: Vec<_> = node
        .named_children(&mut cursor)
        .filter(|c| Some(c.id()) != body_node.map(|b| b.id()))
        .collect();
    let mut children: Vec<Ir> = Vec::new();
    if kids.len() >= 2 {
        // First is iterable → <right>, second is binding → <left>.
        let iter_node = kids[0];
        let bind_node = kids[kids.len() - 1];
        let iter_inner = lower_node(iter_node, source);
        children.push(Ir::SimpleStatement {
            element_name: "right",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![Ir::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![iter_inner],
                range: range_of(iter_node),
                span: span_of(iter_node),
            }],
            range: range_of(iter_node),
            span: span_of(iter_node),
        });
        let bind_inner = lower_node(bind_node, source);
        children.push(Ir::SimpleStatement {
            element_name: "left",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![Ir::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![bind_inner],
                range: range_of(bind_node),
                span: span_of(bind_node),
            }],
            range: range_of(bind_node),
            span: span_of(bind_node),
        });
    } else {
        for c in &kids { children.push(lower_node(*c, source)); }
    }
    if let Some(b) = body_node {
        children.push(body_of(b, source));
    }
    Ir::SimpleStatement {
        element_name: "foreach",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `switch_statement`. Condition wraps in `<condition>`; body
/// is a `switch_block` of case_statement / default_statement siblings.
fn php_switch_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    // Switch body is a switch_block — flatten its case/default children.
    if let Some(b) = body_node {
        let mut bc = b.walk();
        for case in b.named_children(&mut bc) {
            children.push(lower_node(case, source));
        }
    }
    Ir::SimpleStatement {
        element_name: "switch",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `match_expression` — `match ($cond) { ... }`. Wrap condition
/// in `<condition>`; body's arms render flat as siblings.
fn php_match_expression(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        let mut bc = b.walk();
        for arm in b.named_children(&mut bc) {
            children.push(lower_node(arm, source));
        }
    }
    Ir::SimpleStatement {
        element_name: "match",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower a PHP modifier child set (visibility / static / final /
/// abstract / readonly / var). When `default_public` is true and no
/// explicit visibility modifier is present, default access is Public
/// (PHP class members default to public).
fn php_modifiers(node: TsNode<'_>, source: &str, default_public: bool) -> Modifiers {
    let mut m = Modifiers::default();
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "visibility_modifier" => {
                let text = text_of(c, source);
                let trimmed = text.trim();
                m.access = match trimmed {
                    "public" => Some(Access::Public),
                    "private" => Some(Access::Private),
                    "protected" => Some(Access::Protected),
                    _ => m.access,
                };
            }
            "var_modifier" => {
                // PHP 4 `var $x` — equivalent to public.
                if m.access.is_none() {
                    m.access = Some(Access::Public);
                }
            }
            "static_modifier" => {
                // tree-sitter-php emits empty `static_modifier` as
                // "presence absent" — only flag if the node has text.
                let text = text_of(c, source);
                if text.trim() == "static" {
                    m.static_ = true;
                }
            }
            "final_modifier" => {
                let text = text_of(c, source);
                if text.trim() == "final" {
                    m.final_ = true;
                }
            }
            "abstract_modifier" => {
                let text = text_of(c, source);
                if text.trim() == "abstract" {
                    m.abstract_ = true;
                }
            }
            "readonly_modifier" => {
                let text = text_of(c, source);
                if text.trim() == "readonly" {
                    m.readonly = true;
                }
            }
            _ => {}
        }
    }
    if default_public && m.access.is_none() {
        m.access = Some(Access::Public);
    }
    m
}

/// True if the given child is a modifier kind we extracted into
/// `Modifiers`. Used to skip them from the structural children.
fn is_php_modifier(kind: &str) -> bool {
    matches!(
        kind,
        "visibility_modifier" | "var_modifier" | "static_modifier"
        | "final_modifier" | "abstract_modifier" | "readonly_modifier"
    )
}

/// Lower `method_declaration` — wrap body block in `<body>`, extract
/// modifiers, default visibility to public.
fn php_method_declaration(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let modifiers = php_modifiers(node, source, true);
    let mut children: Vec<Ir> = Vec::new();
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if is_php_modifier(c.kind()) {
            // Skip — already encoded in `modifiers`. The renderer
            // surfaces them as `<public/>` etc. extra-markers.
            continue;
        }
        if Some(c.id()) == body_node.map(|b| b.id()) {
            children.push(body_of(c, source));
            continue;
        }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
        element_name: "method",
        modifiers,
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `property_declaration` — extract modifiers, default
/// visibility to public. The property variable name (e.g. `$count`)
/// is emitted as a flat `<name>$count</name>` directly under
/// `<field>` (matching the imperative shape) instead of the
/// expression-form `<variable><name>count</name></variable>`.
fn php_property_declaration(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = php_modifiers(node, source, true);
    let mut children: Vec<Ir> = Vec::new();
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if is_php_modifier(c.kind()) { continue; }
        // property_element wraps the variable name. Emit the inner
        // variable_name as a flat `<name>$x</name>` leaf.
        if c.kind() == "property_element" {
            let mut pc = c.walk();
            for inner in c.named_children(&mut pc) {
                if inner.kind() == "variable_name" {
                    children.push(Ir::Name { range: range_of(inner), span: span_of(inner) });
                } else {
                    children.push(lower_node(inner, source));
                }
            }
        } else {
            children.push(lower_node(c, source));
        }
    }
    Ir::SimpleStatement {
        element_name: "field",
        modifiers,
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `class_declaration` / `interface_declaration` / `trait_declaration`
/// / `enum_declaration` — extract `final`/`abstract`/`readonly` modifiers
/// (no default access for class-level types in PHP).
fn php_class_like(
    node: TsNode<'_>,
    source: &str,
    element_name: &'static str,
) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = php_modifiers(node, source, false);
    let mut children: Vec<Ir> = Vec::new();
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if is_php_modifier(c.kind()) { continue; }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
        element_name,
        modifiers,
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower `function_definition` / `anonymous_function` — wrap body
/// block in `<body>`.
fn php_function_definition(
    node: TsNode<'_>,
    source: &str,
    element_name: &'static str,
    extra_markers: &'static [&'static str],
) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if Some(c.id()) == body_node.map(|b| b.id()) {
            children.push(body_of(c, source));
            continue;
        }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range, span,
    }
}

/// Lower `arrow_function` — `fn ($x) => expr`. Re-tag the single
/// expression as `<body>` for parity with `function_definition`; the
/// per-language `arrow_function` rule re-tags `<body>` to `<value>`.
fn php_arrow_function(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if Some(c.id()) == body_node.map(|b| b.id()) {
            // Single-expression body: wrap in `<body>` SimpleStatement.
            let inner = lower_node(c, source);
            children.push(Ir::SimpleStatement {
                element_name: "body",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![inner],
                range: range_of(c),
                span: span_of(c),
            });
            continue;
        }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
        element_name: "arrow",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
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
