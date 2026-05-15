//! Python tree-sitter CST → tree lowering.
//!
//! Pure function. No global state, no in-place mutation. Each
//! tree-sitter kind maps to exactly one tree variant (or [`SyntaxTree::Unknown`]
//! if not yet covered, or [`SyntaxTree::Inline`] if deliberately
//! shape-neutral).
//!
//! ## Initial coverage
//! Module + a single literal/identifier per expression statement, plus
//! member access, subscript, bare calls, binary `+ - * /`, unary
//! `+ -`. Just enough to validate that byte-range threading and
//! gap-text rendering work end-to-end. Expansion to chained calls,
//! comprehensions, and statements happens incrementally as parity
//! tests grow.


use crate::raw::RawNode;

use crate::tree::lower_helpers::{
    false_of, float_of, int_of, name_of, none_of, range_of, span_of, string_of, text_of, true_of,
};
use crate::tree::types::{Access, AccessSegment, ByteRange, SyntaxTree, Modifiers, Marker, ParamKind};

/// Lower a Python tree-sitter root node to [`SyntaxTree`].
///
/// The root is expected to be `module`. Anything else is returned as
/// [`SyntaxTree::Unknown`] so the parity test can spot the divergence
/// immediately.
pub fn lower_python_root(root: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "module" => SyntaxTree::Module {
            element_name: "module",
            children: merge_python_line_comments(lower_children(root, source), source),
            range,
            span,
        },
        other => SyntaxTree::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

fn lower_node(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // Statement-level wrapper; bare value-producing statement.
        // Per Principle #15, wrap in `<expression>` host.
        // Tree-sitter Python's expression_statement holds exactly one
        // child for bare-expression usage; we punt on tuple/multi cases
        // to Unknown for now.
        "expression_statement" => {
            let mut named: Vec<&RawNode> = Vec::new();
            for child in node.named_children() {
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
                        SyntaxTree::Expression {
                            inner: Box::new(lower_node(*single, source)),
                            marker: None,
                            range,
                            span,
                        }
                    }
                }
                _ => SyntaxTree::Unknown {
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
                        SyntaxTree::Access { receiver, mut segments, range: _, span: _ } => {
                            segments.push(segment);
                            SyntaxTree::Access { receiver, segments, range, span }
                        }
                        other => SyntaxTree::Access {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["self", "super"]),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => SyntaxTree::Unknown {
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
            let indices_ts: Vec<&RawNode> = node
                .named_children()
                .filter(|c| Some(c.id()) != value_node.map(|v| v.id()))
                .collect();
            let indices: Vec<SyntaxTree> = indices_ts.iter().map(|c| lower_node(*c, source)).collect();
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
                        SyntaxTree::Access { receiver, mut segments, range: _, span: _ } => {
                            segments.push(segment);
                            SyntaxTree::Access { receiver, segments, range, span }
                        }
                        other => SyntaxTree::Access {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["self", "super"]),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                None => SyntaxTree::Unknown {
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
                None => return SyntaxTree::Unknown {
                    kind: "call(missing function)".to_string(),
                    range,
                    span,
                },
            };
            let arguments: Vec<SyntaxTree> = match arguments_node {
                Some(a) => {
                    a.named_children()
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            // Chain folding: when the callee is itself an
            // SyntaxTree::Access chain (e.g. `obj.foo()`, `a.b.c()`), absorb
            // the call as the next segment instead of producing a
            // separate SyntaxTree::Call wrapping the chain. The last
            // AccessSegment::Member's name becomes the call's
            // `<name>` so the rendered shape is
            // `<object><name>obj</name><call><name>foo</name>...</call>`
            // rather than `<call><object>obj.foo</object>...</call>`.
            // Mirrors C#'s chain-inversion behaviour and matches the
            // imperative pipeline's chain shape.
            let callee_range = callee.range();
            if let SyntaxTree::Access { receiver, mut segments, .. } = callee {
                // Absorb the trailing Member's name into a new Call
                // segment, OR append a bare Call if no preceding member.
                let last_member = if let Some(AccessSegment::Member { property_range, property_span, .. }) = segments.last() {
                    Some((*property_range, *property_span))
                } else {
                    None
                };
                let call_segment = if let Some((property_range, property_span)) = last_member {
                    segments.pop();
                    let segment_range = ByteRange::new(
                        property_range.start,
                        range.end,
                    );
                    AccessSegment::Call {
                        name: Some(property_range),
                        name_span: Some(property_span),
                        arguments,
                        range: segment_range,
                        span,
                    }
                } else {
                    let segment_range = ByteRange::new(
                        callee_range.end,
                        range.end,
                    );
                    AccessSegment::Call {
                        name: None,
                        name_span: None,
                        arguments,
                        range: segment_range,
                        span,
                    }
                };
                segments.push(call_segment);
                return SyntaxTree::Access { receiver, segments, range, span };
            }
            SyntaxTree::Call {
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
                (Some(l), Some(r), Some(marker)) => SyntaxTree::Binary {
                    element_name: "binary",
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l.wrap_slot("left")),
                    right: Box::new(r.wrap_slot("right")),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
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
        "format_specifier"       => simple_statement(node, "format",       source),

        // Comprehensions and related — `[x for x in y]` etc. Old
        // pipeline names them after their literal kind.
        "list_comprehension"        => simple_statement_marked(node, "list", vec![Marker::implicit("comprehension")], source),
        "set_comprehension"         => simple_statement_marked(node, "set",  vec![Marker::implicit("comprehension")], source),
        "dictionary_comprehension"  => simple_statement_marked(node, "dict", vec![Marker::implicit("comprehension")], source),
        "generator_expression"      => simple_statement(node, "generator", source),
        "for_in_clause"             => simple_statement(node, "for",       source),
        // `if_clause` outside of `case_clause` (which handles its
        // guard explicitly) appears in comprehensions
        // (`[x for x in xs if cond]`) and there should flatten so
        // the `cond` expression becomes a direct child of the
        // surrounding `<list[comprehension]>` / `<dict[comprehension]>`.
        // Mirror imperative `if_clause` Custom transform's flatten
        // branch.
        "if_clause" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        // `with` / `async with`. tree-sitter exposes the `async`
        // keyword as an unnamed child of with_statement; detect by
        // scanning the source slice prefix and add an `<async/>`
        // marker on the tree.
        "with_statement" => {
            let leading = range.slice(source).trim_start();
            if leading.starts_with("async") {
                simple_statement_marked(node, "with", vec![Marker::implicit("async")], source)
            } else {
                simple_statement(node, "with", source)
            }
        }
        // tree-sitter wraps `with` items in a `with_clause` and each
        // `with EXPR [as NAME]` as `with_item`. The imperative
        // pipeline flattens both wrappers (Rule::Flatten on
        // WithClause / WithItem) so the items become direct children
        // of `<with>`. Mirror via SyntaxTree::Inline.
        "with_clause" | "with_item" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        "match_statement"           => simple_statement(node, "match",     source),
        // case_clause: `case PATTERN [if GUARD]: BODY`. Walk
        // children explicitly so we can rename the `if_clause` guard
        // field to `<guard>` (matches imperative shape) and recurse
        // on the rest. tree-sitter exposes the guard via field name,
        // and the bare `if_clause` would otherwise lower to `<if>`.
        "case_clause" => {
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                if c.kind() == "if_clause" {
                    // Lower the inner expression(s) into a `<guard>`.
                    let inner: Vec<SyntaxTree> = c.named_children()
                        .map(|n| lower_node(n, source))
                        .collect();
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "guard",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: inner,
                        range: range_of(c),
                        span: span_of(c),
                    });
                } else {
                    children.push(lower_node(c, source));
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "arm",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range,
                span,
            }
        }
        // `case_pattern` is a pure wrapper around the actual pattern
        // shape (dict_pattern, list_pattern, etc.). Flatten so we
        // don't double-wrap as `<pattern><pattern[dict]>`.
        "case_pattern" => {
            let children: Vec<SyntaxTree> = node.named_children()
                .map(|c| lower_node(c, source))
                .collect();
            // If there's a single named child that's itself a typed
            // pattern, just unwrap. Otherwise emit a bare <pattern>.
            if children.len() == 1 {
                children.into_iter().next().unwrap()
            } else {
                SyntaxTree::Inline { children, list_name: None, range, span }
            }
        }
        "class_pattern"             => simple_statement(node, "pattern",   source),
        "complex_pattern"           => simple_statement(node, "pattern",   source),
        // Discriminator markers on shape-bearing patterns mirror the
        // imperative pipeline's `RenameWithMarker(Pattern, Dict)`
        // etc.: each `<pattern[X]>` says "this is the X-shape".
        // `dict_pattern` is `{ "k1": V1, "k2": V2 }` in match arms.
        // tree-sitter exposes `string` (key) and `case_pattern`
        // (value) children. Wrap each value in `<value>` so the
        // post-pass can list-tag them with `list="values"`.
        "dict_pattern" => {
            let mut children: Vec<SyntaxTree> = Vec::new();
            // Walk children and detect the field name from the parent.
            for (i, c) in node.children().enumerate() {
                if !c.is_named() { continue; }
                let field_name = node.field_name_for_child(i as u32);
                if field_name == Some("value") {
                    let inner = lower_node(c, source);
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "value",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(c),
                        span: span_of(c),
                    });
                } else {
                    children.push(lower_node(c, source));
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "pattern",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("dict")],
                children,
                range,
                span,
            }
        }
        "list_pattern"              => simple_statement_marked(node, "pattern", vec![Marker::implicit("list")],  source),
        "tuple_pattern"             => simple_statement_marked(node, "pattern", vec![Marker::implicit("tuple")], source),
        "union_pattern"             => simple_statement_marked(node, "pattern", vec![Marker::implicit("union")], source),
        "splat_pattern"             => simple_statement_marked(node, "pattern", vec![Marker::implicit("splat")], source),
        "keyword_pattern"           => simple_statement(node, "pattern",   source),
        "union_type"                => simple_statement_marked(node, "type", vec![Marker::implicit("union")], source),
        "named_expression"          => simple_statement(node, "assign",    source),
        "future_import_statement"   => simple_statement(node, "import",    source),
        "interpolation"             => simple_statement(node, "interpolation", source),

        // Simple keyword-prefixed statements that the old pipeline
        // just renames. Each is lowered to SyntaxTree::SimpleStatement with
        // the right element name and the named CST children.
        "assert_statement"  => simple_statement(node, "assert",   source),
        "raise_statement"   => lower_python_raise(node, source),
        "delete_statement"  => simple_statement(node, "delete",   source),
        "global_statement"  => simple_statement(node, "global",   source),
        "nonlocal_statement"=> simple_statement(node, "nonlocal", source),
        "yield"             => lower_python_yield(node, source),
        "concatenated_string" => simple_statement(node, "string", source),

        // Python `lambda`: `lambda x, y: expr`. tree-sitter exposes
        // `parameters` field (a `lambda_parameters` node) and `body`
        // field (the inner expression).
        "lambda" => {
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            // lambda_parameters wraps parameter children; lower each.
            let parameters: Vec<SyntaxTree> = match params_node {
                Some(p) => lower_parameters(p, source),
                None => Vec::new(),
            };
            // Python lambdas are always expression-bodied.
            let body = match body_node {
                Some(b) => crate::tree::types::LambdaBody::Expression(Box::new(lower_node(b, source))),
                None => crate::tree::types::LambdaBody::Expression(Box::new(SyntaxTree::Unknown {
                    kind: "lambda(missing body)".to_string(),
                    range, span,
                })),
            };
            SyntaxTree::Lambda {
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
                Some(a) => SyntaxTree::Unary {
                    op_text: "not".to_string(),
                    op_marker: "not",
                    op_range: kw_range,
                    operand: Box::new(lower_node(a, source)),
                    extra_markers: Vec::new(),
                    range, span,
                },
                None => SyntaxTree::Unknown {
                    kind: "not_operator(missing arg)".to_string(),
                    range, span,
                },
            }
        }

        // `(expr)` — parenthesized expression. Inline the inner;
        // parens become gap text on the surrounding parent.
        "parenthesized_expression" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => SyntaxTree::Inline {
                    children: vec![lower_node(i, source)],
                    list_name: None,
                    range, span,
                },
                None => SyntaxTree::Unknown {
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
                (Some(l), Some(r)) => SyntaxTree::Binary {
                    element_name: "logical",
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l.wrap_slot("left")),
                    right: Box::new(r.wrap_slot("right")),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
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
                (Some(o), Some(marker)) => SyntaxTree::Unary {
                    op_text,
                    op_marker: marker,
                    op_range,
                    operand: Box::new(o),
                    extra_markers: Vec::new(),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "unary_operator(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // ----- Collections ----------------------------------------------

        "tuple" => {
            let children: Vec<SyntaxTree> = node.named_children().map(|n| lower_node(n, source)).collect();
            SyntaxTree::Tuple { children, range, span }
        }
        "list" => {
            let children: Vec<SyntaxTree> = node.named_children().map(|n| lower_node(n, source)).collect();
            SyntaxTree::List { children, range, span }
        }
        "set" => {
            let children: Vec<SyntaxTree> = node.named_children().map(|n| lower_node(n, source)).collect();
            SyntaxTree::Set { children, range, span }
        }
        "dictionary" => {
            let pairs: Vec<SyntaxTree> = node.named_children().map(|n| lower_node(n, source)).collect();
            SyntaxTree::Dictionary { pairs, range, span }
        }
        "pair" => {
            let key = node.child_by_field_name("key").map(|n| lower_node(n, source));
            let value = node.child_by_field_name("value").map(|n| lower_node(n, source));
            match (key, value) {
                (Some(k), Some(v)) => SyntaxTree::Pair {
                    key: Box::new(k),
                    value: Box::new(v),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown { kind: "pair(missing)".to_string(), range, span },
            }
        }

        // ----- Generic type expressions ---------------------------------

        "generic_type" => {
            // tree-sitter Python: generic_type has named children:
            // first the base name (identifier or attribute), then a
            // type_parameter list. The type_parameter list children
            // are the type arguments.
            let mut children = node.named_children();
            let name = match children.next() {
                Some(n) => Box::new(lower_node(n, source)),
                None => return SyntaxTree::Unknown { kind: "generic_type(no name)".to_string(), range, span },
            };
            // Remaining named children form the type-args list — but
            // they're often wrapped in a `type_parameter` container.
            let mut params: Vec<SyntaxTree> = Vec::new();
            for c in children {
                if c.kind() == "type_parameter" {
                    for arg in c.named_children() {
                        params.push(lower_type_arg(arg, source));
                    }
                } else {
                    params.push(lower_type_arg(c, source));
                }
            }
            SyntaxTree::GenericType { name, params, range, span }
        }

        // ----- Comparisons -----------------------------------------------

        "comparison_operator" => {
            // tree-sitter Python: comparison_operator's children are
            // alternating operands and operator tokens. For two-operand
            // case (most common): [left_expr, op_token, right_expr].
            let all: Vec<&RawNode> = node.children().collect();
            // Pick the first named child as left, the last named as right,
            // and find the comparator token between them.
            let named: Vec<&RawNode> = all.iter().filter(|n| n.is_named()).copied().collect();
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
                SyntaxTree::Comparison {
                    left: Box::new(left),
                    op_text,
                    op_marker,
                    op_range,
                    right: Box::new(right),
                    range,
                    span,
                }
            } else {
                SyntaxTree::Unknown { kind: format!("comparison_operator({} operands)", named.len()), range, span }
            }
        }

        // ----- Control flow ---------------------------------------------

        "if_statement" => {
            // Fields: `condition`, `consequence` (the body block);
            // multiple `alternative` fields each holding either an
            // elif_clause or else_clause. tree-sitter-python emits
            // them as flat siblings; we chain them into nested
            // SyntaxTree::ElseIf / SyntaxTree::Else so the renderer can flatten back
            // into `<else_if>`/`<else>` sibling output.
            let cond = node.child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source).wrap_slot("condition")));
            let body = node.child_by_field_name("consequence")
                .map(|n| Box::new(lower_block(n, source)));
            // Collect all alternative children in source order.
            let mut alts: Vec<&RawNode> = Vec::new();
            for (i, c) in node.children().enumerate() {
                if let Some(name) = node.field_name_for_child(i as u32) {
                    if name == "alternative" {
                        alts.push(c);
                    }
                }
            }
            // Build a nested chain right-to-left.
            let mut else_branch: Option<Box<SyntaxTree>> = None;
            for alt in alts.into_iter().rev() {
                let lowered = lower_else_chain_with_tail(alt, else_branch.take(), source);
                else_branch = Some(Box::new(lowered));
            }
            match (cond, body) {
                (Some(c), Some(b)) => SyntaxTree::If {
                    condition: c,
                    body: b,
                    else_branch,
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown { kind: "if_statement(missing field)".to_string(), range, span },
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
                None => Box::new(SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span }),
            };
            let else_body = alt.map(|a| {
                // alternative is an else_clause; lower its inner body.
                let inner = a.child_by_field_name("body").unwrap_or(a);
                Box::new(lower_block(inner, source))
            });
            SyntaxTree::For {
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
                .unwrap_or_else(|| Box::new(SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.start), span }));
            let mut handlers: Vec<SyntaxTree> = Vec::new();
            let mut else_body: Option<Box<SyntaxTree>> = None;
            let mut finally_body: Option<Box<SyntaxTree>> = None;
            for c in node.named_children() {
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
                        let inner = c.named_children()
                            .find(|n| n.kind() == "block")
                            .or_else(|| c.child_by_field_name("body"));
                        if let Some(b) = inner {
                            finally_body = Some(Box::new(lower_block(b, source)));
                        }
                    }
                    _ => {}
                }
            }
            SyntaxTree::Try { try_body, handlers, else_body, finally_body, range, span }
        }

        // PEP 695 type alias: `type Foo[T] = list[T]`. tree-sitter
        // structure: type_alias_statement(type(left), type(right)).
        // The renderer wraps left/right in <type> already, so we
        // unwrap the CST `type` wrapper here to avoid double-nesting.
        "type_alias_statement" => {
            let kids: Vec<&RawNode> = node.named_children().collect();
            fn unwrap_type<'a>(n: &'a RawNode) -> &'a RawNode {
                if n.kind() == "type" {
                    let inner = n.named_children().next();
                    inner.unwrap_or(n)
                } else { n }
            }
            if kids.len() >= 2 {
                let left = lower_node(unwrap_type(kids[0]), source);
                let right = lower_node(unwrap_type(kids[kids.len() - 1]), source);
                SyntaxTree::TypeAlias {
                    name: Box::new(left),
                    type_params: None,
                    value: Box::new(right),
                    range, span,
                }
            } else {
                SyntaxTree::Unknown {
                    kind: format!("type_alias_statement(arity={})", kids.len()),
                    range, span,
                }
            }
        }

        // `*x` in calls / list literals — splat. tree-sitter:
        // list_splat with single named child = the splatted expr.
        "list_splat" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => SyntaxTree::ListSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => SyntaxTree::Unknown {
                    kind: "list_splat(empty)".to_string(),
                    range, span,
                },
            }
        }

        // `**x` in calls / dict literals.
        "dictionary_splat" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => SyntaxTree::DictSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => SyntaxTree::Unknown {
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
                (Some(nn), Some(vv)) => SyntaxTree::KeywordArgument {
                    name: Box::new(name_of(nn, source)),
                    value: Box::new(lower_node(vv, source)),
                    range, span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "keyword_argument(missing field)".to_string(),
                    range, span,
                },
            }
        }

        // Python ternary: `a if cond else b`. tree-sitter exposes
        // children positionally (no field labels): if_true, condition,
        // if_false in source order.
        "conditional_expression" => {
            let kids: Vec<&RawNode> = node.named_children().collect();
            if kids.len() == 3 {
                SyntaxTree::Ternary {
                    if_true: Box::new(lower_node(kids[0], source).wrap_slot("then")),
                    condition: Box::new(lower_node(kids[1], source).wrap_slot("condition")),
                    if_false: Box::new(lower_node(kids[2], source).wrap_slot("else")),
                    range, span,
                }
            } else {
                SyntaxTree::Unknown {
                    kind: format!("conditional_expression(arity={})", kids.len()),
                    range, span,
                }
            }
        }

        "while_statement" => {
            let cond = node.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source).wrap_slot("condition")));
            let body = node.child_by_field_name("body").map(|n| Box::new(lower_block(n, source)));
            let alt = node.child_by_field_name("alternative");
            let else_body = alt.map(|a| {
                let inner = a.child_by_field_name("body").unwrap_or(a);
                Box::new(lower_block(inner, source))
            });
            match (cond, body) {
                (Some(c), Some(b)) => SyntaxTree::While { condition: c, body: b, else_body, range, span },
                _ => SyntaxTree::Unknown { kind: "while_statement(missing field)".to_string(), range, span },
            }
        }

        "break_statement" => SyntaxTree::Break { range, span },
        "continue_statement" => SyntaxTree::Continue { range, span },

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
            let mut decorators: Vec<SyntaxTree> = Vec::new();
            let mut inner: Option<&RawNode> = None;
            for c in node.named_children() {
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
                None => SyntaxTree::Unknown { kind: "decorated_definition(empty)".to_string(), range, span },
            }
        }

        // `return [value]`
        "return_statement" => {
            let value = node.named_children().next();
            SyntaxTree::Return {
                value: value.map(|v| Box::new(lower_node(v, source).wrap_expression_inline_aware())),
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
        // pass we'd need its own handling — to be added when a test surfaces it.
        // For now, treat as Unknown (won't fire because pass-only
        // bodies are caught in lower_block).
        "pass_statement" => SyntaxTree::Unknown {
            kind: "pass_statement".to_string(),
            range,
            span,
        },

        // `# comment text`. Default to neither leading nor trailing
        // — the `merge_python_line_comments` post-pass on each block
        // classifies based on adjacency to the next/prev non-comment
        // sibling.
        "comment" => SyntaxTree::Comment { trailing: false, leading: false, range, span },

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
            SyntaxTree::Assign {
                targets: match left {
                    Some(n) => lower_assign_side(n, source).into_iter().map(SyntaxTree::wrap_expression).collect(),
                    None => vec![],
                },
                type_annotation: type_ann.map(|t| Box::new(lower_type_slot(t, source))),
                op_text,
                op_range,
                op_markers: Vec::new(),
                values: match right {
                    Some(n) => lower_assign_side(n, source).into_iter().map(SyntaxTree::wrap_expression).collect(),
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
            SyntaxTree::Assign {
                targets: match left {
                    Some(n) => lower_assign_side(n, source).into_iter().map(SyntaxTree::wrap_expression).collect(),
                    None => vec![],
                },
                type_annotation: None,
                op_text,
                op_range,
                op_markers,
                values: match right {
                    Some(n) => lower_assign_side(n, source).into_iter().map(SyntaxTree::wrap_expression).collect(),
                    None => vec![],
                },
                range,
                span,
            }
        }

        // ----- Imports --------------------------------------------------

        // `import os` / `import sys as system` / `import a, b`
        "import_statement" => {
            let mut children: Vec<SyntaxTree> = Vec::new();
            let mut has_alias = false;
            for c in node.named_children() {
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
            SyntaxTree::Import { has_alias, children, range, span }
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
                    let inner_path = m.named_children()
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
            let mut imports: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                let same_as_module = module_name.map(|m| m.id()) == Some(c.id());
                if same_as_module { continue; }
                match c.kind() {
                    "dotted_name" => {
                        // For `from x import y`, the imported name is a
                        // single-segment dotted_name. We unwrap to a
                        // bare `SyntaxTree::Name`.
                        let name_node = lower_dotted_first_name(c, source);
                        imports.push(SyntaxTree::FromImport {
                            has_alias: false,
                            name: Box::new(name_node),
                            alias: None,
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                    "aliased_import" => {
                        let (n, a) = lower_aliased_from(c, source);
                        imports.push(SyntaxTree::FromImport {
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
                        // `SyntaxTree::FromImport` with a synthetic Name
                        // covering `*`.
                        imports.push(SyntaxTree::FromImport {
                            has_alias: false,
                            name: Box::new(name_of(c, source)),
                            alias: None,
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                    _ => {
                        imports.push(SyntaxTree::Unknown {
                            kind: c.kind().to_string(),
                            range: range_of(c),
                            span: span_of(c),
                        });
                    }
                }
            }

            SyntaxTree::From { relative, path, imports, range, span }
        }

        // `dotted_name` outside of import context. Default lowering as
        // a Path. Specific call sites (import, from) use
        // `lower_dotted_as_path` directly.
        "dotted_name" => lower_dotted_as_path(node, source),

        // Atoms — leaf-level value carriers. Text is `source[range]`.
        "identifier" => name_of(node, source),

        // tree-sitter-python wraps type-position expressions in a
        // `type` node (parameter annotations, return types, generic
        // arguments). The tree's typed shape has no `<type>` wrapping
        // here — the renderer adds it when emitting via a
        // `type_ann`/`Returns` slot. Unwrap to the inner expression
        // and lower it directly.
        "type" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => lower_node(i, source),
                None => SyntaxTree::Unknown { kind: "type(empty)".to_string(), range, span },
            }
        }

        // `expression_list` / `pattern_list` are pure grouping nodes
        // in tree-sitter-python (tuple-without-parens, used in
        // `return a, b`, `for a, b in xs`, `a, b = pair`). The
        // imperative pipeline flattens both via Rule::Flatten so the
        // items become direct children of the enclosing
        // `<return>`/`<for>`/`<assign>`. Mirror via SyntaxTree::Inline.
        "expression_list" | "pattern_list" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }

        // `constrained_type` (`T: Bound`) — SyntaxTree::TypeParameter shape.
        "constrained_type" => {
            let kids: Vec<&RawNode> = node.named_children().collect();
            if kids.len() >= 2 {
                SyntaxTree::TypeParameter {
                    name: Box::new(lower_node(kids[0], source)),
                    constraint: Some(Box::new(lower_node(kids[1], source))),
                    range, span,
                }
            } else if kids.len() == 1 {
                SyntaxTree::TypeParameter {
                    name: Box::new(lower_node(kids[0], source)),
                    constraint: None,
                    range, span,
                }
            } else {
                SyntaxTree::Unknown { kind: "constrained_type(empty)".to_string(), range, span }
            }
        }

        // `splat_type` (`*Ts`) — variadic generic. Lower as a marked
        // type whose source bytes carry the `*`.
        "splat_type" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => SyntaxTree::ListSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => SyntaxTree::Unknown { kind: "splat_type(empty)".to_string(), range, span },
            }
        }

        // `list_splat_pattern` (`*rest`) — splat in a parameter list
        // or assignment LHS.
        "list_splat_pattern" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => SyntaxTree::ListSplat {
                    inner: Box::new(lower_node(i, source)),
                    range, span,
                },
                None => SyntaxTree::Unknown { kind: "list_splat_pattern(empty)".to_string(), range, span },
            }
        }

        // `EXPR as NAME` — tree-sitter `as_pattern` kind. Lowers to
        // `<as>` element (matches imperative `Rename(As)`). Walks
        // named children so the inner expression and `as_pattern_target`
        // both surface.
        "as_pattern" => simple_statement(node, "as", source),
        // `as_pattern_target` is the binding-side name in `as NAME` —
        // imperative pipeline wraps it. For the tree we just unwrap and
        // recurse.
        "as_pattern_target" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => lower_node(i, source),
                None => SyntaxTree::Unknown { kind: "as_pattern_target(empty)".to_string(), range, span },
            }
        }

        // `await x` in Python — tree-sitter exposes as `await` kind
        // with the expression as a named child. Lower as SyntaxTree::Expression
        // with an `await` marker (matches the imperative pipeline's
        // `<expression[await]>` shape via Principle #15).
        "await" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => SyntaxTree::Expression {
                    inner: Box::new(lower_node(i, source)),
                    marker: Some("await"),
                    range,
                    span,
                },
                None => SyntaxTree::Unknown { kind: "await(empty)".to_string(), range, span },
            }
        }

        // tree-sitter sometimes exposes a bare `block` outside the
        // function/class body's normal slot (e.g. inside `try` /
        // `with`). Lower as SyntaxTree::Body without block_wrap.
        "block" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Body {
                children,
                pass_only: false,
                block_wrap: false,
                range,
                span,
            }
        }
        "integer"    => int_of(node, source),
        "float"      => float_of(node, source),
        // tree-sitter-python: `string` always has named children
        // (`string_start`, `string_content`, `string_end`, plus
        // optional `interpolation`/`escape_sequence`). Plain strings
        // (just delimiters + literal text) stay scalar leaves. Only
        // when there's an `interpolation` or `escape_sequence` child
        // do we lift to a SimpleStatement so the structured chunk
        // survives.
        "string"     => {
            let has_structured = node.named_children().any(|c| {
                matches!(c.kind(), "interpolation" | "escape_sequence")
            });
            if has_structured {
                let children: Vec<SyntaxTree> = node.named_children()
                    .filter(|c| matches!(c.kind(), "interpolation" | "escape_sequence"))
                    .map(|c| lower_node(c, source))
                    .collect();
                SyntaxTree::SimpleStatement {
                    element_name: "string",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children,
                    range,
                    span,
                }
            } else {
                string_of(node, source)
            }
        }
        "true"       => true_of(node, source),
        "false"      => false_of(node, source),
        "none"       => none_of(node, source),

        other => SyntaxTree::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// Lower a type-argument expression in a generic type. Wraps in a
