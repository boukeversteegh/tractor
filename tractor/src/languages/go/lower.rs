//! Go tree-sitter CST → tree lowering.
//!
//! Mirrors the Rust tree pipeline pattern. Each per-kind arm
//! recursively lowers children; unhandled kinds fall through to
//! `SyntaxTree::Unknown`. The renderer in `crate::tree::to_xot` is shared.
//!
//! Production parser routes Go through this lowering end-to-end
//! (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/go/{rules,transformations,transform}.rs` modules
//! were retired alongside this migration.


use crate::raw::RawNode;

use crate::tree::lower_helpers::{
    false_of, float_of, int_of, name_of, range_of, span_of, string_of, text_of, true_of,
};
use crate::tree::types::{AccessSegment, ByteRange, SyntaxTree, Modifiers, Marker, Span};

pub fn lower_go_root(root: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "source_file" => SyntaxTree::Module {
            element_name: "file",
            children: merge_go_line_comments(lower_children(root, source), source),
            range,
            span,
        },
        other => SyntaxTree::Unknown { kind: other.to_string(), range, span },
    }
}

/// Classify Go comments into leading/trailing/floating. Mirrors
/// merge_rust_line_comments — Go uses `//` and `/*...*/`, and like
/// rust, tree-sitter Go includes the trailing \n in the comment range.
fn merge_go_line_comments(children: Vec<SyntaxTree>, source: &str) -> Vec<SyntaxTree> {
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
                let no_newline = !gap.contains('\n') && gap.chars().all(|c| c.is_whitespace());
                let prev_text = &source[prev_range.start as usize..prev_range.end as usize];
                let curr_text = &source[range.start as usize..range.end as usize];
                let prev_is_line_comment = prev_text.trim_start().starts_with("//");
                let curr_is_line_comment = curr_text.trim_start().starts_with("//");
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: true, .. }));
                if no_newline
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
            let trailing = trailing || curr_is_trailing;
            out.push(SyntaxTree::Comment { leading, trailing, range, span });
        } else {
            out.push(child);
        }
    }
    let n = out.len();
    for i in 0..n {
        if let SyntaxTree::Comment { trailing, range, .. } = &out[i] {
            if *trailing { continue; }
            let comment_end = range.end as usize;
            let next = out.iter().skip(i + 1).find(|c| !matches!(c, SyntaxTree::Comment { .. }));
            if let Some(next_ir) = next {
                let next_start = next_ir.range().start as usize;
                let between = &source[comment_end..next_start];
                let newlines = between.chars().filter(|&c| c == '\n').count();
                if newlines <= 1 && between.chars().all(|c| c.is_whitespace()) {
                    if let SyntaxTree::Comment { leading, .. } = &mut out[i] {
                        *leading = true;
                    }
                }
            }
        }
    }
    out
}

