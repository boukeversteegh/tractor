//! Python tree-sitter CST → IR lowering.
//!
//! Pure function. No global state, no in-place mutation. Each
//! tree-sitter kind maps to exactly one IR variant (or [`Ir::Unknown`]
//! if not yet covered, or [`Ir::Inline`] if deliberately
//! shape-neutral).
//!
//! ## Initial coverage
//! Module + a single literal/identifier per expression statement, plus
//! member access, subscript, bare calls, binary `+ - * /`, unary
//! `+ -`. Just enough to validate that byte-range threading and
//! gap-text rendering work end-to-end. Expansion to chained calls,
//! comprehensions, and statements happens incrementally as parity
//! tests grow.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{AccessSegment, ByteRange, Ir, Modifiers, ParamKind, Span};

/// Lower a Python tree-sitter root node to [`Ir`].
///
/// The root is expected to be `module`. Anything else is returned as
/// [`Ir::Unknown`] so the parity test can spot the divergence
/// immediately.
pub fn lower_python_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "module" => Ir::Module {
            element_name: "module",
            children: lower_children(root, source),
            range,
            span,
        },
        other => Ir::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // Statement-level wrapper; bare value-producing statement.
        // Per Principle #15, wrap in `<expression>` host.
        // Tree-sitter Python's expression_statement holds exactly one
        // child for bare-expression usage; we punt on tuple/multi cases
        // to Unknown for now.
        "expression_statement" => {
            let mut named: Vec<TsNode> = Vec::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                named.push(child);
            }
            match named.as_slice() {
                [single] => {
                    // Skip the `<expression>` wrap for kinds that the
                    // existing pipeline treats as direct statements:
                    // assignments, yield, raise. Their CST is an
                    // `expression_statement` only because tree-sitter
                    // groups them syntactically; semantically they're
                    // statement-level and should not be wrapped.
                    let inner_kind = single.kind();
                    let bypass = matches!(
                        inner_kind,
                        "assignment" | "augmented_assignment" | "yield" | "raise_statement",
                    );
                    if bypass {
                        lower_node(*single, source)
                    } else {
                        Ir::Expression {
                            inner: Box::new(lower_node(*single, source)),
                            marker: None,
                            range,
                            span,
                        }
                    }
                }
                _ => Ir::Unknown {
                    kind: "expression_statement(multi)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Member access `obj.attr`. tree-sitter Python: `attribute`
        // with fields `object` and `attribute`. Lowering accumulates
        // segments left-to-right by inspecting whether the lowered
        // object is already an `Access` (chain extension) or a fresh
        // atom (chain root). Chain inversion is *implicit* in lowering;
        // no separate cross-cutting pass is required.
        //
        // Segment range for `a.b`: covers the dot + property (e.g.
        // `.b`), so the renderer can derive the dot as gap text and
        // the name as a leaf inside `<member>`. For chained `a.b.c`,
        // each segment's range covers only its OWN portion (`.b`,
        // `.c`); the renderer handles the right-nesting.
        "attribute" => {
            let object_node = node.child_by_field_name("object");
            let attr_node = node.child_by_field_name("attribute");
            match (object_node, attr_node) {
                (Some(object), Some(attr)) => {
                    let object_ir = lower_node(object, source);
                    let property_range = range_of(attr);
                    let property_span = span_of(attr);
                    // Segment covers from end-of-object to end-of-attribute,
                    // i.e. the `.b` portion. This gives the renderer
                    // [object_end .. property_start) as the gap (`.`)
                    // and property_range as the name leaf.
                    let segment_range = ByteRange::new(
                        object_ir.range().end,
                        property_range.end,
                    );
                    let segment = AccessSegment::Member {
                        property_range,
                        property_span,
                        optional: false,  // Python has no `?.`
                        range: segment_range,
                        span,
                    };
                    match object_ir {
                        Ir::Access { receiver, mut segments, range: _, span: _ } => {
                            segments.push(segment);
                            Ir::Access { receiver, segments, range, span }
                        }
                        other => Ir::Access {
                            receiver: Box::new(other),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => Ir::Unknown {
                    kind: "attribute(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Subscript `obj[idx]`. tree-sitter Python: `subscript` with
        // field `value` (the object) and one or more unnamed children
        // representing the index expression(s). Same chain-accumulation
        // pattern as `attribute`.
        //
        // Segment range covers from end-of-value to end-of-subscript-node,
        // i.e. `[indices]` including brackets.
        "subscript" => {
            let value_node = node.child_by_field_name("value");
            // All non-`value` named children are indices.
            let mut cursor = node.walk();
            let indices_ts: Vec<TsNode> = node
                .named_children(&mut cursor)
                .filter(|c| Some(c.id()) != value_node.map(|v| v.id()))
                .collect();
            let indices: Vec<Ir> = indices_ts.iter().map(|c| lower_node(*c, source)).collect();
            match value_node {
                Some(object) => {
                    let object_ir = lower_node(object, source);
                    let segment_range = ByteRange::new(
                        object_ir.range().end,
                        range.end,
                    );
                    let segment = AccessSegment::Index {
                        indices,
                        range: segment_range,
                        span,
                    };
                    match object_ir {
                        Ir::Access { receiver, mut segments, range: _, span: _ } => {
                            segments.push(segment);
                            Ir::Access { receiver, segments, range, span }
                        }
                        other => Ir::Access {
                            receiver: Box::new(other),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                None => Ir::Unknown {
                    kind: "subscript(missing value)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Standalone call `f(args)`. tree-sitter Python: `call` with
        // fields `function` and `arguments` (an `argument_list`).
        // For now we only handle the bare-callee form. When `function`
        // is itself a chain, this should fold into `Access` — deferred
        // to the next iteration along with operator-marker support.
        "call" => {
            let function_node = node.child_by_field_name("function");
            let arguments_node = node.child_by_field_name("arguments");
            let callee = match function_node {
                Some(f) => lower_node(f, source),
                None => return Ir::Unknown {
                    kind: "call(missing function)".to_string(),
                    range,
                    span,
                },
            };
            let arguments: Vec<Ir> = match arguments_node {
                Some(a) => {
                    let mut cursor = a.walk();
                    a.named_children(&mut cursor)
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            Ir::Call {
                callee: Box::new(callee),
                arguments,
                range,
                span,
            }
        }

        // Binary `a op b`. tree-sitter Python: `binary_operator` with
        // fields `left`, `operator`, `right`. The operator-marker
        // table is shared cross-language at scale; for the experiment
        // a tiny inline map covers `+ - * /`.
        "binary_operator" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (left, right, op_marker(&op_text)) {
                (Some(l), Some(r), Some(marker)) => Ir::Binary {
                    element_name: "binary",
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l),
                    right: Box::new(r),
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "binary_operator(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Production-readiness handlers for kinds the old pipeline
        // handles but the blueprint doesn't exercise. (set,
        // generic_type, type, type_parameter all have proper
        // typed handlers further down — don't shadow them here.)
        "slice"                  => simple_statement(node, "slice",        source),
        "ellipsis"               => simple_statement(node, "ellipsis",     source),
        "exec_statement"         => simple_statement(node, "exec",         source),
        "print_statement"        => simple_statement(node, "print",        source),
        "format_expression"      => simple_statement(node, "interpolation",source),
        "escape_interpolation"   => simple_statement(node, "interpolation",source),
        "parenthesized_list_splat" => simple_statement(node, "spread",     source),
        "member_type"            => simple_statement(node, "type",         source),
        "chevron"                => simple_statement(node, "chevron",      source),
        "type_conversion"        => simple_statement(node, "cast",         source),

        // Comprehensions and related — `[x for x in y]` etc. Old
        // pipeline names them after their literal kind.
        "list_comprehension"        => simple_statement(node, "list",      source),
        "set_comprehension"         => simple_statement(node, "set",       source),
        "dictionary_comprehension"  => simple_statement(node, "dict",      source),
        "generator_expression"      => simple_statement(node, "generator", source),
        "for_in_clause"             => simple_statement(node, "for",       source),
        "if_clause"                 => simple_statement(node, "if",        source),
        // `with` / `async with`. tree-sitter exposes the `async`
        // keyword as an unnamed child of with_statement; detect by
        // scanning the source slice prefix and add an `<async/>`
        // marker on the IR.
        "with_statement" => {
            let leading = range.slice(source).trim_start();
            if leading.starts_with("async") {
                simple_statement_marked(node, "with", &["async"], source)
            } else {
                simple_statement(node, "with", source)
            }
        }
        // tree-sitter wraps `with` items in a `with_clause` and each
        // `with EXPR [as NAME]` as `with_item`. The imperative
        // pipeline flattens both wrappers (Rule::Flatten on
        // WithClause / WithItem) so the items become direct children
        // of `<with>`. Mirror via Ir::Inline.
        "with_clause" | "with_item" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }
        "match_statement"           => simple_statement(node, "match",     source),
        "case_clause"               => simple_statement(node, "arm",       source),
        "case_pattern"              => simple_statement(node, "pattern",   source),
        "class_pattern"             => simple_statement(node, "pattern",   source),
        "complex_pattern"           => simple_statement(node, "pattern",   source),
        "dict_pattern"              => simple_statement(node, "pattern",   source),
        "keyword_pattern"           => simple_statement(node, "pattern",   source),
        "splat_pattern"             => simple_statement(node, "pattern",   source),
        "list_pattern"              => simple_statement(node, "pattern",   source),
        "tuple_pattern"             => simple_statement(node, "pattern",   source),
        "union_pattern"             => simple_statement(node, "pattern",   source),
        "union_type"                => simple_statement(node, "type",      source),
        "named_expression"          => simple_statement(node, "assign",    source),
        "future_import_statement"   => simple_statement(node, "import",    source),
        "interpolation"             => simple_statement(node, "interpolation", source),

        // Simple keyword-prefixed statements that the old pipeline
        // just renames. Each is lowered to Ir::SimpleStatement with
        // the right element name and the named CST children.
        "assert_statement"  => simple_statement(node, "assert",   source),
        "raise_statement"   => simple_statement(node, "raise",    source),
        "delete_statement"  => simple_statement(node, "delete",   source),
        "global_statement"  => simple_statement(node, "global",   source),
        "nonlocal_statement"=> simple_statement(node, "nonlocal", source),
        "yield"             => simple_statement(node, "yield",    source),
        "concatenated_string" => simple_statement(node, "string", source),

        // Python `lambda`: `lambda x, y: expr`. tree-sitter exposes
        // `parameters` field (a `lambda_parameters` node) and `body`
        // field (the inner expression).
        "lambda" => {
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            // lambda_parameters wraps parameter children; lower each.
            let parameters: Vec<Ir> = match params_node {
                Some(p) => lower_parameters(p, source),
                None => Vec::new(),
            };
            let body = match body_node {
                Some(b) => Box::new(lower_node(b, source)),
                None => Box::new(Ir::Unknown {
                    kind: "lambda(missing body)".to_string(),
                    range, span,
                }),
            };
            Ir::Lambda {
                modifiers: Modifiers::default(),
                parameters,
                body,
                range, span,
            }
        }

        // `not x` — Python's logical NOT. tree-sitter:
        // `not_operator` with `argument` field.
        "not_operator" => {
            let arg = node.child_by_field_name("argument");
            // The `not` keyword is unnamed; locate its range.
            let kw_range = locate_token(source, range.start as usize, range.end as usize, "not");
            match arg {
                Some(a) => Ir::Unary {
                    op_text: "not".to_string(),
                    op_marker: "not",
                    op_range: kw_range,
                    operand: Box::new(lower_node(a, source)),
                    extra_markers: &[],
                    range, span,
                },
                None => Ir::Unknown {
                    kind: "not_operator(missing arg)".to_string(),
                    range, span,
                },
            }
        }

        // `(expr)` — parenthesized expression. Inline the inner;
        // parens become gap text on the surrounding parent.
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::Inline {
                    children: vec![lower_node(i, source)],
                    list_name: None,
                    range, span,
                },
                None => Ir::Unknown {
                    kind: "parenthesized_expression(empty)".to_string(),
                    range, span,
                },
            }
        }

        // `a and b`, `a or b` — short-circuit logical. tree-sitter
        // exposes `left`, `operator`, `right` like binary_operator.
        // Renders as `<logical>` (distinct from `<binary>`).
        "boolean_operator" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let marker = match op_text.as_str() {
                "and" => "and",
                "or"  => "or",
                _     => "and",  // fallback
            };
            match (left, right) {
                (Some(l), Some(r)) => Ir::Binary {
                    element_name: "logical",
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l),
                    right: Box::new(r),
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "boolean_operator(missing)".to_string(),
                    range, span,
                },
            }
        }

        // `not x` — handled by unary_operator below; the operator text
        // "not" already maps via op_marker.

        // Unary `op x`. tree-sitter Python: `unary_operator` with
        // fields `operator` and `argument`.
        "unary_operator" => {
            let operand = node.child_by_field_name("argument").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (operand, op_marker(&op_text)) {
                (Some(o), Some(marker)) => Ir::Unary {
                    op_text,
                    op_marker: marker,
                    op_range,
                    operand: Box::new(o),
                    extra_markers: &[],
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "unary_operator(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // ----- Collections ----------------------------------------------

        "tuple" => {
            let mut c = node.walk();
            let children: Vec<Ir> = node.named_children(&mut c).map(|n| lower_node(n, source)).collect();
            Ir::Tuple { children, range, span }
        }
        "list" => {
            let mut c = node.walk();
            let children: Vec<Ir> = node.named_children(&mut c).map(|n| lower_node(n, source)).collect();
            Ir::List { children, range, span }
        }
        "set" => {
            let mut c = node.walk();
            let children: Vec<Ir> = node.named_children(&mut c).map(|n| lower_node(n, source)).collect();
            Ir::Set { children, range, span }
        }
        "dictionary" => {
            let mut c = node.walk();
            let pairs: Vec<Ir> = node.named_children(&mut c).map(|n| lower_node(n, source)).collect();
            Ir::Dictionary { pairs, range, span }
        }
        "pair" => {
            let key = node.child_by_field_name("key").map(|n| lower_node(n, source));
            let value = node.child_by_field_name("value").map(|n| lower_node(n, source));
            match (key, value) {
                (Some(k), Some(v)) => Ir::Pair {
                    key: Box::new(k),
                    value: Box::new(v),
                    range,
                    span,
                },
                _ => Ir::Unknown { kind: "pair(missing)".to_string(), range, span },
            }
        }

        // ----- Generic type expressions ---------------------------------

        "generic_type" => {
            // tree-sitter Python: generic_type has named children:
            // first the base name (identifier or attribute), then a
            // type_parameter list. The type_parameter list children
            // are the type arguments.
            let mut cur = node.walk();
            let mut children = node.named_children(&mut cur);
            let name = match children.next() {
                Some(n) => Box::new(lower_node(n, source)),
                None => return Ir::Unknown { kind: "generic_type(no name)".to_string(), range, span },
            };
            // Remaining named children form the type-args list — but
            // they're often wrapped in a `type_parameter` container.
            let mut params: Vec<Ir> = Vec::new();
            for c in children {
                if c.kind() == "type_parameter" {
                    let mut cc = c.walk();
                    for arg in c.named_children(&mut cc) {
                        params.push(lower_type_arg(arg, source));
                    }
                } else {
                    params.push(lower_type_arg(c, source));
                }
            }
            Ir::GenericType { name, params, range, span }
        }

        // ----- Comparisons -----------------------------------------------

        "comparison_operator" => {
            // tree-sitter Python: comparison_operator's children are
            // alternating operands and operator tokens. For two-operand
            // case (most common): [left_expr, op_token, right_expr].
            let mut cur = node.walk();
            let all: Vec<TsNode> = node.children(&mut cur).collect();
            // Pick the first named child as left, the last named as right,
            // and find the comparator token between them.
            let named: Vec<TsNode> = all.iter().filter(|n| n.is_named()).copied().collect();
            if named.len() == 2 {
                let left = lower_node(named[0], source);
                let right = lower_node(named[1], source);
                // Operator: the unnamed/named token between them.
                // Scan all children in source order; find the first token
                // between left.end and right.start.
                let between_start = named[0].byte_range().end;
                let between_end = named[1].byte_range().start;
                let op_text = source[between_start..between_end].trim().to_string();
                let op_range = locate_token(source, between_start, between_end, &op_text);
                let op_marker = comparison_op_marker(&op_text).unwrap_or("equal");
                Ir::Comparison {
                    left: Box::new(left),
                    op_text,
                    op_marker,
                    op_range,
                    right: Box::new(right),
                    range,
                    span,
                }
            } else {
                Ir::Unknown { kind: format!("comparison_operator({} operands)", named.len()), range, span }
            }
        }

        // ----- Control flow ---------------------------------------------

        "if_statement" => {
            // Fields: `condition`, `consequence` (the body block);
            // `alternative` is an elif_clause or else_clause (optional).
            let cond = node.child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source)));
            let body = node.child_by_field_name("consequence")
                .map(|n| Box::new(lower_block(n, source)));
            let alt = node.child_by_field_name("alternative");
            let else_branch = alt.map(|a| Box::new(lower_else_chain(a, source)));
            match (cond, body) {
                (Some(c), Some(b)) => Ir::If {
                    condition: c,
                    body: b,
                    else_branch,
                    range,
                    span,
                },
                _ => Ir::Unknown { kind: "if_statement(missing field)".to_string(), range, span },
            }
        }

        "for_statement" => {
            let is_async = source[range.start as usize..(range.start as usize + 5).min(source.len())]
                .starts_with("async");
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");
            let body = node.child_by_field_name("body");
            let alt = node.child_by_field_name("alternative");
            let targets = match left {
                Some(l) => lower_assign_side(l, source),
                None => Vec::new(),
            };
            let iterables = match right {
                Some(r) => lower_assign_side(r, source),
                None => Vec::new(),
            };
            let body = match body {
                Some(b) => Box::new(lower_block(b, source)),
                None => Box::new(Ir::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span }),
            };
            let else_body = alt.map(|a| {
                // alternative is an else_clause; lower its inner body.
                let inner = a.child_by_field_name("body").unwrap_or(a);
                Box::new(lower_block(inner, source))
            });
            Ir::For {
                is_async,
                targets,
                iterables,
                body,
                else_body,
                range,
                span,
            }
        }

        // `try: ... except E: ... else: ... finally: ...`. tree-sitter
        // exposes `body` field (a `block`) and a sequence of clauses:
        // `except_clause`(s), `except_group_clause`(s), an optional
        // `else_clause`, an optional `finally_clause`.
        "try_statement" => {
            let body_node = node.child_by_field_name("body");
            let try_body = body_node
                .map(|b| Box::new(lower_block(b, source)))
                .unwrap_or_else(|| Box::new(Ir::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.start), span }));
            let mut cursor = node.walk();
            let mut handlers: Vec<Ir> = Vec::new();
            let mut else_body: Option<Box<Ir>> = None;
            let mut finally_body: Option<Box<Ir>> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "except_clause" | "except_group_clause" => {
                        handlers.push(lower_python_except_clause(c, source));
                    }
                    "else_clause" => {
                        // else_clause has a body field.
                        let inner = c.child_by_field_name("body").unwrap_or(c);
                        else_body = Some(Box::new(lower_block(inner, source)));
                    }
                    "finally_clause" => {
                        // tree-sitter-python's `finally_clause` doesn't
                        // expose a `body` field — find the inner
                        // `block` named child explicitly. Falling back
                        // to `c` itself produces two `<body>` wrappers
                        // (the finally-clause range *and* the block
                        // range) which the no-repeated-parent-child
                        // contract rejects.
                        let mut cc = c.walk();
                        let inner = c.named_children(&mut cc)
                            .find(|n| n.kind() == "block")
                            .or_else(|| c.child_by_field_name("body"));
                        if let Some(b) = inner {
                            finally_body = Some(Box::new(lower_block(b, source)));
                        }
                    }
                    _ => {}
                }
            }
            Ir::Try { try_body, handlers, else_body, finally_body, range, span }
        }

        // PEP 695 type alias: `type Foo[T] = list[T]`. tree-sitter
        // structure: type_alias_statement(type(left), type(right)).
        // The renderer wraps left/right in <type> already, so we
        // unwrap the CST `type` wrapper here to avoid double-nesting.
        "type_alias_statement" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            fn unwrap_type<'a>(n: TsNode<'a>) -> TsNode<'a> {
                if n.kind() == "type" {
                    let mut c = n.walk();
                    let inner = n.named_children(&mut c).next();
                    inner.unwrap_or(n)
                } else { n }
            }
            if kids.len() >= 2 {
                let left = lower_node(unwrap_type(kids[0]), source);
                let right = lower_node(unwrap_type(kids[kids.len() - 1]), source);
                Ir::TypeAlias {
                    name: Box::new(left),
                    type_params: None,
                    value: Box::new(right),
                    range, span,
                }
            } else {
                Ir::Unknown {
                    kind: format!("type_alias_statement(arity={})", kids.len()),
                    range, span,
                }
            }
        }

        // `*x` in calls / list literals — splat. tree-sitter:
        // list_splat with single named child = the splatted expr.
        "list_splat" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::ListSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => Ir::Unknown {
                    kind: "list_splat(empty)".to_string(),
                    range, span,
                },
            }
        }

        // `**x` in calls / dict literals.
        "dictionary_splat" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::DictSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => Ir::Unknown {
                    kind: "dictionary_splat(empty)".to_string(),
                    range, span,
                },
            }
        }

        // `name=value` keyword argument. Fields: name, value.
        "keyword_argument" => {
            let n = node.child_by_field_name("name");
            let v = node.child_by_field_name("value");
            match (n, v) {
                (Some(nn), Some(vv)) => Ir::KeywordArgument {
                    name: Box::new(Ir::Name { range: range_of(nn), span: span_of(nn) }),
                    value: Box::new(lower_node(vv, source)),
                    range, span,
                },
                _ => Ir::Unknown {
                    kind: "keyword_argument(missing field)".to_string(),
                    range, span,
                },
            }
        }

        // Python ternary: `a if cond else b`. tree-sitter exposes
        // children positionally (no field labels): if_true, condition,
        // if_false in source order.
        "conditional_expression" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            if kids.len() == 3 {
                Ir::Ternary {
                    if_true: Box::new(lower_node(kids[0], source)),
                    condition: Box::new(lower_node(kids[1], source)),
                    if_false: Box::new(lower_node(kids[2], source)),
                    range, span,
                }
            } else {
                Ir::Unknown {
                    kind: format!("conditional_expression(arity={})", kids.len()),
                    range, span,
                }
            }
        }

        "while_statement" => {
            let cond = node.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source)));
            let body = node.child_by_field_name("body").map(|n| Box::new(lower_block(n, source)));
            let alt = node.child_by_field_name("alternative");
            let else_body = alt.map(|a| {
                let inner = a.child_by_field_name("body").unwrap_or(a);
                Box::new(lower_block(inner, source))
            });
            match (cond, body) {
                (Some(c), Some(b)) => Ir::While { condition: c, body: b, else_body, range, span },
                _ => Ir::Unknown { kind: "while_statement(missing field)".to_string(), range, span },
            }
        }

        "break_statement" => Ir::Break { range, span },
        "continue_statement" => Ir::Continue { range, span },

        // ----- Function / class declarations ----------------------------

        // `def f(...)` / `async def f(...)` / `def f[T](...)`.
        // tree-sitter Python: function_definition has fields `name`,
        // `parameters`, `return_type` (optional), `body`,
        // `type_parameters` (optional, PEP 695). The `async` keyword
        // appears as an unnamed token; presence is detected by
        // scanning the leading text.
        "function_definition" => {
            let is_async = source[range.start as usize..(range.start as usize + 5).min(source.len())]
                .starts_with("async");
            lower_function(node, source, is_async, Vec::new())
        }

        // `class C(bases): ...`. Fields: `name`, `superclasses`
        // (optional argument_list), `body`, `type_parameters`
        // (optional).
        "class_definition" => lower_class(node, source, Vec::new()),

        // Wraps decorators around an inner function/class. Hoist the
        // decorators into the inner def per the existing pipeline.
        "decorated_definition" => {
            let mut cursor = node.walk();
            let mut decorators: Vec<Ir> = Vec::new();
            let mut inner: Option<TsNode> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "decorator" => decorators.push(lower_decorator(c, source)),
                    _ => inner = Some(c),
                }
            }
            match inner {
                Some(n) if n.kind() == "function_definition" => {
                    let is_async = source[n.byte_range().start..(n.byte_range().start + 5).min(source.len())]
                        .starts_with("async");
                    lower_function(n, source, is_async, decorators)
                }
                Some(n) if n.kind() == "class_definition" => lower_class(n, source, decorators),
                Some(n) => lower_node(n, source),
                None => Ir::Unknown { kind: "decorated_definition(empty)".to_string(), range, span },
            }
        }

        // `return [value]`
        "return_statement" => {
            let mut cursor = node.walk();
            let value = node.named_children(&mut cursor).next();
            Ir::Return {
                value: value.map(|v| Box::new(lower_node(v, source))),
                range,
                span,
            }
        }

        // `pass` — represented by an empty `<body[pass]>` when used
        // as a function/class body (handled there). At statement
        // level it appears inline; we'll handle that case as part of
        // `block` lowering. Direct `pass_statement` here renders as a
        // bare `<pass/>` marker — but actually the existing pipeline
        // uses `<body[pass]>` for "body is just pass". For mid-block
        // pass we'd need its own handling — TODO when test surfaces it.
        // For now, treat as Unknown (won't fire because pass-only
        // bodies are caught in lower_block).
        "pass_statement" => Ir::Unknown {
            kind: "pass_statement".to_string(),
            range,
            span,
        },

        // `# comment text`. Leading-vs-trailing classification is
        // adjacency-based in the existing pipeline (a separate
        // post-walk). For the experiment, we default to `leading: true`
        // since all blueprint comments precede the construct they
        // describe. Proper classification = TODO.
        "comment" => Ir::Comment { trailing: false,
            leading: true,
            range,
            span,
        },

        // ----- Assignments ----------------------------------------------

        // `target = value` / `target: type = value` / `target: type`
        "assignment" => {
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");
            let type_ann = node.child_by_field_name("type");
            // The `=` token is anonymous in the CST; locate it by
            // scanning the source between the children. Convention:
            // op_text = "=", op_range = position of `=` if present.
            let (op_text, op_range) = locate_assign_eq(node, source, left, type_ann, right);
            Ir::Assign {
                targets: match left {
                    Some(n) => lower_assign_side(n, source),
                    None => vec![],
                },
                type_annotation: type_ann.map(|t| Box::new(lower_type_slot(t, source))),
                op_text,
                op_range,
                op_markers: Vec::new(),
                values: match right {
                    Some(n) => lower_assign_side(n, source),
                    None => vec![],
                },
                range,
                span,
            }
        }

        // `target OP= value` for OP in `+ - * / // % @ ** & | ^ >> <<`
        "augmented_assignment" => {
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let op_markers = augmented_op_markers(&op_text);
            Ir::Assign {
                targets: match left {
                    Some(n) => lower_assign_side(n, source),
                    None => vec![],
                },
                type_annotation: None,
                op_text,
                op_range,
                op_markers,
                values: match right {
                    Some(n) => lower_assign_side(n, source),
                    None => vec![],
                },
                range,
                span,
            }
        }

        // ----- Imports --------------------------------------------------

        // `import os` / `import sys as system` / `import a, b`
        "import_statement" => {
            let mut cursor = node.walk();
            let mut children: Vec<Ir> = Vec::new();
            let mut has_alias = false;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "dotted_name" => children.push(lower_dotted_as_path(c, source)),
                    "aliased_import" => {
                        has_alias = true;
                        let (path, aliased) = lower_aliased_top(c, source);
                        children.push(path);
                        children.push(aliased);
                    }
                    _ => children.push(lower_node(c, source)),
                }
            }
            Ir::Import { has_alias, children, range, span }
        }

        // `from x import y` / `from . import x` / `from .x import y as z`
        "import_from_statement" => {
            // tree-sitter Python: import_from_statement has fields
            // `module_name` (relative_import OR dotted_name) and
            // unnamed children for the imported names (after the
            // `import` keyword).
            let module_name = node.child_by_field_name("module_name");
            let (relative, path) = match module_name {
                Some(m) if m.kind() == "relative_import" => {
                    // Relative: may have inner dotted_name (path) or be just dots.
                    let mut c = m.walk();
                    let inner_path = m.named_children(&mut c)
                        .find(|n| n.kind() == "dotted_name");
                    (true, inner_path.map(|n| Box::new(lower_dotted_as_path(n, source))))
                }
                Some(m) if m.kind() == "dotted_name" => {
                    (false, Some(Box::new(lower_dotted_as_path(m, source))))
                }
                _ => (false, None),
            };

            // Imported names: collect `name`-field children plus
            // wildcard markers. tree-sitter exposes them as the
            // `name` field (one or more) plus possibly a
            // `wildcard_import` child.
            let mut imports: Vec<Ir> = Vec::new();
            let mut cursor2 = node.walk();
            for c in node.named_children(&mut cursor2) {
                let same_as_module = module_name.map(|m| m.id()) == Some(c.id());
                if same_as_module { continue; }
                match c.kind() {
                    "dotted_name" => {
                        // For `from x import y`, the imported name is a
                        // single-segment dotted_name. We unwrap to a
                        // bare `Ir::Name`.
                        let name_node = lower_dotted_first_name(c, source);
                        imports.push(Ir::FromImport {
                            has_alias: false,
                            name: Box::new(name_node),
                            alias: None,
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                    "aliased_import" => {
                        let (n, a) = lower_aliased_from(c, source);
                        imports.push(Ir::FromImport {
                            has_alias: true,
                            name: Box::new(n),
                            alias: Some(Box::new(a)),
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                    "wildcard_import" => {
                        // `from x import *` — emit as a special
                        // marker-bearing import. For now, treat as
                        // `Ir::FromImport` with a synthetic Name
                        // covering `*`.
                        imports.push(Ir::FromImport {
                            has_alias: false,
                            name: Box::new(Ir::Name { range: range_of(c), span: span_of(c) }),
                            alias: None,
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                    _ => {
                        imports.push(Ir::Unknown {
                            kind: c.kind().to_string(),
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                }
            }

            Ir::From { relative, path, imports, range, span }
        }

        // `dotted_name` outside of import context. Default lowering as
        // a Path. Specific call sites (import, from) use
        // `lower_dotted_as_path` directly.
        "dotted_name" => lower_dotted_as_path(node, source),

        // Atoms — leaf-level value carriers. Text is `source[range]`.
        "identifier" => Ir::Name { range, span },

        // tree-sitter-python wraps type-position expressions in a
        // `type` node (parameter annotations, return types, generic
        // arguments). The IR's typed shape has no `<type>` wrapping
        // here — the renderer adds it when emitting via a
        // `type_ann`/`Returns` slot. Unwrap to the inner expression
        // and lower it directly.
        "type" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => lower_node(i, source),
                None => Ir::Unknown { kind: "type(empty)".to_string(), range, span },
            }
        }

        // `expression_list` is tree-sitter-python's tuple-without-
        // parens (e.g. `a, b = 1, 2`). Lower as Ir::Tuple — the
        // renderer emits flat children which matches imperative shape.
        "expression_list" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Tuple { children, range, span }
        }

        // `pattern_list` (tree-sitter): `for a, b in ...`. Same
        // treatment as `expression_list` — lower to Ir::Tuple.
        "pattern_list" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Tuple { children, range, span }
        }

        // `constrained_type` (`T: Bound`) — Ir::TypeParameter shape.
        "constrained_type" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            if kids.len() >= 2 {
                Ir::TypeParameter {
                    name: Box::new(lower_node(kids[0], source)),
                    constraint: Some(Box::new(lower_node(kids[1], source))),
                    range, span,
                }
            } else if kids.len() == 1 {
                Ir::TypeParameter {
                    name: Box::new(lower_node(kids[0], source)),
                    constraint: None,
                    range, span,
                }
            } else {
                Ir::Unknown { kind: "constrained_type(empty)".to_string(), range, span }
            }
        }

        // `splat_type` (`*Ts`) — variadic generic. Lower as a marked
        // type whose source bytes carry the `*`.
        "splat_type" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::ListSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => Ir::Unknown { kind: "splat_type(empty)".to_string(), range, span },
            }
        }

        // `list_splat_pattern` (`*rest`) — splat in a parameter list
        // or assignment LHS.
        "list_splat_pattern" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::ListSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => Ir::Unknown { kind: "list_splat_pattern(empty)".to_string(), range, span },
            }
        }

        // `EXPR as NAME` — tree-sitter `as_pattern` kind. Lowers to
        // `<as>` element (matches imperative `Rename(As)`). Walks
        // named children so the inner expression and `as_pattern_target`
        // both surface.
        "as_pattern" => simple_statement(node, "as", source),
        // `as_pattern_target` is the binding-side name in `as NAME` —
        // imperative pipeline wraps it. For the IR we just unwrap and
        // recurse.
        "as_pattern_target" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => lower_node(i, source),
                None => Ir::Unknown { kind: "as_pattern_target(empty)".to_string(), range, span },
            }
        }

        // `await x` in Python — tree-sitter exposes as `await` kind
        // with the expression as a named child. Lower as Ir::Expression
        // with an `await` marker (matches the imperative pipeline's
        // `<expression[await]>` shape via Principle #15).
        "await" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::Expression {
                    inner: Box::new(lower_node(i, source)),
                    marker: Some("await"),
                    range,
                    span,
                },
                None => Ir::Unknown { kind: "await(empty)".to_string(), range, span },
            }
        }

        // tree-sitter sometimes exposes a bare `block` outside the
        // function/class body's normal slot (e.g. inside `try` /
        // `with`). Lower as Ir::Body without block_wrap.
        "block" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Body {
                children,
                pass_only: false,
                block_wrap: false,
                range,
                span,
            }
        }
        "integer"    => Ir::Int  { range, span },
        "float"      => Ir::Float { range, span },
        "string"     => Ir::String { range, span },
        "true"       => Ir::True { range, span },
        "false"      => Ir::False { range, span },
        "none"       => Ir::None { range, span },

        other => Ir::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// Lower a type-argument expression in a generic type. Wraps in a
