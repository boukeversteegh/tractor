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

use super::types::{AccessSegment, ByteRange, Ir, Span};

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
                [single] => Ir::Expression {
                    inner: Box::new(lower_node(*single, source)),
                    range,
                    span,
                },
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

        // Atoms — leaf-level value carriers. Text is `source[range]`.
        "identifier" => Ir::Name { range, span },
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
        _ => return None,
    })
}

fn text_of(node: TsNode<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .map(|s| s.to_string())
        .unwrap_or_default()
}