/// `<type>` element by emitting the inner expression — the renderer
/// for `SyntaxTree::GenericType` handles the actual `<type>` wrapping.
fn lower_type_arg(node: &RawNode, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

/// Lower a keyword-prefixed simple statement (`assert`, `raise`,
/// `delete`, `global`, `nonlocal`, `yield`) to `SyntaxTree::SimpleStatement`.
/// Children are the CST's named children, lowered recursively.
/// Python statements don't carry modifiers; the field is empty.
fn simple_statement(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let children: Vec<SyntaxTree> = node.named_children()
        .map(|c| lower_node(c, source))
        .collect();
    SyntaxTree::SimpleStatement { element_name, modifiers: Modifiers::default(), extra_markers: Vec::new(), children, range, span }
}

/// Lower a Python `raise X` / `raise X from Y` so the raised
/// expression sits under an `<expression>` host (Principle #5,
/// matches `<yield>` / `<return>`). The `from` cause stays as a
/// trailing bare child for now — distinguishing it from the
/// primary expression cleanly would need a `<from>` slot, which
/// is a separate iter.
fn lower_python_raise(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut children: Vec<SyntaxTree> = Vec::new();
    let mut first = true;
    for c in node.named_children() {
        let inner = lower_node(c, source);
        if first {
            // Wrap the raised expression in `<expression>` — same
            // shape `<return>` and `<yield>` use for their operands.
            let cr = inner.range();
            let cs = inner.span();
            children.push(SyntaxTree::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner],
                range: cr,
                span: cs,
            });
            first = false;
        } else {
            children.push(inner);
        }
    }
    SyntaxTree::SimpleStatement {
        element_name: "raise",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range,
        span,
    }
}

