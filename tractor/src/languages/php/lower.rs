//! PHP tree-sitter CST → tree lowering.
//!
//! Production parser routes PHP through this lowering end-to-end
//! (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/php/{rules,transformations,transform}.rs` modules
//! were retired in 7c13d427.


use crate::raw::RawNode;

use crate::tree::lower_helpers::{
    float_of, int_of, name_of, null_of, range_of, span_of, string_of, text_of,
};
use crate::tree::types::{Access, AccessSegment, ByteRange, Flag, SyntaxTree, Modifiers, Marker, OperatorKind, Span};

pub fn lower_php_root(root: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => SyntaxTree::Module {
            children: merge_php_line_comments(lower_children(root, source), source),
            range, span,
        },
        other => SyntaxTree::Unknown { kind: other.to_string(), range, span },
    }
}

/// Classify PHP comments (line `//`, line `#`, block `/* */`).
/// Adjacent line comments separated by a single newline merge into one
/// `<comment>` block. Pattern lifted from rust_lang/ruby tree.
fn merge_php_line_comments(children: Vec<SyntaxTree>, source: &str) -> Vec<SyntaxTree> {
    let mut out: Vec<SyntaxTree> = Vec::with_capacity(children.len());
    for child in children {
        if let SyntaxTree::Comment { leading, trailing, range, span } = child {
            let prev_non_comment = out.iter().rev().find(|c| !matches!(c, SyntaxTree::Comment { .. }));
            let curr_is_trailing = prev_non_comment.map_or(false, |prev| {
                let prev_end = prev.range().end as usize;
                let between = &source[prev_end..range.start as usize];
                !between.contains('\n')
            });
            if let Some(SyntaxTree::Comment { range: prev_range, .. }) = out.last() {
                let gap = &source[prev_range.end as usize..range.start as usize];
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_text = &source[prev_range.start as usize..prev_range.end as usize];
                let curr_text = &source[range.start as usize..range.end as usize];
                let prev_is_line_comment = prev_text.trim_start().starts_with("//")
                    || prev_text.trim_start().starts_with('#');
                let curr_is_line_comment = curr_text.trim_start().starts_with("//")
                    || curr_text.trim_start().starts_with('#');
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: Flag::On { .. }, .. }));
                if only_one_newline
                    && prev_is_line_comment
                    && curr_is_line_comment
                    && !prev_was_trailing
                    && !curr_is_trailing
                {
                    if let Some(SyntaxTree::Comment { range: r, span: s, .. }) = out.last_mut() {
                        r.end = range.end;
                        s.end_line = span.end_line;
                        s.end_column = span.end_column;
                    }
                    continue;
                }
            }
            let trailing = if trailing.is_set() || curr_is_trailing {
                Flag::implicit_at(span.line, span.column)
            } else {
                Flag::Off
            };
            out.push(SyntaxTree::Comment { leading, trailing, range, span });
        } else {
            out.push(child);
        }
    }
    let n = out.len();
    for i in 0..n {
        if let SyntaxTree::Comment { trailing, range, span, .. } = &out[i] {
            if trailing.is_set() { continue; }
            let comment_end = range.end as usize;
            let comment_range = *range;
            let comment_span = *span;
            let next = out.iter().skip(i + 1).find(|c| !matches!(c, SyntaxTree::Comment { .. }));
            if let Some(next_ir) = next {
                let next_start = next_ir.range().start as usize;
                let between = &source[comment_end..next_start];
                let newlines = between.chars().filter(|&c| c == '\n').count();
                if newlines <= 1 && between.chars().all(|c| c.is_whitespace()) {
                    if let SyntaxTree::Comment { leading, .. } = &mut out[i] {
                        *leading = Flag::implicit_at(comment_span.line, comment_span.column);
                        let _ = comment_range;
                    }
                }
            }
        }
    }
    out
}