/// `<type>` element by emitting the inner expression — the renderer
/// for `Ir::GenericType` handles the actual `<type>` wrapping.
fn lower_type_arg(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

/// Lower a keyword-prefixed simple statement (`assert`, `raise`,
/// `delete`, `global`, `nonlocal`, `yield`) to `Ir::SimpleStatement`.
/// Children are the CST's named children, lowered recursively.
/// Python statements don't carry modifiers; the field is empty.
fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let children: Vec<Ir> = node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
    Ir::SimpleStatement { element_name, modifiers: Modifiers::default(), extra_markers: &[], children, range, span }
}

/// Same as `simple_statement` but adds explicit `<marker/>` siblings
/// (e.g. `<async/>` on `async with`).
fn simple_statement_marked(
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers: &'static [&'static str],
    source: &str,
) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let children: Vec<Ir> = node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
    Ir::SimpleStatement { element_name, modifiers: Modifiers::default(), extra_markers, children, range, span }
}

/// Lower a Python `except_clause` to `Ir::ExceptHandler` with
/// kind="except". Structure: `except [Type [as Name]]: body`.
/// tree-sitter exposes positional children (no fields):
///   - optional type expression
///   - optional `as_pattern` for `as Name`
///   - the body block
fn lower_python_except_clause(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let mut type_target: Option<Box<Ir>> = None;
    let mut binding: Option<Box<Ir>> = None;
    let mut body: Option<Box<Ir>> = None;
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "block" if body.is_none() => {
                body = Some(Box::new(lower_block(c, source)));
            }
            "as_pattern" => {
                // `Type as Name` — first child is the type, then
                // `as_pattern_target` containing the name.
                let mut ac = c.walk();
                let kids: Vec<TsNode> = c.named_children(&mut ac).collect();
                if let Some(t) = kids.first() {
                    if type_target.is_none() {
                        type_target = Some(Box::new(lower_node(*t, source)));
                    }
                }
                if kids.len() >= 2 {
                    let last = kids[kids.len() - 1];
                    let inner = if last.kind() == "as_pattern_target" {
                        let mut tc = last.walk();
                        let n = last.named_children(&mut tc).next();
                        n.unwrap_or(last)
                    } else { last };
                    binding = Some(Box::new(Ir::Name { range: range_of(inner), span: span_of(inner) }));
                }
            }
            _ if type_target.is_none() && body.is_none() => {
                // First non-block, non-as_pattern child: the type.
                type_target = Some(Box::new(lower_node(c, source)));
            }
            _ => {}
        }
    }
    Ir::ExceptHandler {
        kind: "except",
        type_target,
        binding,
        filter: None,
        body: body.unwrap_or_else(|| Box::new(Ir::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span })),
        range, span,
    }
}