/// Lower a Python `yield expr` / `yield from expr` so the yielded
/// expression sits under an `<expression>` host (matching `<return>`
/// — Principle #5). Without the wrap, `<yield><name>x</name></yield>`
/// diverges from the equivalent return shape
/// `<return><expression><name>x</name></expression></return>`.
///
/// The bare keyword form (`yield`, no operand) emits an empty
/// `<yield>` with no expression child — the empty-element pass
/// folds it into a marker on the parent statement.
fn lower_python_yield(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let is_from = node.children().any(|c| !c.is_named() && c.kind() == "from");
    let inner: Vec<SyntaxTree> = node.named_children()
        .map(|c| lower_node(c, source))
        .collect();
    let children: Vec<SyntaxTree> = if inner.is_empty() {
        Vec::new()
    } else {
        // Wrap the operand(s) in `<expression>` (mirrors `<return>`'s
        // post-pass-driven shape). The combined range covers all
        // operands so source-text reconstruction stays accurate.
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
        element_name: "yield",
        modifiers: Modifiers::default(),
        extra_markers: if is_from { vec![Marker::implicit("from")] } else { Vec::new() },
        children,
        range,
        span,
    }
}

/// Same as `simple_statement` but adds explicit `<marker/>` siblings
/// (e.g. `<async/>` on `async with`).
fn simple_statement_marked(
    node: &RawNode,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let children: Vec<SyntaxTree> = node.named_children()
        .map(|c| lower_node(c, source))
        .collect();
    SyntaxTree::SimpleStatement { element_name, modifiers: Modifiers::default(), extra_markers, children, range, span }
}