pub fn lower_go_node(node: &RawNode, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

fn lower_node(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "identifier" | "type_identifier" | "field_identifier"
        | "package_identifier" | "blank_identifier" | "label_name"
        | "iota" | "dot" => name_of(node, source),

        "int_literal" => int_of(node, source),
        "float_literal" | "imaginary_literal" => float_of(node, source),
        "interpreted_string_literal" => string_of(node, source),
        "raw_string_literal" => simple_statement_marked(node, "string", vec![Marker::implicit("raw")], source),
        "rune_literal" => simple_statement(node, "char", source),
        "true" => true_of(node, source),
        "false" => false_of(node, source),
        "nil" => SyntaxTree::SimpleStatement {
            element_name: "nil",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: Vec::new(),
            range, span,
        },
        "comment" => SyntaxTree::Comment { leading: false, trailing: false, range, span },

        // ----- Top-level structure -------------------------------------
        "package_clause" => simple_statement(node, "package", source),
        // Each Go `import_spec` becomes its own `<import>` element so
        // that `import (a; b; c)` block-form imports surface as N
        // sibling `<import>` declarations (matches the line-form `import "a"`
        // shape). The `import_declaration` keyword + grouping
        // parentheses flow through as gap text.
        "import_declaration" => SyntaxTree::Inline {
            children: node
                .named_children()
                .filter(|c| matches!(c.kind(), "import_spec" | "import_spec_list"))
                .map(|c| lower_node(c, source))
                .collect(),
            list_name: None,
            range, span,
        },
        "import_spec" => lower_go_import_spec(node, source),
        "import_spec_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Type declarations ---------------------------------------
        // `type Foo bar` / `type Foo = bar` / type-decl block.
        // type_declaration is a wrapper that holds one or more type_spec
        // / type_alias children. Inline so each spec/alias becomes a
        // direct sibling of the parent file/body.
        "type_declaration" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        // `Foo bar` (defined type). Emit `<type>` with `<exported/>` /
        // `<unexported/>` marker, name, and type wrapped in `<type>`.
        "type_spec" => go_type_spec(node, "type", Vec::new(), source),
        "type_alias" => go_type_spec(node, "alias", Vec::new(), source),
        "type_parameter_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: Some("generics"),
            range, span,
        },
        "type_parameter_declaration" => simple_statement(node, "generic", source),
        "type_constraint" | "type_elem" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "type_arguments" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },

        // ----- Type-shape grammar --------------------------------------
        "function_type" => simple_statement_marked(node, "type", vec![Marker::implicit("function")], source),
        "generic_type" => simple_statement_marked(node, "type", vec![Marker::implicit("generic")], source),
        "negated_type" => simple_statement_marked(node, "type", vec![Marker::implicit("approximation")], source),
        "array_type" => simple_statement(node, "array", source),
        "implicit_length_array_type" => simple_statement_marked(node, "array", vec![Marker::implicit("implicit")], source),
        "slice_type" => simple_statement(node, "slice", source),
        "map_type" => simple_statement(node, "map", source),
        "channel_type" => simple_statement(node, "chan", source),
        "pointer_type" => simple_statement(node, "pointer", source),
        "struct_type" => simple_statement(node, "struct", source),
        "interface_type" => simple_statement(node, "interface", source),
        "qualified_type" => simple_statement(node, "type", source),
        "parenthesized_type" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Const / Var declarations --------------------------------
        // const_declaration / var_declaration are wrappers around one
        // or more specs — flatten so each spec becomes a direct
        // sibling of the parent file/body.
        "const_declaration" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "var_declaration" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "const_spec" => go_var_const_spec(node, "const", source),
        "var_spec" => go_var_const_spec(node, "var", source),
        "var_spec_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "short_var_declaration" => {
            // `i, j := 0, 1` — emit `<variable[short]>` with `<left>` /
            // `<right>` slot wrappers.
            let left_node = node.child_by_field_name("left");
            let right_node = node.child_by_field_name("right");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(l) = left_node {
                let inner = lower_node(l, source);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "left",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(l),
                        span: span_of(l),
                    }],
                    range: range_of(l),
                    span: span_of(l),
                });
            }
            if let Some(r) = right_node {
                let inner = lower_node(r, source);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "right",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(r),
                        span: span_of(r),
                    }],
                    range: range_of(r),
                    span: span_of(r),
                });
            }
            SyntaxTree::SimpleStatement {
                element_name: "variable",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("short")],
                children,
                range, span,
            }
        }

        // ----- Functions / methods -------------------------------------
        "function_declaration" => go_decl_with_export(node, "function", source),
        "method_declaration" => go_decl_with_export(node, "method", source),
        "func_literal" => simple_statement(node, "closure", source),
        "method_elem" => {
            // `Method() returnType` inside an interface body.
            let name_node = node.child_by_field_name("name");
            let parameters_node = node.child_by_field_name("parameters");
            let result_node = node.child_by_field_name("result");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            if let Some(p) = parameters_node {
                children.push(lower_node(p, source));
            }
            if let Some(r) = result_node {
                let inner = lower_node(r, source);
                children.push(SyntaxTree::Returns {
                    type_ann: Box::new(go_wrap_in_type_if_leaf(inner, range_of(r), span_of(r))),
                    range: range_of(r),
                    span: span_of(r),
                });
            }
            SyntaxTree::SimpleStatement {
                element_name: "method",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "field_declaration" => go_field_declaration(node, source),
        "field_declaration_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "parameter_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: Some("parameters"),
            range, span,
        },
        "parameter_declaration" => simple_statement(node, "parameter", source),
        "variadic_parameter_declaration" => simple_statement_marked(node, "parameter", vec![Marker::implicit("variadic")], source),

        // ----- Control flow --------------------------------------------
        "if_statement" => go_if_statement(node, source),
        "for_statement" => go_for_statement(node, source),
        "for_clause" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "range_clause" => {
            // `k, v := range items` — emit `<range>` with `<left>` and
            // `<right>` slot wrappers. tree-sitter Go uses fields
            // `left` and `right` (right being the iterable).
            let left_node = node.child_by_field_name("left");
            let right_node = node.child_by_field_name("right");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(l) = left_node {
                // The left can be an expression_list with multiple items.
                let mut left_children: Vec<SyntaxTree> = Vec::new();
                if l.kind() == "expression_list" {
                    for e in l.named_children() {
                        let inner = lower_node(e, source);
                        left_children.push(SyntaxTree::SimpleStatement {
                            element_name: "expression",
                            modifiers: Modifiers::default(),
                            extra_markers: Vec::new(),
                            children: vec![inner],
                            range: range_of(e),
                            span: span_of(e),
                        });
                    }
                } else {
                    let inner = lower_node(l, source);
                    left_children.push(SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(l),
                        span: span_of(l),
                    });
                }
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "left",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: left_children,
                    range: range_of(l),
                    span: span_of(l),
                });
            }
            if let Some(r) = right_node {
                let inner = lower_node(r, source);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "right",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(r),
                        span: span_of(r),
                    }],
                    range: range_of(r),
                    span: span_of(r),
                });
            }
            SyntaxTree::SimpleStatement {
                element_name: "range",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "expression_switch_statement" => go_switch(node, false, source),
        "type_switch_statement" => go_switch(node, true, source),
        "expression_case" | "type_case" | "communication_case" => go_case(node, source),
        "default_case" => simple_statement(node, "default", source),
        "select_statement" => simple_statement(node, "select", source),
        "send_statement" => simple_statement(node, "send", source),
        "receive_statement" => simple_statement(node, "receive", source),
        "return_statement" => {
            // Go's `return x, y, z` — wrap EACH returned value in
            // `<expression>` so consumers see one expression per value
            // (multi-return is the same archetype as multi-target call
            // args). Replaces the imperative `wrap_expression_positions`
            // post-walk for the return slot.
            let children: Vec<SyntaxTree> = node.named_children()
                .flat_map(|c| {
                    if c.kind() == "expression_list" {
                        c.named_children()
                            .map(|item| *crate::tree::Expression::wrap(lower_node(item, source)).inner)
                            .collect::<Vec<_>>()
                    } else {
                        vec![*crate::tree::Expression::wrap(lower_node(c, source)).inner]
                    }
                })
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "return",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range: range_of(node),
                span: span_of(node),
            }
        }
        "break_statement" => simple_statement(node, "break", source),
        "continue_statement" => simple_statement(node, "continue", source),
        "goto_statement" => simple_statement(node, "goto", source),
        "go_statement" => simple_statement(node, "go", source),
        "defer_statement" => simple_statement(node, "defer", source),
        "labeled_statement" => simple_statement(node, "labeled", source),
        "fallthrough_statement" => simple_statement(node, "fallthrough", source),
        "expression_statement" => simple_statement(node, "expression", source),
        "empty_statement" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        "block" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Expressions ---------------------------------------------
        "binary_expression" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (left, right, op_marker(&op_text)) {
                (Some(l), Some(r), Some(marker)) => SyntaxTree::Binary {
                    element_name: if matches!(op_text.as_str(), "&&" | "||") { "logical" } else { "binary" },
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l.wrap_slot("left")),
                    right: Box::new(r.wrap_slot("right")),
                    range, span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "binary_expression(missing)".to_string(),
                    range, span,
                },
            }
        }
        "unary_expression" => {
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let operand = node.named_children().next();
            let marker = match op_text.as_str() {
                "+" => "plus",
                "-" => "minus",
                "*" => "dereference",
                "&" => "address",
                "!" => "not",
                "^" => "bitwise_not",
                "<-" => "receive",
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
        "assignment_statement" => {
            // `lhs = rhs` / `lhs += rhs` — emit `<assign>` with `<op>`
            // marker and `<left><expression>` / `<right><expression>`
            // slots. tree-sitter Go uses fields `left`, `right`,
            // `operator`.
            let left_node = node.child_by_field_name("left");
            let right_node = node.child_by_field_name("right");
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let op_marker_text = match op_text.as_str() {
                "=" => "assign",
                "+=" => "plus",
                "-=" => "minus",
                "*=" => "multiply",
                "/=" => "divide",
                "%=" => "modulo",
                "&=" => "bitwise_and",
                "|=" => "bitwise_or",
                "^=" => "bitwise_xor",
                "<<=" => "shift_left",
                ">>=" => "shift_right",
                "&^=" => "bitwise_clear",
                _ => "assign",
            };
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
                _ => SyntaxTree::Unknown {
                    kind: "assignment_statement(missing)".to_string(),
                    range, span,
                },
            }
        }
        "inc_statement" => go_inc_dec(node, "increment", "++", source),
        "dec_statement" => go_inc_dec(node, "decrement", "--", source),
        // Chain inversion for Go: selector_expression (`obj.field`) +
        // call_expression — fold into SyntaxTree::Access mirroring TS/Rust.
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
                        SyntaxTree::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::Access { receiver, segments, range, span }
                        }
                        other => SyntaxTree::Access {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &[]),
                            segments: vec![segment],
                            range, span,
                        },
                    }
                }
                _ => SyntaxTree::Unknown { kind: "selector_expression(missing)".to_string(), range, span },
            }
        }
        "call_expression" => {
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
                    if let SyntaxTree::Access { receiver, mut segments, .. } = callee {
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
                        return SyntaxTree::Access { receiver, segments, range, span };
                    }
                    SyntaxTree::Call { callee: Box::new(callee), arguments, range, span }
                }
                None => SyntaxTree::Unknown { kind: "call_expression(missing)".to_string(), range, span },
            }
        }
        "type_conversion_expression" => simple_statement_marked(node, "call", vec![Marker::implicit("type")], source),
        "type_instantiation_expression" => simple_statement_marked(node, "type", vec![Marker::implicit("generic")], source),
        "index_expression" => {
            // `arr[i]` — fold into `SyntaxTree::Access { receiver, segments: [Index] }`
            // so single-step bracket access produces the same
            // `<object[access]><index>...</index></object>` shape as a
            // multi-step chain. Mirrors TypeScript / C# / Rust.
            let operand_node = node.child_by_field_name("operand");
            let index_node = node.child_by_field_name("index");
            match (operand_node, index_node) {
                (Some(operand), Some(idx)) => {
                    let object_ir = lower_node(operand, source);
                    let segment = AccessSegment::Index {
                        indices: vec![lower_node(idx, source)],
                        range: ByteRange::new(object_ir.range().end, range.end),
                        span,
                    };
                    match object_ir {
                        SyntaxTree::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::Access { receiver, segments, range, span }
                        }
                        other => SyntaxTree::Access {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &[]),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => simple_statement(node, "index", source),
            }
        }
        "slice_expression" => {
            // `s[i:j]` / `s[i:j:k]` — chain-fold into SyntaxTree::Access only
            // when bounds exist. Full-slice `s[:]` stays un-inverted
            // (renders as `<index[slice]><object>s</object></index>`)
            // matching the imperative pipeline shape.
            let operand_node = node.child_by_field_name("operand");
            let start_node = node.child_by_field_name("start");
            let end_node = node.child_by_field_name("end");
            let capacity_node = node.child_by_field_name("capacity");

            let no_bounds = start_node.is_none() && end_node.is_none() && capacity_node.is_none();
            if no_bounds {
                let mut children: Vec<SyntaxTree> = Vec::new();
                if let Some(o) = operand_node {
                    let inner = lower_node(o, source);
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "object",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(o),
                        span: span_of(o),
                    });
                }
                return SyntaxTree::SimpleStatement {
                    element_name: "index",
                    modifiers: Modifiers::default(),
                    extra_markers: vec![Marker::implicit("slice")],
                    children,
                    range, span,
                };
            }

            let mut slice_children: Vec<SyntaxTree> = Vec::new();
            if let Some(s) = start_node {
                let inner = lower_node(s, source);
                slice_children.push(SyntaxTree::SimpleStatement {
                    element_name: "from",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![inner],
                    range: range_of(s),
                    span: span_of(s),
                });
            }
            if let Some(e) = end_node {
                let inner = lower_node(e, source);
                slice_children.push(SyntaxTree::SimpleStatement {
                    element_name: "to",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![inner],
                    range: range_of(e),
                    span: span_of(e),
                });
            }
            if let Some(c) = capacity_node {
                let inner = lower_node(c, source);
                slice_children.push(SyntaxTree::SimpleStatement {
                    element_name: "capacity",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![inner],
                    range: range_of(c),
                    span: span_of(c),
                });
            }
            // Wrap into a single Inline holding slice marker + slots so the
            // renderer treats them as a single index argument (no <argument>
            // wrap).
            let slice_marker_then_slots = vec![
                SyntaxTree::Inline {
                    children: {
                        let mut v: Vec<SyntaxTree> = Vec::new();
                        // Empty <slice/> marker first.
                        v.push(SyntaxTree::SimpleStatement {
                            element_name: "slice",
                            modifiers: Modifiers::default(),
                            extra_markers: Vec::new(),
                            children: Vec::new(),
                            range: ByteRange::empty_at(range.start),
                            span,
                        });
                        v.extend(slice_children);
                        v
                    },
                    list_name: None,
                    range,
                    span,
                },
            ];
            match operand_node {
                Some(obj) => {
                    let object_ir = lower_node(obj, source);
                    let segment_range = ByteRange::new(object_ir.range().end, range.end);
                    let segment = AccessSegment::Index {
                        indices: slice_marker_then_slots,
                        range: segment_range,
                        span,
                    };
                    match object_ir {
                        SyntaxTree::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::Access { receiver, segments, range, span }
                        }
                        other => SyntaxTree::Access {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &[]),
                            segments: vec![segment],
                            range, span,
                        },
                    }
                }
                None => simple_statement_marked(node, "index", vec![Marker::implicit("slice")], source),
            }
        }
        "type_assertion_expression" => simple_statement(node, "assert", source),
        "composite_literal" => simple_statement(node, "literal", source),
        "literal_value" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "literal_element" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "keyed_element" => {
            // `key: value` inside a composite literal. tree-sitter Go
            // doesn't field-name the children — the first is the key,
            // second is the value. Wrap the value in `<value>` to avoid
            // nested `<pair><pair>` when the value is itself a composite
            // literal.
            let kids: Vec<&RawNode> = node.named_children().collect();
            let mut children: Vec<SyntaxTree> = Vec::new();
            for (i, c) in kids.iter().enumerate() {
                if i == 0 {
                    children.push(lower_node(*c, source));
                } else {
                    let inner = lower_node(*c, source);
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "value",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(*c),
                        span: span_of(*c),
                    });
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "pair",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "argument_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },
        "variadic_argument" => simple_statement(node, "spread", source),
        "expression_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "parenthesized_expression" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- String / escape -----------------------------------------
        "interpreted_string_literal_content" | "raw_string_literal_content"
        | "escape_sequence" => SyntaxTree::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        other => SyntaxTree::Unknown { kind: other.to_string(), range, span },
    }
}