pub fn lower_php_node(node: &RawNode, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

fn lower_node(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "name" => name_of(node, source),
        "variable_name" => simple_statement(node, "variable", source),
        "namespace_name" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "qualified_name" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "integer" => int_of(node, source),
        "float" => float_of(node, source),
        "string" => string_of(node, source),
        "encapsed_string" => {
            // PHP `"hello $name"` — wrap each variable / expression
            // child in `<interpolation>`. string_value text and
            // escape_sequence stay flat (their gap-text holds the
            // literal string content).
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| {
                    match c.kind() {
                        "variable_name" | "dynamic_variable_name"
                        | "subscript_expression" | "member_access_expression"
                        | "nullsafe_member_access_expression"
                        | "member_call_expression"
                        | "nullsafe_member_call_expression"
                        | "function_call_expression" => {
                            let inner = lower_node(c, source);
                            SyntaxTree::SimpleStatement {
                                element_name: "interpolation",
                                modifiers: Modifiers::default(),
                                extra_markers: Vec::new(),
                                children: vec![inner],
                                range: range_of(c),
                                span: span_of(c),
                            }
                        }
                        _ => lower_node(c, source),
                    }
                })
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "string",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "heredoc" => simple_statement_marked(node, "string", vec![Marker::implicit("heredoc")], source),
        "nowdoc" | "nowdoc_string" => simple_statement_marked(node, "string", vec![Marker::implicit("nowdoc")], source),
        "boolean" => SyntaxTree::SimpleStatement {
            element_name: "bool",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: Vec::new(),
            range, span,
        },
        "null" => null_of(node, source),
        "comment" => SyntaxTree::Comment { leading: Flag::Off, trailing: Flag::Off, range, span },

        // ----- PHP tag -------------------------------------------------
        "php_tag" => simple_statement_marked(node, "tag", vec![Marker::implicit("open")], source),
        "text" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Top-level structures ------------------------------------
        "namespace_definition" => simple_statement(node, "namespace", source),
        // `use Foo\Bar;` — single use. Lower as `<use>` with the path
        // children inlined.
        "namespace_use_declaration" | "use_declaration" => {
            // Detect a `namespace_use_group` child; if present, mark
            // this use as a group and its inner namespace_use_clauses
            // emit as `<use>` siblings.
            let has_group = node.named_children()
                .any(|c| c.kind() == "namespace_use_group");
            if has_group {
                php_use_group(node, source)
            } else {
                php_use_single(node, source)
            }
        }
        "namespace_use_clause" | "use_as_clause"
        | "use_instead_of_clause" | "use_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        // namespace_use_group: emits multiple <use> siblings — handled
        // inside php_use_group at the parent level.
        "namespace_use_group" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "declare_statement" => simple_statement(node, "declare", source),
        "declare_directive" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Class / interface / trait / enum -----------------------
        "class_declaration" => php_class_like(node, source, "class"),
        "interface_declaration" => php_class_like(node, source, "interface"),
        "trait_declaration" => php_class_like(node, source, "trait"),
        "enum_declaration" => php_class_like(node, source, "enum"),
        "anonymous_class" => simple_statement_marked(node, "class", vec![Marker::implicit("anonymous")], source),
        "base_clause" => php_wrap_extends_implements(node, "extends", source),
        "class_interface_clause" => php_wrap_extends_implements(node, "implements", source),
        "method_declaration" => php_method_declaration(node, source),
        "property_declaration" => php_property_declaration(node, source),
        "property_element" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "property_hook" => simple_statement(node, "method", source),
        "property_hook_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "const_declaration" => {
            // Class constants can have visibility modifiers (`public
            // const X = 1;`). Lift them onto the `<const>` element.
            let modifiers = php_modifiers(node, source, false);
            let children: Vec<SyntaxTree> = node
                .named_children()
                .filter(|c| !is_php_modifier(c.kind()))
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "const",
                modifiers,
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "const_element" => simple_statement(node, "constant", source),
        "enum_case" => simple_statement(node, "constant", source),
        "enum_declaration_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "declaration_list" => SyntaxTree::Inline {
            children: merge_php_line_comments(lower_children(node, source), source),
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
            SyntaxTree::Inline {
                children: Vec::new(),
                list_name: None,
                range, span,
            }
        }
        "reference_modifier" | "by_ref" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Functions / parameters ---------------------------------
        "function_definition" => php_function_definition(node, source, "function", Vec::new()),
        "anonymous_function" => php_function_definition(node, source, "function", vec![Marker::implicit("anonymous")]),
        "arrow_function" => php_arrow_function(node, source),
        "anonymous_function_use_clause" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "formal_parameters" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: Some("parameters"),
            range, span,
        },
        "simple_parameter" => simple_statement(node, "parameter", source),
        "variadic_parameter" => simple_statement_marked(node, "parameter", vec![Marker::implicit("variadic")], source),
        "property_promotion_parameter" => {
            // Constructor promotion: `public readonly int $x` —
            // lift the visibility / readonly modifiers onto the
            // <parameter> element.
            let modifiers = php_modifiers(node, source, false);
            let children: Vec<SyntaxTree> = node
                .named_children()
                .filter(|c| !is_php_modifier(c.kind()))
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "parameter",
                modifiers,
                extra_markers: vec![Marker::implicit("promoted")],
                children,
                range, span,
            }
        }

        // ----- Types ---------------------------------------------------
        "named_type" => simple_statement(node, "type", source),
        "primitive_type" => simple_statement(node, "type", source),
        "optional_type" => simple_statement_marked(node, "type", vec![Marker::implicit("optional")], source),
        "union_type" => simple_statement_marked(node, "type", vec![Marker::implicit("union")], source),
        "intersection_type" => simple_statement_marked(node, "type", vec![Marker::implicit("intersection")], source),
        "bottom_type" => simple_statement_marked(node, "type", vec![Marker::implicit("bottom")], source),
        "disjunctive_normal_form_type" => simple_statement_marked(node, "type", vec![Marker::implicit("disjunctive")], source),
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
        "match_default_expression" => simple_statement_marked(node, "arm", vec![Marker::implicit("default")], source),
        "for_statement" => php_for_statement(node, source),
        "foreach_statement" => php_foreach_statement(node, source),
        "while_statement" => php_while_statement(node, source),
        "do_statement" => php_do_statement(node, source),
        "try_statement" => {
            // Wrap the body's compound_statement in `<body>`. catch_clause
            // / finally_clause children stay positional.
            let body_node = node.child_by_field_name("body");
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                if Some(c.id()) == body_node.map(|b| b.id()) {
                    children.push(body_of(c, source));
                    continue;
                }
                children.push(lower_node(c, source));
            }
            SyntaxTree::SimpleStatement {
                element_name: "try",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "catch_clause" => {
            // Body block → <body>.
            let body_node = node.child_by_field_name("body");
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                if Some(c.id()) == body_node.map(|b| b.id()) {
                    children.push(body_of(c, source));
                    continue;
                }
                children.push(lower_node(c, source));
            }
            SyntaxTree::SimpleStatement {
                element_name: "catch",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "finally_clause" => {
            // Body block → <body>.
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                if c.kind() == "compound_statement" {
                    children.push(body_of(c, source));
                    continue;
                }
                children.push(lower_node(c, source));
            }
            SyntaxTree::SimpleStatement {
                element_name: "finally",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "throw_expression" => lower_php_throw(node, source),
        "return_statement" => simple_statement(node, "return", source),
        "break_statement" => simple_statement(node, "break", source),
        "continue_statement" => simple_statement(node, "continue", source),
        "goto_statement" => simple_statement(node, "goto", source),
        "named_label_statement" => simple_statement(node, "label", source),
        "echo_statement" => simple_statement(node, "echo", source),
        "exit_statement" => simple_statement(node, "exit", source),
        "yield_expression" => simple_statement(node, "yield", source),
        "expression_statement" => simple_statement(node, "expression", source),
        "compound_statement" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "colon_block" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "switch_block" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "match_block" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "match_condition_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "empty_statement" => SyntaxTree::Inline {
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
            let arguments: Vec<SyntaxTree> = match args_node {
                Some(a) => {
                    a.named_children().map(|c| lower_node(c, source)).collect()
                }
                None => Vec::new(),
            };
            match function_node {
                Some(f) => {
                    let callee = lower_node(f, source);
                    let callee_range = callee.range();
                    if let SyntaxTree::ObjectAccess { receiver, mut segments, .. } = callee {
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
                        return SyntaxTree::ObjectAccess { receiver, segments, range, span };
                    }
                    SyntaxTree::Call { callee: Box::new(callee), arguments, range, span }
                }
                None => SyntaxTree::Unknown { kind: "function_call_expression(missing)".to_string(), range, span },
            }
        }
        "scoped_call_expression" => simple_statement_marked(node, "call", vec![Marker::implicit("static")], source),
        // `$obj->method(args)` — fold into SyntaxTree::ObjectAccess with a Call
        // segment carrying the method name. tree-sitter PHP fields:
        // object, name, arguments.
        "member_call_expression" | "nullsafe_member_call_expression" => {
            let object_node = node.child_by_field_name("object");
            let name_node = node.child_by_field_name("name");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<SyntaxTree> = match args_node {
                Some(a) => {
                    a.named_children().map(|c| lower_node(c, source)).collect()
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
                        SyntaxTree::ObjectAccess { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::ObjectAccess { receiver, segments, range, span }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "self"]),
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
                        SyntaxTree::ObjectAccess { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::ObjectAccess { receiver, segments, range, span }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "self"]),
                            segments: vec![segment],
                            range, span,
                        },
                    }
                }
                _ => simple_statement(node, "member", source),
            }
        }
        "scoped_property_access_expression" => simple_statement_marked(node, "member", vec![Marker::implicit("static")], source),
        "nullsafe_member_access_expression" => simple_statement_marked(node, "member", vec![Marker::implicit("nullsafe")], source),
        "class_constant_access_expression" => simple_statement_marked(node, "member", vec![Marker::implicit("static")], source),
        "subscript_expression" => {
            // `$arr[0]` — fold into `SyntaxTree::ObjectAccess { receiver, segments: [Index] }`
            // so single-step bracket access produces the same
            // `<object[access]><index>...</index></object>` shape as a
            // multi-step chain. Mirrors TypeScript / C# / Go.
            //
            // tree-sitter-php's `subscript_expression` exposes the
            // dereferenced expression and the index as positional
            // named children rather than fields, so walk them in
            // order.
            let mut named = node.named_children();
            let object_node = named.next();
            let index_node = named.next();
            match object_node {
                Some(obj) => {
                    let object_ir = lower_node(obj, source);
                    let indices: Vec<SyntaxTree> = index_node
                        .map(|i| vec![lower_node(i, source)])
                        .unwrap_or_default();
                    let segment = AccessSegment::Index {
                        indices,
                        range: ByteRange::new(object_ir.range().end, range.end),
                        span,
                    };
                    match object_ir {
                        SyntaxTree::ObjectAccess { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::ObjectAccess { receiver, segments, range, span }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "self"]),
                            segments: vec![segment],
                            range, span,
                        },
                    }
                }
                None => simple_statement(node, "index", source),
            }
        }
        "object_creation_expression" => simple_statement(node, "new", source),
        "cast_expression" => simple_statement(node, "cast", source),
        "clone_expression" => simple_statement(node, "clone", source),
        "unset_statement" => simple_statement(node, "unset", source),
        "include_expression" | "include_once_expression"
        | "require_expression" | "require_once_expression" => simple_statement(node, "require", source),
        "print_intrinsic" => simple_statement(node, "print", source),
        "shell_command_expression" => simple_statement(node, "shell", source),
        "array_creation_expression" => simple_statement(node, "array", source),
        "array_element_initializer" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "list_literal" => simple_statement(node, "array", source),
        "pair" => simple_statement(node, "pair", source),
        "argument" => simple_statement(node, "argument", source),
        "arguments" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },
        "variadic_unpacking" => simple_statement(node, "spread", source),
        "variadic_placeholder" => simple_statement_marked(node, "argument", vec![Marker::implicit("variadic")], source),
        "parenthesized_expression" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "sequence_expression" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "primary_expression" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Variables -----------------------------------------------
        "function_static_declaration" => simple_statement_marked(node, "variable", vec![Marker::implicit("static")], source),
        "static_variable_declaration" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "global_declaration" => simple_statement_marked(node, "variable", vec![Marker::implicit("global")], source),
        "dynamic_variable_name" => simple_statement_marked(node, "variable", vec![Marker::implicit("dynamic")], source),

        // ----- Strings / interpolation ---------------------------------
        "text_interpolation" => simple_statement(node, "interpolation", source),
        "string_value" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "string_content" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "escape_sequence" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "heredoc_body" | "heredoc_start" | "heredoc_end" | "nowdoc_body" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Attributes ----------------------------------------------
        "attribute" => simple_statement(node, "attribute", source),
        "attribute_group" | "attribute_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Misc ----------------------------------------------------
        "relative_scope" => simple_statement(node, "scope", source),
        // Supertype-style structural kinds.
        "expression" | "literal" | "operation" | "statement" | "type" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        other => SyntaxTree::Unknown { kind: other.to_string(), range, span },
    }
}