/// Lower a Python `except_clause` to `SyntaxTree::ExceptHandler` with
/// kind="except". Structure: `except [Type [as Name]]: body`.
/// tree-sitter exposes positional children (no fields):
///   - optional type expression
///   - optional `as_pattern` for `as Name`
///   - the body block
fn lower_python_except_clause(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut type_target: Option<Box<SyntaxTree>> = None;
    let mut binding: Option<Box<SyntaxTree>> = None;
    let mut body: Option<Box<SyntaxTree>> = None;
    for c in node.named_children() {
        match c.kind() {
            "block" if body.is_none() => {
                body = Some(Box::new(lower_block(c, source)));
            }
            "as_pattern" => {
                // `Type as Name` — first child is the type, then
                // `as_pattern_target` containing the name.
                let kids: Vec<&RawNode> = c.named_children().collect();
                if let Some(t) = kids.first() {
                    if type_target.is_none() {
                        type_target = Some(Box::new(lower_node(*t, source)));
                    }
                }
                if kids.len() >= 2 {
                    let last = kids[kids.len() - 1];
                    let inner = if last.kind() == "as_pattern_target" {
                        let n = last.named_children().next();
                        n.unwrap_or(last)
                    } else { last };
                    binding = Some(Box::new(name_of(inner, source)));
                }
            }
            _ if type_target.is_none() && body.is_none() => {
                // First non-block, non-as_pattern child: the type.
                type_target = Some(Box::new(lower_node(c, source)));
            }
            _ => {}
        }
    }
    SyntaxTree::ExceptHandler {
        kind: "except",
        type_target,
        binding,
        filter: None,
        body: body.unwrap_or_else(|| Box::new(SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span })),
        range, span,
    }
}