fn simple_statement(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| lower_node(c, source))
        .collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

fn simple_statement_marked(
    node: &RawNode,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| lower_node(c, source))
        .collect();
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
    node.named_children()
        .map(|c| lower_node(c, source))
        .collect()
}

/// Build `<path>/<name>+` from a Go interpreted-string-literal import
/// path. The slash-delimited segments of `"net/http/pprof"` become
/// individual `<name>` segments under a single `<path>` parent, so the
/// cross-language `//import//name='net'` query is uniform across Go,
/// Java, Python, C# (Principle #5 — unified concepts within and
/// across languages where the cost-benefit favours unification).
///
/// The surrounding double quotes flow through as gap text on the
/// enclosing `<import>` (anchored renderer property), not as content
/// of `<path>`.
fn build_go_import_path(node: &RawNode, source: &str) -> SyntaxTree {
    let outer_span = span_of(node);
    let outer_range = range_of(node);
    let raw = node.utf8_text(source);
    // The string literal includes surrounding quotes; the segments
    // live in `raw[1..raw.len()-1]` mapped to the source offsets
    // `outer_range.start+1 .. outer_range.end-1`.
    let trimmed = raw.trim_start_matches('"').trim_end_matches('"');
    let inner_start = outer_range.start.saturating_add(1);
    let inner_end = outer_range.end.saturating_sub(1);
    let inner_range = ByteRange::new(inner_start, inner_end);
    let mut segments: Vec<SyntaxTree> = Vec::new();
    let mut offset = inner_start;
    for seg in trimmed.split('/') {
        let seg_len = seg.len() as u32;
        let seg_start = offset;
        let seg_end = offset.saturating_add(seg_len);
        segments.push(SyntaxTree::Name {
            text: seg.to_string(),
            range: ByteRange::new(seg_start, seg_end),
            span: outer_span,
        });
        offset = seg_end.saturating_add(1); // skip '/'
    }
    SyntaxTree::Path { segments, range: inner_range, span: outer_span }
}