fn simple_statement(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node.named_children().map(|c| lower_node(c, source)).collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Lower a PHP `base_clause` (`extends Foo`) or `class_interface_clause`
/// (`implements A, B`) into `<extends>/<type>/<name>` shape. Wraps each
/// base/interface identifier in a `<type>` slot so the canonical
/// type-reference vocabulary applies (Principle #14, mirrors the
/// Go/Ruby/Java analogues — see `wrap_go_interface_embed`).
fn php_wrap_extends_implements(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| {
            let inner = lower_node(c, source);
            let inner_range = inner.range();
            let inner_span = inner.span();
            match &inner {
                SyntaxTree::SimpleStatement { element_name: "type", .. } => inner,
                _ => SyntaxTree::SimpleStatement {
                    element_name: "type",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![inner],
                    range: inner_range,
                    span: inner_span,
                },
            }
        })
        .collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range,
        span,
    }
}

/// Lower PHP `throw expr` so the thrown expression sits under an
/// `<expression>` host (Principle #5 — matches the throw shapes
/// across Java / C# / TypeScript and the equivalent yield / raise
/// / return shapes).
fn lower_php_throw(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let inner: Vec<SyntaxTree> = node.named_children()
        .map(|c| lower_node(c, source))
        .collect();
    let children: Vec<SyntaxTree> = if inner.is_empty() {
        Vec::new()
    } else {
        let expr_start = inner.first().map(|i| i.range().start).unwrap_or(range.start);
        let expr_end = inner.last().map(|i| i.range().end).unwrap_or(range.end);
        let expr_range = ByteRange::new(expr_start, expr_end);
        vec![SyntaxTree::SimpleStatement {
            element_name: "expression",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: inner,
            range: expr_range,
            span,
        }]
    };
    SyntaxTree::SimpleStatement {
        element_name: "throw",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range,
        span,
    }
}