/// Lower a single `else_clause` or `elif_clause` and chain the
/// already-built `tail` (the next sibling in the if-chain) into its
/// `else_branch` slot.
fn lower_else_chain_with_tail(
    node: &RawNode,
    tail: Option<Box<SyntaxTree>>,
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        "elif_clause" => {
            let cond = node.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source).wrap_slot("condition")));
            let body = node.child_by_field_name("consequence").map(|n| Box::new(lower_block(n, source)));
            match (cond, body) {
                (Some(c), Some(b)) => SyntaxTree::ElseIf {
                    condition: c,
                    body: b,
                    else_branch: tail,
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown { kind: "elif_clause(missing)".to_string(), range, span },
            }
        }
        "else_clause" => {
            let body = node.child_by_field_name("body").map(|n| Box::new(lower_block(n, source)));
            match body {
                Some(b) => SyntaxTree::Else { body: b, range, span },
                None => SyntaxTree::Unknown { kind: "else_clause(missing body)".to_string(), range, span },
            }
        }
        _ => SyntaxTree::Unknown { kind: format!("else_chain({})", node.kind()), range, span },
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

/// Lower a `function_definition` CST node into `SyntaxTree::Function`.
///
/// When called from a `decorated_definition` wrapper, the caller
/// passes `outer_range` covering the whole decorated declaration so
/// the decorators (whose source positions precede `def`) fall inside
/// the function's range and gap rendering works.
fn lower_function(node: &RawNode, source: &str, is_async: bool, decorators: Vec<SyntaxTree>) -> SyntaxTree {
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
            let r = node.named_children().find(|n| n.kind() == "type_parameter");
            r
        });

    let name = match name_node {
        Some(n) => Box::new(name_of(n, source)),
        None => Box::new(SyntaxTree::Unknown {
            kind: "function(missing name)".to_string(),
            range,
            span,
        }),
    };

    let generics: Vec<SyntaxTree> = type_params_node.map(|tp| lower_type_parameters(tp, source)).unwrap_or_default();

    let parameters = params_node.map(|p| lower_parameters(p, source)).unwrap_or_default();

    let returns = return_type_node.map(|rt| Box::new(SyntaxTree::Returns {
        type_ann: Box::new(lower_type_slot(rt, source)),
        range: range_of(rt),
        span: span_of(rt),
    }));

    let body: Option<Box<SyntaxTree>> = body_node.map(|b| Box::new(lower_block(b, source)));

    SyntaxTree::Function {
        element_name: "function",
        modifiers: Modifiers {
            async_: crate::tree::types::Flag::from_bool(is_async),
            ..Modifiers::default()
        },
        decorators,
        name,
        generics,
        parameters,
        returns,
        throws: Vec::new(),
        body,
        range,
        span,
    }
}