/// Lower an `else_clause` or `elif_clause` chain into nested
/// `Ir::ElseIf` / `Ir::Else`.
fn lower_else_chain(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        "elif_clause" => {
            let cond = node.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source)));
            let body = node.child_by_field_name("consequence").map(|n| Box::new(lower_block(n, source)));
            let alt = node.child_by_field_name("alternative");
            let else_branch = alt.map(|a| Box::new(lower_else_chain(a, source)));
            match (cond, body) {
                (Some(c), Some(b)) => Ir::ElseIf {
                    condition: c,
                    body: b,
                    else_branch,
                    range,
                    span,
                },
                _ => Ir::Unknown { kind: "elif_clause(missing)".to_string(), range, span },
            }
        }
        "else_clause" => {
            let body = node.child_by_field_name("body").map(|n| Box::new(lower_block(n, source)));
            match body {
                Some(b) => Ir::Else { body: b, range, span },
                None => Ir::Unknown { kind: "else_clause(missing body)".to_string(), range, span },
            }
        }
        _ => Ir::Unknown { kind: format!("else_chain({})", node.kind()), range, span },
    }
}

/// Locate `token` literally in `source[start..end]`. Returns its
/// byte range. Falls back to the start position if not found.
fn locate_token(source: &str, start: usize, end: usize, token: &str) -> ByteRange {
    if let Some(rel) = source[start..end].find(token) {
        let abs = start + rel;
        ByteRange::new(abs as u32, (abs + token.len()) as u32)
    } else {
        ByteRange::empty_at(start as u32)
    }
}