/// Lower one `import_spec` into a canonical `<import>` element.
///
/// Pre-IR Go pinned four import-kind variants as mutually-exclusive
/// markers on `<import>` (Principle #9 — Exhaustive Markers):
///   - `import "fmt"`            → `<import>/<path>/<name>fmt`
///   - `import f "fmt"`          → `<import[alias]>/<path>/<name>fmt/<aliased>/<name>f`
///   - `import . "strings"`      → `<import[dot]>/<path>/<name>strings`
///   - `import _ "x"`            → `<import[blank]>/<path>/<name>x`
fn lower_go_import_spec(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let path_node = node.child_by_field_name("path");
    let name_node = node.child_by_field_name("name");

    let kind: Vec<crate::tree::types::Marker> = match name_node {
        Some(n) => match n.utf8_text(source) {
            "." => vec![Marker::implicit("dot")],
            "_" => vec![Marker::implicit("blank")],
            _ => vec![Marker::implicit("alias")],
        },
        None => Vec::new(),
    };

    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(p) = path_node {
        children.push(build_go_import_path(p, source));
    }
    if kind.iter().any(|m| m.name == "alias") {
        if let Some(n) = name_node {
            let nspan = span_of(n);
            let nrange = range_of(n);
            children.push(SyntaxTree::Aliased {
                inner: Box::new(name_of(n, source)),
                range: nrange,
                span: nspan,
            });
        }
    }

    SyntaxTree::SimpleStatement {
        element_name: "import",
        modifiers: Modifiers::default(),
        extra_markers: kind,
        children,
        range,
        span,
    }
}


