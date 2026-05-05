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

use super::types::{AccessSegment, ByteRange, Ir, ParamKind, Span};

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

        // `# comment text`
        "comment" => Ir::Comment {
            leading: false,
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
    let type_params_node = node.child_by_field_name("type_parameters");

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
        None => Box::new(Ir::Body {
            children: Vec::new(),
            pass_only: false,
            range: ByteRange::empty_at(range.end),
            span,
        }),
    };

    Ir::Function {
        is_async,
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
    let type_params_node = node.child_by_field_name("type_parameters");

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
        None => Box::new(Ir::Body {
            children: Vec::new(),
            pass_only: false,
            range: ByteRange::empty_at(range.end),
            span,
        }),
    };

    Ir::Class { decorators, name, generics, bases, body, range, span }
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
                    kind: ParamKind::Regular,
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
                    kind: ParamKind::Regular,
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
                    kind: ParamKind::Regular,
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
                    kind: ParamKind::Regular,
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
                    kind: ParamKind::Args,
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
                    kind: ParamKind::Kwargs,
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

/// Lower a `type_parameters` CST node into `Ir::Generic`. Each child
/// is a `type_parameter` (PEP 695) with an inner identifier.
fn lower_type_parameters(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let items: Vec<Ir> = node.named_children(&mut cursor).map(|c| {
        let cspan = span_of(c);
        let crange = range_of(c);
        match c.kind() {
            "type_parameter" => {
                let mut cur = c.walk();
                let inner = c.named_children(&mut cur).next();
                let name = match inner {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown { kind: "type_parameter(empty)".to_string(), range: crange, span: cspan },
                };
                Ir::TypeParameter {
                    name: Box::new(name),
                    constraint: None,
                    range: crange,
                    span: cspan,
                }
            }
            _ => Ir::Unknown { kind: format!("type_parameter({})", c.kind()), range: crange, span: cspan },
        }
    }).collect();
    Ir::Generic { items, range, span }
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
    Ir::Body { children, pass_only, range, span }
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
        _ => return None,
    })
}

fn text_of(node: TsNode<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .map(|s| s.to_string())
        .unwrap_or_default()
}