/// Map a comparison operator (`==`, `!=`, `<`, `<=`, `>`, `>=`,
/// `is`, `is not`, `in`, `not in`) to its marker name. Returns
/// `None` for unrecognized.
fn comparison_op_marker(op: &str) -> Option<&'static str> {
    Some(match op {
        "==" => "equal",
        "!=" => "not_equal",
        "<" => "less",
        "<=" => "less_or_equal",
        ">" => "greater",
        ">=" => "greater_or_equal",
        "is" => "is",
        "is not" => "is_not",
        "in" => "in",
        "not in" => "not_in",
        _ => return None,
    })
}

/// Lower a `function_definition` CST node into `Ir::Function`.
///
/// When called from a `decorated_definition` wrapper, the caller
/// passes `outer_range` covering the whole decorated declaration so
/// the decorators (whose source positions precede `def`) fall inside
/// the function's range and gap rendering works.
fn lower_function(node: TsNode<'_>, source: &str, is_async: bool, decorators: Vec<Ir>) -> Ir {
    let span = span_of(node);
    let inner_range = range_of(node);
    // If decorators precede the inner def, the effective range starts
    // at the first decorator's position.
    let range = if let Some(first_dec) = decorators.first() {
        ByteRange::new(first_dec.range().start, inner_range.end)
    } else {
        inner_range
    };
    let name_node = node.child_by_field_name("name");
    let params_node = node.child_by_field_name("parameters");
    let return_type_node = node.child_by_field_name("return_type");
    let body_node = node.child_by_field_name("body");
    let type_params_node = node.child_by_field_name("type_parameters")
        .or_else(|| {
            let mut cur = node.walk();
            let r = node.named_children(&mut cur).find(|n| n.kind() == "type_parameter");
            r
        });

    let name = match name_node {
        Some(n) => Box::new(Ir::Name { range: range_of(n), span: span_of(n) }),
        None => Box::new(Ir::Unknown {
            kind: "function(missing name)".to_string(),
            range,
            span,
        }),
    };

    let generics = type_params_node.map(|tp| Box::new(lower_type_parameters(tp, source)));

    let parameters = params_node.map(|p| lower_parameters(p, source)).unwrap_or_default();

    let returns = return_type_node.map(|rt| Box::new(Ir::Returns {
        type_ann: Box::new(lower_type_slot(rt, source)),
        range: range_of(rt),
        span: span_of(rt),
    }));

    let body = match body_node {
        Some(b) => Box::new(lower_block(b, source)),
        None => Box::new(Ir::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span }),
    };

    Ir::Function {
        element_name: "function",
        modifiers: Modifiers { async_: is_async, ..Modifiers::default() },
        decorators,
        name,
        generics,
        parameters,
        returns,
        body,
        range,
        span,
    }
}