/// Go convention: name starting with uppercase → exported; lowercase → unexported.
fn is_exported(name: &str) -> bool {
    name.chars().next().map_or(false, |c| c.is_uppercase())
}

/// Lower a Go type_spec / type_alias with proper `<type>` shape.
/// When the RHS is a struct_type or interface_type, hoist it: emit
/// `<struct>` or `<interface>` directly (with name as a child) instead
/// of `<type><name/></type>...<type><struct/></type>`. This matches
/// the imperative pipeline shape.
fn go_type_spec(
    node: &RawNode,
    element_name: &'static str,
    extra_markers_in: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let name_node = node.child_by_field_name("name");
    let type_node = node.child_by_field_name("type");
    let name_text = name_node.map(|n| text_of(n, source)).unwrap_or_default();
    let _ = extra_markers_in;
    let extra_markers: Vec<crate::tree::types::Marker> = if is_exported(&name_text) {
        vec![Marker::implicit("exported")]
    } else {
        vec![Marker::implicit("unexported")]
    };

    // Hoist struct/interface directly to top-level.
    if let Some(t) = type_node {
        if matches!(t.kind(), "struct_type" | "interface_type") {
            let element = if t.kind() == "struct_type" { "struct" } else { "interface" };
            // Lower the struct/interface body but make the parent
            // element_name be `struct`/`interface` and add the name child.
            let mut inner_children: Vec<SyntaxTree> = Vec::new();
            if let Some(n) = name_node {
                inner_children.push(name_of(n, source));
            }
            let is_interface = t.kind() == "interface_type";
            for c in t.named_children() {
                if is_interface && is_go_embedded_interface(c) {
                    inner_children.push(wrap_go_interface_embed(c, source));
                } else {
                    inner_children.push(lower_node(c, source));
                }
            }
            return SyntaxTree::SimpleStatement {
                element_name: element,
                modifiers: Modifiers::default(),
                extra_markers,
                children: inner_children,
                range: range_of(node),
                span: span_of(node),
            };
        }
    }

    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(n) = name_node {
        children.push(name_of(n, source));
    }
    if let Some(t) = type_node {
        let inner = lower_node(t, source);
        children.push(go_wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
    }
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Whether `node` is an embedded-interface target inside an
/// `interface_type` body (`interface { io.Reader; ... }`). Excludes
/// `method_spec` children (the method signatures); everything else
/// inside an interface body is a type-relationship target.
fn is_go_embedded_interface(node: &RawNode) -> bool {
    matches!(
        node.kind(),
        "qualified_type" | "type_identifier" | "generic_type" | "type_elem"
    )
}

/// Wrap an embedded-interface target in `<extends>/<type>/{lowered}`
/// (Principle #18 — name the relationship after the operator;
/// Principle #14 — every type-reference slot carries a `<type>` child).
fn wrap_go_interface_embed(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let inner = lower_node(node, source);
    // Wrap leaf-like inner trees in `<type>` so the canonical
    // `<extends>/<type>` shape always has the namespace marker.
    let type_slot = match &inner {
        SyntaxTree::SimpleStatement { element_name: "type", .. }
        | SyntaxTree::GenericType { .. } => inner,
        _ => SyntaxTree::SimpleStatement {
            element_name: "type",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![inner],
            range,
            span,
        },
    };
    SyntaxTree::SimpleStatement {
        element_name: "extends",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children: vec![type_slot],
        range,
        span,
    }
}

/// Wrap a leaf-like SyntaxTree in `<type>` so it surfaces as `<type><name>X</name></type>`.
fn go_wrap_in_type_if_leaf(inner: SyntaxTree, range: ByteRange, span: Span) -> SyntaxTree {
    match &inner {
        SyntaxTree::Name { .. } => SyntaxTree::SimpleStatement {
            element_name: "type",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![inner],
            range, span,
        },
        _ => inner,
    }
}

/// Lower a Go inc/dec_statement (`i++` / `i--`) to SyntaxTree::Unary with the
/// appropriate op marker.
fn go_inc_dec(
    node: &RawNode,
    op_marker: &'static str,
    op_text_str: &str,
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let operand = node.named_children().next();
    // Locate the unnamed `++`/`--` token.
    let mut op_byte_range = ByteRange::empty_at(range.end);
    for c in node.children() {
        if !c.is_named() && text_of(c, source) == op_text_str {
            op_byte_range = range_of(c);
            break;
        }
    }
    match operand {
        Some(o) => SyntaxTree::Unary {
            op_text: op_text_str.to_string(),
            op_marker,
            op_range: op_byte_range,
            operand: Box::new(lower_node(o, source)),
            extra_markers: vec![Marker::implicit("postfix")],
            range,
            span,
        },
        None => SyntaxTree::Unknown {
            kind: format!("{} (no operand)", node.kind()),
            range, span,
        },
    }
}

/// Lower a Go const_spec / var_spec: `name = value` / `name type = value`.
/// tree-sitter Go uses `name` (multiple), `type`, `value` fields. The
/// value child is an expression_list — its inner expressions become
/// `<value><expression>...` slot wrappers.
fn go_var_const_spec(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let type_node = node.child_by_field_name("type");
    // Collect name field children.
    let mut name_nodes: Vec<&RawNode> = Vec::new();
    let mut value_nodes: Vec<&RawNode> = Vec::new();
    {
        for (i, ch) in node.children().enumerate() {
            match node.field_name_for_child(i as u32) {
                Some("name") => name_nodes.push(ch),
                Some("value") => value_nodes.push(ch),
                _ => {}
            }
        }
    }
    let first_name_text = name_nodes.first().map(|n| text_of(*n, source)).unwrap_or_default();
    let extra_markers: Vec<crate::tree::types::Marker> = if first_name_text.is_empty() {
        Vec::new()
    } else if is_exported(&first_name_text) {
        vec![Marker::implicit("exported")]
    } else {
        vec![Marker::implicit("unexported")]
    };
    let mut children: Vec<SyntaxTree> = Vec::new();
    for n in &name_nodes {
        children.push(name_of(*n, source));
    }
    if let Some(t) = type_node {
        let inner = lower_node(t, source);
        children.push(go_wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
    }
    // Value field: tree-sitter Go gives `value` as expression_list. Each
    // inner expression wraps in `<value><expression>...</expression></value>`.
    for v in &value_nodes {
        let mut emitted_any = false;
        if v.kind() == "expression_list" {
            for e in v.named_children() {
                let inner = lower_node(e, source);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "value",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(e),
                        span: span_of(e),
                    }],
                    range: range_of(e),
                    span: span_of(e),
                });
                emitted_any = true;
            }
        }
        if !emitted_any {
            let inner = lower_node(*v, source);
            children.push(SyntaxTree::SimpleStatement {
                element_name: "value",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![SyntaxTree::SimpleStatement {
                    element_name: "expression",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![inner],
                    range: range_of(*v),
                    span: span_of(*v),
                }],
                range: range_of(*v),
                span: span_of(*v),
            });
        }
    }
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range, span,
    }
}