/// Lower a `class_definition` CST node into `SyntaxTree::Class`. As with
/// `lower_function`, decorators expand the effective range backward.
fn lower_class(node: &RawNode, source: &str, decorators: Vec<SyntaxTree>) -> SyntaxTree {
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
            let r = node.named_children().find(|n| n.kind() == "type_parameter");
            r
        });

    let name = match name_node {
        Some(n) => Box::new(name_of(n, source)),
        None => Box::new(SyntaxTree::Unknown {
            kind: "class(missing name)".to_string(),
            range,
            span,
        }),
    };

    let generics: Vec<SyntaxTree> = type_params_node.map(|tp| lower_type_parameters(tp, source)).unwrap_or_default();

    let bases: Vec<SyntaxTree> = superclasses_node.map(|s| {
        s.named_children().map(|n| lower_node(n, source).wrap_extends()).collect()
    }).unwrap_or_default();

    let mut body = match body_node {
        Some(b) => Box::new(lower_block(b, source)),
        None => Box::new(SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span }),
    };
    // Inject Python name-convention visibility (`__x__` public,
    // `__x` private, `_x` protected, `x` public) onto direct
    // function children. Replaces `inject_python_visibility_markers`
    // post-walk; encoded at lowering time so the SyntaxTree::Function carries
    // the access modifier and the renderer emits the marker via
    // `Modifiers::marker_names`.
    set_python_class_member_visibility(&mut body, source);

    SyntaxTree::Class {
        kind: "class",
        // Python: no access modifiers, no static/abstract/etc on class.
        modifiers: Modifiers::default(),
        decorators, name, generics, bases, where_clauses: Vec::new(), body, range, span,
    }
}

/// Walk a class body's direct children, setting `SyntaxTree::Function.modifiers.access`
/// based on Python name conventions. Only applies to direct
/// children (Principle #9 — visibility scope is the immediate class
/// body, not nested functions).
fn set_python_class_member_visibility(body: &mut SyntaxTree, source: &str) {
    let children = match body {
        SyntaxTree::Body { children, .. } => children,
        _ => return,
    };
    for child in children {
        // Decorated functions wrap SyntaxTree::Function in SyntaxTree::Decorated; reach into it.
        let target: &mut SyntaxTree = match child {
            SyntaxTree::Function { .. } => child,
            _ => continue,
        };
        let SyntaxTree::Function { modifiers, name, .. } = target else { continue };
        if modifiers.access.is_some() {
            continue;
        }
        let name_text = match name.as_ref() {
            SyntaxTree::Name { text, .. } => text.as_str(),
            _ => continue,
        };
        let access = if name_text.starts_with("__") && name_text.ends_with("__") && name_text.len() > 4 {
            Access::Public
        } else if name_text.starts_with("__") {
            Access::Private
        } else if name_text.starts_with('_') {
            Access::Protected
        } else {
            Access::Public
        };
        modifiers.access = Some(access);
    }
}