/// Lower a `class_definition` CST node into `Ir::Class`. As with
/// `lower_function`, decorators expand the effective range backward.
fn lower_class(node: TsNode<'_>, source: &str, decorators: Vec<Ir>) -> Ir {
    let span = span_of(node);
    let inner_range = range_of(node);
    let range = if let Some(first_dec) = decorators.first() {
        ByteRange::new(first_dec.range().start, inner_range.end)
    } else {
        inner_range
    };
    let name_node = node.child_by_field_name("name");
    let superclasses_node = node.child_by_field_name("superclasses");
    let body_node = node.child_by_field_name("body");
    let type_params_node = node.child_by_field_name("type_parameters")
        .or_else(|| {
            let mut cur = node.walk();
            let r = node.named_children(&mut cur).find(|n| n.kind() == "type_parameter");
            r
        });

    let name = match name_node {
        Some(n) => Box::new(Ir::Name { range: range_of(n), span: span_of(n) }),
        None => Box::new(Ir::Unknown {
            kind: "class(missing name)".to_string(),
            range,
            span,
        }),
    };

    let generics = type_params_node.map(|tp| Box::new(lower_type_parameters(tp, source)));

    let bases = superclasses_node.map(|s| {
        let mut c = s.walk();
        s.named_children(&mut c).map(|n| lower_node(n, source)).collect()
    }).unwrap_or_default();

    let body = match body_node {
        Some(b) => Box::new(lower_block(b, source)),
        None => Box::new(Ir::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span }),
    };

    Ir::Class {
        kind: "class",
        // Python: no access modifiers, no static/abstract/etc on class.
        modifiers: Modifiers::default(),
        decorators, name, generics, bases, where_clauses: Vec::new(), body, range, span,
    }
}