/// Lower a Go field_declaration: `Name1, Name2 Type` or `Name1 Type`.
/// tree-sitter Go uses field name "name" (multiple) and "type".
/// Emit `<field>` with name child(ren) and `<type>` wrapper.
fn go_field_declaration(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let type_node = node.child_by_field_name("type");
    // Collect all "name" field children to determine export.
    let mut name_nodes: Vec<&RawNode> = Vec::new();
    {
        for (i, ch) in node.children().enumerate() {
            if node.field_name_for_child(i as u32) == Some("name") {
                name_nodes.push(ch);
            }
        }
    }
    let first_name_text = name_nodes.first().map(|n| text_of(*n, source)).unwrap_or_default();
    let extra_markers: Vec<crate::tree::types::Marker> = if first_name_text.is_empty() {
        Vec::new()
    } else if is_exported(&first_name_text) {
        vec![Marker::implicit("exported")]
    } else {
        vec![Marker::implicit("unexported")]
    };
    let mut children: Vec<SyntaxTree> = Vec::new();
    for n in &name_nodes {
        children.push(name_of(*n, source));
    }
    if let Some(t) = type_node {
        let inner = lower_node(t, source);
        children.push(go_wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
    }
    // Process other named children (e.g. tag).
    for c in node.named_children() {
        if let Some(t) = type_node { if c.id() == t.id() { continue; } }
        if name_nodes.iter().any(|n| n.id() == c.id()) { continue; }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name: "field",
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range,
        span,
    }
}

/// Lower a Go switch (expression_switch / type_switch). Wraps
/// `value` field in `<value><expression>...` host. Type switch
/// adds `[type]` marker.
fn go_switch(node: &RawNode, type_switch: bool, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let value_node = node.child_by_field_name("value");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(v) = value_node {
        let inner = lower_node(v, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![SyntaxTree::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner],
                range: range_of(v),
                span: span_of(v),
            }],
            range: range_of(v),
            span: span_of(v),
        });
    }
    for c in node.named_children() {
        if let Some(v) = value_node {
            if c.id() == v.id() {
                continue;
            }
        }
        // Skip the `type` token (unnamed in tree-sitter for type-switch).
        if c.kind() == "type" {
            continue;
        }
        children.push(lower_node(c, source));
    }
    let extra_markers: Vec<crate::tree::types::Marker> = if type_switch { vec![Marker::implicit("type")] } else { Vec::new() };
    SyntaxTree::SimpleStatement {
        element_name: "switch",
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range, span,
    }
}