fn simple_statement_marked(
    node: &RawNode,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node.named_children().map(|c| lower_node(c, source)).collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

fn lower_children(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    node.named_children().map(|c| lower_node(c, source)).collect()
}


/// Wrap a single SyntaxTree in `<condition><expression>...</expression></condition>`
/// for control-flow shapes that demand the condition slot host.
fn wrap_condition(inner: SyntaxTree, range: ByteRange, span: Span) -> SyntaxTree {
    SyntaxTree::SimpleStatement {
        element_name: "condition",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children: vec![SyntaxTree::SimpleStatement {
            element_name: "expression",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![inner],
            range, span,
        }],
        range, span,
    }
}

/// Wrap the named children of a tree-sitter block-like node into a
/// `<body>` simple-statement. Used by `php_method_declaration`,
/// `php_function_definition`, and `php_*_statement`.
fn body_of(block: &RawNode, source: &str) -> SyntaxTree {
    let body_children: Vec<SyntaxTree> = block
        .named_children()
        .map(|s| lower_node(s, source))
        .collect();
    SyntaxTree::SimpleStatement {
        element_name: "body",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children: merge_php_line_comments(body_children, source),
        range: range_of(block),
        span: span_of(block),
    }
}

/// Lower `binary_expression`: extract op marker into `<op>`.
fn php_binary_expression(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
    let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
    let op_node = node.child_by_field_name("operator");
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
    let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    match (left, right, php_op_kind(&op_text)) {
        (Some(l), Some(r), Some(kind)) => {
            kind.build_binary(l, r, op_text, op_range, range, span)
        }
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

/// PHP binary / logical operators → typed [`OperatorKind`]. Mirrors
/// `php_op_marker` with the language-specific identical / spaceship
/// distinctions preserved.
fn php_op_kind(op: &str) -> Option<OperatorKind> {
    Some(match op {
        "+" => OperatorKind::Plus,
        "-" => OperatorKind::Minus,
        "*" => OperatorKind::Multiply,
        "/" => OperatorKind::Divide,
        "%" => OperatorKind::Modulo,
        "**" => OperatorKind::Power,
        "." => OperatorKind::Concat,
        "==" => OperatorKind::Equal,
        "!=" | "<>" => OperatorKind::NotEqual,
        "===" => OperatorKind::Identical,
        "!==" => OperatorKind::NotIdentical,
        "<" => OperatorKind::Less,
        "<=" => OperatorKind::LessOrEqual,
        ">" => OperatorKind::Greater,
        ">=" => OperatorKind::GreaterOrEqual,
        "<=>" => OperatorKind::Spaceship,
        "&&" | "and" => OperatorKind::And,
        "||" | "or" => OperatorKind::Or,
        "xor" => OperatorKind::Xor,
        "&" => OperatorKind::BitwiseAnd,
        "|" => OperatorKind::BitwiseOr,
        "^" => OperatorKind::BitwiseXor,
        "<<" => OperatorKind::ShiftLeft,
        ">>" => OperatorKind::ShiftRight,
        "??" => OperatorKind::NullCoalesce,
        "instanceof" => OperatorKind::Instanceof,
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
fn php_unary_op_expression(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let op_node = node.child_by_field_name("operator");
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
    let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    let operand = node.named_children().next();
    let marker = match op_text.as_str() {
        "+" => "plus",
        "-" => "minus",
        "!" => "not",
        "~" => "bitwise_not",
        _ => "",
    };
    match operand {
        Some(o) if !marker.is_empty() => SyntaxTree::Unary {
            op_text,
            op_marker: marker,
            op_range: op_byte_range,
            operand: Box::new(lower_node(o, source)),
            extra_markers: Vec::new(),
            range, span,
        },
        _ => simple_statement(node, "unary", source),
    }
}

/// Lower `error_suppression_expression`: `@expr`.
fn php_error_suppression(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let operand = node.named_children().next();
    let op_node = node.children().find(|c| !c.is_named());
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_else(|| "@".to_string());
    let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    match operand {
        Some(o) => SyntaxTree::Unary {
            op_text,
            op_marker: "suppress",
            op_range: op_byte_range,
            operand: Box::new(lower_node(o, source)),
            extra_markers: Vec::new(),
            range, span,
        },
        None => simple_statement(node, "unary", source),
    }
}

/// Lower `update_expression`: `++$x` / `$x++` / `--$x` / `$x--`.
/// Detect prefix vs postfix from token order.
fn php_update_expression(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let all_children: Vec<&RawNode> = node.children().collect();
    let op_node = all_children.iter().copied().find(|c| !c.is_named());
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
    let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    let was_prefix = match (all_children.first(), op_node) {
        (Some(first), Some(op)) => first.id() == op.id() && !first.is_named(),
        _ => false,
    };
    let operand = node.named_children().next();
    let marker = match op_text.as_str() {
        "++" => "increment",
        "--" => "decrement",
        _ => "",
    };
    let extra_markers: Vec<crate::tree::types::Marker> = if was_prefix { vec![Marker::implicit("prefix")] } else { Vec::new() };
    match operand {
        Some(o) if !marker.is_empty() => SyntaxTree::Unary {
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
fn php_assignment(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let left_node = node.child_by_field_name("left");
    let right_node = node.child_by_field_name("right");
    let op_node = node.child_by_field_name("operator");
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_else(|| "=".to_string());
    let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    let op_marker_text = php_assign_op_marker(&op_text);
    match (left_node, right_node) {
        (Some(l), Some(r)) => SyntaxTree::Assign {
            targets: vec![lower_node(l, source).wrap_expression()],
            type_annotation: None,
            op_text,
            op_range,
            op_markers: vec![op_marker_text],
            values: vec![lower_node(r, source).wrap_expression()],
            range, span,
        },
        _ => simple_statement(node, "assign", source),
    }
}

/// Lower `if_statement` with condition/then slot wrapping and
/// flattened else-if chain.
fn php_if_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let alternative_node = node.child_by_field_name("alternative");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        let body = body_of(b, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "then",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
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
        let mut seen_body = false;
        for c in node.named_children() {
            if Some(c.id()) == body_node.map(|b| b.id()) { seen_body = true; continue; }
            if !seen_body { continue; }
            // c is an alternative (else_clause / else_if_clause).
            children.push(lower_node(c, source));
        }
        let _ = a;
    }
    SyntaxTree::SimpleStatement {
        element_name: "if",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `else_clause` — wrap body in `<else><body>...`.
fn php_else_clause(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    // The else_clause body is its single named child (compound_statement).
    let child = node.named_children().next();
    let inner: Vec<SyntaxTree> = match child {
        Some(c) if matches!(c.kind(), "compound_statement" | "colon_block") => {
            c.named_children().map(|s| lower_node(s, source)).collect()
        }
        Some(c) => vec![lower_node(c, source)],
        None => Vec::new(),
    };
    SyntaxTree::SimpleStatement {
        element_name: "else",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children: vec![SyntaxTree::SimpleStatement {
            element_name: "body",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: inner,
            range, span,
        }],
        range, span,
    }
}

/// Lower `else_if_clause` — wrap as `<else_if><condition>...<body>...`.
fn php_else_if_clause(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        let body = body_of(b, source);
        children.push(body);
    }
    SyntaxTree::SimpleStatement {
        element_name: "else_if",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `while_statement` — `<while><condition>...<body>...`.
fn php_while_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        children.push(body_of(b, source));
    }
    SyntaxTree::SimpleStatement {
        element_name: "while",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `do_statement` — `<do><body>...<condition>...`.
fn php_do_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(b) = body_node {
        children.push(body_of(b, source));
    }
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    SyntaxTree::SimpleStatement {
        element_name: "do",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `for_statement`. Tree-sitter PHP exposes initialize/condition/
/// update fields and a body. Wrap the condition in
/// `<condition><expression>...` and the body in `<body>`. Init and
/// update stay bare.
/// Lower a PHP `use Foo\{Bar, Baz};` group-form. Produces
/// `<use[group]><path>Foo</path><use>Bar</use><use>Baz</use>...</use>`
/// matching the imperative shape.
/// Lower a single (non-group) `use Foo\Bar [as Baz];` declaration
/// into `<use[?alias]><path><name>+</name></path>[<aliased><name></aliased>]</use>`.
/// Aliasing carries the `[alias]` exhaustive marker; the path
/// segments live under `<path>` so queries can target the import path
/// independently of alias targets (Principle #9 markers, Principle #19
/// role-mixed leaves wrap).
fn php_use_single(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut children: Vec<SyntaxTree> = Vec::new();
    let mut extra_markers: Vec<Marker> = Vec::new();
    let push_path = |inner: &RawNode, children: &mut Vec<SyntaxTree>, source: &str| {
        let path_children: Vec<SyntaxTree> =
            inner.named_children().map(|n| lower_node(n, source)).collect();
        children.push(SyntaxTree::SimpleStatement {
            element_name: "path",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: path_children,
            range: range_of(inner),
            span: span_of(inner),
        });
    };
    let push_aliased = |inner: &RawNode, children: &mut Vec<SyntaxTree>, source: &str| {
        let aliased_children: Vec<SyntaxTree> = vec![lower_node(inner, source)];
        children.push(SyntaxTree::SimpleStatement {
            element_name: "aliased",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: aliased_children,
            range: range_of(inner),
            span: span_of(inner),
        });
    };
    let handle_clause = |
        clause: &RawNode,
        children: &mut Vec<SyntaxTree>,
        extra_markers: &mut Vec<Marker>,
        source: &str,
    | {
        // tree-sitter-php exposes `use_as_clause` (older grammar) or
        // `namespace_use_clause` with a trailing `name` sibling (newer
        // grammar) for `Foo\Bar as Baz`. Detect alias structurally: a
        // `name` child that follows a `qualified_name` / `namespace_name`
        // is the alias target.
        let inner_kids: Vec<_> = clause.named_children().collect();
        let path_idx = inner_kids
            .iter()
            .position(|c| matches!(c.kind(), "qualified_name" | "namespace_name"));
        let alias_idx = match path_idx {
            Some(p) => inner_kids
                .iter()
                .enumerate()
                .skip(p + 1)
                .find(|(_, c)| c.kind() == "name")
                .map(|(i, _)| i),
            None => None,
        };
        let is_aliased = clause.kind() == "use_as_clause" || alias_idx.is_some();
        if is_aliased && !extra_markers.iter().any(|m| m.name == "alias") {
            extra_markers.push(Marker::implicit("alias"));
        }
        for (i, inner) in inner_kids.iter().enumerate() {
            match inner.kind() {
                "qualified_name" | "namespace_name" => push_path(inner, children, source),
                "name" if Some(i) == alias_idx => push_aliased(inner, children, source),
                _ => children.push(lower_node(*inner, source)),
            }
        }
    };
    // Some tree-sitter-php versions expose the alias name as a sibling
    // of the qualified-name at the declaration level (not wrapped in
    // a `use_as_clause`). Detect this case: alias is the last `name`
    // child when it follows a `qualified_name` / `namespace_name`.
    let top_kids: Vec<_> = node.named_children().collect();
    let top_path_idx = top_kids
        .iter()
        .position(|c| matches!(c.kind(), "qualified_name" | "namespace_name"));
    let top_alias_idx = match top_path_idx {
        Some(p) => top_kids
            .iter()
            .enumerate()
            .skip(p + 1)
            .find(|(_, c)| c.kind() == "name")
            .map(|(i, _)| i),
        None => None,
    };
    if top_alias_idx.is_some() && !extra_markers.iter().any(|m| m.name == "alias") {
        extra_markers.push(Marker::implicit("alias"));
    }
    for (i, c) in top_kids.iter().enumerate() {
        match c.kind() {
            "namespace_use_clause" | "use_as_clause" => {
                handle_clause(c, &mut children, &mut extra_markers, source);
            }
            "qualified_name" | "namespace_name" => push_path(c, &mut children, source),
            "name" if Some(i) == top_alias_idx => push_aliased(c, &mut children, source),
            _ => children.push(lower_node(*c, source)),
        }
    }
    SyntaxTree::SimpleStatement {
        element_name: "use",
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range,
        span,
    }
}

fn php_use_group(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if c.kind() == "namespace_name" {
            // The path before the `\{...}`.
            let inner = lower_node(c, source);
            children.push(SyntaxTree::SimpleStatement {
                element_name: "path",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner],
                range: range_of(c),
                span: span_of(c),
            });
            continue;
        }
        if c.kind() == "namespace_use_group" {
            // Emit each clause inside as a `<use>` sibling.
            for clause in c.named_children() {
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "use",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![lower_node(clause, source)],
                    range: range_of(clause),
                    span: span_of(clause),
                });
            }
            continue;
        }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name: "use",
        modifiers: Modifiers::default(),
        extra_markers: vec![Marker::implicit("group")],
        children,
        range,
        span,
    }
}

fn php_for_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let condition_node = node.child_by_field_name("condition");
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if Some(c.id()) == body_node.map(|b| b.id()) {
            children.push(body_of(c, source));
            continue;
        }
        if Some(c.id()) == condition_node.map(|cn| cn.id()) {
            let inner = lower_node(c, source);
            children.push(SyntaxTree::SimpleStatement {
                element_name: "condition",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![SyntaxTree::SimpleStatement {
                    element_name: "expression",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
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
    SyntaxTree::SimpleStatement {
        element_name: "for",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
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
fn php_foreach_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    // Collect non-body named children — should be 2 (iterable + binding).
    let kids: Vec<_> = node
        .named_children()
        .filter(|c| Some(c.id()) != body_node.map(|b| b.id()))
        .collect();
    let mut children: Vec<SyntaxTree> = Vec::new();
    if kids.len() >= 2 {
        // First is iterable → <right>, second is binding → <left>.
        let iter_node = kids[0];
        let bind_node = kids[kids.len() - 1];
        let iter_inner = lower_node(iter_node, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "right",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![SyntaxTree::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![iter_inner],
                range: range_of(iter_node),
                span: span_of(iter_node),
            }],
            range: range_of(iter_node),
            span: span_of(iter_node),
        });
        let bind_inner = lower_node(bind_node, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "left",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![SyntaxTree::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
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
    SyntaxTree::SimpleStatement {
        element_name: "foreach",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `switch_statement`. Condition wraps in `<condition>`; body
/// is a `switch_block` of case_statement / default_statement siblings.
fn php_switch_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    // Switch body is a switch_block — flatten its case/default children.
    if let Some(b) = body_node {
        for case in b.named_children() {
            children.push(lower_node(case, source));
        }
    }
    SyntaxTree::SimpleStatement {
        element_name: "switch",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `match_expression` — `match ($cond) { ... }`. Wrap condition
/// in `<condition>`; body's arms render flat as siblings.
fn php_match_expression(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(wrap_condition(inner, range_of(c), span_of(c)));
    }
    if let Some(b) = body_node {
        for arm in b.named_children() {
            children.push(lower_node(arm, source));
        }
    }
    SyntaxTree::SimpleStatement {
        element_name: "match",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower a PHP modifier child set (visibility / static / final /
/// abstract / readonly / var). When `default_public` is true and no
/// explicit visibility modifier is present, default access is Public
/// (PHP class members default to public).
fn php_modifiers(node: &RawNode, source: &str, default_public: bool) -> Modifiers {
    let mut m = Modifiers::default();
    for c in node.named_children() {
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
                    m.static_ = crate::tree::types::Flag::anchored(range_of(c), span_of(c));
                }
            }
            "final_modifier" => {
                let text = text_of(c, source);
                if text.trim() == "final" {
                    m.final_ = crate::tree::types::Flag::anchored(range_of(c), span_of(c));
                }
            }
            "abstract_modifier" => {
                let text = text_of(c, source);
                if text.trim() == "abstract" {
                    m.abstract_ = crate::tree::types::Flag::anchored(range_of(c), span_of(c));
                }
            }
            "readonly_modifier" => {
                let text = text_of(c, source);
                if text.trim() == "readonly" {
                    m.readonly = crate::tree::types::Flag::anchored(range_of(c), span_of(c));
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
fn php_method_declaration(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let modifiers = php_modifiers(node, source, true);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
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
    SyntaxTree::SimpleStatement {
        element_name: "method",
        modifiers,
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `property_declaration` — extract modifiers, default
/// visibility to public. The property variable name (e.g. `$count`)
/// is emitted as a flat `<name>$count</name>` directly under
/// `<field>` (matching the imperative shape) instead of the
/// expression-form `<variable><name>count</name></variable>`.
fn php_property_declaration(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = php_modifiers(node, source, true);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if is_php_modifier(c.kind()) { continue; }
        // property_element wraps the variable name. Emit the inner
        // variable_name as a flat `<name>$x</name>` leaf.
        if c.kind() == "property_element" {
            for inner in c.named_children() {
                if inner.kind() == "variable_name" {
                    children.push(name_of(inner, source));
                } else {
                    children.push(lower_node(inner, source));
                }
            }
        } else {
            children.push(lower_node(c, source));
        }
    }
    SyntaxTree::SimpleStatement {
        element_name: "field",
        modifiers,
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `class_declaration` / `interface_declaration` / `trait_declaration`
/// / `enum_declaration` — extract `final`/`abstract`/`readonly` modifiers
/// (no default access for class-level types in PHP).
fn php_class_like(
    node: &RawNode,
    source: &str,
    element_name: &'static str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = php_modifiers(node, source, false);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if is_php_modifier(c.kind()) { continue; }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers,
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower `function_definition` / `anonymous_function` — wrap body
/// block in `<body>`.
fn php_function_definition(
    node: &RawNode,
    source: &str,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if Some(c.id()) == body_node.map(|b| b.id()) {
            children.push(body_of(c, source));
            continue;
        }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
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
fn php_arrow_function(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if Some(c.id()) == body_node.map(|b| b.id()) {
            // Single-expression body: wrap in `<body>` SimpleStatement.
            let inner = lower_node(c, source);
            children.push(SyntaxTree::SimpleStatement {
                element_name: "body",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner],
                range: range_of(c),
                span: span_of(c),
            });
            continue;
        }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name: "arrow",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}