/// Lower a `parameters` CST node — a parenthesized list of parameter
/// kinds. Returns a flat Vec of `SyntaxTree::Parameter` /
/// `SyntaxTree::PositionalSeparator` / `SyntaxTree::KeywordSeparator` in source
/// order.
fn lower_parameters(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    let mut out: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        let span = span_of(c);
        let range = range_of(c);
        match c.kind() {
            "identifier" => {
                out.push(SyntaxTree::Parameter {
                    kind: ParamKind::Regular, extra_markers: Vec::new(),
                    modifiers: Modifiers::default(),
                    name: Box::new(name_of(c, source)),
                    type_ann: None,
                    default: None,
                    range,
                    span,
                });
            }
            "default_parameter" => {
                let n = c.child_by_field_name("name");
                let v = c.child_by_field_name("value");
                out.push(SyntaxTree::Parameter {
                    kind: ParamKind::Regular, extra_markers: Vec::new(),
                    modifiers: Modifiers::default(),
                    name: Box::new(match n {
                        Some(n) => name_of(n, source),
                        None => SyntaxTree::Unknown { kind: "default_parameter(no name)".to_string(), range, span },
                    }),
                    type_ann: None,
                    default: v.map(|n| Box::new(lower_node(n, source))),
                    range,
                    span,
                });
            }
            "typed_parameter" => {
                let mut name_n: Option<&RawNode> = None;
                for ch in c.named_children() {
                    if ch.kind() == "identifier" { name_n = Some(ch); break; }
                }
                let type_n = c.child_by_field_name("type");
                out.push(SyntaxTree::Parameter {
                    kind: ParamKind::Regular, extra_markers: Vec::new(),
                    modifiers: Modifiers::default(),
                    name: Box::new(match name_n {
                        Some(n) => name_of(n, source),
                        None => SyntaxTree::Unknown { kind: "typed_parameter(no name)".to_string(), range, span },
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
                out.push(SyntaxTree::Parameter {
                    kind: ParamKind::Regular, extra_markers: Vec::new(),
                    modifiers: Modifiers::default(),
                    name: Box::new(match n {
                        Some(n) => name_of(n, source),
                        None => SyntaxTree::Unknown { kind: "typed_default_parameter(no name)".to_string(), range, span },
                    }),
                    type_ann: t.map(|t| Box::new(lower_type_slot(t, source))),
                    default: v.map(|n| Box::new(lower_node(n, source))),
                    range,
                    span,
                });
            }
            "list_splat_pattern" => {
                // *args — has one named child (identifier).
                let inner = c.named_children().next();
                let name = match inner {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown { kind: "list_splat(no name)".to_string(), range, span },
                };
                out.push(SyntaxTree::Parameter {
                    kind: ParamKind::Args, extra_markers: Vec::new(),
                    modifiers: Modifiers::default(),
                    name: Box::new(name),
                    type_ann: None,
                    default: None,
                    range,
                    span,
                });
            }
            "dictionary_splat_pattern" => {
                // **kwargs
                let inner = c.named_children().next();
                let name = match inner {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown { kind: "dict_splat(no name)".to_string(), range, span },
                };
                out.push(SyntaxTree::Parameter {
                    kind: ParamKind::Kwargs, extra_markers: Vec::new(),
                    modifiers: Modifiers::default(),
                    name: Box::new(name),
                    type_ann: None,
                    default: None,
                    range,
                    span,
                });
            }
            "positional_separator" => {
                out.push(SyntaxTree::PositionalSeparator { range, span });
            }
            "keyword_separator" => {
                out.push(SyntaxTree::KeywordSeparator { range, span });
            }
            other => {
                out.push(SyntaxTree::Unknown {
                    kind: format!("parameter({other})"),
                    range,
                    span,
                });
            }
        }
    }
    out
}

/// Lower the type-parameter list `[T, U: bound, *Ts]` into `SyntaxTree::Generic`.
///
/// The CST passed in is whatever the function/class field
/// `type_parameters` returns. tree-sitter Python often wraps the list
/// in a single `type_parameter` node containing inner `type_parameter`
/// items (PEP 695 grammar quirk). We handle both single-level and
/// nested cases by walking ALL named descendants until we find
/// identifiers, splats, or constrained types — each becomes one
/// tree::TypeParameter.
fn lower_type_parameters(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    let _span = span_of(node);
    let _range = range_of(node);
    let mut items: Vec<SyntaxTree> = Vec::new();
    collect_type_param_items(node, source, &mut items);
    items
}

fn collect_type_param_items(node: &RawNode, source: &str, out: &mut Vec<SyntaxTree>) {
    for c in node.named_children() {
        let cspan = span_of(c);
        let crange = range_of(c);
        let tp: SyntaxTree = match c.kind() {
            // Per-item wrapper kind: `type` containing the
            // identifier (and optional constraint).
            "type" => {
                let inner = c.named_children().next();
                let name = match inner {
                    Some(n) if n.kind() == "identifier" =>
                        name_of(n, source),
                    Some(n) => lower_node(n, source),
                    None => SyntaxTree::Unknown {
                        kind: "type_param(empty type)".to_string(),
                        range: crange,
                        span: cspan,
                    },
                };
                SyntaxTree::TypeParameter {
                    name: Box::new(name),
                    constraint: None,
                    range: crange,
                    span: cspan,
                }
            }
            "identifier" => SyntaxTree::TypeParameter {
                name: Box::new(name_of(c, source)),
                constraint: None,
                range: crange,
                span: cspan,
            },
            "constrained_type" | "splat_type" => SyntaxTree::TypeParameter {
                name: Box::new(lower_node(c, source)),
                constraint: None,
                range: crange,
                span: cspan,
            },
            "type_parameter" => {
                // Nested wrapper (PEP 695 grammar quirk). Recurse to
                // collect items inside (each recursive call wraps in
                // <generic> itself, so don't double-wrap here).
                collect_type_param_items(c, source, out);
                continue;
            }
            other => SyntaxTree::Unknown {
                kind: format!("type_param_item({other})"),
                range: crange,
                span: cspan,
            },
        };
        // Wrap each TypeParameter in a `<generic>` SimpleStatement so
        // the rendered shape is `<generic>/<type>/<name>` (S16-Z16),
        // matching the pre-IR Python contract for PEP 695 type
        // parameter declarations.
        out.push(SyntaxTree::SimpleStatement {
            element_name: "generic",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![tp],
            range: crange,
            span: cspan,
        });
    }
}

/// Lower a `block` CST node (function/class body) into `SyntaxTree::Body`.
/// Recognises pass-only bodies (a single `pass_statement` child) and
/// renders them as `<body[pass]>` empty.
fn lower_block(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let named: Vec<&RawNode> = node.named_children().collect();
    let pass_only = named.len() == 1 && named[0].kind() == "pass_statement";
    let children: Vec<SyntaxTree> = if pass_only {
        Vec::new()
    } else {
        named.iter().filter(|n| n.kind() != "pass_statement").map(|n| lower_node(*n, source)).collect()
    };
    let children = merge_python_line_comments(children, source);
    SyntaxTree::Body { children, pass_only, block_wrap: false, range, span }
}

/// Group consecutive `#` line-comment children that sit on adjacent
/// lines into a single comment, then classify each as `trailing`
/// (same line as preceding code), `leading` (immediately precedes
/// the next non-comment sibling on the next line), or floating
/// (neither). Mirrors the imperative pipeline's `classify_and_group`.
fn merge_python_line_comments(children: Vec<SyntaxTree>, source: &str) -> Vec<SyntaxTree> {
    let mut out: Vec<SyntaxTree> = Vec::with_capacity(children.len());
    for child in children {
        if let SyntaxTree::Comment { leading, trailing, range, span } = child {
            let prev_non_comment = out.iter().rev()
                .find(|c| !matches!(c, SyntaxTree::Comment { .. }));
            let curr_is_trailing = prev_non_comment.map_or(false, |prev| {
                let prev_end = prev.range().end as usize;
                let between = &source[prev_end..range.start as usize];
                !between.contains('\n')
            });

            if let Some(SyntaxTree::Comment { range: prev_range, .. }) = out.last() {
                let gap = &source[prev_range.end as usize..range.start as usize];
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_is_line_comment = source[prev_range.start as usize..prev_range.end as usize]
                    .trim_start().starts_with('#');
                let curr_is_line_comment = source[range.start as usize..range.end as usize]
                    .trim_start().starts_with('#');
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: true, .. }));
                if only_one_newline && prev_is_line_comment && curr_is_line_comment
                    && !prev_was_trailing && !curr_is_trailing
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

    // Phase 2: mark `leading = true` iff next non-comment starts on
    // the very next line.
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
                if newlines == 1 && between.chars().all(|c| c.is_whitespace()) {
                    if let SyntaxTree::Comment { leading, .. } = &mut out[i] {
                        *leading = true;
                    }
                }
            }
        }
    }
    out
}