/// Lower an expression_case / type_case / communication_case.
/// Type-case: each type field becomes `<type>` (wrapped if leaf).
/// Expression-case: each value field wraps in `<value><expression>`.
fn go_case(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut children: Vec<SyntaxTree> = Vec::new();
    if node.kind() == "type_case" {
        // Collect all `type` field children.
        for c in node.children() {
            // Check field name.
            let mut field = None;
            for (i, ch) in node.children().enumerate() {
                if ch.id() == c.id() {
                    field = node.field_name_for_child(i as u32);
                    break;
                }
            }
            if field == Some("type") && c.is_named() {
                let inner = lower_node(c, source);
                children.push(go_wrap_in_type_if_leaf(inner, range_of(c), span_of(c)));
            } else if c.is_named() && field != Some("type") {
                // Other named children (statements after the case label).
                children.push(lower_node(c, source));
            }
        }
    } else {
        // expression_case / communication_case — value field is the
        // case discriminator(s).
        for c in node.named_children() {
            // Determine field name.
            let mut field = None;
            for (i, ch) in node.children().enumerate() {
                if ch.id() == c.id() {
                    field = node.field_name_for_child(i as u32);
                    break;
                }
            }
            if field == Some("value") {
                let inner = lower_node(c, source);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "value",
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
            } else {
                children.push(lower_node(c, source));
            }
        }
    }
    SyntaxTree::SimpleStatement {
        element_name: "case",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower a Go for_statement covering all 4 forms (C-style, while,
/// infinite, range). The body block field renames to `<body>`. A bare
/// condition expression (while-form) wraps in `<condition><expression>`.
/// for_clause children are inlined with init slot bare, condition
/// wrapped in `<condition><expression>`, update bare.
fn go_for_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if let Some(b) = body_node {
            if c.id() == b.id() {
                let body_children: Vec<SyntaxTree> = c
                    .named_children()
                    .map(|s| lower_node(s, source))
                    .collect();
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: body_children,
                    range: range_of(c),
                    span: span_of(c),
                });
                continue;
            }
        }
        if c.kind() == "for_clause" {
            // Inline: each of init/cond/update lifted with proper wrap.
            let init_node = c.child_by_field_name("initializer");
            let cond_node = c.child_by_field_name("condition");
            let upd_node = c.child_by_field_name("update");
            if let Some(i) = init_node {
                children.push(lower_node(i, source));
            }
            if let Some(cn) = cond_node {
                let inner = lower_node(cn, source);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "condition",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(cn),
                        span: span_of(cn),
                    }],
                    range: range_of(cn),
                    span: span_of(cn),
                });
            }
            if let Some(u) = upd_node {
                children.push(lower_node(u, source));
            }
            continue;
        }
        // Wrap a bare expression (while-cond form) in
        // `<condition><expression>...`.
        if matches!(
            c.kind(),
            "binary_expression" | "unary_expression" | "call_expression"
                | "selector_expression" | "identifier" | "true" | "false"
        ) {
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
        range,
        span,
    }
}

