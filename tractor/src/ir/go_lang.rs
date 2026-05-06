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
        "const_declaration" => simple_statement(node, "const", source),
        "const_spec" => go_decl_with_export_first_name(node, "const", source),
        "var_declaration" => simple_statement(node, "var", source),
        "var_spec" => go_decl_with_export_first_name(node, "var", source),
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
        "method_elem" => simple_statement(node, "method", source),
        "field_declaration" => go_decl_with_export_first_name(node, "field", source),
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
        "unary_expression" => simple_statement(node, "unary", source),
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

fn text_of(node: TsNode<'_>, source: &str) -> String {
    source[node.start_byte()..node.end_byte()].to_string()
}

/// Go convention: name starting with uppercase → exported; lowercase → unexported.
fn is_exported(name: &str) -> bool {
    name.chars().next().map_or(false, |c| c.is_uppercase())
}

/// Lower a Go type_spec / type_alias with proper `<type>` shape.
/// The RHS gets wrapped in `<type>` if it's a leaf identifier so the
/// shape is `<type[name='MyInt']><type[name='int']/></type>` (matching
/// the imperative pipeline).
fn go_type_spec(
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers_in: &'static [&'static str],
    source: &str,
) -> Ir {
    let name_node = node.child_by_field_name("name");
    let type_node = node.child_by_field_name("type");
    let name_text = name_node.map(|n| text_of(n, source)).unwrap_or_default();
    let mut children: Vec<Ir> = Vec::new();
    if let Some(n) = name_node {
        children.push(Ir::Name { range: range_of(n), span: span_of(n) });
    }
    if let Some(t) = type_node {
        let inner = lower_node(t, source);
        children.push(go_wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
    }
    let _ = extra_markers_in;
    let extra_markers: &'static [&'static str] = if is_exported(&name_text) {
        &["exported"]
    } else {
        &["unexported"]
    };
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
                    children: body_children,
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