/// Lower a `parameters` CST node — a parenthesized list of parameter
/// kinds. Returns a flat Vec of `Ir::Parameter` /
/// `Ir::PositionalSeparator` / `Ir::KeywordSeparator` in source
/// order.
fn lower_parameters(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = node.walk();
    let mut out: Vec<Ir> = Vec::new();
    for c in node.named_children(&mut cursor) {
        let span = span_of(c);
        let range = range_of(c);
        match c.kind() {
            "identifier" => {
                out.push(Ir::Parameter {
                    kind: ParamKind::Regular, extra_markers: &[],
                    name: Box::new(Ir::Name { range, span }),
                    type_ann: None,
                    default: None,
                    range,
                    span,
                });
            }
            "default_parameter" => {
                let n = c.child_by_field_name("name");
                let v = c.child_by_field_name("value");
                out.push(Ir::Parameter {
                    kind: ParamKind::Regular, extra_markers: &[],
                    name: Box::new(match n {
                        Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                        None => Ir::Unknown { kind: "default_parameter(no name)".to_string(), range, span },
                    }),
                    type_ann: None,
                    default: v.map(|n| Box::new(lower_node(n, source))),
                    range,
                    span,
                });
            }
            "typed_parameter" => {
                let mut cur = c.walk();
                let mut name_n: Option<TsNode> = None;
                for ch in c.named_children(&mut cur) {
                    if ch.kind() == "identifier" { name_n = Some(ch); break; }
                }
                let type_n = c.child_by_field_name("type");
                out.push(Ir::Parameter {
                    kind: ParamKind::Regular, extra_markers: &[],
                    name: Box::new(match name_n {
                        Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                        None => Ir::Unknown { kind: "typed_parameter(no name)".to_string(), range, span },
                    }),
                    type_ann: type_n.map(|t| Box::new(lower_type_slot(t, source))),
                    default: None,
                    range,
                    span,
                });
            }
            "typed_default_parameter" => {
                let n = c.child_by_field_name("name");
                let t = c.child_by_field_name("type");
                let v = c.child_by_field_name("value");
                out.push(Ir::Parameter {
                    kind: ParamKind::Regular, extra_markers: &[],
                    name: Box::new(match n {
                        Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                        None => Ir::Unknown { kind: "typed_default_parameter(no name)".to_string(), range, span },
                    }),
                    type_ann: t.map(|t| Box::new(lower_type_slot(t, source))),
                    default: v.map(|n| Box::new(lower_node(n, source))),
                    range,
                    span,
                });
            }
            "list_splat_pattern" => {
                // *args — has one named child (identifier).
                let mut cur = c.walk();
                let inner = c.named_children(&mut cur).next();
                let name = match inner {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown { kind: "list_splat(no name)".to_string(), range, span },
                };
                out.push(Ir::Parameter {
                    kind: ParamKind::Args, extra_markers: &[],
                    name: Box::new(name),
                    type_ann: None,
                    default: None,
                    range,
                    span,
                });
            }
            "dictionary_splat_pattern" => {
                // **kwargs
                let mut cur = c.walk();
                let inner = c.named_children(&mut cur).next();
                let name = match inner {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown { kind: "dict_splat(no name)".to_string(), range, span },
                };
                out.push(Ir::Parameter {
                    kind: ParamKind::Kwargs, extra_markers: &[],
                    name: Box::new(name),
                    type_ann: None,
                    default: None,
                    range,
                    span,
                });
            }
            "positional_separator" => {
                out.push(Ir::PositionalSeparator { range, span });
            }
            "keyword_separator" => {
                out.push(Ir::KeywordSeparator { range, span });
            }
            other => {
                out.push(Ir::Unknown {
                    kind: format!("parameter({other})"),
                    range,
                    span,
                });
            }
        }
    }
    out
}