/// Lower a Go if_statement to canonical `<if>` shape with
/// condition/then/else slots. Else-if chains collapse to flat
/// `<else_if>`/`<else>` siblings.
fn go_if_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let init_node = node.child_by_field_name("initializer");
    let cond_node = node.child_by_field_name("condition");
    let consequence_node = node.child_by_field_name("consequence");
    let alternative_node = node.child_by_field_name("alternative");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(i) = init_node {
        children.push(lower_node(i, source));
    }
    if let Some(c) = cond_node {
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
    }
    if let Some(c) = consequence_node {
        let body_children: Vec<SyntaxTree> = c
            .named_children()
            .map(|s| lower_node(s, source))
            .collect();
        children.push(SyntaxTree::SimpleStatement {
            element_name: "then",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![SyntaxTree::SimpleStatement {
                element_name: "body",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: body_children,
                range: range_of(c),
                span: span_of(c),
            }],
            range: range_of(c),
            span: span_of(c),
        });
    }
    let mut cur_alt = alternative_node;
    while let Some(a) = cur_alt {
        if a.kind() == "if_statement" {
            // else-if chain — flatten as <else_if>.
            let inner_cond = a.child_by_field_name("condition");
            let inner_cons = a.child_by_field_name("consequence");
            let inner_alt = a.child_by_field_name("alternative");
            let mut else_if_children: Vec<SyntaxTree> = Vec::new();
            if let Some(c) = inner_cond {
                let inner = lower_node(c, source);
                else_if_children.push(SyntaxTree::SimpleStatement {
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
            }
            if let Some(c) = inner_cons {
                let body_children: Vec<SyntaxTree> = c
                    .named_children()
                    .map(|s| lower_node(s, source))
                    .collect();
                else_if_children.push(SyntaxTree::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: body_children,
                    range: range_of(c),
                    span: span_of(c),
                });
            }
            children.push(SyntaxTree::SimpleStatement {
                element_name: "else_if",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: else_if_children,
                range: range_of(a),
                span: span_of(a),
            });
            cur_alt = inner_alt;
        } else {
            // Plain else block.
            let body_children: Vec<SyntaxTree> = a
                .named_children()
                .map(|s| lower_node(s, source))
                .collect();
            children.push(SyntaxTree::SimpleStatement {
                element_name: "else",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![SyntaxTree::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: body_children,
                    range: range_of(a),
                    span: span_of(a),
                }],
                range: range_of(a),
                span: span_of(a),
            });
            cur_alt = None;
        }
    }
    SyntaxTree::SimpleStatement {
        element_name: "if",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower a Go function/method declaration with `<exported/>`/`<unexported/>`
/// marker. The body block field renames to `<body>` with inner
/// statements lowered directly.
fn go_decl_with_export(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let name_node = node.child_by_field_name("name");
    let body_node = node.child_by_field_name("body");
    let name_text = name_node.map(|n| text_of(n, source)).unwrap_or_default();
    let extra_markers: Vec<crate::tree::types::Marker> = if is_exported(&name_text) {
        vec![Marker::implicit("exported")]
    } else {
        vec![Marker::implicit("unexported")]
    };
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if let Some(b) = body_node {
            if c.id() == b.id() {
                let body_children: Vec<SyntaxTree> = c
                    .named_children()
                    .map(|s| lower_node(s, source))
                    .collect();
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: merge_go_line_comments(body_children, source),
                    range: range_of(c),
                    span: span_of(c),
                });
                continue;
            }
        }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

fn op_marker(op: &str) -> Option<&'static str> {
    Some(match op {
        "+" => "plus",
        "-" => "minus",
        "*" => "multiply",
        "/" => "divide",
        "%" => "modulo",
        "==" => "equal",
        "!=" => "not_equal",
        "<" => "less",
        "<=" => "less_or_equal",
        ">" => "greater",
        ">=" => "greater_or_equal",
        "&&" => "and",
        "||" => "or",
        "!" => "not",
        "&" => "bitwise_and",
        "|" => "bitwise_or",
        "^" => "bitwise_xor",
        "<<" => "shift_left",
        ">>" => "shift_right",
        "&^" => "bitwise_clear",
        "<-" => "channel_receive",
        _ => return None,
    })
}


