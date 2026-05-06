//! Go tree-sitter CST → IR lowering.
//!
//! Mirrors the Rust IR pipeline pattern. Each per-kind arm
//! recursively lowers children; unhandled kinds fall through to
//! `Ir::Unknown`. The renderer in `crate::ir::to_xot` is shared.
//!
//! Production parser routes Go through this lowering end-to-end
//! (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/go/{rules,transformations,transform}.rs` modules
//! were retired alongside this migration.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{AccessSegment, ByteRange, Ir, Modifiers, Span};

pub fn lower_go_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "source_file" => Ir::Module {
            element_name: "file",
            children: merge_go_line_comments(lower_children(root, source), source),
            range,
            span,
        },
        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

/// Classify Go comments into leading/trailing/floating. Mirrors
/// merge_rust_line_comments — Go uses `//` and `/*...*/`, and like
/// rust, tree-sitter Go includes the trailing \n in the comment range.
fn merge_go_line_comments(children: Vec<Ir>, source: &str) -> Vec<Ir> {
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
                let no_newline = !gap.contains('\n') && gap.chars().all(|c| c.is_whitespace());
                let prev_text = &source[prev_range.start as usize..prev_range.end as usize];
                let curr_text = &source[range.start as usize..range.end as usize];
                let prev_is_line_comment = prev_text.trim_start().starts_with("//");
                let curr_is_line_comment = curr_text.trim_start().starts_with("//");
                let prev_was_trailing = matches!(out.last(), Some(Ir::Comment { trailing: true, .. }));
                if no_newline
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
        "interpreted_string_literal" => Ir::String { range, span },
        "raw_string_literal" => simple_statement_marked(node, "string", &["raw"], source),
        "rune_literal" => simple_statement(node, "char", source),
        "true" => Ir::True { range, span },
        "false" => Ir::False { range, span },
        "nil" => Ir::SimpleStatement {
            element_name: "nil",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range, span,
        },
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
        // `type Foo bar` / `type Foo = bar` / type-decl block.
        // type_declaration is a wrapper that holds one or more type_spec
        // / type_alias children. Inline so each spec/alias becomes a
        // direct sibling of the parent file/body.
        "type_declaration" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        // `Foo bar` (defined type). Emit `<type>` with `<exported/>` /
        // `<unexported/>` marker, name, and type wrapped in `<type>`.
        "type_spec" => go_type_spec(node, "type", &[], source),
        "type_alias" => go_type_spec(node, "alias", &[], source),
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
        // const_declaration / var_declaration are wrappers around one
        // or more specs — flatten so each spec becomes a direct
        // sibling of the parent file/body.
        "const_declaration" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "var_declaration" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "const_spec" => go_var_const_spec(node, "const", source),
        "var_spec" => go_var_const_spec(node, "var", source),
        "var_spec_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "short_var_declaration" => {
            // `i, j := 0, 1` — emit `<variable[short]>` with `<left>` /
            // `<right>` slot wrappers.
            let left_node = node.child_by_field_name("left");
            let right_node = node.child_by_field_name("right");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(l) = left_node {
                let inner = lower_node(l, source);
                children.push(Ir::SimpleStatement {
                    element_name: "left",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
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
                children.push(Ir::SimpleStatement {
                    element_name: "right",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(r),
                        span: span_of(r),
                    }],
                    range: range_of(r),
                    span: span_of(r),
                });
            }
            Ir::SimpleStatement {
                element_name: "variable",
                modifiers: Modifiers::default(),
                extra_markers: &["short"],
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
            let mut children: Vec<Ir> = Vec::new();
            if let Some(n) = name_node {
                children.push(Ir::Name { range: range_of(n), span: span_of(n) });
            }
            if let Some(p) = parameters_node {
                children.push(lower_node(p, source));
            }
            if let Some(r) = result_node {
                let inner = lower_node(r, source);
                children.push(Ir::Returns {
                    type_ann: Box::new(go_wrap_in_type_if_leaf(inner, range_of(r), span_of(r))),
                    range: range_of(r),
                    span: span_of(r),
                });
            }
            Ir::SimpleStatement {
                element_name: "method",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "field_declaration" => go_field_declaration(node, source),
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
        "if_statement" => go_if_statement(node, source),
        "for_statement" => go_for_statement(node, source),
        "for_clause" => Ir::Inline {
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
            let mut children: Vec<Ir> = Vec::new();
            if let Some(l) = left_node {
                // The left can be an expression_list with multiple items.
                let mut left_children: Vec<Ir> = Vec::new();
                if l.kind() == "expression_list" {
                    let mut lc = l.walk();
                    for e in l.named_children(&mut lc) {
                        let inner = lower_node(e, source);
                        left_children.push(Ir::SimpleStatement {
                            element_name: "expression",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![inner],
                            range: range_of(e),
                            span: span_of(e),
                        });
                    }
                } else {
                    let inner = lower_node(l, source);
                    left_children.push(Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(l),
                        span: span_of(l),
                    });
                }
                children.push(Ir::SimpleStatement {
                    element_name: "left",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: left_children,
                    range: range_of(l),
                    span: span_of(l),
                });
            }
            if let Some(r) = right_node {
                let inner = lower_node(r, source);
                children.push(Ir::SimpleStatement {
                    element_name: "right",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(r),
                        span: span_of(r),
                    }],
                    range: range_of(r),
                    span: span_of(r),
                });
            }
            Ir::SimpleStatement {
                element_name: "range",
                modifiers: Modifiers::default(),
                extra_markers: &[],
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
        "binary_expression" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (left, right, op_marker(&op_text)) {
                (Some(l), Some(r), Some(marker)) => Ir::Binary {
                    element_name: if matches!(op_text.as_str(), "&&" | "||") { "logical" } else { "binary" },
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l),
                    right: Box::new(r),
                    range, span,
                },
                _ => Ir::Unknown {
                    kind: "binary_expression(missing)".to_string(),
                    range, span,
                },
            }
        }
        "unary_expression" => {
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let mut cursor = node.walk();
            let operand = node.named_children(&mut cursor).next();
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
                (Some(l), Some(r)) => Ir::Assign {
                    targets: vec![lower_node(l, source)],
                    type_annotation: None,
                    op_text,
                    op_range,
                    op_markers: vec![op_marker_text],
                    values: vec![lower_node(r, source)],
                    range, span,
                },
                _ => Ir::Unknown {
                    kind: "assignment_statement(missing)".to_string(),
                    range, span,
                },
            }
        }
        "inc_statement" => go_inc_dec(node, "increment", "++", source),
        "dec_statement" => go_inc_dec(node, "decrement", "--", source),
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
        "slice_expression" => {
            // `s[i:j]` / `s[i:j:k]` — chain-fold into Ir::Access only
            // when bounds exist. Full-slice `s[:]` stays un-inverted
            // (renders as `<index[slice]><object>s</object></index>`)
            // matching the imperative pipeline shape.
            let operand_node = node.child_by_field_name("operand");
            let start_node = node.child_by_field_name("start");
            let end_node = node.child_by_field_name("end");
            let capacity_node = node.child_by_field_name("capacity");

            let no_bounds = start_node.is_none() && end_node.is_none() && capacity_node.is_none();
            if no_bounds {
                let mut children: Vec<Ir> = Vec::new();
                if let Some(o) = operand_node {
                    let inner = lower_node(o, source);
                    children.push(Ir::SimpleStatement {
                        element_name: "object",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(o),
                        span: span_of(o),
                    });
                }
                return Ir::SimpleStatement {
                    element_name: "index",
                    modifiers: Modifiers::default(),
                    extra_markers: &["slice"],
                    children,
                    range, span,
                };
            }

            let mut slice_children: Vec<Ir> = Vec::new();
            if let Some(s) = start_node {
                let inner = lower_node(s, source);
                slice_children.push(Ir::SimpleStatement {
                    element_name: "from",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![inner],
                    range: range_of(s),
                    span: span_of(s),
                });
            }
            if let Some(e) = end_node {
                let inner = lower_node(e, source);
                slice_children.push(Ir::SimpleStatement {
                    element_name: "to",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![inner],
                    range: range_of(e),
                    span: span_of(e),
                });
            }
            if let Some(c) = capacity_node {
                let inner = lower_node(c, source);
                slice_children.push(Ir::SimpleStatement {
                    element_name: "capacity",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![inner],
                    range: range_of(c),
                    span: span_of(c),
                });
            }
            // Wrap into a single Inline holding slice marker + slots so the
            // renderer treats them as a single index argument (no <argument>
            // wrap).
            let slice_marker_then_slots = vec![
                Ir::Inline {
                    children: {
                        let mut v: Vec<Ir> = Vec::new();
                        // Empty <slice/> marker first.
                        v.push(Ir::SimpleStatement {
                            element_name: "slice",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
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
                None => simple_statement_marked(node, "index", &["slice"], source),
            }
        }
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
        "keyed_element" => {
            // `key: value` inside a composite literal. tree-sitter Go
            // doesn't field-name the children — the first is the key,
            // second is the value. Wrap the value in `<value>` to avoid
            // nested `<pair><pair>` when the value is itself a composite
            // literal.
            let mut cursor = node.walk();
            let kids: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
            let mut children: Vec<Ir> = Vec::new();
            for (i, c) in kids.iter().enumerate() {
                if i == 0 {
                    children.push(lower_node(*c, source));
                } else {
                    let inner = lower_node(*c, source);
                    children.push(Ir::SimpleStatement {
                        element_name: "value",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(*c),
                        span: span_of(*c),
                    });
                }
            }
            Ir::SimpleStatement {
                element_name: "pair",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
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

fn text_of(node: TsNode<'_>, source: &str) -> String {
    source[node.start_byte()..node.end_byte()].to_string()
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
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers_in: &'static [&'static str],
    source: &str,
) -> Ir {
    let name_node = node.child_by_field_name("name");
    let type_node = node.child_by_field_name("type");
    let name_text = name_node.map(|n| text_of(n, source)).unwrap_or_default();
    let _ = extra_markers_in;
    let extra_markers: &'static [&'static str] = if is_exported(&name_text) {
        &["exported"]
    } else {
        &["unexported"]
    };

    // Hoist struct/interface directly to top-level.
    if let Some(t) = type_node {
        if matches!(t.kind(), "struct_type" | "interface_type") {
            let element = if t.kind() == "struct_type" { "struct" } else { "interface" };
            // Lower the struct/interface body but make the parent
            // element_name be `struct`/`interface` and add the name child.
            let mut inner_children: Vec<Ir> = Vec::new();
            if let Some(n) = name_node {
                inner_children.push(Ir::Name { range: range_of(n), span: span_of(n) });
            }
            // Lower the struct/interface contents — for struct_type
            // that's the field_declaration_list child.
            let mut tcursor = t.walk();
            for c in t.named_children(&mut tcursor) {
                inner_children.push(lower_node(c, source));
            }
            return Ir::SimpleStatement {
                element_name: element,
                modifiers: Modifiers::default(),
                extra_markers,
                children: inner_children,
                range: range_of(node),
                span: span_of(node),
            };
        }
    }

    let mut children: Vec<Ir> = Vec::new();
    if let Some(n) = name_node {
        children.push(Ir::Name { range: range_of(n), span: span_of(n) });
    }
    if let Some(t) = type_node {
        let inner = lower_node(t, source);
        children.push(go_wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
    }
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Wrap a leaf-like Ir in `<type>` so it surfaces as `<type><name>X</name></type>`.
fn go_wrap_in_type_if_leaf(inner: Ir, range: ByteRange, span: Span) -> Ir {
    match &inner {
        Ir::Name { .. } => Ir::SimpleStatement {
            element_name: "type",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![inner],
            range, span,
        },
        _ => inner,
    }
}

/// Lower a Go inc/dec_statement (`i++` / `i--`) to Ir::Unary with the
/// appropriate op marker.
fn go_inc_dec(
    node: TsNode<'_>,
    op_marker: &'static str,
    op_text_str: &str,
    source: &str,
) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let operand = node.named_children(&mut cursor).next();
    // Locate the unnamed `++`/`--` token.
    let mut cursor2 = node.walk();
    let mut op_byte_range = ByteRange::empty_at(range.end);
    for c in node.children(&mut cursor2) {
        if !c.is_named() && text_of(c, source) == op_text_str {
            op_byte_range = range_of(c);
            break;
        }
    }
    match operand {
        Some(o) => Ir::Unary {
            op_text: op_text_str.to_string(),
            op_marker,
            op_range: op_byte_range,
            operand: Box::new(lower_node(o, source)),
            extra_markers: &["postfix"],
            range,
            span,
        },
        None => Ir::Unknown {
            kind: format!("{} (no operand)", node.kind()),
            range, span,
        },
    }
}

/// Lower a Go const_spec / var_spec: `name = value` / `name type = value`.
/// tree-sitter Go uses `name` (multiple), `type`, `value` fields. The
/// value child is an expression_list — its inner expressions become
/// `<value><expression>...` slot wrappers.
fn go_var_const_spec(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let type_node = node.child_by_field_name("type");
    // Collect name field children.
    let mut name_nodes: Vec<TsNode<'_>> = Vec::new();
    let mut value_nodes: Vec<TsNode<'_>> = Vec::new();
    {
        let mut p = node.walk();
        for (i, ch) in node.children(&mut p).enumerate() {
            match node.field_name_for_child(i as u32) {
                Some("name") => name_nodes.push(ch),
                Some("value") => value_nodes.push(ch),
                _ => {}
            }
        }
    }
    let first_name_text = name_nodes.first().map(|n| text_of(*n, source)).unwrap_or_default();
    let extra_markers: &'static [&'static str] = if first_name_text.is_empty() {
        &[]
    } else if is_exported(&first_name_text) {
        &["exported"]
    } else {
        &["unexported"]
    };
    let mut children: Vec<Ir> = Vec::new();
    for n in &name_nodes {
        children.push(Ir::Name { range: range_of(*n), span: span_of(*n) });
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
            let mut vc = v.walk();
            for e in v.named_children(&mut vc) {
                let inner = lower_node(e, source);
                children.push(Ir::SimpleStatement {
                    element_name: "value",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
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
            children.push(Ir::SimpleStatement {
                element_name: "value",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![Ir::SimpleStatement {
                    element_name: "expression",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![inner],
                    range: range_of(*v),
                    span: span_of(*v),
                }],
                range: range_of(*v),
                span: span_of(*v),
            });
        }
    }
    Ir::SimpleStatement {
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
fn go_field_declaration(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let type_node = node.child_by_field_name("type");
    let mut cursor = node.walk();
    // Collect all "name" field children to determine export.
    let mut name_nodes: Vec<TsNode<'_>> = Vec::new();
    {
        let mut p = node.walk();
        for (i, ch) in node.children(&mut p).enumerate() {
            if node.field_name_for_child(i as u32) == Some("name") {
                name_nodes.push(ch);
            }
        }
    }
    let first_name_text = name_nodes.first().map(|n| text_of(*n, source)).unwrap_or_default();
    let extra_markers: &'static [&'static str] = if first_name_text.is_empty() {
        &[]
    } else if is_exported(&first_name_text) {
        &["exported"]
    } else {
        &["unexported"]
    };
    let mut children: Vec<Ir> = Vec::new();
    for n in &name_nodes {
        children.push(Ir::Name { range: range_of(*n), span: span_of(*n) });
    }
    if let Some(t) = type_node {
        let inner = lower_node(t, source);
        children.push(go_wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
    }
    // Process other named children (e.g. tag).
    for c in node.named_children(&mut cursor) {
        if let Some(t) = type_node { if c.id() == t.id() { continue; } }
        if name_nodes.iter().any(|n| n.id() == c.id()) { continue; }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
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
fn go_switch(node: TsNode<'_>, type_switch: bool, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let value_node = node.child_by_field_name("value");
    let mut cursor = node.walk();
    let mut children: Vec<Ir> = Vec::new();
    if let Some(v) = value_node {
        let inner = lower_node(v, source);
        children.push(Ir::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![Ir::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![inner],
                range: range_of(v),
                span: span_of(v),
            }],
            range: range_of(v),
            span: span_of(v),
        });
    }
    for c in node.named_children(&mut cursor) {
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
    let extra_markers: &'static [&'static str] = if type_switch { &["type"] } else { &[] };
    Ir::SimpleStatement {
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
fn go_case(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut children: Vec<Ir> = Vec::new();
    if node.kind() == "type_case" {
        // Collect all `type` field children.
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            // Check field name.
            let mut p = node.walk();
            let mut field = None;
            for (i, ch) in node.children(&mut p).enumerate() {
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
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            // Determine field name.
            let mut p = node.walk();
            let mut field = None;
            for (i, ch) in node.children(&mut p).enumerate() {
                if ch.id() == c.id() {
                    field = node.field_name_for_child(i as u32);
                    break;
                }
            }
            if field == Some("value") {
                let inner = lower_node(c, source);
                children.push(Ir::SimpleStatement {
                    element_name: "value",
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
            } else {
                children.push(lower_node(c, source));
            }
        }
    }
    Ir::SimpleStatement {
        element_name: "case",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower a Go for_statement covering all 4 forms (C-style, while,
/// infinite, range). The body block field renames to `<body>`. A bare
/// condition expression (while-form) wraps in `<condition><expression>`.
/// for_clause children are inlined with init slot bare, condition
/// wrapped in `<condition><expression>`, update bare.
fn go_for_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut cursor = node.walk();
    let mut children: Vec<Ir> = Vec::new();
    for c in node.named_children(&mut cursor) {
        if let Some(b) = body_node {
            if c.id() == b.id() {
                let mut bc = c.walk();
                let body_children: Vec<Ir> = c
                    .named_children(&mut bc)
                    .map(|s| lower_node(s, source))
                    .collect();
                children.push(Ir::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
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
                children.push(Ir::SimpleStatement {
                    element_name: "condition",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
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
        range,
        span,
    }
}

/// Lower a Go if_statement to canonical `<if>` shape with
/// condition/then/else slots. Else-if chains collapse to flat
/// `<else_if>`/`<else>` siblings.
fn go_if_statement(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let init_node = node.child_by_field_name("initializer");
    let cond_node = node.child_by_field_name("condition");
    let consequence_node = node.child_by_field_name("consequence");
    let alternative_node = node.child_by_field_name("alternative");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(i) = init_node {
        children.push(lower_node(i, source));
    }
    if let Some(c) = cond_node {
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
    }
    if let Some(c) = consequence_node {
        let mut bc = c.walk();
        let body_children: Vec<Ir> = c
            .named_children(&mut bc)
            .map(|s| lower_node(s, source))
            .collect();
        children.push(Ir::SimpleStatement {
            element_name: "then",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![Ir::SimpleStatement {
                element_name: "body",
                modifiers: Modifiers::default(),
                extra_markers: &[],
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
            let mut else_if_children: Vec<Ir> = Vec::new();
            if let Some(c) = inner_cond {
                let inner = lower_node(c, source);
                else_if_children.push(Ir::SimpleStatement {
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
            }
            if let Some(c) = inner_cons {
                let mut bc = c.walk();
                let body_children: Vec<Ir> = c
                    .named_children(&mut bc)
                    .map(|s| lower_node(s, source))
                    .collect();
                else_if_children.push(Ir::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: body_children,
                    range: range_of(c),
                    span: span_of(c),
                });
            }
            children.push(Ir::SimpleStatement {
                element_name: "else_if",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: else_if_children,
                range: range_of(a),
                span: span_of(a),
            });
            cur_alt = inner_alt;
        } else {
            // Plain else block.
            let mut bc = a.walk();
            let body_children: Vec<Ir> = a
                .named_children(&mut bc)
                .map(|s| lower_node(s, source))
                .collect();
            children.push(Ir::SimpleStatement {
                element_name: "else",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![Ir::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
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
    Ir::SimpleStatement {
        element_name: "if",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower a declaration whose name child uses field name "name", and
/// add `<exported/>`/`<unexported/>` marker by case of first
/// character. `_` (blank identifier) emits no marker.
fn go_decl_with_export_first_name(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    // Find the first identifier-shaped named child (skips wrappers).
    let mut cursor = node.walk();
    let name_text = node
        .named_children(&mut cursor)
        .find(|c| matches!(
            c.kind(),
            "identifier" | "field_identifier" | "type_identifier" | "package_identifier"
        ))
        .map(|c| text_of(c, source))
        .unwrap_or_default();
    let extra_markers: &'static [&'static str] = if name_text == "_" || name_text.is_empty() {
        &[]
    } else if is_exported(&name_text) {
        &["exported"]
    } else {
        &["unexported"]
    };
    let mut cursor2 = node.walk();
    let children: Vec<Ir> = node
        .named_children(&mut cursor2)
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

/// Lower a Go function/method declaration with `<exported/>`/`<unexported/>`
/// marker. The body block field renames to `<body>` with inner
/// statements lowered directly.
fn go_decl_with_export(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let name_node = node.child_by_field_name("name");
    let body_node = node.child_by_field_name("body");
    let name_text = name_node.map(|n| text_of(n, source)).unwrap_or_default();
    let extra_markers: &'static [&'static str] = if is_exported(&name_text) {
        &["exported"]
    } else {
        &["unexported"]
    };
    let mut cursor = node.walk();
    let mut children: Vec<Ir> = Vec::new();
    for c in node.named_children(&mut cursor) {
        if let Some(b) = body_node {
            if c.id() == b.id() {
                let mut bc = c.walk();
                let body_children: Vec<Ir> = c
                    .named_children(&mut bc)
                    .map(|s| lower_node(s, source))
                    .collect();
                children.push(Ir::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: merge_go_line_comments(body_children, source),
                    range: range_of(c),
                    span: span_of(c),
                });
                continue;
            }
        }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
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