/// Lower the type-parameter list `[T, U: bound, *Ts]` into `Ir::Generic`.
///
/// The CST passed in is whatever the function/class field
/// `type_parameters` returns. tree-sitter Python often wraps the list
/// in a single `type_parameter` node containing inner `type_parameter`
/// items (PEP 695 grammar quirk). We handle both single-level and
/// nested cases by walking ALL named descendants until we find
/// identifiers, splats, or constrained types — each becomes one
/// IR::TypeParameter.
fn lower_type_parameters(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut items: Vec<Ir> = Vec::new();
    collect_type_param_items(node, source, &mut items);
    Ir::Generic { items, range, span }
}

fn collect_type_param_items(node: TsNode<'_>, source: &str, out: &mut Vec<Ir>) {
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        let cspan = span_of(c);
        let crange = range_of(c);
        match c.kind() {
            // Per-item wrapper kind: `type` containing the
            // identifier (and optional constraint).
            "type" => {
                let mut cc = c.walk();
                let inner = c.named_children(&mut cc).next();
                let name = match inner {
                    Some(n) if n.kind() == "identifier" =>
                        Ir::Name { range: range_of(n), span: span_of(n) },
                    Some(n) => lower_node(n, source),
                    None => Ir::Unknown {
                        kind: "type_param(empty type)".to_string(),
                        range: crange,
                        span: cspan,
                    },
                };
                out.push(Ir::TypeParameter {
                    name: Box::new(name),
                    constraint: None,
                    range: crange,
                    span: cspan,
                });
            }
            "identifier" => {
                out.push(Ir::TypeParameter {
                    name: Box::new(Ir::Name { range: crange, span: cspan }),
                    constraint: None,
                    range: crange,
                    span: cspan,
                });
            }
            "constrained_type" | "splat_type" => {
                out.push(Ir::TypeParameter {
                    name: Box::new(lower_node(c, source)),
                    constraint: None,
                    range: crange,
                    span: cspan,
                });
            }
            "type_parameter" => {
                // Nested wrapper (PEP 695 grammar quirk). Recurse to
                // collect items inside.
                collect_type_param_items(c, source, out);
            }
            other => {
                out.push(Ir::Unknown {
                    kind: format!("type_param_item({other})"),
                    range: crange,
                    span: cspan,
                });
            }
        }
    }
}

/// Lower a `block` CST node (function/class body) into `Ir::Body`.
/// Recognises pass-only bodies (a single `pass_statement` child) and
/// renders them as `<body[pass]>` empty.
fn lower_block(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let named: Vec<TsNode> = node.named_children(&mut cursor).collect();
    let pass_only = named.len() == 1 && named[0].kind() == "pass_statement";
    let children: Vec<Ir> = if pass_only {
        Vec::new()
    } else {
        named.iter().filter(|n| n.kind() != "pass_statement").map(|n| lower_node(*n, source)).collect()
    };
    Ir::Body { children, pass_only, block_wrap: false, range, span }
}

/// Lower a `decorator` CST node. The decorator's inner is the
/// expression being applied (a name, attribute, or call).
fn lower_decorator(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let inner = node.named_children(&mut cursor).next();
    let inner_ir = match inner {
        Some(n) => lower_node(n, source),
        None => Ir::Unknown { kind: "decorator(empty)".to_string(), range, span },
    };
    Ir::Decorator { inner: Box::new(inner_ir), range, span }
}

/// Lower an assignment side (LHS targets or RHS values). If the node
/// is a multi-element pattern (pattern_list / tuple_pattern /
/// expression_list), each child becomes a separate Vec entry —
/// matching the existing pipeline's flat `<left><expression/>...</left>`
/// / `<right><expression/>...</right>` layout for multi-target /
/// multi-value assignments. Single-target / single-value cases return
/// a one-element vec.
fn lower_assign_side(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    match node.kind() {
        "pattern_list" | "tuple_pattern" | "expression_list" => {
            let mut c = node.walk();
            node.named_children(&mut c).map(|n| lower_node(n, source)).collect()
        }
        _ => vec![lower_node(node, source)],
    }
}