/// Lower a `decorator` CST node. The decorator's inner is the
/// expression being applied (a name, attribute, or call).
fn lower_decorator(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let inner = node.named_children().next();
    let inner_ir = match inner {
        Some(n) => lower_node(n, source),
        None => SyntaxTree::Unknown { kind: "decorator(empty)".to_string(), range, span },
    };
    SyntaxTree::Decorator { inner: Box::new(inner_ir), range, span }
}

/// Lower an assignment side (LHS targets or RHS values). If the node
/// is a multi-element pattern (pattern_list / tuple_pattern /
/// expression_list), each child becomes a separate Vec entry —
/// matching the existing pipeline's flat `<left><expression/>...</left>`
/// / `<right><expression/>...</right>` layout for multi-target /
/// multi-value assignments. Single-target / single-value cases return
/// a one-element vec.
fn lower_assign_side(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    match node.kind() {
        "pattern_list" | "tuple_pattern" | "expression_list" => {
            node.named_children().map(|n| lower_node(n, source)).collect()
        }
        _ => vec![lower_node(node, source)],
    }
}

/// Type-annotation slot lowering. tree-sitter Python wraps the type
/// expression in a `type` node; we unwrap and lower the inner.
fn lower_type_slot(node: &RawNode, source: &str) -> SyntaxTree {
    // The `type` field can be a `type` CST kind (with one named child)
    // or a bare expression. Unwrap if it's the wrapping `type` kind.
    let inner_node = if node.kind() == "type" {
        node.named_children().next().unwrap_or(node)
    } else {
        node
    };
    // PEP 604 union type `T | U`: tree-sitter Python parses this as a
    // `binary_operator` with `|`. In type position, lower it as
    // `<type[union]>/{lowered-children}` (Principle #11 — name the
    // construct concretely; the marker on `<type>` distinguishes the
    // union form from a regular type slot).
    if inner_node.kind() == "binary_operator" {
        if let Some(op) = inner_node.children().find(|c| !c.is_named()) {
            if op.utf8_text(source) == "|" {
                let span = span_of(inner_node);
                let range = range_of(inner_node);
                let left = inner_node.child_by_field_name("left");
                let right = inner_node.child_by_field_name("right");
                let mut children: Vec<SyntaxTree> = Vec::new();
                if let Some(l) = left {
                    children.push(lower_type_slot(l, source));
                }
                if let Some(r) = right {
                    children.push(lower_type_slot(r, source));
                }
                return SyntaxTree::SimpleStatement {
                    element_name: "type",
                    modifiers: Modifiers::default(),
                    extra_markers: vec![Marker::implicit("union")],
                    children,
                    range,
                    span,
                };
            }
        }
    }
    lower_node(inner_node, source).wrap_type()
}

/// Locate the `=` token inside a plain `assignment` CST node. tree-sitter
/// Python doesn't surface this as a named child, so we scan the source
/// between the left/type/right named children for the literal `=`. This
/// is enough for the source-text invariant — gap text covers all
/// non-token bytes between named children.
fn locate_assign_eq(
    node: &RawNode,
    source: &str,
    left: Option<&RawNode>,
    type_ann: Option<&RawNode>,
    right: Option<&RawNode>,
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

/// Lower a `dotted_name` CST node to `SyntaxTree::Path` with one
/// `SyntaxTree::Name` per segment. Single-segment dotted_names (`os`) become a
/// `Path` with one segment, matching the existing pipeline shape
/// (always wrap module paths in `<path>`).
fn lower_dotted_as_path(node: &RawNode, source: &str) -> SyntaxTree {
    let segments: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| name_of(c, source))
        .collect();
    SyntaxTree::Path {
        segments,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Lower a `dotted_name` to its first name segment as a bare `SyntaxTree::Name`
/// (used in `from x import y` where each imported name is a
/// dotted_name in the CST but renders as a bare `<name>` in the
/// existing pipeline). Falls back to `Unknown` if the dotted_name has
/// multiple segments (shouldn't happen for `from` imports).
fn lower_dotted_first_name(node: &RawNode, source: &str) -> SyntaxTree {
    let mut iter = node.named_children();
    if let Some(first) = iter.next() {
        name_of(first, source)
    } else {
        SyntaxTree::Unknown {
            kind: "dotted_name(empty)".to_string(),
            range: range_of(node),
            span: span_of(node),
        }
    }
}

/// Lower an `aliased_import` in *top-level* import context. Emits a
/// pair: (`SyntaxTree::Path`, `SyntaxTree::Aliased`) — both become flat children of
/// the enclosing `<import>`.
fn lower_aliased_top(node: &RawNode, source: &str) -> (SyntaxTree, SyntaxTree) {
    // tree-sitter Python: aliased_import has `name` field (dotted_name)
    // and `alias` field (identifier).
    let name_node = node.child_by_field_name("name");
    let alias_node = node.child_by_field_name("alias");
    let path = match name_node {
        Some(n) => lower_dotted_as_path(n, source),
        None => SyntaxTree::Unknown {
            kind: "aliased_import(missing name)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    let aliased = match alias_node {
        Some(a) => SyntaxTree::Aliased {
            inner: Box::new(name_of(a, source)),
            range: range_of(a),
            span: span_of(a),
        },
        None => SyntaxTree::Unknown {
            kind: "aliased_import(missing alias)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    (path, aliased)
}

/// Lower an `aliased_import` inside a `from X import` context. Emits a
/// pair: (bare `SyntaxTree::Name`, `SyntaxTree::Aliased`) — without `<path>` wrapping
/// on the imported name (that's how `from m import x as y` renders).
fn lower_aliased_from(node: &RawNode, source: &str) -> (SyntaxTree, SyntaxTree) {
    let name_node = node.child_by_field_name("name");
    let alias_node = node.child_by_field_name("alias");
    let name = match name_node {
        Some(n) if n.kind() == "dotted_name" => lower_dotted_first_name(n, source),
        Some(n) => name_of(n, source),
        None => SyntaxTree::Unknown {
            kind: "aliased_import(missing name)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    let aliased = match alias_node {
        Some(a) => SyntaxTree::Aliased {
            inner: Box::new(name_of(a, source)),
            range: range_of(a),
            span: span_of(a),
        },
        None => SyntaxTree::Unknown {
            kind: "aliased_import(missing alias)".to_string(),
            range: range_of(node),
            span: span_of(node),
        },
    };
    (name, aliased)
}

fn lower_children(parent: &RawNode, source: &str) -> Vec<SyntaxTree> {
    parent
        .named_children()
        .map(|c| lower_node(c, source))
        .collect()
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

