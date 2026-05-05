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

use super::types::{Access, AccessSegment, ByteRange, Ir, Modifiers, ParamKind, Span};

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
            children: merge_adjacent_line_comments(lower_children(root, source), source),
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
                    AccessSegment::Call { range, .. }   => *range = ByteRange::new(new_start, range.end),
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

        // ----- Parameter (single, inside parameter_list) -----------------
        //
        // tree-sitter-c-sharp: `parameter` with `type` and `name` field
        // children, plus optional `equals_value_clause` for default
        // values. Parameter modifiers (ref / out / in / params / this)
        // appear as `modifier` children — for now we lower as Regular
        // and let the modifier text fall into gap text.
        "parameter" => {
            let type_node = node.child_by_field_name("type");
            let name_node = node.child_by_field_name("name");
            let mut cursor = node.walk();
            let default_node = node.named_children(&mut cursor)
                .find(|c| c.kind() == "equals_value_clause");
            Ir::Parameter {
                kind: ParamKind::Regular,
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown {
                        kind: "parameter(missing name)".to_string(),
                        range, span,
                    },
                }),
                type_ann: type_node.map(|t| Box::new(lower_node(t, source))),
                default: default_node.and_then(|d| {
                    let mut c = d.walk();
                    let inner = d.named_children(&mut c).next();
                    inner.map(|n| Box::new(lower_node(n, source)))
                }),
                range, span,
            }
        }

        // `assignment_expression` — `x = value` / `x += value` etc.
        // Reuses Ir::Assign (same as Python). The assignment is
        // expression-level in C# but we model it the same way; the
        // wrapping `expression_statement` is bypassed for assignments
        // (handled below) so the rendered shape stays clean.
        "assignment_expression" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_text = node.child_by_field_name("operator")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let op_range = node.child_by_field_name("operator").map(range_of)
                .unwrap_or(ByteRange::empty_at(range.start));
            // Map op text to op_markers (same as Python).
            let op_markers: Vec<&'static str> = match op_text.as_str() {
                "=" => vec![],
                "+=" => vec!["assign", "plus"],
                "-=" => vec!["assign", "minus"],
                "*=" => vec!["assign", "multiply"],
                "/=" => vec!["assign", "divide"],
                "%=" => vec!["assign", "modulo"],
                "&=" => vec!["assign", "bitwise_and"],
                "|=" => vec!["assign", "bitwise_or"],
                "^=" => vec!["assign", "bitwise_xor"],
                "<<=" => vec!["assign", "shift_left"],
                ">>=" => vec!["assign", "shift_right"],
                "??=" => vec!["assign", "null_coalesce"],
                _ => vec!["assign"],
            };
            match (left, right) {
                (Some(l), Some(r)) => Ir::Assign {
                    targets: vec![l],
                    type_annotation: None,
                    op_text,
                    op_range,
                    op_markers,
                    values: vec![r],
                    range, span,
                },
                _ => Ir::Unknown {
                    kind: "assignment_expression(missing operand)".to_string(),
                    range, span,
                },
            }
        }

        // ----- Statements with simple structure -------------------------

        "comment" => Ir::Comment { leading: false, range, span },

        // `using System;` / `using static System.Math;` / `using A = B;`
        "using_directive" => {
            let mut cursor = node.walk();
            let mut is_static = false;
            let mut alias: Option<Box<Ir>> = None;
            let mut path_node: Option<TsNode> = None;
            for c in node.children(&mut cursor) {
                if !c.is_named() {
                    if let Ok(t) = c.utf8_text(source.as_bytes()) {
                        if t == "static" { is_static = true; }
                    }
                    continue;
                }
                match c.kind() {
                    "name_equals" => {
                        // `A =` part of `using A = B;`
                        let mut cc = c.walk();
                        let n = c.named_children(&mut cc).next();
                        if let Some(n) = n {
                            alias = Some(Box::new(Ir::Name { range: range_of(n), span: span_of(n) }));
                        }
                    }
                    _ => path_node = Some(c),
                }
            }
            let path_ir = match path_node {
                Some(n) => lower_node(n, source),
                None => Ir::Unknown {
                    kind: "using_directive(missing path)".to_string(),
                    range, span,
                },
            };
            Ir::Using {
                is_static,
                alias,
                path: Box::new(path_ir),
                range, span,
            }
        }

        // `qualified_name` (`System.Linq`) — same shape as Python's path.
        "qualified_name" => {
            // Flatten nested qualified_name (System.Collections.Generic
            // is parsed as nested pairs) into a single flat Ir::Path
            // with all name segments — matches imperative pipeline.
            let mut segments: Vec<Ir> = Vec::new();
            collect_qualified_name_segments(node, source, &mut segments);
            Ir::Path { segments, range, span }
        }

        // ----- Enums ----------------------------------------------------

        "enum_declaration" => {
            let name_node = node.child_by_field_name("name");
            let body_node = node.child_by_field_name("body");
            let modifiers = lower_csharp_modifiers(node, source, Some(Access::Internal));
            // Underlying type lives between the name and the body —
            // tree-sitter exposes it as a `base_list`'s child, or
            // sometimes a separate `_type` field. For minimal scope,
            // skip and let it appear in gap text.
            let members: Vec<Ir> = match body_node {
                Some(b) => {
                    let mut c = b.walk();
                    b.named_children(&mut c).map(|n| lower_node(n, source)).collect()
                }
                None => Vec::new(),
            };
            Ir::Enum {
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown { kind: "enum(missing name)".to_string(), range, span },
                }),
                underlying_type: None,
                members,
                range, span,
            }
        }

        "enum_member_declaration" => {
            let name_node = node.child_by_field_name("name");
            let value_node = node.child_by_field_name("value");
            Ir::EnumMember {
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown { kind: "enum_member(missing name)".to_string(), range, span },
                }),
                value: value_node.map(|n| Box::new(lower_node(n, source))),
                range, span,
            }
        }

        // ----- Properties ----------------------------------------------

        "property_declaration" => {
            let type_node = node.child_by_field_name("type");
            let name_node = node.child_by_field_name("name");
            let modifiers = lower_csharp_modifiers(node, source, Some(Access::Private));
            let mut cursor = node.walk();
            let mut accessors: Vec<Ir> = Vec::new();
            let mut value: Option<Box<Ir>> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "accessor_list" => {
                        let mut ac = c.walk();
                        for a in c.named_children(&mut ac) {
                            if a.kind() == "accessor_declaration" {
                                accessors.push(lower_accessor_declaration(a, source));
                            }
                        }
                    }
                    "arrow_expression_clause" => {
                        // Expression-bodied: `=> expr;`
                        let mut ac = c.walk();
                        let inner = c.named_children(&mut ac).next();
                        if let Some(i) = inner {
                            // Treat as a get-only accessor with body.
                            accessors.push(Ir::Accessor {
                                modifiers: Modifiers::default(),
                                kind: "get",
                                body: Some(Box::new(lower_node(i, source))),
                                range: range_of(c),
                                span: span_of(c),
                            });
                        }
                    }
                    _ => {}
                }
            }
            // Initializer expression (`int X { get; } = 42;`) — only
            // when distinct from any accessor we already added.
            // tree-sitter sometimes routes arrow_expression_clause's
            // inner via the `value` field too; guard against that
            // double-add by checking node identity.
            if let Some(v) = node.child_by_field_name("value") {
                let v_range = v.byte_range();
                let already_added = accessors.iter().any(|a| {
                    let r = a.range();
                    r.start as usize <= v_range.start && r.end as usize >= v_range.end
                });
                if !already_added {
                    value = Some(Box::new(lower_node(v, source)));
                }
            }
            Ir::Property {
                modifiers,
                decorators: Vec::new(),
                type_ann: type_node.map(|t| Box::new(lower_node(t, source))),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown { kind: "property(missing name)".to_string(), range, span },
                }),
                accessors,
                value,
                range, span,
            }
        }

        // ----- Constructors --------------------------------------------

        "constructor_declaration" => {
            let name_node = node.child_by_field_name("name");
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            let modifiers = lower_csharp_modifiers(node, source, Some(Access::Private));
            Ir::Constructor {
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown { kind: "constructor(missing name)".to_string(), range, span },
                }),
                parameters: lower_csharp_parameter_list(params_node, source),
                body: Box::new(match body_node {
                    Some(b) => lower_block_like(b, source),
                    None => Ir::Body {
                        children: Vec::new(),
                        pass_only: false,
                        range: ByteRange::empty_at(range.end),
                        span,
                    },
                }),
                range, span,
            }
        }

        // ----- Structural scaffolding (so recursion reaches expressions)

        // `namespace X { ... }` — block-scoped namespace. tree-sitter:
        // `namespace_declaration` with `name` field and a body
        // (declaration_list).
        "namespace_declaration" => {
            let name_node = node.child_by_field_name("name");
            let mut cursor = node.walk();
            let body_node = node.named_children(&mut cursor)
                .find(|c| c.kind() == "declaration_list");
            // For namespace, the imperative pipeline collapses a
            // qualified_name (`Tractor.Fixtures.Traditional`) into a
            // single `<name>` leaf with the full dotted text. Keep
            // parity by emitting Ir::Name with the qualified node's
            // whole range.
            let name_ir = match name_node {
                Some(n) if n.kind() == "qualified_name" || n.kind() == "identifier" => {
                    Ir::Name { range: range_of(n), span: span_of(n) }
                }
                Some(n) => lower_node(n, source),
                None => Ir::Unknown {
                    kind: "namespace(missing name)".to_string(),
                    range, span,
                },
            };
            let children: Vec<Ir> = match body_node {
                Some(b) => {
                    let mut c = b.walk();
                    let raw: Vec<Ir> = b.named_children(&mut c).map(|n| lower_node(n, source)).collect();
                    merge_adjacent_line_comments(raw, source)
                }
                None => Vec::new(),
            };
            Ir::Namespace { name: Box::new(name_ir), children, range, span }
        }

        // `class C { ... }` — name + body + full Modifiers.
        // Default access for top-level types: Internal.
        "class_declaration" | "struct_declaration" | "interface_declaration" | "record_declaration" => {
            let kind: &'static str = match node.kind() {
                "struct_declaration"    => "struct",
                "interface_declaration" => "interface",
                "record_declaration"    => "record",
                _                       => "class",
            };
            let name_node = node.child_by_field_name("name");
            let mut cursor = node.walk();
            let body_node = node.named_children(&mut cursor)
                .find(|c| c.kind() == "declaration_list");
            let modifiers = lower_csharp_modifiers(node, source, /*default_access*/ Some(Access::Internal));
            Ir::Class {
                kind,
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown {
                        kind: format!("{}(missing name)", kind),
                        range, span,
                    },
                }),
                generics: None,
                bases: Vec::new(),
                body: Box::new(match body_node {
                    Some(b) => lower_block_like(b, source),
                    None => Ir::Body {
                        children: Vec::new(),
                        pass_only: false,
                        range: ByteRange::empty_at(range.end),
                        span,
                    },
                }),
                range, span,
            }
        }

        // `int LocalFn(int x) => x * 2;` — local function inside a
        // method. Same shape as method_declaration except no access
        // modifiers (always private to the enclosing scope).
        "local_function_statement" => {
            let name_node = node.child_by_field_name("name");
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            let modifiers = lower_csharp_modifiers(node, source, None);
            // Body may be a block or `arrow_expression_clause` (for
            // `=>` form) — both lower correctly via lower_node /
            // lower_block_like.
            let body = match body_node {
                Some(b) if b.kind() == "block" => Box::new(lower_block_like(b, source)),
                Some(b) => {
                    // Arrow-bodied: wrap the inner expression in a
                    // synthetic Ir::Body covering the arrow clause's
                    // range so the renderer treats it consistently.
                    let r = range_of(b);
                    let s = span_of(b);
                    Box::new(Ir::Body {
                        children: vec![lower_node(b, source)],
                        pass_only: false,
                        range: r, span: s,
                    })
                }
                None => Box::new(Ir::Body {
                    children: Vec::new(), pass_only: false,
                    range: ByteRange::empty_at(range.end), span,
                }),
            };
            Ir::Function {
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown {
                        kind: "local_function(missing name)".to_string(),
                        range, span,
                    },
                }),
                generics: None,
                parameters: lower_csharp_parameter_list(params_node, source),
                returns: None,
                body,
                range, span,
            }
        }

        // `[modifiers] returntype Name(params) { body }` — name +
        // body + parameters + full Modifiers. Default access for
        // class members: Private. Return type still deferred (would
        // need an Ir::Returns wrap; tree-sitter exposes it as a
        // sibling of `name` rather than a labelled field).
        "method_declaration" => {
            let name_node = node.child_by_field_name("name");
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            let modifiers = lower_csharp_modifiers(node, source, /*default_access*/ Some(Access::Private));
            Ir::Function {
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown {
                        kind: "method(missing name)".to_string(),
                        range, span,
                    },
                }),
                generics: None,
                parameters: lower_csharp_parameter_list(params_node, source),
                returns: None,
                body: Box::new(match body_node {
                    Some(b) => lower_block_like(b, source),
                    None => Ir::Body {
                        children: Vec::new(),
                        pass_only: false,
                        range: ByteRange::empty_at(range.end),
                        span,
                    },
                }),
                range, span,
            }
        }

        // `block` (method body, free-standing block in C#).
        "block" => lower_block_like(node, source),

        // ----- Control flow ---------------------------------------------
        //
        // C# allows non-block bodies (`if (c) stmt;`, `while (c) stmt;`).
        // Wrap single statements in a synthetic `Ir::Body` so the
        // shared rendering arms (which expect Body) work uniformly.
        "if_statement" => {
            let cond = node.child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source)));
            let body = node.child_by_field_name("consequence")
                .map(|n| Box::new(lower_csharp_consequence(n, source)));
            // The `else` part is exposed either as a child kind
            // `else_clause` (older grammars) or via a labelled field
            // in newer ones. Try both.
            let else_node = node.child_by_field_name("alternative").or_else(|| {
                let mut c = node.walk();
                let r = node.named_children(&mut c).find(|c| c.kind() == "else_clause");
                r
            });
            let else_branch = else_node.map(|a| Box::new(lower_csharp_else_chain(a, source)));
            match (cond, body) {
                (Some(c), Some(b)) => Ir::If {
                    condition: c,
                    body: b,
                    else_branch,
                    range, span,
                },
                _ => Ir::Unknown { kind: "if_statement(missing field)".to_string(), range, span },
            }
        }

        "while_statement" => {
            let cond = node.child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source)));
            let body = node.child_by_field_name("body")
                .map(|n| Box::new(lower_csharp_consequence(n, source)));
            match (cond, body) {
                (Some(c), Some(b)) => Ir::While {
                    condition: c, body: b, else_body: None, range, span,
                },
                _ => Ir::Unknown { kind: "while_statement(missing field)".to_string(), range, span },
            }
        }

        // `foreach (T x in collection) body` (and `await foreach`).
        // tree-sitter fields: `type`, `left` (the loop variable —
        // identifier or tuple_pattern), `right` (the collection),
        // `body`.
        "foreach_statement" => {
            let type_node = node.child_by_field_name("type");
            let left_node = node.child_by_field_name("left");
            let right_node = node.child_by_field_name("right");
            let body_node = node.child_by_field_name("body");
            match (left_node, right_node, body_node) {
                (Some(l), Some(r), Some(b)) => Ir::Foreach {
                    type_ann: type_node.map(|t| Box::new(lower_node(t, source))),
                    target: Box::new(lower_node(l, source)),
                    iterable: Box::new(lower_node(r, source)),
                    body: Box::new(lower_csharp_consequence(b, source)),
                    range, span,
                },
                _ => Ir::Unknown { kind: "foreach_statement(missing field)".to_string(), range, span },
            }
        }

        // `for (init; cond; update) body` — C-style. tree-sitter
        // fields: `initializer` (declaration or expression list, may
        // be missing), `condition` (expression, optional), `update`
        // (vec of expressions via repeated `update` field),
        // `body`. The semicolons live in gap text.
        "for_statement" => {
            let init_node = node.child_by_field_name("initializer");
            let cond_node = node.child_by_field_name("condition");
            let body_node = node.child_by_field_name("body");
            // tree-sitter exposes multiple `update` fields as
            // separate child_by_field_name lookups; use children_by_field_name.
            let mut update_cursor = node.walk();
            let updates: Vec<Ir> = node
                .children_by_field_name("update", &mut update_cursor)
                .map(|n| lower_node(n, source))
                .collect();
            match body_node {
                Some(b) => Ir::CFor {
                    initializer: init_node.map(|n| Box::new(lower_node(n, source))),
                    condition: cond_node.map(|n| Box::new(lower_node(n, source))),
                    updates,
                    body: Box::new(lower_csharp_consequence(b, source)),
                    range, span,
                },
                None => Ir::Unknown { kind: "for_statement(missing body)".to_string(), range, span },
            }
        }

        // `do body while(cond);`. tree-sitter fields: `body` and
        // `condition`. The `do`/`while` keywords + `;` live in gap
        // text.
        "do_statement" => {
            let body_node = node.child_by_field_name("body");
            let cond_node = node.child_by_field_name("condition");
            match (body_node, cond_node) {
                (Some(b), Some(c)) => Ir::DoWhile {
                    body: Box::new(lower_csharp_consequence(b, source)),
                    condition: Box::new(lower_node(c, source)),
                    range, span,
                },
                _ => Ir::Unknown { kind: "do_statement(missing field)".to_string(), range, span },
            }
        }

        "break_statement" => Ir::Break { range, span },
        "continue_statement" => Ir::Continue { range, span },

        // `return [value];`
        "return_statement" => {
            let mut cursor = node.walk();
            let value = node.named_children(&mut cursor).next();
            Ir::Return {
                value: value.map(|v| Box::new(lower_node(v, source))),
                range, span,
            }
        }


        // Simple keyword-prefixed statements / expressions whose old
        // pipeline rule is a plain Rename. Lowered to
        // Ir::SimpleStatement with the right element name.
        "yield_statement"     => simple_statement(node, "yield",     source),
        "lock_statement"      => simple_statement(node, "lock",      source),
        "goto_statement"      => simple_statement(node, "goto",      source),
        "labeled_statement"   => simple_statement(node, "label",     source),
        "checked_statement"   => simple_statement(node, "checked",   source),
        "checked_expression"  => simple_statement(node, "checked",   source),
        "typeof_expression"   => simple_statement(node, "typeof",    source),
        "default_expression"  => simple_statement(node, "default",   source),
        "sizeof_expression"   => simple_statement(node, "sizeof",    source),
        "delegate_declaration"          => simple_statement(node, "delegate",   source),
        "destructor_declaration"        => simple_statement(node, "destructor", source),
        "indexer_declaration"           => simple_statement(node, "indexer",    source),
        "event_field_declaration"       => simple_statement(node, "event",      source),
        "event_declaration"             => simple_statement(node, "event",      source),
        "conversion_operator_declaration" => simple_statement(node, "operator", source),
        "file_scoped_namespace_declaration" => simple_statement(node, "namespace", source),
        "operator_declaration"          => simple_statement(node, "operator",   source),
        "fixed_statement"               => simple_statement(node, "fixed",      source),
        "unsafe_statement"              => simple_statement(node, "unsafe",     source),
        "using_statement"               => simple_statement(node, "using",      source),
        "empty_statement"               => simple_statement(node, "empty",      source),
        "throw_statement"               => simple_statement(node, "throw",      source),
        "throw_expression"              => simple_statement(node, "throw",      source),
        "with_expression"               => simple_statement(node, "with",       source),
        "range_expression"              => simple_statement(node, "range",      source),
        "tuple_expression"              => simple_statement(node, "tuple",      source),
        "from_clause"                   => simple_statement(node, "from",       source),
        "where_clause"                  => simple_statement(node, "where",      source),
        "select_clause"                 => simple_statement(node, "select",     source),
        "order_by_clause"               => simple_statement(node, "order",      source),
        "join_clause"                   => simple_statement(node, "join",       source),
        "group_clause"                  => simple_statement(node, "group",      source),
        "let_clause"                    => simple_statement(node, "let",        source),
        "query_expression"              => simple_statement(node, "query",      source),
        "query_continuation"            => simple_statement(node, "query",      source),
        // Attributes
        "attribute"                     => simple_statement(node, "attribute",  source),
        "attribute_list"                => simple_statement(node, "attribute",  source),
        "attribute_argument"            => simple_statement(node, "argument",   source),
        "attribute_argument_list"       => simple_statement(node, "arguments",  source),
        "attribute_target_specifier"    => simple_statement(node, "target",     source),
        "global_attribute"              => simple_statement(node, "attribute",  source),
        // Generics & constraints
        "type_parameter_constraint"     => simple_statement(node, "constraint", source),
        "type_parameter_constraints_clause" => simple_statement(node, "where",  source),
        "constructor_constraint"        => simple_statement(node, "new",        source),
        // Patterns
        "positional_pattern_clause"     => simple_statement(node, "positional", source),
        "parenthesized_variable_designation" => simple_statement(node, "designation", source),
        // Pointer & function pointer types
        "pointer_type"                  => simple_statement(node, "type",       source),
        "function_pointer_parameter"    => simple_statement(node, "parameter",  source),
        // Misc
        "alias_qualified_name"          => simple_statement(node, "name",       source),
        "declaration_expression"        => simple_statement(node, "declaration",source),
        "extern_alias_directive"        => simple_statement(node, "import",     source),
        "primary_constructor_base_type" => simple_statement(node, "base",       source),
        "constructor_initializer"       => simple_statement(node, "initializer",source),
        "calling_convention"            => simple_statement(node, "calling",    source),
        "explicit_interface_specifier"  => simple_statement(node, "interface",  source),
        "preproc_if"                    => simple_statement(node, "preproc",    source),
        "preproc_else"                  => simple_statement(node, "preproc",    source),
        "preproc_elif"                  => simple_statement(node, "preproc",    source),
        "preproc_define"                => simple_statement(node, "preproc",    source),
        "preproc_endregion"             => simple_statement(node, "preproc",    source),
        "preproc_error"                 => simple_statement(node, "preproc",    source),
        "preproc_line"                  => simple_statement(node, "preproc",    source),
        "preproc_nullable"              => simple_statement(node, "preproc",    source),
        "preproc_pragma"                => simple_statement(node, "preproc",    source),
        "preproc_region"                => simple_statement(node, "preproc",    source),
        "preproc_undef"                 => simple_statement(node, "preproc",    source),
        "preproc_warning"               => simple_statement(node, "preproc",    source),
        "preproc_if_in_attribute_list"  => simple_statement(node, "preproc",    source),
        "ref_expression"                => simple_statement(node, "ref",        source),
        "shebang_directive"             => simple_statement(node, "shebang",    source),
        "member_binding_expression"     => simple_statement(node, "member",     source),
        "element_binding_expression"    => simple_statement(node, "index",      source),
        // Type kinds — render under <type> when used as standalone.
        "implicit_type"                 => simple_statement(node, "type",       source),
        "array_type"                    => simple_statement(node, "type",       source),
        "tuple_type"                    => simple_statement(node, "type",       source),
        "nullable_type"                 => simple_statement(node, "type",       source),
        "ref_type"                      => simple_statement(node, "type",       source),
        "scoped_type"                   => simple_statement(node, "type",       source),
        "function_pointer_type"         => simple_statement(node, "type",       source),
        // Array / collection creation — render as <new>.
        "array_creation_expression"          => simple_statement(node, "new",   source),
        "implicit_array_creation_expression" => simple_statement(node, "new",   source),
        "anonymous_object_creation_expression" => simple_statement(node, "new", source),
        "stackalloc_expression"              => simple_statement(node, "new",   source),
        "implicit_stackalloc_expression"     => simple_statement(node, "new",   source),
        // String interpolation — render as <string>.
        "interpolated_string_expression"     => simple_statement(node, "string",source),
        // Pattern-matching expressions / statements.
        "is_pattern_expression"              => simple_statement(node, "is",    source),
        "switch_statement"                   => simple_statement(node, "switch",source),
        "switch_expression"                  => simple_statement(node, "switch",source),
        "switch_expression_arm"              => simple_statement(node, "case",  source),
        "switch_section"                     => simple_statement(node, "case",  source),
        "with_initializer"                   => simple_statement(node, "with",  source),
        "interpolation"                      => simple_statement(node, "interpolation", source),
        // Pattern kinds — old pipeline uses RenameWithMarker(Pattern, X);
        // for parity-first we use plain "pattern" without a marker.
        "constant_pattern"          => simple_statement(node, "pattern", source),
        "declaration_pattern"       => simple_statement(node, "pattern", source),
        "recursive_pattern"         => simple_statement(node, "pattern", source),
        "relational_pattern"        => simple_statement(node, "pattern", source),
        "tuple_pattern"             => simple_statement(node, "pattern", source),
        "and_pattern"               => simple_statement(node, "pattern", source),
        "or_pattern"                => simple_statement(node, "pattern", source),
        "negated_pattern"           => simple_statement(node, "pattern", source),
        "list_pattern"              => simple_statement(node, "pattern", source),
        "var_pattern"               => simple_statement(node, "pattern", source),
        "type_pattern"              => simple_statement(node, "pattern", source),
        "property_pattern_clause"   => simple_statement(node, "properties", source),
        "subpattern"                => simple_statement(node, "subpattern", source),
        "discard"                   => simple_statement(node, "discard", source),
        "tuple_element"             => simple_statement(node, "element", source),
        "when_clause"               => simple_statement(node, "when", source),
        // type_parameter — `<generic>` with optional variance + name.
        "type_parameter" => simple_statement(node, "generic", source),

        // Flatten-only kinds — render as Inline so children promote
        // to the parent's element.
        "switch_body"
        | "bracketed_parameter_list"
        | "array_rank_specifier"
        | "interpolation_alignment_clause"
        | "interpolation_format_clause"
        | "parenthesized_pattern"
        | "interpolation_brace"
        | "interpolation_start"
        | "string_content"
        | "raw_string_content"
        | "raw_string_start"
        | "raw_string_end"
        | "interpolation_quote"
        | "string_literal_encoding"
        | "escape_sequence"
        | "character_literal_content"
        | "join_into_clause"
        | "type_parameter_list"
        | "parameter_list"
        | "type_argument_list"
        | "argument_list" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node.named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, range, span }
        }
        // Parenthesized expression: parens become gap text on parent.
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::Inline {
                    children: vec![lower_node(i, source)],
                    range, span,
                },
                None => Ir::Unknown {
                    kind: "parenthesized_expression(empty)".to_string(),
                    range, span,
                },
            }
        }

        // `try { body } catch (...) { ... } finally { ... }`.
        // tree-sitter children: a `block` (try body), then any number
        // of `catch_clause`s, optionally a `finally_clause`.
        "try_statement" => {
            let mut cursor = node.walk();
            let mut try_body: Option<Box<Ir>> = None;
            let mut handlers: Vec<Ir> = Vec::new();
            let mut finally_body: Option<Box<Ir>> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "block" if try_body.is_none() => {
                        try_body = Some(Box::new(lower_block_like(c, source)));
                    }
                    "catch_clause" => {
                        handlers.push(lower_csharp_catch_clause(c, source));
                    }
                    "finally_clause" => {
                        // finally_clause has a single block child.
                        let mut fc = c.walk();
                        let inner = c.named_children(&mut fc).find(|n| n.kind() == "block");
                        if let Some(b) = inner {
                            finally_body = Some(Box::new(lower_block_like(b, source)));
                        }
                    }
                    _ => {}
                }
            }
            let try_body = try_body.unwrap_or_else(|| Box::new(Ir::Body {
                children: Vec::new(), pass_only: false,
                range: ByteRange::empty_at(range.start), span,
            }));
            Ir::Try {
                try_body,
                handlers,
                else_body: None,
                finally_body,
                range, span,
            }
        }

        // `cond ? a : b` — ternary. tree-sitter fields: condition,
        // consequence, alternative.
        "conditional_expression" => {
            let cond = node.child_by_field_name("condition");
            let cons = node.child_by_field_name("consequence");
            let alt = node.child_by_field_name("alternative");
            match (cond, cons, alt) {
                (Some(c), Some(t), Some(f)) => Ir::Ternary {
                    condition: Box::new(lower_node(c, source)),
                    if_true: Box::new(lower_node(t, source)),
                    if_false: Box::new(lower_node(f, source)),
                    range, span,
                },
                _ => Ir::Unknown {
                    kind: "conditional_expression(missing field)".to_string(),
                    range, span,
                },
            }
        }

        // `new Foo(args) { Init }` — explicit type. tree-sitter
        // children: type identifier, argument_list, optional
        // initializer_expression. Field labels are not consistently
        // exposed; iterate named children.
        "object_creation_expression" | "implicit_object_creation_expression" => {
            let is_implicit = node.kind() == "implicit_object_creation_expression";
            let mut cursor = node.walk();
            let mut type_target: Option<Box<Ir>> = None;
            let mut arguments: Vec<Ir> = Vec::new();
            let mut initializer: Option<Box<Ir>> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "argument_list" => {
                        let mut ac = c.walk();
                        arguments = c.named_children(&mut ac).map(|a| {
                            if a.kind() == "argument" {
                                let mut cc = a.walk();
                                let inner = a.named_children(&mut cc).next();
                                inner.map(|i| lower_node(i, source))
                                    .unwrap_or_else(|| Ir::Unknown {
                                        kind: "argument(empty)".to_string(),
                                        range: range_of(a), span: span_of(a),
                                    })
                            } else {
                                lower_node(a, source)
                            }
                        }).collect();
                    }
                    "initializer_expression" => {
                        // Wrap inner expressions in Ir::Inline so they
                        // render at the `<new>` parent's level; brace
                        // text lives in gap.
                        let mut ic = c.walk();
                        let inner: Vec<Ir> = c.named_children(&mut ic)
                            .map(|n| lower_node(n, source))
                            .collect();
                        initializer = Some(Box::new(Ir::Inline {
                            children: inner,
                            range: range_of(c),
                            span: span_of(c),
                        }));
                    }
                    _ if !is_implicit && type_target.is_none() => {
                        // First non-arg, non-init child is the type.
                        type_target = Some(Box::new(lower_node(c, source)));
                    }
                    _ => {}
                }
            }
            Ir::ObjectCreation { type_target, arguments, initializer, range, span }
        }

        // C# lambda — `x => expr`, `(x, y) => expr`, `x => { ... }`,
        // `async x => ...`. tree-sitter exposes `parameters` (either a
        // single bare identifier or a `parameter_list`) and `body`
        // (either a `block` for block-bodied or any expression for
        // expression-bodied). The `=>` token is anonymous.
        "lambda_expression" => {
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            // `async` modifier appears as an unnamed token.
            let mut modifiers = Modifiers::default();
            let mut cur = node.walk();
            for c in node.children(&mut cur) {
                if !c.is_named() {
                    if let Ok(t) = c.utf8_text(source.as_bytes()) {
                        if t == "async" { modifiers.async_ = true; }
                    }
                }
            }
            // tree-sitter-c-sharp doesn't expose `parameters` as a
            // labelled field on `lambda_expression` — instead the
            // single-param form has a child `implicit_parameter`,
            // and the parens form has a child `parameter_list`.
            // Scan named children for either.
            let mut pcur = node.walk();
            let parameters: Vec<Ir> = node.named_children(&mut pcur)
                .find(|c| matches!(c.kind(), "parameter_list" | "implicit_parameter"))
                .map(|p| match p.kind() {
                    "parameter_list" => lower_csharp_parameter_list(Some(p), source),
                    _ => {
                        // implicit_parameter — single identifier-shaped param.
                        let pr = range_of(p);
                        let ps = span_of(p);
                        vec![Ir::Parameter {
                            kind: ParamKind::Regular,
                            name: Box::new(Ir::Name { range: pr, span: ps }),
                            type_ann: None,
                            default: None,
                            range: pr,
                            span: ps,
                        }]
                    }
                })
                .unwrap_or_default();
            let _ = params_node; // unused when fields aren't exposed; kept for compat
            // Body: similarly may not be a labelled field. Scan
            // named children, skipping the parameter slot — last
            // remaining named child is the body.
            let body_node = body_node.or_else(|| {
                let mut bcur = node.walk();
                node.named_children(&mut bcur)
                    .filter(|c| !matches!(c.kind(), "parameter_list" | "implicit_parameter" | "attribute_list"))
                    .last()
            });
            let body = match body_node {
                Some(b) if b.kind() == "block" => Box::new(lower_block_like(b, source)),
                Some(b) => Box::new(lower_node(b, source)),
                None => Box::new(Ir::Unknown {
                    kind: "lambda(missing body)".to_string(),
                    range, span,
                }),
            };
            Ir::Lambda {
                modifiers,
                parameters,
                body,
                range, span,
            }
        }

        // `var x = expr;` / `int x = expr;` / `int x;` —
        // local_declaration_statement contains a variable_declaration
        // which contains type + variable_declarator. Each
        // variable_declarator is one Ir::Variable. For multi-variable
        // declarations (`int a, b = 1;`), produce multiple variables.
        "local_declaration_statement" | "field_declaration" => {
            let mut cursor = node.walk();
            let var_decl = node.named_children(&mut cursor)
                .find(|c| c.kind() == "variable_declaration");
            match var_decl {
                Some(vd) => {
                    // variable_declaration has a `type` field and one or
                    // more `variable_declarator` children.
                    let type_node = vd.child_by_field_name("type");
                    let mut vc = vd.walk();
                    let declarators: Vec<TsNode> = vd.named_children(&mut vc)
                        .filter(|n| n.kind() == "variable_declarator")
                        .collect();
                    if declarators.len() == 1 {
                        let d = declarators[0];
                        lower_variable_declarator(d, type_node, source, range, span)
                    } else {
                        // Multi-variable: emit each as its own
                        // Ir::Variable, returned via Inline.
                        let children: Vec<Ir> = declarators.into_iter().map(|d| {
                            lower_variable_declarator(
                                d, type_node, source,
                                range_of(d), span_of(d),
                            )
                        }).collect();
                        Ir::Inline { children, range, span }
                    }
                }
                None => Ir::Unknown {
                    kind: "local_declaration_statement(no var_decl)".to_string(),
                    range, span,
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
        // expression. Skip the wrap for assignment-style and other
        // statement-level kinds (matches Python's bypass logic).
        "expression_statement" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(n) => {
                    let bypass = matches!(
                        n.kind(),
                        "assignment_expression" | "throw_expression"
                    );
                    if bypass {
                        lower_node(n, source)
                    } else {
                        Ir::Expression {
                            inner: Box::new(lower_node(n, source)),
                            marker: None,
                            range, span,
                        }
                    }
                }
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
        // `arguments` (`argument_list`). When the function is itself
        // an access chain (member/index/conditional), fold the call
        // into the chain as a `Call` segment — matches the existing
        // pipeline's `<object[access]>...<call>...</call></object>`
        // shape for `a.b()`. Otherwise emit a standalone `Ir::Call`.
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
            // Chain-fold: if the callee is an access chain, append a
            // Call segment. Otherwise standalone Call.
            match callee {
                Ir::Access { receiver, mut segments, range: _, span: _ } => {
                    let segment_range = ByteRange::new(
                        segments.last().map(|s| s.range().end).unwrap_or(receiver.range().end),
                        range.end,
                    );
                    segments.push(AccessSegment::Call {
                        arguments,
                        range: segment_range,
                        span,
                    });
                    Ir::Access { receiver, segments, range, span }
                }
                callee => Ir::Call {
                    callee: Box::new(callee),
                    arguments,
                    range,
                    span,
                },
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
            // C# `&&`/`||` are also handled by binary_expression
            // (no separate boolean_operator kind). Use logical
            // element name when the operator is short-circuit.
            let element_name = match op_text.as_str() {
                "&&" | "||" => "logical",
                _ => "binary",
            };
            match (left, right, op_marker(&op_text)) {
                (Some(l), Some(r), Some(marker)) => Ir::Binary {
                    element_name,
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

        // ----- Postfix unary `!` (non-null assertion) ------------------
        //
        // C#'s `obj!` declares the value non-null at the type level.
        // The existing pipeline marks this as `<expression[non_null]>`
        // — a marker on the expression host (Principle #15).
        // Architectural payoff: same shape as any value-position
        // expression, plus a marker. `obj` and `obj!` differ only in
        // the marker.
        "postfix_unary_expression" => {
            let mut cursor = node.walk();
            let operand = node.named_children(&mut cursor).next();
            // The operator is an unnamed `!` token; we don't need it
            // separately — the `non_null` marker comes from the
            // construct kind itself.
            match operand {
                Some(o) => Ir::Expression {
                    inner: Box::new(lower_node(o, source)),
                    marker: Some("non_null"),
                    range,
                    span,
                },
                None => Ir::Unknown {
                    kind: "postfix_unary_expression(missing operand)".to_string(),
                    range,
                    span,
                },
            }
        }

        // ----- as-expression `x as T` -----------------------------------
        // tree-sitter: `as_expression` with two named children
        // (value, type). Render as `<as>` element. Cast-like.
        "as_expression" => simple_statement(node, "as", source),

        // ----- anonymous_method_expression -----------------------------
        // `delegate(int x) { return x; }` — older form of lambda. Same
        // shape as Ir::Lambda would be ideal, but for parity-track
        // we just render as <lambda> via SimpleStatement.
        "anonymous_method_expression" => simple_statement(node, "lambda", source),

        // ----- preprocessor argument ----------------------------------
        "preproc_arg" => simple_statement(node, "preproc", source),

        // ----- is-expression `x is Type` --------------------------------
        //
        // tree-sitter: `is_expression` with two named children
        // (value, type-or-pattern). The `is` keyword is anonymous.
        // For now only the simple type form (`is int`, `is Widget`)
        // is covered; pattern forms (`is Widget w`, `is null`, etc.)
        // would extend the right side.
        "is_expression" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            if kids.len() == 2 {
                Ir::Is {
                    value: Box::new(lower_node(kids[0], source)),
                    type_target: Box::new(lower_node(kids[1], source)),
                    range,
                    span,
                }
            } else {
                Ir::Unknown {
                    kind: format!("is_expression(arity={})", kids.len()),
                    range,
                    span,
                }
            }
        }

        // ----- cast `(Type)expr` ----------------------------------------
        //
        // C#'s `(int)x` produces `<cast><type>...</type><value><expression>...</expression></value></cast>`.
        // tree-sitter: `cast_expression` with two named children:
        // the type and the value. (No fields, just positional.)
        "cast_expression" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            if kids.len() == 2 {
                Ir::Cast {
                    type_ann: Box::new(lower_node(kids[0], source)),
                    value: Box::new(lower_node(kids[1], source)),
                    range,
                    span,
                }
            } else {
                Ir::Unknown {
                    kind: format!("cast_expression(arity={})", kids.len()),
                    range,
                    span,
                }
            }
        }

        // ----- await -----------------------------------------------------
        //
        // `await x` similarly decorates the expression host with
        // `<await/>`. tree-sitter-c-sharp uses
        // `await_expression(operand)`.
        "await_expression" => {
            let mut cursor = node.walk();
            let operand = node.named_children(&mut cursor).next();
            match operand {
                Some(o) => Ir::Expression {
                    inner: Box::new(lower_node(o, source)),
                    marker: Some("await"),
                    range,
                    span,
                },
                None => Ir::Unknown {
                    kind: "await_expression(missing operand)".to_string(),
                    range,
                    span,
                },
            }
        }

        // C# `prefix_unary_expression` has fields `operator` and
        // `operand`.
        "prefix_unary_expression" => {
            // tree-sitter-c-sharp doesn't always expose the operator
            // as a labelled `operator` field — for compound operators
            // like `++` it's just an unnamed leading token. Find it
            // by scanning all children: the operand is the only named
            // child, the operator is whatever unnamed token comes
            // before it.
            let operand_node = node.child_by_field_name("operand").or_else(|| {
                let mut c = node.walk();
                let r = node.named_children(&mut c).next();
                r
            });
            let op_node = node.child_by_field_name("operator").or_else(|| {
                let mut c = node.walk();
                let r = node.children(&mut c).find(|c| !c.is_named());
                r
            });
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let operand = operand_node.map(|n| lower_node(n, source));
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
                    kind: format!("prefix_unary_expression(op={:?})", op_text),
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

/// Extract C#'s modifier set from a declaration's `modifier` children.
/// Each modifier keyword (`public`, `static`, `abstract`, …) is a
/// separate `modifier` CST node. Compound access forms (`protected
/// internal`, `private protected`) appear as two adjacent modifier
/// nodes — we detect them by source-text co-presence.
///
/// `default_access` is the access level used when no explicit access
/// modifier is given — varies per declaration kind:
/// - Top-level types: `Internal`
/// - Class members: `Private`
/// - Interface members: `Public` (passed by caller)
fn lower_csharp_modifiers(
    node: TsNode<'_>,
    source: &str,
    default_access: Option<Access>,
) -> Modifiers {
    let mut cursor = node.walk();
    let words: Vec<&str> = node.named_children(&mut cursor)
        .filter(|c| c.kind() == "modifier")
        .filter_map(|c| c.utf8_text(source.as_bytes()).ok())
        .collect();

    let mut m = Modifiers::default();
    let has_prot = words.contains(&"protected");
    let has_int = words.contains(&"internal");
    let has_priv = words.contains(&"private");

    // Access level — compound forms first, then singletons.
    if has_prot && has_int {
        m.access = Some(Access::ProtectedInternal);
    } else if has_priv && has_prot {
        m.access = Some(Access::PrivateProtected);
    } else {
        for w in &words {
            if let Some(a) = Access::from_csharp_modifier_text(w) {
                m.access = Some(a);
                break;
            }
        }
        if m.access.is_none() {
            m.access = default_access;
        }
    }

    // Boolean flags. `protected` / `internal` / `private` consumed
    // above for access; not flagged separately.
    for w in &words {
        match *w {
            "static"   => m.static_   = true,
            "abstract" => m.abstract_ = true,
            "sealed"   => m.sealed    = true,
            "virtual"  => m.virtual_  = true,
            "override" => m.override_ = true,
            "readonly" => m.readonly  = true,
            "partial"  => m.partial   = true,
            "async"    => m.async_    = true,
            "const"    => m.const_    = true,
            "extern"   => m.extern_   = true,
            "unsafe"   => m.unsafe_   = true,
            "volatile" => m.volatile  = true,
            "new"      => m.new_      = true,
            "required" => m.required  = true,
            // access keywords already handled above.
            "public" | "private" | "protected" | "internal" | "file" => {}
            _ => {} // unknown — ignored for now.
        }
    }
    m
}

/// Lower a C# `parameter_list` into a Vec of `Ir::Parameter` (and any
/// other parameter-like kinds we add later). Skips punctuation; only
/// keeps named children of kind `parameter`.
fn lower_csharp_parameter_list(node: Option<TsNode<'_>>, source: &str) -> Vec<Ir> {
    let Some(n) = node else { return Vec::new() };
    let mut cursor = n.walk();
    n.named_children(&mut cursor)
        .filter(|c| c.kind() == "parameter")
        .map(|c| lower_node(c, source))
        .collect()
}

/// Lower an `accessor_declaration` (`get`, `set`, `init` inside a
/// property's `{ ... }`).
fn lower_accessor_declaration(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    // The kind keyword (`get`/`set`/`init`) is an unnamed token
    // child. Find it by scanning unnamed children.
    let mut cursor = node.walk();
    let mut kind: &'static str = "get";  // default fallback
    for c in node.children(&mut cursor) {
        if !c.is_named() {
            if let Ok(t) = c.utf8_text(source.as_bytes()) {
                match t {
                    "get" => { kind = "get"; break; }
                    "set" => { kind = "set"; break; }
                    "init" => { kind = "init"; break; }
                    _ => {}
                }
            }
        }
    }
    let modifiers = lower_csharp_modifiers(node, source, None);
    let body_node = node.child_by_field_name("body")
        .or_else(|| {
            // Expression-bodied accessor (`get => expr;`)
            let mut c = node.walk();
            let r = node.named_children(&mut c).find(|n| n.kind() == "arrow_expression_clause");
            r
        });
    let body = body_node.map(|b| Box::new(lower_node(b, source)));
    Ir::Accessor { modifiers, kind, body, range, span }
}

/// Lower a control-flow consequence (the body of `if`/`while`/`for`/
/// `foreach`/`do`). C# allows either a `block` or a single statement.
/// For the single-statement form we wrap it in a synthetic `Ir::Body`
/// covering exactly the statement's range, so all renderer arms can
/// expect `Body`.
fn lower_csharp_consequence(node: TsNode<'_>, source: &str) -> Ir {
    if node.kind() == "block" {
        lower_block_like(node, source)
    } else {
        let r = range_of(node);
        let s = span_of(node);
        Ir::Body {
            children: vec![lower_node(node, source)],
            pass_only: false,
            range: r,
            span: s,
        }
    }
}

/// Lower a C# `else_clause` to `Ir::ElseIf` (when its inner is an
/// `if_statement`) or `Ir::Else` otherwise. tree-sitter exposes
/// `else_clause` as a child of `if_statement` whose first named child
/// is the inner statement (block / if_statement / single statement).
fn lower_csharp_else_chain(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    if node.kind() != "else_clause" {
        // Defensive: caller passed something unexpected.
        return Ir::Unknown { kind: format!("else_chain({})", node.kind()), range, span };
    }
    let mut cursor = node.walk();
    let inner = node.named_children(&mut cursor).next();
    let Some(inner) = inner else {
        return Ir::Unknown { kind: "else_clause(empty)".to_string(), range, span };
    };
    if inner.kind() == "if_statement" {
        // `else if` — emit Ir::ElseIf using the inner if's parts.
        let cond = inner.child_by_field_name("condition")
            .map(|n| Box::new(lower_node(n, source)));
        let body = inner.child_by_field_name("consequence")
            .map(|n| Box::new(lower_csharp_consequence(n, source)));
        let else_node = inner.child_by_field_name("alternative").or_else(|| {
            let mut c = inner.walk();
            let r = inner.named_children(&mut c).find(|c| c.kind() == "else_clause");
            r
        });
        let else_branch = else_node.map(|a| Box::new(lower_csharp_else_chain(a, source)));
        match (cond, body) {
            (Some(c), Some(b)) => Ir::ElseIf {
                condition: c, body: b, else_branch, range, span,
            },
            _ => Ir::Unknown { kind: "else_if(missing)".to_string(), range, span },
        }
    } else {
        Ir::Else {
            body: Box::new(lower_csharp_consequence(inner, source)),
            range, span,
        }
    }
}

/// Lower a keyword-prefixed simple statement / expression
/// (`yield`, `lock`, `goto`, `typeof`, etc.) to
/// `Ir::SimpleStatement`. Children are the CST's named children
/// lowered recursively, with field-aware wrapping that mirrors
/// the imperative pipeline's `apply_field_wrappings` pass.
/// Modifiers are extracted from any `modifier` child nodes —
/// declaration-shaped kinds (delegate/event/indexer/destructor)
/// need them; statement kinds simply have none.
fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = lower_csharp_modifiers(node, source, None);
    let mut cursor = node.walk();
    // Walk via cursor to access field_name() per child (same path the
    // imperative builder uses).
    let mut children: Vec<Ir> = Vec::new();
    if cursor.goto_first_child() {
        loop {
            let c = cursor.node();
            if c.is_named() && c.kind() != "modifier" {
                let field_name = cursor.field_name();
                let inner = lower_node(c, source);
                children.push(maybe_wrap_field(field_name, inner));
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    Ir::SimpleStatement { element_name, modifiers, children, range, span }
}

/// Wrap an IR node in `Ir::FieldWrap` if its tree-sitter field name
/// has an entry in the C# field-wrapping table. Mirrors the
/// imperative pipeline's `apply_field_wrappings` pass, but applied
/// at lowering time so the IR is already correctly nested.
fn maybe_wrap_field(field_name: Option<&str>, inner: Ir) -> Ir {
    let Some(field) = field_name else { return inner };
    // Same table as `CSHARP_FIELD_WRAPPINGS` in src/languages/mod.rs.
    let wrapper: &'static str = match field {
        "name"        => "name",
        "value"       => "value",
        "left"        => "left",
        "right"       => "right",
        "body"        => "body",
        "condition"   => "condition",
        "consequence" => "then",
        "returns"     => "returns",
        "type"        => "type",
        _             => return inner,
    };
    let r = inner.range();
    let s = inner.span();
    Ir::FieldWrap { wrapper, inner: Box::new(inner), range: r, span: s }
}

/// Lower a C# `catch_clause` to `Ir::ExceptHandler` with kind="catch".
/// Children:
///   - optional `catch_declaration` containing type and optional binding
///   - optional `catch_filter_clause` (the `when (...)` form)
///   - `block` (the handler body)
fn lower_csharp_catch_clause(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let mut type_target: Option<Box<Ir>> = None;
    let mut binding: Option<Box<Ir>> = None;
    let mut filter: Option<Box<Ir>> = None;
    let mut body: Option<Box<Ir>> = None;
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "catch_declaration" => {
                // catch_declaration has a `type` field and optional
                // `name` field (the binding).
                let t = c.child_by_field_name("type");
                let n = c.child_by_field_name("name");
                if let Some(t) = t {
                    type_target = Some(Box::new(lower_node(t, source)));
                }
                if let Some(n) = n {
                    binding = Some(Box::new(Ir::Name { range: range_of(n), span: span_of(n) }));
                }
            }
            "catch_filter_clause" => {
                // First named child is the filter expression.
                let mut fc = c.walk();
                let inner = c.named_children(&mut fc).next();
                if let Some(i) = inner {
                    filter = Some(Box::new(lower_node(i, source)));
                }
            }
            "block" if body.is_none() => {
                body = Some(Box::new(lower_block_like(c, source)));
            }
            _ => {}
        }
    }
    Ir::ExceptHandler {
        kind: "catch",
        type_target,
        binding,
        filter,
        body: body.unwrap_or_else(|| Box::new(Ir::Body {
            children: Vec::new(), pass_only: false,
            range: ByteRange::empty_at(range.end), span,
        })),
        range, span,
    }
}

/// Lower a `block` or `declaration_list` into `Ir::Body`.
fn lower_block_like(node: TsNode<'_>, source: &str) -> Ir {
    let mut cursor = node.walk();
    let children: Vec<Ir> = node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
    let children = merge_adjacent_line_comments(children, source);
    Ir::Body {
        children,
        pass_only: false,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Recursively collect identifier segments from a `qualified_name`
/// CST node, producing a flat list of `Ir::Name`. The grammar nests
/// `qualified_name(qualified_name(a, b), c)` for `a.b.c`; we want
/// flat `[a, b, c]`.
fn collect_qualified_name_segments(node: TsNode<'_>, source: &str, out: &mut Vec<Ir>) {
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "qualified_name" => collect_qualified_name_segments(c, source, out),
            "identifier" => out.push(Ir::Name { range: range_of(c), span: span_of(c) }),
            _ => out.push(lower_node(c, source)),
        }
    }
}

/// Group consecutive `Ir::Comment` children that are line comments on
/// adjacent lines into a single comment whose range spans them all,
/// then mark `leading = true` on any comment that is immediately
/// followed by a non-comment IR sibling on the very next line.
/// Mirrors the imperative pipeline's `classify_and_group` behaviour.
fn merge_adjacent_line_comments(children: Vec<Ir>, source: &str) -> Vec<Ir> {
    // Phase 1: merge runs of adjacent line comments.
    let mut out: Vec<Ir> = Vec::with_capacity(children.len());
    for child in children {
        if let Ir::Comment { leading, range, span } = child {
            if let Some(Ir::Comment { range: prev_range, .. }) = out.last() {
                let gap = &source[prev_range.end as usize..range.start as usize];
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_is_line_comment = source[prev_range.start as usize..prev_range.end as usize]
                    .trim_start().starts_with("//");
                let curr_is_line_comment = source[range.start as usize..range.end as usize]
                    .trim_start().starts_with("//");
                if only_one_newline && prev_is_line_comment && curr_is_line_comment {
                    if let Some(Ir::Comment { range: r, .. }) = out.last_mut() {
                        r.end = range.end;
                    }
                    continue;
                }
            }
            out.push(Ir::Comment { leading, range, span });
        } else {
            out.push(child);
        }
    }

    // Phase 2: mark a comment as `leading = true` iff the next
    // non-comment sibling starts on the very next line (no blank
    // line in between).
    let n = out.len();
    for i in 0..n {
        if let Ir::Comment { range, .. } = &out[i] {
            let comment_end = range.end as usize;
            // Find next non-comment sibling.
            let next = out.iter().skip(i + 1).find(|c| !matches!(c, Ir::Comment { .. }));
            if let Some(next_ir) = next {
                let next_start = next_ir.range().start as usize;
                let between = &source[comment_end..next_start];
                let newlines = between.chars().filter(|&c| c == '\n').count();
                if newlines == 1 && between.chars().all(|c| c.is_whitespace()) {
                    if let Ir::Comment { leading, .. } = &mut out[i] {
                        *leading = true;
                    }
                }
            }
        }
    }

    out
}

/// Lower a `variable_declarator` into `Ir::Variable`. The type
/// annotation comes from the parent's `type` field (variable_declaration
/// holds the type for the whole declarator group).
///
/// Falls back to `Ir::Unknown` for tuple-deconstruction forms like
/// `var (a, b) = …` because tree-sitter-c-sharp gives those a `name`
/// field whose byte range *overlaps* with the implicit_type's range
/// (it spans `var (a, b)` rather than just `(a, b)`). Source-text
/// recovery would emit "var" twice. The Unknown fallback preserves
/// the round-trip invariant; structural support for tuple
/// deconstruction is a future enhancement.
fn lower_variable_declarator(
    declarator: TsNode<'_>,
    type_node: Option<TsNode<'_>>,
    source: &str,
    range: ByteRange,
    span: Span,
) -> Ir {
    let name_node = declarator.child_by_field_name("name");
    // tree-sitter-c-sharp doesn't expose a `value` field on
    // variable_declarator — the initializer expression is just an
    // unlabelled child after the `=`. Find it by scanning named
    // children: skip the name and any `bracketed_argument_list`
    // (subscript form), take the last remaining child.
    let value_node = declarator.child_by_field_name("value").or_else(|| {
        let name_id = name_node.map(|n| n.id());
        // First, look for the modern grammar's flat form: a non-name
        // named child after the identifier.
        let mut cursor = declarator.walk();
        let direct = declarator.named_children(&mut cursor)
            .filter(|c| {
                Some(c.id()) != name_id
                    && c.kind() != "bracketed_argument_list"
                    && c.kind() != "equals_value_clause"
            })
            .last();
        if direct.is_some() { return direct; }
        // Fallback: older grammars wrap the value in equals_value_clause.
        let mut cc = declarator.walk();
        let eqv = declarator.named_children(&mut cc)
            .find(|c| c.kind() == "equals_value_clause");
        eqv.and_then(|e| {
            let mut ec = e.walk();
            let r = e.named_children(&mut ec).next();
            r
        })
    });

    // Bail out for the cases that would break round-trip identity:
    // 1. Missing `name` field (tuple deconstruction, exotic patterns).
    // 2. Name range overlaps with type range (some tree-sitter
    //    variants put the type *inside* the name's reported range).
    let Some(n) = name_node else {
        return Ir::Unknown {
            kind: "variable_declarator(no_name)".to_string(),
            range, span,
        };
    };
    if let Some(t) = type_node {
        if n.byte_range().start < t.byte_range().end {
            return Ir::Unknown {
                kind: "variable_declarator(overlapping_type_and_name)".to_string(),
                range, span,
            };
        }
    }

    let name_ir = Ir::Name { range: range_of(n), span: span_of(n) };
    let type_ir = type_node.map(|t| Box::new(lower_node(t, source)));
    let value_ir = value_node.map(|v| Box::new(lower_node(v, source)));
    Ir::Variable {
        type_ann: type_ir,
        name: Box::new(name_ir),
        value: value_ir,
        range,
        span,
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
                    AccessSegment::Call { range, .. }   => range.end,
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
                AccessSegment::Call { range, .. }   => range.end,
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
        "&" => "bitwise_and",
        "|" => "bitwise_or",
        "^" => "bitwise_xor",
        "~" => "bitwise_not",
        "<<" => "shift_left",
        ">>" => "shift_right",
        ">>>" => "shift_right_unsigned",
        "++" => "increment",
        "--" => "decrement",
        "??" => "null_coalesce",
        _ => return None,
    })
}