/// Type-annotation slot lowering. tree-sitter Python wraps the type
/// expression in a `type` node; we unwrap and lower the inner.
fn lower_type_slot(node: TsNode<'_>, source: &str) -> Ir {
    // The `type` field can be a `type` CST kind (with one named child)
    // or a bare expression. Unwrap if it's the wrapping `type` kind.
    if node.kind() == "type" {
        let mut c = node.walk();
        let inner = node.named_children(&mut c).next();
        if let Some(inner) = inner {
            return lower_node(inner, source);
        }
    }
    lower_node(node, source)
}

/// Locate the `=` token inside a plain `assignment` CST node. tree-sitter
/// Python doesn't surface this as a named child, so we scan the source
/// between the left/type/right named children for the literal `=`. This
/// is enough for the source-text invariant — gap text covers all
/// non-token bytes between named children.
fn locate_assign_eq(
    node: TsNode<'_>,
    source: &str,
    left: Option<TsNode<'_>>,
    type_ann: Option<TsNode<'_>>,
    right: Option<TsNode<'_>>,
) -> (String, ByteRange) {
    // Pure-type-only declaration `x: int` has no `=`.
    let after_type_or_left = type_ann.map(|t| t.end_byte())
        .or_else(|| left.map(|l| l.end_byte()))
        .unwrap_or(node.start_byte());
    let until = right.map(|r| r.start_byte()).unwrap_or(node.end_byte());
    if let Some(rel) = source[after_type_or_left..until].find('=') {
        let abs = after_type_or_left + rel;
        ("=".to_string(), ByteRange::new(abs as u32, (abs + 1) as u32))
    } else {
        // No `=`. Empty range at the end of left/type.
        ("".to_string(), ByteRange::empty_at(after_type_or_left as u32))
    }
}

/// Map an augmented-assignment operator (`+=`, `//=`, `@=`, …) to its
/// marker list.
fn augmented_op_markers(op: &str) -> Vec<&'static str> {
    let base: Option<&'static str> = match op {
        "+=" => Some("plus"),
        "-=" => Some("minus"),
        "*=" => Some("multiply"),
        "/=" => Some("divide"),
        "//=" => Some("floor"),
        "%=" => Some("modulo"),
        "@=" => Some("matmul"),
        "**=" => Some("power"),
        "&=" => Some("bitwise_and"),
        "|=" => Some("bitwise_or"),
        "^=" => Some("bitwise_xor"),
        ">>=" => Some("shift_right"),
        "<<=" => Some("shift_left"),
        _ => None,
    };
    match base {
        Some(b) => vec!["assign", b],
        None => vec!["assign"],
    }
}

/// Lower a `dotted_name` CST node to `Ir::Path` with one
/// `Ir::Name` per segment. Single-segment dotted_names (`os`) become a
/// `Path` with one segment, matching the existing pipeline shape
/// (always wrap module paths in `<path>`).
fn lower_dotted_as_path(node: TsNode<'_>, source: &str) -> Ir {
    let mut cursor = node.walk();
    let segments: Vec<Ir> = node
        .named_children(&mut cursor)
        .map(|c| Ir::Name { range: range_of(c), span: span_of(c) })
        .collect();
    Ir::Path {
        segments,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Lower a `dotted_name` to its first name segment as a bare `Ir::Name`
/// (used in `from x import y` where each imported name is a
/// dotted_name in the CST but renders as a bare `<name>` in the
/// existing pipeline). Falls back to `Unknown` if the dotted_name has
/// multiple segments (shouldn't happen for `from` imports).
fn lower_dotted_first_name(node: TsNode<'_>, source: &str) -> Ir {
    let mut cursor = node.walk();
    let mut iter = node.named_children(&mut cursor);
    if let Some(first) = iter.next() {
        Ir::Name { range: range_of(first), span: span_of(first) }
    } else {
        Ir::Unknown {
            kind: "dotted_name(empty)".to_string(),
            range: range_of(node),
            span: span_of(node),
        }
    }
}

/// Lower an `aliased_import` in *top-level* import context. Emits a
/// pair: (`Ir::Path`, `Ir::Aliased`) — both become flat children of
/// the enclosing `<import>`.
fn lower_aliased_top(node: TsNode<'_>, source: &str) -> (Ir, Ir) {
    // tree-sitter Python: aliased_import has `name` field (dotted_name)
    // and `alias` field (identifier).
    let name_node = node.child_by_field_name("name");
    let alias_node = node.child_by_field_name("alias");
    let path = match name_node {
        Some(n) => lower_dotted_as_path(n, source),
        None => Ir::Unknown {
            kind: "aliased_import(missing name)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    let aliased = match alias_node {
        Some(a) => Ir::Aliased {
            inner: Box::new(Ir::Name { range: range_of(a), span: span_of(a) }),
            range: range_of(a),
            span: span_of(a),
        },
        None => Ir::Unknown {
            kind: "aliased_import(missing alias)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    (path, aliased)
}

/// Lower an `aliased_import` inside a `from X import` context. Emits a
/// pair: (bare `Ir::Name`, `Ir::Aliased`) — without `<path>` wrapping
/// on the imported name (that's how `from m import x as y` renders).
fn lower_aliased_from(node: TsNode<'_>, source: &str) -> (Ir, Ir) {
    let name_node = node.child_by_field_name("name");
    let alias_node = node.child_by_field_name("alias");
    let name = match name_node {
        Some(n) if n.kind() == "dotted_name" => lower_dotted_first_name(n, source),
        Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
        None => Ir::Unknown {
            kind: "aliased_import(missing name)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    let aliased = match alias_node {
        Some(a) => Ir::Aliased {
            inner: Box::new(Ir::Name { range: range_of(a), span: span_of(a) }),
            range: range_of(a),
            span: span_of(a),
        },
        None => Ir::Unknown {
            kind: "aliased_import(missing alias)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    (name, aliased)
}

fn lower_children(parent: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect()
}

fn span_of(node: TsNode<'_>) -> Span {
    let s = node.start_position();
    let e = node.end_position();
    Span {
        line: (s.row + 1) as u32,
        column: (s.column + 1) as u32,
        end_line: (e.row + 1) as u32,
        end_column: (e.column + 1) as u32,
    }
}

fn range_of(node: TsNode<'_>) -> ByteRange {
    let r = node.byte_range();
    ByteRange::new(r.start as u32, r.end as u32)
}

/// Tiny inline operator-marker map. At scale this lives in the
/// shared `transform/operators.rs` table and is consulted by every
/// language; for the experiment a Python-only fragment is enough to
/// hit parity for `+ - * /` and prefix `- !`.
fn op_marker(op: &str) -> Option<&'static str> {
    Some(match op {
        "+" => "plus",
        "-" => "minus",
        "*" => "multiply",
        "/" => "divide",
        "//" => "floor_divide",
        "%" => "modulo",
        "**" => "power",
        "@" => "matrix_multiply",
        "&" => "bitwise_and",
        "|" => "bitwise_or",
        "^" => "bitwise_xor",
        "<<" => "shift_left",
        ">>" => "shift_right",
        _ => return None,
    })
}

fn text_of(node: TsNode<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .map(|s| s.to_string())
        .unwrap_or_default()
}
