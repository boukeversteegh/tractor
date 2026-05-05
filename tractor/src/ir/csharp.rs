//! C# tree-sitter CST → IR lowering.
//!
//! Pure function. No global state, no in-place mutation. C# is the
//! whack-a-mole champion of the existing pipeline (86 commits, the
//! unsolved `?.` conditional-access design problem, the chain-inversion
//! adapter, plus operator-extraction quirks). A successful slice here
//! is strong evidence that the typed-IR architecture handles
//! cross-language reuse: most variants are shared with Python, with
//! C#-specific additions (e.g. `Ir::Null`) only where the construct
//! genuinely differs.
//!
//! ## Initial coverage
//! Atoms (identifier, literals, null), member access (single +
//! chained), subscript, calls, binary, unary. No statements, no
//! declarations yet — proves the IR vocabulary works for the
//! expression core before tackling C#'s syntactic surface.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{AccessSegment, ByteRange, Ir, Span};

/// Lower a C# tree-sitter root node to [`Ir`].
///
/// The root is `compilation_unit`. Anything else returns
/// [`Ir::Unknown`].
pub fn lower_csharp_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "compilation_unit" => Ir::Module {
            element_name: "unit",
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

/// Public entry point for lowering an arbitrary C# CST node — useful
/// for tests that want to lower a single expression without the
/// surrounding declaration scaffolding (which we haven't yet covered).
pub fn lower_csharp_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Conditional (`?.`) access -------------------------------
        //
        // tree-sitter-c-sharp models `a?.b` as
        //   conditional_access_expression
        //     <object-expression>
        //     member_binding_expression(.b)
        //       identifier(b)
        //
        // Architectural payoff: lower this to the SAME `Ir::Access`
        // shape as regular `.b`, with `optional: true` on the
        // segment. No special chain-inversion adapter, no
        // pre-pass to undo tree-sitter's structure, no
        // `<member[conditional]>` parent + `<condition>` wrapper —
        // just one extra marker on a uniform shape.
        //
        // This is the concrete answer to backlog 5d (todo/39…md):
        // the deferred C# design problem (`Root.MaybeProperty?.Property`
        // not isomorphic to `Root.MaybeProperty.Property`) ceases to
        // exist in the typed-IR world.
        "conditional_access_expression" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            if kids.len() != 2 {
                return Ir::Unknown {
                    kind: "conditional_access_expression(unexpected arity)".to_string(),
                    range, span,
                };
            }
            let object_node = kids[0];
            let binding_node = kids[1];
            let object_ir = lower_node(object_node, source);
            // Decode the binding into one or more access segments.
            // For `member_binding_expression(.b)` we get a single
            // Member segment with optional=true.
            let mut new_segments = lower_binding_to_segments(binding_node, source, true);
            // The first new segment's range should cover from end-of-object
            // through end-of-binding (so gap rendering picks up `?.`).
            if let Some(first) = new_segments.first_mut() {
                let new_start = object_ir.range().end;
                match first {
                    AccessSegment::Member { range, .. } => *range = ByteRange::new(new_start, range.end),
                    AccessSegment::Index { range, .. }  => *range = ByteRange::new(new_start, range.end),
                }
            }
            match object_ir {
                Ir::Access { receiver, mut segments, range: _, span: _ } => {
                    segments.extend(new_segments);
                    Ir::Access { receiver, segments, range, span }
                }
                other => Ir::Access {
                    receiver: Box::new(other),
                    segments: new_segments,
                    range,
                    span,
                },
            }
        }

        // C#-specific wrappers ------------------------------------------

        // `global_statement` wraps top-level statements in C# 9+. Just
        // unwrap to the inner. Existing pipeline handles this similarly.
        "global_statement" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(n) => lower_node(n, source),
                None => Ir::Unknown { kind: "global_statement(empty)".to_string(), range, span },
            }
        }

        // `expression_statement` — wrap in <expression> host
        // (Principle #15) when its inner is a value-producing
        // expression. Like Python, skip the wrap for assignments and
        // similar (added later when we have those variants).
        "expression_statement" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(n) => Ir::Expression {
                    inner: Box::new(lower_node(n, source)),
                    range,
                    span,
                },
                None => Ir::Unknown { kind: "expression_statement(empty)".to_string(), range, span },
            }
        }

        // tree-sitter ERROR nodes appear when the parser couldn't
        // recover. Common when feeding test snippets that aren't
        // valid C# at the top level. Pass children through as-is so
        // the structural view shows useful content.
        "ERROR" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node.named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            // Single child: unwrap (avoid double-nesting).
            if children.len() == 1 {
                children.into_iter().next().unwrap()
            } else {
                Ir::Inline { children, range, span }
            }
        }

        // ----- Atoms ----------------------------------------------------

        "identifier" => Ir::Name { range, span },
        "predefined_type" => Ir::Name { range, span }, // int / string / bool / etc.
        "integer_literal" => Ir::Int { range, span },
        "real_literal" => Ir::Float { range, span },
        // C# strings: `string_literal`, `verbatim_string_literal`,
        // `interpolated_string_text`. For the slice we treat all as
        // `Ir::String` with verbatim source text.
        "string_literal"
        | "verbatim_string_literal"
        | "raw_string_literal"
        | "character_literal" => Ir::String { range, span },
        "boolean_literal" => {
            // Distinguish true/false by source text.
            let t = range.slice(source);
            if t == "true" {
                Ir::True { range, span }
            } else {
                Ir::False { range, span }
            }
        }
        "null_literal" => Ir::Null { range, span },

        // ----- Member access (chain inversion via accumulation) ---------

        // C# member-access: `member_access_expression` with fields
        // `expression` (object) and `name` (the member identifier).
        "member_access_expression" => {
            let object_node = node.child_by_field_name("expression");
            let name_node = node.child_by_field_name("name");
            match (object_node, name_node) {
                (Some(object), Some(attr)) => {
                    let object_ir = lower_node(object, source);
                    let property_range = range_of(attr);
                    let property_span = span_of(attr);
                    let segment_range = ByteRange::new(
                        object_ir.range().end,
                        property_range.end,
                    );
                    let segment = AccessSegment::Member {
                        property_range,
                        property_span,
                        optional: false,
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
                    kind: "member_access_expression(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        // C# element access: `element_access_expression` with fields
        // `expression` and `subscript_arguments` (a `bracketed_argument_list`).
        "element_access_expression" => {
            let object_node = node.child_by_field_name("expression");
            let subscript_node = node.child_by_field_name("subscript_arguments");
            let indices: Vec<Ir> = match subscript_node {
                Some(s) => {
                    let mut c = s.walk();
                    s.named_children(&mut c)
                        .map(|n| {
                            // `argument` → unwrap to inner expression
                            if n.kind() == "argument" {
                                let mut cc = n.walk();
                                let inner = n.named_children(&mut cc).next();
                                inner.map(|i| lower_node(i, source))
                                    .unwrap_or_else(|| Ir::Unknown {
                                        kind: "argument(empty)".to_string(),
                                        range: range_of(n),
                                        span: span_of(n),
                                    })
                            } else {
                                lower_node(n, source)
                            }
                        })
                        .collect()
                }
                None => Vec::new(),
            };
            match object_node {
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
                    kind: "element_access_expression(missing object)".to_string(),
                    range,
                    span,
                },
            }
        }

        // ----- Calls ----------------------------------------------------

        // C#: `invocation_expression` with fields `function` and
        // `arguments` (`argument_list`).
        "invocation_expression" => {
            let function_node = node.child_by_field_name("function");
            let arguments_node = node.child_by_field_name("arguments");
            let callee = match function_node {
                Some(f) => lower_node(f, source),
                None => return Ir::Unknown {
                    kind: "invocation_expression(missing function)".to_string(),
                    range,
                    span,
                },
            };
            let arguments: Vec<Ir> = match arguments_node {
                Some(a) => {
                    let mut cursor = a.walk();
                    a.named_children(&mut cursor)
                        .map(|c| {
                            if c.kind() == "argument" {
                                let mut cc = c.walk();
                                let inner = c.named_children(&mut cc).next();
                                inner.map(|i| lower_node(i, source))
                                    .unwrap_or_else(|| Ir::Unknown {
                                        kind: "argument(empty)".to_string(),
                                        range: range_of(c),
                                        span: span_of(c),
                                    })
                            } else {
                                lower_node(c, source)
                            }
                        })
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

        // ----- Binary / unary ------------------------------------------

        // C# `binary_expression` has fields `left`, `operator`, `right`.
        "binary_expression" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (left, right, op_marker(&op_text)) {
                (Some(l), Some(r), Some(marker)) => Ir::Binary {
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l),
                    right: Box::new(r),
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "binary_expression(missing/unknown op)".to_string(),
                    range,
                    span,
                },
            }
        }

        // C# `prefix_unary_expression` has fields `operator` and
        // `operand`.
        "prefix_unary_expression" => {
            let operand = node.child_by_field_name("operand").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (operand, op_marker(&op_text)) {
                (Some(o), Some(marker)) => Ir::Unary {
                    op_text,
                    op_marker: marker,
                    op_range,
                    operand: Box::new(o),
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "prefix_unary_expression(missing/unknown op)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Fallback ------------------------------------------------------
        other => Ir::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// Decode the right side of a `conditional_access_expression` (the
/// `member_binding_expression` / `element_binding_expression`) into
/// access segments. `optional_first` controls whether the first
/// segment carries `<optional/>` — for `a?.b.c.d`, the binding
/// expression is `b.c.d` and only the first (`b`) is conditional.
///
/// member_binding_expression / element_binding_expression are
/// tree-sitter's representation of the part *after* `?.` — they
/// chain together using regular member_access_expression /
/// element_access_expression for the non-conditional steps.
fn lower_binding_to_segments(node: TsNode<'_>, source: &str, optional_first: bool) -> Vec<AccessSegment> {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        "member_binding_expression" => {
            // Single member segment from this binding.
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(name) => vec![AccessSegment::Member {
                    property_range: range_of(name),
                    property_span: span_of(name),
                    optional: optional_first,
                    range,
                    span,
                }],
                None => Vec::new(),
            }
        }
        "element_binding_expression" => {
            // `?[idx]` form. Lower the inner argument list.
            let arg_list = node.child_by_field_name("subscript_arguments");
            let indices = match arg_list {
                Some(a) => {
                    let mut c = a.walk();
                    a.named_children(&mut c).map(|n| {
                        if n.kind() == "argument" {
                            let mut cc = n.walk();
                            let inner = n.named_children(&mut cc).next();
                            inner.map(|i| lower_node(i, source))
                                .unwrap_or_else(|| Ir::Unknown {
                                    kind: "argument(empty)".to_string(),
                                    range: range_of(n),
                                    span: span_of(n),
                                })
                        } else { lower_node(n, source) }
                    }).collect()
                }
                None => Vec::new(),
            };
            // Index segment doesn't currently support optional — but
            // we tag the eventual IR variant with optionality on the
            // PARENT chain. For this slice we wire it through a
            // future `optional` field on Index when we add it. For
            // now, mark Member-style optional only.
            // TODO: extend AccessSegment::Index with an optional flag.
            vec![AccessSegment::Index { indices, range, span }]
        }
        // Tree-sitter sometimes nests further accesses inside the
        // binding (e.g. `?.b.c` becomes member_access(member_binding(b), c))
        // — handled by member_access_expression's own arm. For
        // unexpected kinds, fall back to a single Unknown-wrapped
        // segment.
        "member_access_expression" => {
            // Recurse: the inner is the member_binding (optional first
            // segment), and this access adds a non-optional segment.
            let object_node = node.child_by_field_name("expression");
            let name_node = node.child_by_field_name("name");
            let mut segments = match object_node {
                Some(o) => lower_binding_to_segments(o, source, optional_first),
                None => Vec::new(),
            };
            if let Some(name) = name_node {
                let property_range = range_of(name);
                let property_span = span_of(name);
                let last_end = segments.last().map(|s| match s {
                    AccessSegment::Member { range, .. } => range.end,
                    AccessSegment::Index { range, .. }  => range.end,
                }).unwrap_or(range.start);
                segments.push(AccessSegment::Member {
                    property_range,
                    property_span,
                    optional: false,  // chained `.x` after `?.` is regular
                    range: ByteRange::new(last_end, property_range.end),
                    span: span_of(node),
                });
            }
            segments
        }
        "element_access_expression" => {
            let object_node = node.child_by_field_name("expression");
            let subscript_node = node.child_by_field_name("subscript_arguments");
            let mut segments = match object_node {
                Some(o) => lower_binding_to_segments(o, source, optional_first),
                None => Vec::new(),
            };
            let indices = match subscript_node {
                Some(s) => {
                    let mut c = s.walk();
                    s.named_children(&mut c).map(|n| {
                        if n.kind() == "argument" {
                            let mut cc = n.walk();
                            let inner = n.named_children(&mut cc).next();
                            inner.map(|i| lower_node(i, source))
                                .unwrap_or_else(|| Ir::Unknown {
                                    kind: "argument(empty)".to_string(),
                                    range: range_of(n),
                                    span: span_of(n),
                                })
                        } else { lower_node(n, source) }
                    }).collect()
                }
                None => Vec::new(),
            };
            let last_end = segments.last().map(|s| match s {
                AccessSegment::Member { range, .. } => range.end,
                AccessSegment::Index { range, .. }  => range.end,
            }).unwrap_or(range.start);
            segments.push(AccessSegment::Index {
                indices,
                range: ByteRange::new(last_end, range.end),
                span: span_of(node),
            });
            segments
        }
        _ => {
            // Unhandled binding kind — preserve as a Member with the
            // whole node as the property (lossy but at least visible).
            vec![AccessSegment::Member {
                property_range: range,
                property_span: span,
                optional: optional_first,
                range,
                span,
            }]
        }
    }
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

fn text_of(node: TsNode<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Operator-marker map. Same names as Python's where the operators
/// match (Principle #5: same concept → same marker name); language-
/// specific operators get their own.
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
        _ => return None,
    })
}
