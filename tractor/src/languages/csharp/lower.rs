//! C# tree-sitter CST → tree lowering.
//!
//! Pure function. No global state, no in-place mutation. C# is the
//! whack-a-mole champion of the existing pipeline (86 commits, the
//! unsolved `?.` conditional-access design problem, the chain-inversion
//! adapter, plus operator-extraction quirks). A successful slice here
//! is strong evidence that the typed-tree architecture handles
//! cross-language reuse: most variants are shared with Python, with
//! C#-specific additions (e.g. `SyntaxTree::Null`) only where the construct
//! genuinely differs.
//!
//! ## Initial coverage
//! Atoms (identifier, literals, null), member access (single +
//! chained), subscript, calls, binary, unary. No statements, no
//! declarations yet — proves the tree vocabulary works for the
//! expression core before tackling C#'s syntactic surface.


use std::cell::RefCell;
use std::collections::HashMap;

use crate::raw::RawNode;

use crate::tree::lower_helpers::{
    float_of, int_of, name_of, null_of, range_of, span_of, string_of, text_of,
};
use crate::tree::types::{Access, AccessSegment, ByteRange, Flag, SyntaxTree, Modifiers, Marker, OperatorKind, ParamKind, Span};

// Parent-map context. `RawNode` is a parent-less tree (the tree-sitter
// `Node::parent()` API does not map cleanly to an owned, borrowed-
// children representation), so the few lowerings that walked up the
// CST instead consult a parent map built once per `lower_csharp_root`
// call. The map is keyed by `RawNode::id` (pointer address); values
// are raw pointers stable for the duration of the lowering because
// `lower_csharp_root` holds the root by reference for that span.
thread_local! {
    static PARENT_MAP: RefCell<HashMap<usize, *const RawNode>> = RefCell::new(HashMap::new());
}

fn populate_parent_map(node: &RawNode, map: &mut HashMap<usize, *const RawNode>) {
    let parent_ptr = node as *const RawNode;
    for child in node.children() {
        map.insert(child.id(), parent_ptr);
        populate_parent_map(child, map);
    }
}

/// Lower a C# tree-sitter root node to [`SyntaxTree`].
///
/// The root is `compilation_unit`. Anything else returns
/// [`SyntaxTree::Unknown`].
pub fn lower_csharp_root(root: &RawNode, source: &str) -> SyntaxTree {
    PARENT_MAP.with(|p| {
        let mut map = HashMap::new();
        populate_parent_map(root, &mut map);
        *p.borrow_mut() = map;
    });
    let result = lower_csharp_root_inner(root, source);
    PARENT_MAP.with(|p| p.borrow_mut().clear());
    result
}

fn lower_csharp_root_inner(root: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "compilation_unit" => {
            let mut children = lower_children(root, source);
            // File-scoped namespace `namespace Foo;` lives at the
            // compilation_unit level with sibling declarations after
            // it. Tree-sitter exposes them as flat siblings rather
            // than as namespace children. Fold any siblings AFTER a
            // file-scoped SyntaxTree::Namespace into that namespace's
            // children — matches the `<namespace>` shape used by
            // queries (`//namespace[name='Foo']/class[...]`).
            fold_file_scoped_namespace_siblings(&mut children, source);
            SyntaxTree::Module {
                children: merge_adjacent_line_comments(children, source),
                range,
                span,
            }
        }
        other => SyntaxTree::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// Find a file-scoped `SyntaxTree::Namespace` child and absorb every sibling
/// after it into its `children` list. The result mirrors what the
/// imperative pipeline's `unify_file_scoped_namespace` post-pass
/// produced.
fn fold_file_scoped_namespace_siblings(children: &mut Vec<SyntaxTree>, _source: &str) {
    // Locate the index of the first file-scoped SyntaxTree::Namespace.
    let pos = children.iter().position(|c| {
        matches!(c, SyntaxTree::Namespace { file_scoped: true, .. })
    });
    let Some(idx) = pos else { return };
    if idx + 1 >= children.len() { return; }
    // Drain trailing siblings; append into the namespace's children.
    let trailing: Vec<SyntaxTree> = children.drain(idx + 1..).collect();
    if let SyntaxTree::Namespace { children: ns_children, range, .. } = &mut children[idx] {
        // Extend the namespace's source-range to cover the trailing
        // siblings (so XPath text-recovery on the namespace returns
        // the full document body, matching the imperative shape).
        if let Some(last) = trailing.last() {
            range.end = last.range().end;
        }
        ns_children.extend(trailing);
    }
}

/// Public entry point for lowering an arbitrary C# CST node — useful
/// for tests that want to lower a single expression without the
/// surrounding declaration scaffolding (which we haven't yet covered).
pub fn lower_csharp_node(node: &RawNode, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

fn lower_node(node: &RawNode, source: &str) -> SyntaxTree {
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
        // Architectural payoff: lower this to the SAME `SyntaxTree::ObjectAccess`
        // shape as regular `.b`, with `optional: true` on the
        // segment. No special chain-inversion adapter, no
        // pre-pass to undo tree-sitter's structure, no
        // `<member[conditional]>` parent + `<condition>` wrapper —
        // just one extra marker on a uniform shape.
        //
        // This is the concrete answer to backlog 5d (todo/39…md):
        // the deferred C# design problem (`Root.MaybeProperty?.Property`
        // not isomorphic to `Root.MaybeProperty.Property`) ceases to
        // exist in the typed-tree world.
        "conditional_access_expression" => {
            let kids: Vec<&RawNode> = node.named_children().collect();
            if kids.len() != 2 {
                return SyntaxTree::Unknown {
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
                SyntaxTree::ObjectAccess { receiver, mut segments, range: _, span: _ } => {
                    segments.extend(new_segments);
                    SyntaxTree::ObjectAccess { receiver, segments, range, span }
                }
                other => SyntaxTree::ObjectAccess {
                    receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "base"]),
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
            let default_node = node.named_children()
                .find(|c| c.kind() == "equals_value_clause");
            // Pick up parameter modifiers (`ref`/`out`/`in`/`params`/`this`)
            // from any `modifier` child. Each becomes an empty marker
            // sibling on the rendered `<parameter>`.
            let extra_markers: Vec<crate::tree::types::Marker> = {
                let mut found: Vec<&'static str> = Vec::new();
                for c in node.named_children() {
                    if c.kind() == "modifier" {
                        { let t = c.utf8_text(source);
                            match t {
                                "ref"    => found.push("ref"),
                                "out"    => found.push("out"),
                                "in"     => found.push("in"),
                                "params" => found.push("params"),
                                "this"   => found.push("this"),
                                _ => {}
                            }
                        }
                    }
                }
                match found.as_slice() {
                    []          => Vec::new(),
                    ["ref"]     => vec![Marker::implicit("ref")],
                    ["out"]     => vec![Marker::implicit("out")],
                    ["in"]      => vec![Marker::implicit("in")],
                    ["params"]  => vec![Marker::implicit("params")],
                    ["this"]    => vec![Marker::implicit("this")],
                    _           => Vec::new(),
                }
            };
            SyntaxTree::Parameter {
                kind: ParamKind::Regular,
                extra_markers,
                modifiers: Modifiers::default(),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown {
                        kind: "parameter(missing name)".to_string(),
                        range, span,
                    },
                }),
                type_ann: type_node.map(|t| Box::new(lower_node(t, source))),
                default: default_node.and_then(|d| {
                    let inner = d.named_children().next();
                    inner.map(|n| Box::new(lower_node(n, source)))
                }),
                range, span,
            }
        }

        // `assignment_expression` — `x = value` / `x += value` etc.
        // Reuses SyntaxTree::Assign (same as Python). The assignment is
        // expression-level in C# but we model it the same way; the
        // wrapping `expression_statement` is bypassed for assignments
        // (handled below) so the rendered shape stays clean.
        "assignment_expression" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_text = node.child_by_field_name("operator")
                .map(|n| n.utf8_text(source))
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
                (Some(l), Some(r)) => SyntaxTree::Assign {
                    targets: vec![l.wrap_expression()],
                    type_annotation: None,
                    op_text,
                    op_range,
                    op_markers,
                    values: vec![r.wrap_expression()],
                    range, span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "assignment_expression(missing operand)".to_string(),
                    range, span,
                },
            }
        }

        // ----- Statements with simple structure -------------------------

        "comment" => SyntaxTree::Comment { leading: Flag::Off, trailing: Flag::Off, range, span },

        // `using System;` / `using static System.Math;` / `using A = B;`
        "using_directive" => {
            let mut is_static = false;
            let mut alias: Option<Box<SyntaxTree>> = None;
            let mut path_node: Option<&RawNode> = None;
            for c in node.children() {
                if !c.is_named() {
                    { let t = c.utf8_text(source);
                        if t == "static" { is_static = true; }
                    }
                    continue;
                }
                match c.kind() {
                    "name_equals" => {
                        // `A =` part of `using A = B;`
                        let n = c.named_children().next();
                        if let Some(n) = n {
                            alias = Some(Box::new(name_of(n, source)));
                        }
                    }
                    _ => path_node = Some(c),
                }
            }
            let path_ir = match path_node {
                Some(n) => lower_node(n, source),
                None => SyntaxTree::Unknown {
                    kind: "using_directive(missing path)".to_string(),
                    range, span,
                },
            };
            SyntaxTree::Using {
                is_static,
                alias,
                path: Box::new(path_ir),
                range, span,
            }
        }

        // `qualified_name` (`System.Linq`) — same shape as Python's path.
        "qualified_name" => {
            // Flatten nested qualified_name (System.Collections.Generic
            // is parsed as nested pairs) into a single flat SyntaxTree::Path
            // with all name segments — matches imperative pipeline.
            let mut segments: Vec<SyntaxTree> = Vec::new();
            collect_qualified_name_segments(node, source, &mut segments);
            SyntaxTree::Path { segments, range, span }
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
            let members: Vec<SyntaxTree> = match body_node {
                Some(b) => {
                    b.named_children().map(|n| lower_node(n, source)).collect()
                }
                None => Vec::new(),
            };
            SyntaxTree::Enum {
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown { kind: "enum(missing name)".to_string(), range, span },
                }),
                underlying_type: None,
                members,
                range, span,
            }
        }

        "enum_member_declaration" => {
            let name_node = node.child_by_field_name("name");
            let value_node = node.child_by_field_name("value");
            SyntaxTree::EnumMember {
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown { kind: "enum_member(missing name)".to_string(), range, span },
                }),
                value: value_node.map(|n| Box::new(lower_node(n, source))),
                range, span,
            }
        }

        // ----- Properties ----------------------------------------------

        "property_declaration" => {
            let type_node = node.child_by_field_name("type");
            let name_node = node.child_by_field_name("name");
            let default_access = match enclosing_type_kind(node) {
                Some("interface_declaration") => Some(Access::Public),
                _ => Some(Access::Private),
            };
            let modifiers = lower_csharp_modifiers(node, source, default_access);
            let mut accessors: Vec<SyntaxTree> = Vec::new();
            let mut value: Option<Box<SyntaxTree>> = None;
            for c in node.named_children() {
                match c.kind() {
                    "accessor_list" => {
                        for a in c.named_children() {
                            if a.kind() == "accessor_declaration" {
                                accessors.push(lower_accessor_declaration(a, source));
                            }
                        }
                    }
                    "arrow_expression_clause" => {
                        // Expression-bodied: `=> expr;`
                        let inner = c.named_children().next();
                        if let Some(i) = inner {
                            // Treat as a get-only accessor with body.
                            accessors.push(SyntaxTree::Accessor {
                                modifiers: Modifiers::default(),
                                kind: crate::tree::types::AccessorKind::Get,
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
            // Attributes on the property (each `attribute_list` child).
            let decorators: Vec<SyntaxTree> = node.named_children()
                .filter(|c| c.kind() == "attribute_list")
                .flat_map(|al| {
                    al.named_children()
                        .filter(|c| c.kind() == "attribute")
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            SyntaxTree::Property {
                modifiers,
                decorators,
                type_ann: type_node.map(|t| Box::new(lower_node(t, source))),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown { kind: "property(missing name)".to_string(), range, span },
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
            SyntaxTree::Constructor {
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown { kind: "constructor(missing name)".to_string(), range, span },
                }),
                parameters: lower_csharp_parameter_list(params_node, source),
                body: Box::new(match body_node {
                    Some(b) => lower_block_like(b, source),
                    None => SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span },
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
            let body_node = node.named_children()
                .find(|c| c.kind() == "declaration_list");
            // For namespace, the imperative pipeline collapses a
            // qualified_name (`Tractor.Fixtures.Traditional`) into a
            // single `<name>` leaf with the full dotted text. Keep
            // parity by emitting SyntaxTree::Name with the qualified node's
            // whole range.
            let name_ir = match name_node {
                Some(n) if n.kind() == "qualified_name" || n.kind() == "identifier" => {
                    name_of(n, source)
                }
                Some(n) => lower_node(n, source),
                None => SyntaxTree::Unknown {
                    kind: "namespace(missing name)".to_string(),
                    range, span,
                },
            };
            let children: Vec<SyntaxTree> = match body_node {
                Some(b) => {
                    let raw: Vec<SyntaxTree> = b.named_children().map(|n| lower_node(n, source)).collect();
                    merge_adjacent_line_comments(raw, source)
                }
                None => Vec::new(),
            };
            SyntaxTree::Namespace { name: Box::new(name_ir), children, file_scoped: false, range, span }
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
            // Extract generics + bases + decorators from named children.
            // tree-sitter-c-sharp exposes `type_parameter_list`,
            // `base_list`, and `attribute_list` as direct children
            // of the type declaration.
            let body_node = node.named_children()
                .find(|c| c.kind() == "declaration_list");
            let type_param_list = node.named_children()
                .find(|c| c.kind() == "type_parameter_list");
            let base_list = node.named_children()
                .find(|c| c.kind() == "base_list");
            let decorators: Vec<SyntaxTree> = node.named_children()
                .filter(|c| c.kind() == "attribute_list")
                .flat_map(|al| {
                    al.named_children()
                        .filter(|c| c.kind() == "attribute")
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            let modifiers = lower_csharp_modifiers(node, source, /*default_access*/ Some(Access::Internal));
            // Generics: lower the whole type_parameter_list as SyntaxTree::Generic
            // (its items are SyntaxTree::TypeParameter via the type_parameter arm).
            let generics: Vec<SyntaxTree> = match type_param_list {
                Some(tpl) => tpl.named_children().map(|c| lower_node(c, source)).collect(),
                None => Vec::new(),
            };
            // Bases: each named child of base_list becomes one base,
            // wrapped via `wrap_extends()` so the renderer doesn't
            // have to inspect.
            let bases: Vec<SyntaxTree> = match base_list {
                Some(bl) => {
                    bl.named_children()
                        .map(|c| lower_node(c, source).wrap_extends())
                        .collect()
                }
                None => Vec::new(),
            };
            // Where clauses (`where T : ...`). Each
            // `type_parameter_constraints_clause` child is consumed
            // twice: once by `fold_csharp_where_clauses_into_generics`,
            // which translates each constraint and appends it to the
            // matching `<generic>` item; once for `where_clauses` so
            // its source-range bytes flow through `render_tree_class`
            // (which renders C# where clauses as gap text — the
            // structural query path runs through the merged generics).
            let raw_where_clauses: Vec<SyntaxTree> = node.named_children()
                .filter(|c| c.kind() == "type_parameter_constraints_clause")
                .map(|c| lower_node(c, source))
                .collect();
            let where_for_render: Vec<SyntaxTree> = node.named_children()
                .filter(|c| c.kind() == "type_parameter_constraints_clause")
                .map(|c| lower_node(c, source))
                .collect();
            let generics = fold_csharp_where_clauses_into_generics(generics, raw_where_clauses, source);
            let name = Box::new(match name_node {
                Some(n) => name_of(n, source),
                None => SyntaxTree::Unknown {
                    kind: format!("{}(missing name)", kind),
                    range, span,
                },
            });
            let body = Box::new(match body_node {
                Some(b) => lower_block_like(b, source),
                None => SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span },
            });
            let where_clauses = where_for_render;
            match kind {
                "struct" => SyntaxTree::Struct {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
                "interface" => SyntaxTree::Interface {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
                "record" => SyntaxTree::Record {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
                _ => SyntaxTree::Class {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
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
            let body: Option<Box<SyntaxTree>> = match body_node {
                Some(b) if b.kind() == "block" => Some(Box::new(lower_block_like(b, source))),
                Some(b) => {
                    // Arrow-bodied: wrap the inner expression in a
                    // synthetic SyntaxTree::Body covering the arrow clause's
                    // range so the renderer treats it consistently.
                    let r = range_of(b);
                    let s = span_of(b);
                    Some(Box::new(SyntaxTree::Body {
                        children: vec![lower_node(b, source)],
                        pass_only: false,
                        block_wrap: false,
                        range: r, span: s,
                    }))
                }
                None => None,
            };
            SyntaxTree::Method {
                modifiers,
                decorators: Vec::new(),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown {
                        kind: "local_function(missing name)".to_string(),
                        range, span,
                    },
                }),
                generics: Vec::new(),
                parameters: lower_csharp_parameter_list(params_node, source),
                returns: None,
                throws: Vec::new(),
                body,
                range, span,
            }
        }

        // `[modifiers] returntype Name(params) { body }` — name +
        // body + parameters + full Modifiers. Default access for
        // class members: Private. Return type still deferred (would
        // need an SyntaxTree::Returns wrap; tree-sitter exposes it as a
        // sibling of `name` rather than a labelled field).
        "method_declaration" => {
            let name_node = node.child_by_field_name("name");
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            // Default access: Private for class members, Public for
            // interface members.
            let default_access = match enclosing_type_kind(node) {
                Some("interface_declaration") => Some(Access::Public),
                _ => Some(Access::Private),
            };
            let modifiers = lower_csharp_modifiers(node, source, default_access);
            // Extract type_parameter_list (generics) + attribute_list
            // (decorators) as direct named children.
            let type_param_list = node.named_children()
                .find(|c| c.kind() == "type_parameter_list");
            let generics: Vec<SyntaxTree> = match type_param_list {
                Some(tpl) => tpl.named_children().map(|c| lower_node(c, source)).collect(),
                None => Vec::new(),
            };
            let decorators: Vec<SyntaxTree> = node.named_children()
                .filter(|c| c.kind() == "attribute_list")
                .flat_map(|al| {
                    al.named_children()
                        .filter(|c| c.kind() == "attribute")
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            SyntaxTree::Method {
                modifiers,
                decorators,
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown {
                        kind: "method(missing name)".to_string(),
                        range, span,
                    },
                }),
                generics,
                parameters: lower_csharp_parameter_list(params_node, source),
                returns: None,
                throws: Vec::new(),
                body: body_node.map(|b| Box::new(lower_block_like(b, source))),
                range, span,
            }
        }

        // `block` (method body, free-standing block in C#).
        "block" => lower_block_like(node, source),

        // ----- Control flow ---------------------------------------------
        //
        // C# allows non-block bodies (`if (c) stmt;`, `while (c) stmt;`).
        // Wrap single statements in a synthetic `SyntaxTree::Body` so the
        // shared rendering arms (which expect Body) work uniformly.
        "if_statement" => {
            let cond = node.child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source).wrap_expression()));
            let body = node.child_by_field_name("consequence")
                .map(|n| Box::new(lower_csharp_consequence(n, source)));
            // The `else` part is exposed either as a child kind
            // `else_clause` (older grammars) or via a labelled field
            // in newer ones. Try both.
            let else_node = node.child_by_field_name("alternative").or_else(|| {
                let r = node.named_children().find(|c| c.kind() == "else_clause");
                r
            });
            let else_branch = else_node.map(|a| Box::new(lower_csharp_else_chain(a, source)));
            match (cond, body) {
                (Some(c), Some(b)) => SyntaxTree::If {
                    condition: c,
                    body: b,
                    else_branch,
                    range, span,
                },
                _ => SyntaxTree::Unknown { kind: "if_statement(missing field)".to_string(), range, span },
            }
        }

        "while_statement" => {
            let cond = node.child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source).wrap_expression()));
            let body = node.child_by_field_name("body")
                .map(|n| Box::new(lower_csharp_consequence(n, source)));
            match (cond, body) {
                (Some(c), Some(b)) => SyntaxTree::While {
                    condition: c, body: b, else_body: None, range, span,
                },
                _ => SyntaxTree::Unknown { kind: "while_statement(missing field)".to_string(), range, span },
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
                (Some(l), Some(r), Some(b)) => SyntaxTree::Foreach {
                    type_ann: type_node.map(|t| Box::new(lower_node(t, source))),
                    target: Box::new(lower_node(l, source).wrap_expression()),
                    iterable: Box::new(lower_node(r, source).wrap_expression()),
                    body: Box::new(lower_csharp_consequence(b, source)),
                    range, span,
                },
                _ => SyntaxTree::Unknown { kind: "foreach_statement(missing field)".to_string(), range, span },
            }
        }

        // `for (init; cond; update) body` — C-style. tree-sitter
        // fields: `initializer` (declaration or expression list, may
        // be missing), `condition` (expression, optional), `update`
        // (vec of expressions via repeated `update` field),
        // `body`. The semicolons live in gap text.
        "for_statement" => {
            // tree-sitter-c-sharp's field labels for for_statement are
            // unreliable in the pinned grammar version. Identify
            // children by position: named children in order are
            // [initializer?, condition?, updates*..., body]. The body
            // is always the last named child.
            let kids: Vec<&RawNode> = node.named_children().collect();
            if kids.is_empty() {
                SyntaxTree::Unknown { kind: "for_statement(empty)".to_string(), range, span }
            } else {
                let body_node = *kids.last().unwrap();
                let header = &kids[..kids.len() - 1];
                // Slice the source between `for (` and `)` to find `;`
                // separators that mark init/condition/update boundaries.
                let header_text_start = range.start as usize;
                let header_text_end = body_node.byte_range().start;
                let header_text = &source[header_text_start..header_text_end];
                // Count `;` to bucket children.
                let semi_positions: Vec<usize> = header_text.match_indices(';')
                    .map(|(i, _)| header_text_start + i)
                    .collect();
                let init_end = semi_positions.first().copied();
                let cond_end = semi_positions.get(1).copied();
                let mut initializer: Option<Box<SyntaxTree>> = None;
                let mut condition: Option<Box<SyntaxTree>> = None;
                let mut updates: Vec<SyntaxTree> = Vec::new();
                for k in header {
                    let pos = k.byte_range().start;
                    if let Some(ie) = init_end {
                        if pos < ie {
                            if initializer.is_none() {
                                initializer = Some(Box::new(lower_node(*k, source)));
                            }
                            continue;
                        }
                    }
                    if let Some(ce) = cond_end {
                        if pos < ce {
                            if condition.is_none() {
                                condition = Some(Box::new(lower_node(*k, source)));
                            }
                            continue;
                        }
                    }
                    updates.push(lower_node(*k, source));
                }
                SyntaxTree::CFor {
                    initializer,
                    condition,
                    updates,
                    body: Box::new(lower_csharp_consequence(body_node, source)),
                    range, span,
                }
            }
        }

        // `do body while(cond);`. tree-sitter fields: `body` and
        // `condition`. The `do`/`while` keywords + `;` live in gap
        // text.
        "do_statement" => {
            let body_node = node.child_by_field_name("body");
            let cond_node = node.child_by_field_name("condition");
            match (body_node, cond_node) {
                (Some(b), Some(c)) => SyntaxTree::DoWhile {
                    body: Box::new(lower_csharp_consequence(b, source)),
                    condition: Box::new(lower_node(c, source)),
                    range, span,
                },
                _ => SyntaxTree::Unknown { kind: "do_statement(missing field)".to_string(), range, span },
            }
        }

        "break_statement" => SyntaxTree::Break { range, span },
        "continue_statement" => SyntaxTree::Continue { range, span },

        // `return [value];`
        "return_statement" => {
            let value = node.named_children().next();
            SyntaxTree::Return {
                value: value.map(|v| Box::new(lower_node(v, source).wrap_expression_inline_aware())),
                range, span,
            }
        }


        // Simple keyword-prefixed statements / expressions whose old
        // pipeline rule is a plain Rename. Lowered to
        // SyntaxTree::SimpleStatement with the right element name.
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
        "event_field_declaration"       => lower_event_field_declaration(node, source),
        "event_declaration"             => simple_statement(node, "event",      source),
        "conversion_operator_declaration" => simple_statement(node, "operator", source),
        // `namespace Foo.Bar;` (file-scoped) — same as block-scoped:
        // collapse the qualified name to a flat `<name>` leaf.
        "file_scoped_namespace_declaration" => {
            let name_node = node.child_by_field_name("name");
            // The "body" of file-scoped namespace is the rest of the
            // file's declarations after the `;` — children of the
            // file_scoped_namespace_declaration node itself.
            let children: Vec<SyntaxTree> = node.named_children()
                .filter(|c| !matches!(c.kind(), "qualified_name" | "identifier" | "modifier"))
                .filter(|c| Some(c.id()) != name_node.map(|n| n.id()))
                .map(|c| lower_node(c, source))
                .collect();
            let name_ir = match name_node {
                Some(n) => name_of(n, source),
                None => SyntaxTree::Unknown { kind: "file_scoped_namespace(missing name)".to_string(), range, span },
            };
            SyntaxTree::Namespace {
                name: Box::new(name_ir),
                children: merge_adjacent_line_comments(children, source),
                file_scoped: true,
                range, span,
            }
        }
        "operator_declaration"          => simple_statement(node, "operator",   source),
        "fixed_statement"               => simple_statement(node, "fixed",      source),
        // `unsafe { ... }` — render as `<block[unsafe]>` containing
        // the inner block's statements directly. Do NOT recurse into
        // the child `block` node via simple_statement (that would
        // produce `<block[unsafe]><body><block>...</block></body></block>`,
        // which trips the `block-nested-directly-under-block` contract).
        "unsafe_statement" => {
            let inner_block = node.named_children()
                .find(|c| c.kind() == "block");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(b) = inner_block {
                children.extend(b.named_children().map(|c| lower_node(c, source)));
            }
            SyntaxTree::SimpleStatement {
                element_name: "block",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("unsafe")],
                children,
                range, span,
            }
        }
        "using_statement"               => simple_statement(node, "using",      source),
        "throw_statement"               => lower_csharp_throw(node, source),
        "throw_expression"              => lower_csharp_throw(node, source),
        "with_expression"               => simple_statement(node, "with",       source),
        "range_expression"              => simple_statement(node, "range",      source),
        "tuple_expression"              => simple_statement(node, "tuple",      source),
        // `from n in numbers` — first identifier is the loop variable
        // (rendered as <name>); the source expression is wrapped in
        // <value><expression>...</expression></value>. Plus a
        // synthetic `<in/>` marker for the `in` keyword.
        "from_clause" => {
            let kids: Vec<&RawNode> = node.named_children().collect();
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(name) = kids.first() {
                let nr = range_of(*name);
                let ns = span_of(*name);
                children.push(SyntaxTree::FieldWrap {
                    wrapper: "name",
                    inner: Box::new(name_of(*name, source)),
                    range: nr, span: ns,
                });
            }
            if kids.len() > 1 {
                let value = kids[kids.len() - 1];
                let vr = range_of(value);
                let vs = span_of(value);
                children.push(SyntaxTree::FieldWrap {
                    wrapper: "value",
                    inner: Box::new(lower_node(value, source).wrap_expression()),
                    range: vr, span: vs,
                });
            }
            SyntaxTree::SimpleStatement {
                element_name: "from",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("in")],
                children,
                range, span,
            }
        }
        "where_clause"                  => simple_statement(node, "where",      source),
        "select_clause"                 => simple_statement(node, "select",     source),
        "order_by_clause"               => simple_statement(node, "order",      source),
        "join_clause"                   => simple_statement_marked(node, "join", vec![Marker::implicit("in")], source),
        "group_clause"                  => simple_statement(node, "group",      source),
        "let_clause"                    => simple_statement(node, "let",        source),
        "query_expression"              => simple_statement(node, "query",      source),
        "query_continuation"            => simple_statement(node, "query",      source),
        // Attributes
        // `attribute` — flatten the `attribute_argument_list` so each
        // argument becomes a direct `<argument>` child of `<attribute>`
        // (matches the imperative pipeline's
        // `Flatten { distribute_list: Some("arguments") }` shape on
        // attribute_argument_list).
        "attribute" => {
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                match c.kind() {
                    "attribute_argument_list" => {
                        for a in c.named_children() {
                            children.push(lower_node(a, source));
                        }
                    }
                    _ => children.push(lower_node(c, source)),
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "attribute",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range,
                span,
            }
        }
        "attribute_argument"            => simple_statement(node, "argument",   source),
        "attribute_argument_list"       => simple_statement(node, "arguments",  source),
        "attribute_target_specifier"    => simple_statement(node, "target",     source),
        // Generics & constraints
        "type_parameter_constraint"     => simple_statement(node, "constraint", source),
        "type_parameter_constraints_clause" => simple_statement(node, "where",  source),
        "constructor_constraint"        => simple_statement(node, "new",        source),
        // Patterns — flatten (the imperative pipeline does the same).
        "parenthesized_variable_designation" => simple_statement(node, "designation", source),
        // Pointer & function pointer types
        "pointer_type"                  => simple_statement(node, "type",       source),
        "function_pointer_parameter"    => simple_statement(node, "parameter",  source),
        // Misc
        // `global::System.X` / `extern alias X` — flat name, full
        // dotted text concatenated (matches imperative pipeline's
        // descendant_text path for qualified-style names).
        "alias_qualified_name"          => name_of(node, source),
        "declaration_expression"        => simple_statement(node, "declaration",source),
        "extern_alias_directive"        => simple_statement(node, "import",     source),
        // `: Money(Amount, Currency)` after a record's primary
        // constructor — flatten so the inner type/identifier and
        // arguments render at the surrounding base-list level. The
        // imperative pipeline does the same via `Flatten`.
        "primary_constructor_base_type" => {
            let children: Vec<SyntaxTree> = node.named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        "constructor_initializer"       => simple_statement(node, "initializer",source),
        "calling_convention"            => simple_statement(node, "calling",    source),
        "explicit_interface_specifier"  => simple_statement(node, "interface",  source),
        "ref_expression"                => simple_statement_marked(node, "expression", vec![Marker::implicit("ref")], source),
        "shebang_directive"             => simple_statement(node, "shebang",    source),
        "member_binding_expression"     => simple_statement(node, "member",     source),
        "element_binding_expression"    => simple_statement(node, "index",      source),
        // Type kinds — render under <type> when used as standalone.
        // `var` (implicit_type) — render as a bare `SyntaxTree::Name` so the
        // surrounding type slot (variable's type, foreach's Slot::Type)
        // wraps it in `<type>` naturally, yielding
        // `<type><name>var</name></type>` for query consistency with
        // `predefined_type`.
        "implicit_type" => name_of(node, source),
        "tuple_type"                    => simple_statement_marked(node, "type", vec![Marker::implicit("tuple")], source),
        "array_type"                    => simple_statement_marked(node, "type", vec![Marker::implicit("array")], source),
        "nullable_type"                 => simple_statement_marked(node, "type", vec![Marker::implicit("nullable")], source),
        "ref_type"                      => simple_statement_marked(node, "type", vec![Marker::implicit("ref")], source),
        "scoped_type"                   => simple_statement_marked(node, "type", vec![Marker::implicit("scoped")], source),
        "function_pointer_type"         => simple_statement(node, "type",       source),
        // Array / collection creation — render as <new>. stackalloc
        // forms additionally carry a `<stackalloc/>` marker; anonymous
        // object creation carries `<anonymous/>`.
        "array_creation_expression"          => simple_statement(node, "new",   source),
        "implicit_array_creation_expression" => simple_statement(node, "new",   source),
        "anonymous_object_creation_expression" => simple_statement_marked(node, "new", vec![Marker::implicit("anonymous")], source),
        "stackalloc_expression"              => simple_statement_marked(node, "new", vec![Marker::implicit("stackalloc")], source),
        "implicit_stackalloc_expression"     => simple_statement_marked(node, "new", vec![Marker::implicit("stackalloc")], source),
        // String interpolation — render as <string>.
        "interpolated_string_expression"     => simple_statement(node, "string",source),
        // Pattern-matching expressions / statements.
        "is_pattern_expression"              => simple_statement(node, "is",    source),
        "switch_statement"                   => simple_statement(node, "switch",source),
        "switch_expression"                  => simple_statement(node, "switch",source),
        "switch_expression_arm"              => simple_statement(node, "arm",   source),
        "switch_section"                     => simple_statement(node, "arm",   source),
        // `o with { X = 1 }`'s brace block — imperative renames to
        // <literal> (matching object/array initializer shape).
        "with_initializer"                   => simple_statement(node, "literal", source),
        "interpolation"                      => simple_statement(node, "interpolation", source),
        // Pattern kinds — old pipeline uses RenameWithMarker(Pattern, X).
        // Each pattern shape carries a kind marker so XPath queries
        // can distinguish `<pattern[constant]>` from `<pattern[declaration]>`
        // etc. (Principle #15: stable shape markers.)
        "constant_pattern"          => simple_statement_marked(node, "pattern", vec![Marker::implicit("constant")], source),
        "declaration_pattern"       => simple_statement_marked(node, "pattern", vec![Marker::implicit("declaration")], source),
        "recursive_pattern"         => simple_statement_marked(node, "pattern", vec![Marker::implicit("recursive")], source),
        "relational_pattern"        => simple_statement_marked(node, "pattern", vec![Marker::implicit("relational")], source),
        "tuple_pattern"             => simple_statement_marked(node, "pattern", vec![Marker::implicit("tuple")], source),
        "and_pattern"               => simple_statement_marked(node, "pattern", vec![Marker::implicit("and")], source),
        "or_pattern"                => simple_statement_marked(node, "pattern", vec![Marker::implicit("or")],  source),
        "negated_pattern"           => simple_statement_marked(node, "pattern", vec![Marker::implicit("negated")], source),
        "list_pattern"              => simple_statement_marked(node, "pattern", vec![Marker::implicit("list")], source),
        "var_pattern"               => simple_statement_marked(node, "pattern", vec![Marker::implicit("var")], source),
        "type_pattern"              => simple_statement(node, "pattern", source),
        "property_pattern_clause"   => simple_statement(node, "properties", source),
        "subpattern"                => simple_statement(node, "subpattern", source),
        "discard"                   => simple_statement(node, "discard", source),
        "tuple_element"             => simple_statement(node, "element", source),
        "when_clause"               => simple_statement(node, "when", source),
        // type_parameter — `<generic>` with optional variance marker.
        // Variance keywords `in`/`out` appear as unnamed children in the
        // type_parameter; surface them as `<in/>` / `<out/>` markers.
        "type_parameter" => {
            let variance: Vec<crate::tree::types::Marker> = {
                let mut found: Option<&'static str> = None;
                for c in node.children() {
                    if !c.is_named() {
                        { let t = c.utf8_text(source);
                            match t {
                                "in"  => found = Some("in"),
                                "out" => found = Some("out"),
                                _ => {}
                            }
                        }
                    }
                }
                match found {
                    Some("in")  => vec![Marker::implicit("in")],
                    Some("out") => vec![Marker::implicit("out")],
                    _ => Vec::new(),
                }
            };
            simple_statement_marked(node, "generic", variance, source)
        }

        // Flatten-only kinds — render as Inline so children promote
        // to the parent's element. `list_name` mirrors the imperative
        // pipeline's `Flatten { distribute_list: Some("X") }` rule
        // — same-named children get a `list="X"` attribute so JSON
        // projection collects them under a plural key.
        "switch_body"
        | "bracketed_parameter_list"
        | "array_rank_specifier"
        | "interpolation_alignment_clause"
        | "interpolation_format_clause"
        | "parenthesized_pattern"
        | "positional_pattern_clause"
        | "empty_statement"
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
        | "argument_list"
        | "preproc_if"
        | "preproc_else"
        | "preproc_elif"
        | "preproc_define"
        | "preproc_endregion"
        | "preproc_error"
        | "preproc_line"
        | "preproc_nullable"
        | "preproc_pragma"
        | "preproc_region"
        | "preproc_undef"
        | "preproc_warning"
        | "preproc_if_in_attribute_list"
        | "preproc_arg"
        | "attribute_list"
        | "global_attribute" => {
            let list_name: Option<&'static str> = match node.kind() {
                "accessor_list"                                  => Some("accessors"),
                "argument_list" | "attribute_argument_list"
                | "type_argument_list"                           => Some("arguments"),
                "attribute_list"                                 => Some("attributes"),
                "bracketed_parameter_list" | "parameter_list"    => Some("parameters"),
                "type_parameter_list"                            => Some("generics"),
                _                                                => None,
            };
            let children: Vec<SyntaxTree> = node.named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name, range, span }
        }
        // Parenthesized expression: parens become gap text on parent.
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

        // `try { body } catch (...) { ... } finally { ... }`.
        // tree-sitter children: a `block` (try body), then any number
        // of `catch_clause`s, optionally a `finally_clause`.
        "try_statement" => {
            let mut try_body: Option<Box<SyntaxTree>> = None;
            let mut handlers: Vec<SyntaxTree> = Vec::new();
            let mut finally_body: Option<Box<SyntaxTree>> = None;
            for c in node.named_children() {
                match c.kind() {
                    "block" if try_body.is_none() => {
                        try_body = Some(Box::new(lower_block_like(c, source)));
                    }
                    "catch_clause" => {
                        handlers.push(lower_csharp_catch_clause(c, source));
                    }
                    "finally_clause" => {
                        // finally_clause has a single block child.
                        let inner = c.named_children().find(|n| n.kind() == "block");
                        if let Some(b) = inner {
                            finally_body = Some(Box::new(lower_block_like(b, source).wrap_clause("finally")));
                        }
                    }
                    _ => {}
                }
            }
            let try_body = try_body.unwrap_or_else(|| Box::new(SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.start), span }));
            SyntaxTree::Try {
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
                (Some(c), Some(t), Some(f)) => SyntaxTree::Ternary {
                    condition: Box::new(lower_node(c, source).wrap_expression()),
                    if_true: Box::new(lower_node(t, source).wrap_expression()),
                    if_false: Box::new(lower_node(f, source).wrap_expression()),
                    range, span,
                },
                _ => SyntaxTree::Unknown {
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
            let mut type_target: Option<Box<SyntaxTree>> = None;
            let mut arguments: Vec<SyntaxTree> = Vec::new();
            let mut initializer: Option<Box<SyntaxTree>> = None;
            for c in node.named_children() {
                match c.kind() {
                    "argument_list" => {
                        arguments = c.named_children().map(|a| {
                            if a.kind() == "argument" {
                                let inner = a.named_children().next();
                                inner.map(|i| lower_node(i, source))
                                    .unwrap_or_else(|| SyntaxTree::Inline { children: Vec::new(), list_name: None, range: range_of(a), span: span_of(a) })
                            } else {
                                lower_node(a, source)
                            }
                        }).collect();
                    }
                    "initializer_expression" => {
                        // Wrap inner expressions in SyntaxTree::Inline so they
                        // render at the `<new>` parent's level; brace
                        // text lives in gap.
                        let inner: Vec<SyntaxTree> = c.named_children()
                            .map(|n| lower_node(n, source))
                            .collect();
                        initializer = Some(Box::new(SyntaxTree::Inline {
                            children: inner,
                            list_name: None,
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
            SyntaxTree::ObjectCreation { type_target, arguments, initializer, range, span }
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
            for c in node.children() {
                if !c.is_named() {
                    let t = c.utf8_text(source);
                    if t == "async" {
                        modifiers.async_ = crate::tree::types::Flag::anchored(
                            range_of(c),
                            span_of(c),
                        );
                    }
                }
            }
            // tree-sitter-c-sharp doesn't expose `parameters` as a
            // labelled field on `lambda_expression` — instead the
            // single-param form has a child `implicit_parameter`,
            // and the parens form has a child `parameter_list`.
            // Scan named children for either.
            let parameters: Vec<SyntaxTree> = node.named_children()
                .find(|c| matches!(c.kind(), "parameter_list" | "implicit_parameter"))
                .map(|p| match p.kind() {
                    "parameter_list" => lower_csharp_parameter_list(Some(p), source),
                    _ => {
                        // implicit_parameter — single identifier-shaped param.
                        let pr = range_of(p);
                        let ps = span_of(p);
                        vec![SyntaxTree::Parameter {
                            kind: ParamKind::Regular,
                            extra_markers: Vec::new(),
                            modifiers: Modifiers::default(),
                            name: Box::new(name_of(p, source)),
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
                node.named_children()
                    .filter(|c| !matches!(c.kind(), "parameter_list" | "implicit_parameter" | "attribute_list"))
                    .last()
            });
            let body = match body_node {
                Some(b) if b.kind() == "block" => crate::tree::types::LambdaBody::Block(Box::new(lower_block_like(b, source))),
                Some(b) => crate::tree::types::LambdaBody::Expression(Box::new(lower_node(b, source))),
                None => crate::tree::types::LambdaBody::Expression(Box::new(SyntaxTree::Unknown {
                    kind: "lambda(missing body)".to_string(),
                    range, span,
                })),
            };
            SyntaxTree::Lambda {
                modifiers,
                parameters,
                body,
                range, span,
            }
        }

        // CST wrappers that should pass through to their inner.
        "argument" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => lower_node(i, source),
                None => SyntaxTree::Inline { children: Vec::new(), list_name: None, range, span },
            }
        }
        "arrow_expression_clause" => {
            // `=> expr` — unwrap to the inner expression.
            let inner = node.named_children().next();
            match inner {
                Some(i) => lower_node(i, source),
                None => SyntaxTree::Unknown { kind: "arrow_expression_clause(empty)".to_string(), range, span },
            }
        }
        "generic_name" => {
            // `Foo<T, U>` — name + type-argument list. Render as
            // <type[generic]><name>Foo</name>...</type> via SyntaxTree::GenericType.
            let kids: Vec<&RawNode> = node.named_children().collect();
            if let Some(name_node) = kids.first() {
                let name = Box::new(name_of(*name_node, source));
                let mut params: Vec<SyntaxTree> = Vec::new();
                for k in kids.iter().skip(1) {
                    if k.kind() == "type_argument_list" {
                        for arg in k.named_children() {
                            params.push(lower_node(arg, source));
                        }
                    }
                }
                SyntaxTree::GenericType { name, params, range, span }
            } else {
                SyntaxTree::Unknown { kind: "generic_name(empty)".to_string(), range, span }
            }
        }
        "initializer_expression" => {
            // Brace-form initializer — children flatten into parent.
            let children: Vec<SyntaxTree> = node.named_children()
                .map(|n| lower_node(n, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        "accessor_list" => {
            // `{ get; set; }` — flatten accessors with list tagging.
            let children: Vec<SyntaxTree> = node.named_children()
                .map(|n| lower_node(n, source))
                .collect();
            SyntaxTree::Inline { children, list_name: Some("accessors"), range, span }
        }
        "accessor_declaration" => lower_accessor_declaration(node, source),
        "this" | "this_expression" | "base" | "base_expression" => {
            // `this` / `base` keyword — atomic name leaf.
            name_of(node, source)
        }

        // Bare `variable_declaration` (e.g. inside event_field_declaration).
        // Lower as if it were a local_declaration_statement; the parent
        // element provides the keyword context.
        "variable_declaration" => {
            let element_name: &'static str = "variable";
            let modifiers = Modifiers::default();
            let type_node = node.child_by_field_name("type");
            let declarators: Vec<&RawNode> = node.named_children()
                .filter(|n| n.kind() == "variable_declarator")
                .collect();
            if declarators.len() == 1 {
                let d = declarators[0];
                lower_variable_declarator(d, type_node, source, range, span, element_name, modifiers, Vec::new())
            } else if !declarators.is_empty() {
                let children: Vec<SyntaxTree> = declarators.into_iter().map(|d| {
                    lower_variable_declarator(
                        d, type_node, source,
                        range_of(d), span_of(d), element_name, modifiers, Vec::new(),
                    )
                }).collect();
                SyntaxTree::Inline { children, list_name: None, range, span }
            } else {
                SyntaxTree::Unknown { kind: "variable_declaration(no declarators)".to_string(), range, span }
            }
        }

        // `var x = expr;` / `int x = expr;` / `int x;` —
        // local_declaration_statement contains a variable_declaration
        // which contains type + variable_declarator. Each
        // variable_declarator is one SyntaxTree::Variable. For multi-variable
        // declarations (`int a, b = 1;`), produce multiple variables.
        "local_declaration_statement" | "field_declaration" => {
            // `<variable>` for locals; `<field>` for class fields —
            // matches the imperative pipeline's distribute-and-rename.
            let is_field = node.kind() == "field_declaration";
            let element_name: &'static str = if is_field { "field" } else { "variable" };
            // Fields take their access from `private` (default for
            // class members); locals don't carry access modifiers.
            let modifiers = if is_field {
                lower_csharp_modifiers(node, source, Some(Access::Private))
            } else {
                lower_csharp_modifiers(node, source, None)
            };
            // Attributes on field declarations: each `attribute_list`
            // child of the field is a top-level `[A]` or `[A][B]`
            // group; flatten the attributes into one Vec.
            let decorators: Vec<SyntaxTree> = node.named_children()
                .filter(|c| c.kind() == "attribute_list")
                .flat_map(|al| {
                    al.named_children()
                        .filter(|c| c.kind() == "attribute")
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            let var_decl = node.named_children()
                .find(|c| c.kind() == "variable_declaration");
            match var_decl {
                Some(vd) => {
                    let type_node = vd.child_by_field_name("type");
                    let declarators: Vec<&RawNode> = vd.named_children()
                        .filter(|n| n.kind() == "variable_declarator")
                        .collect();
                    if declarators.len() == 1 {
                        let d = declarators[0];
                        lower_variable_declarator(d, type_node, source, range, span, element_name, modifiers, decorators)
                    } else {
                        // Multi-declarator: attributes attach to the
                        // outer Inline; per-declarator decorators stay
                        // empty (they share the same group).
                        let mut children: Vec<SyntaxTree> = decorators;
                        children.extend(declarators.into_iter().map(|d| {
                            lower_variable_declarator(
                                d, type_node, source,
                                range_of(d), span_of(d),
                                element_name, modifiers, Vec::new(),
                            )
                        }));
                        SyntaxTree::Inline { children, list_name: None, range, span }
                    }
                }
                None => SyntaxTree::Unknown {
                    kind: "local_declaration_statement(no var_decl)".to_string(),
                    range, span,
                },
            }
        }

        // C#-specific wrappers ------------------------------------------

        // `global_statement` wraps top-level statements in C# 9+. Just
        // unwrap to the inner. Existing pipeline handles this similarly.
        "global_statement" => {
            let inner = node.named_children().next();
            match inner {
                Some(n) => lower_node(n, source),
                None => SyntaxTree::Unknown { kind: "global_statement(empty)".to_string(), range, span },
            }
        }

        // `expression_statement` — wrap in <expression> host
        // (Principle #15) when its inner is a value-producing
        // expression. Skip the wrap for assignment-style and other
        // statement-level kinds (matches Python's bypass logic).
        "expression_statement" => {
            let inner = node.named_children().next();
            match inner {
                Some(n) => {
                    // Bypass list: kinds that already produce their own
                    // statement-shaped tree or expression wrapper.
                    // postfix_unary_expression, await_expression and
                    // is_pattern_expression all produce SyntaxTree::Expression
                    // hosts themselves — wrapping again would yield
                    // <expression><expression>...</expression></expression>.
                    // Don't double-wrap when the inner already produces
                    // an SyntaxTree::Expression host. postfix_unary_expression
                    // produces <expression> only for `obj!` (non-null);
                    // `i++` becomes <unary>, which DOES need wrapping.
                    let bypass = matches!(
                        n.kind(),
                        "assignment_expression"
                            | "throw_expression"
                            | "await_expression"
                            | "is_pattern_expression"
                    ) || (n.kind() == "postfix_unary_expression"
                          && n.utf8_text(source).trim_end().ends_with('!'));
                    if bypass {
                        lower_node(n, source)
                    } else {
                        SyntaxTree::Expression {
                            inner: Box::new(lower_node(n, source)),
                            marker: None,
                            range, span,
                        }
                    }
                }
                None => SyntaxTree::Unknown { kind: "expression_statement(empty)".to_string(), range, span },
            }
        }

        // tree-sitter ERROR nodes appear when the parser couldn't
        // recover. Common when feeding test snippets that aren't
        // valid C# at the top level. Pass children through as-is so
        // the structural view shows useful content.
        "ERROR" => {
            let children: Vec<SyntaxTree> = node.named_children()
                .map(|c| lower_node(c, source))
                .collect();
            // Single child: unwrap (avoid double-nesting).
            if children.len() == 1 {
                children.into_iter().next().unwrap()
            } else {
                SyntaxTree::Inline { children, list_name: None, range, span }
            }
        }

        // ----- Atoms ----------------------------------------------------

        "identifier" => name_of(node, source),
        "predefined_type" => name_of(node, source), // int / string / bool / etc.
        "integer_literal" => int_of(node, source),
        "real_literal" => float_of(node, source),
        // C# strings: `string_literal`, `verbatim_string_literal`,
        // `interpolated_string_text`. For the slice we treat all as
        // `SyntaxTree::String` with verbatim source text.
        "string_literal"
        | "verbatim_string_literal"
        | "raw_string_literal"
        | "character_literal" => string_of(node, source),
        "boolean_literal" => {
            // C# boolean literals render as `<bool>true</bool>` /
            // `<bool>false</bool>` (matches the imperative pipeline's
            // `bool` element). Python uses `<true>`/`<false>`.
            SyntaxTree::SimpleStatement {
                element_name: "bool",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: Vec::new(),
                range,
                span,
            }
        }
        "null_literal" => null_of(node, source),

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
                        SyntaxTree::ObjectAccess { receiver, mut segments, range: _, span: _ } => {
                            segments.push(segment);
                            SyntaxTree::ObjectAccess { receiver, segments, range, span }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "base"]),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => SyntaxTree::Unknown {
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
            let subscript_node = node.child_by_field_name("subscript");
            let indices: Vec<SyntaxTree> = match subscript_node {
                Some(s) => {
                    s.named_children()
                        .map(|n| {
                            // `argument` → unwrap to inner expression
                            if n.kind() == "argument" {
                                let inner = n.named_children().next();
                                inner.map(|i| lower_node(i, source))
                                    .unwrap_or_else(|| SyntaxTree::Inline { children: Vec::new(), list_name: None, range: range_of(n), span: span_of(n) })
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
                        SyntaxTree::ObjectAccess { receiver, mut segments, range: _, span: _ } => {
                            segments.push(segment);
                            SyntaxTree::ObjectAccess { receiver, segments, range, span }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "base"]),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                None => SyntaxTree::Unknown {
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
        // shape for `a.b()`. Otherwise emit a standalone `SyntaxTree::Call`.
        "invocation_expression" => {
            let function_node = node.child_by_field_name("function");
            let arguments_node = node.child_by_field_name("arguments");
            let callee = match function_node {
                Some(f) => lower_node(f, source),
                None => return SyntaxTree::Unknown {
                    kind: "invocation_expression(missing function)".to_string(),
                    range,
                    span,
                },
            };
            let arguments: Vec<SyntaxTree> = match arguments_node {
                Some(a) => {
                    a.named_children()
                        .map(|c| {
                            if c.kind() == "argument" {
                                let inner = c.named_children().next();
                                inner.map(|i| lower_node(i, source))
                                    .unwrap_or_else(|| SyntaxTree::Inline { children: Vec::new(), list_name: None, range: range_of(c), span: span_of(c) })
                            } else {
                                lower_node(c, source)
                            }
                        })
                        .collect()
                }
                None => Vec::new(),
            };
            // Chain-fold: if the callee is an access chain, append a
            // Call segment. When the chain's last step was a Member
            // (`.Method`), absorb its property name into the Call so
            // the rendered `<call>` carries the `<name>Method</name>`
            // child — matches the imperative pipeline's
            // `<object[access]><name>obj</name><call><name>Method</name>...</call>`
            // shape. Otherwise standalone Call.
            match callee {
                SyntaxTree::ObjectAccess { receiver, mut segments, range: _, span: _ } => {
                    // Extract method name from the trailing Member, if
                    // any, and shorten that segment's range so the
                    // call's range starts where the member did.
                    let mut call_start = segments.last().map(|s| s.range().end).unwrap_or(receiver.range().end);
                    let (call_name, call_name_span) = match segments.last() {
                        Some(AccessSegment::Member { property_range, property_span, optional: false, range: m_range, .. }) => {
                            let pr = *property_range;
                            let ps = *property_span;
                            call_start = m_range.start;
                            segments.pop();
                            (Some(pr), Some(ps))
                        }
                        _ => (None, None),
                    };
                    let segment_range = ByteRange::new(call_start, range.end);
                    segments.push(AccessSegment::Call {
                        name: call_name,
                        name_span: call_name_span,
                        arguments,
                        range: segment_range,
                        span,
                    });
                    SyntaxTree::ObjectAccess { receiver, segments, range, span }
                }
                callee => SyntaxTree::Call {
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
            match (left, right, op_kind(&op_text)) {
                (Some(l), Some(r), Some(kind)) => {
                    kind.build_binary(l, r, op_text, op_range, range, span)
                }
                _ => SyntaxTree::Unknown {
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
            // `obj!` (non-null assertion — Principle #15 marker on
            // `<expression>` host) vs `i++` / `i--` (postfix
            // increment/decrement — `<unary>` with `<op><increment/>`
            // and a `<postfix/>` marker).
            let kids: Vec<&RawNode> = node.children().collect();
            let op_node = kids.iter().copied().find(|c| !c.is_named());
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.end));
            let operand_node = kids.iter().copied().find(|c| c.is_named());
            match (operand_node, op_text.as_str()) {
                (Some(o), "!") => SyntaxTree::Expression {
                    inner: Box::new(lower_node(o, source)),
                    marker: Some("non_null"),
                    range, span,
                },
                (Some(o), _) => match op_marker(&op_text) {
                    Some(marker) => SyntaxTree::Unary {
                        op_text,
                        op_marker: marker,
                        op_range,
                        operand: Box::new(lower_node(o, source)),
                        extra_markers: vec![Marker::implicit("postfix")],
                        range,
                        span,
                    },
                    None => simple_statement(node, "unary", source),
                },
                _ => simple_statement(node, "unary", source),
            }
        }

        // ----- as-expression `x as T` -----------------------------------
        // tree-sitter: `as_expression` with two named children
        // (value, type). Render as `<as>` element. Cast-like.
        // `x as T` — same element as `is_expression` to match the
        // imperative pipeline's "is/as" grouping.
        "as_expression" => simple_statement(node, "is", source),

        // ----- anonymous_method_expression -----------------------------
        // `delegate(int x) { return x; }` — older form of lambda. Same
        // shape as SyntaxTree::Lambda would be ideal, but for parity-track
        // we just render as <lambda> via SimpleStatement.
        "anonymous_method_expression" => simple_statement(node, "lambda", source),

        // ----- preprocessor argument ----------------------------------

        // ----- is-expression `x is Type` --------------------------------
        //
        // tree-sitter: `is_expression` with two named children
        // (value, type-or-pattern). The `is` keyword is anonymous.
        // For now only the simple type form (`is int`, `is Widget`)
        // is covered; pattern forms (`is Widget w`, `is null`, etc.)
        // would extend the right side.
        "is_expression" => {
            let kids: Vec<&RawNode> = node.named_children().collect();
            if kids.len() == 2 {
                SyntaxTree::Is {
                    value: Box::new(lower_node(kids[0], source)),
                    type_target: Box::new(lower_node(kids[1], source)),
                    range,
                    span,
                }
            } else {
                SyntaxTree::Unknown {
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
            let kids: Vec<&RawNode> = node.named_children().collect();
            if kids.len() == 2 {
                SyntaxTree::Cast {
                    type_ann: Box::new(lower_node(kids[0], source)),
                    value: Box::new(lower_node(kids[1], source)),
                    range,
                    span,
                }
            } else {
                SyntaxTree::Unknown {
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
            let operand = node.named_children().next();
            match operand {
                Some(o) => SyntaxTree::Expression {
                    inner: Box::new(lower_node(o, source)),
                    marker: Some("await"),
                    range,
                    span,
                },
                None => SyntaxTree::Unknown {
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
                let r = node.named_children().next();
                r
            });
            let op_node = node.child_by_field_name("operator").or_else(|| {
                let r = node.children().find(|c| !c.is_named());
                r
            });
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let operand = operand_node.map(|n| lower_node(n, source));
            // `<prefix/>` marker only for `++`/`--` (which have a
            // postfix counterpart). Bare `-x`/`!x`/`~x` are
            // unambiguously prefix; the marker would just be noise.
            let extra_markers: Vec<crate::tree::types::Marker> = match op_text.as_str() {
                "++" | "--" => vec![Marker::implicit("prefix")],
                _ => Vec::new(),
            };
            match (operand, op_marker(&op_text)) {
                (Some(o), Some(marker)) => SyntaxTree::Unary {
                    op_text,
                    op_marker: marker,
                    op_range,
                    operand: Box::new(o),
                    extra_markers,
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: format!("prefix_unary_expression(op={:?})", op_text),
                    range,
                    span,
                },
            }
        }

        // Fallback ------------------------------------------------------
        other => SyntaxTree::Unknown {
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
    node: &RawNode,
    source: &str,
    default_access: Option<Access>,
) -> Modifiers {
    use crate::tree::types::Flag;
    let modifier_nodes: Vec<&RawNode> = node.named_children()
        .filter(|c| c.kind() == "modifier")
        .collect();
    let words: Vec<&str> = modifier_nodes.iter()
        .map(|c| c.utf8_text(source))
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

    // Boolean flags carry the source-position of their keyword. The
    // XML projection uses these spans to position each empty marker
    // element where the keyword appeared in the source.
    for n in &modifier_nodes {
        let text = n.utf8_text(source);
        let flag = Flag::anchored(range_of(n), span_of(n));
        match text {
            "static"   => m.static_   = flag,
            "abstract" => m.abstract_ = flag,
            "sealed"   => m.sealed    = flag,
            "virtual"  => m.virtual_  = flag,
            "override" => m.override_ = flag,
            "readonly" => m.readonly  = flag,
            "partial"  => m.partial   = flag,
            "async"    => m.async_    = flag,
            "const"    => m.const_    = flag,
            "extern"   => m.extern_   = flag,
            "unsafe"   => m.unsafe_   = flag,
            "volatile" => m.volatile  = flag,
            "new"      => m.new_      = flag,
            "required" => m.required  = flag,
            // access keywords already handled above.
            "public" | "private" | "protected" | "internal" | "file" => {}
            _ => {} // unknown — ignored for now.
        }
    }
    m
}

/// Lower a C# `parameter_list` into a Vec of `SyntaxTree::Parameter` (and any
/// other parameter-like kinds we add later). Skips punctuation; only
/// keeps named children of kind `parameter`.
fn lower_csharp_parameter_list(node: Option<&RawNode>, source: &str) -> Vec<SyntaxTree> {
    let Some(n) = node else { return Vec::new() };
    n.named_children()
        .filter(|c| c.kind() == "parameter")
        .map(|c| lower_node(c, source))
        .collect()
}

/// Lower an `accessor_declaration` (`get`, `set`, `init` inside a
/// property's `{ ... }`).
fn lower_accessor_declaration(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    // The kind keyword (`get`/`set`/`init`) is an unnamed token
    // child. Find it by scanning unnamed children.
    let mut kind = crate::tree::types::AccessorKind::Get;  // default fallback
    for c in node.children() {
        if !c.is_named() {
            let t = c.utf8_text(source);
            match t {
                "get" => { kind = crate::tree::types::AccessorKind::Get; break; }
                "set" => { kind = crate::tree::types::AccessorKind::Set; break; }
                "init" => { kind = crate::tree::types::AccessorKind::Init; break; }
                _ => {}
            }
        }
    }
    let modifiers = lower_csharp_modifiers(node, source, None);
    let body_node = node.child_by_field_name("body")
        .or_else(|| {
            // Expression-bodied accessor (`get => expr;`)
            let r = node.named_children().find(|n| n.kind() == "arrow_expression_clause");
            r
        });
    let body = body_node.map(|b| Box::new(lower_node(b, source)));
    SyntaxTree::Accessor { modifiers, kind, body, range, span }
}

/// Lower a control-flow consequence (the body of `if`/`while`/`for`/
/// `foreach`/`do`). C# allows either a `block` or a single statement.
/// For the single-statement form we wrap it in a synthetic `SyntaxTree::Body`
/// covering exactly the statement's range, so all renderer arms can
/// expect `Body`.
fn lower_csharp_consequence(node: &RawNode, source: &str) -> SyntaxTree {
    if node.kind() == "block" {
        lower_block_like(node, source)
    } else {
        let r = range_of(node);
        let s = span_of(node);
        SyntaxTree::Body {
            children: vec![lower_node(node, source)],
            pass_only: false,
            block_wrap: false,
            range: r,
            span: s,
        }
    }
}

/// Lower the `alternative` field of an `if_statement` to an
/// `SyntaxTree::ElseIf` (chained else-if) or `SyntaxTree::Else` (terminal else).
///
/// tree-sitter-c-sharp exposes the alternative directly (no
/// intervening `else_clause` kind): for `else if`, it's another
/// `if_statement`; for terminal `else`, it's a `block` or single
/// statement. Older grammars wrap in `else_clause` — handled
/// transparently by unwrapping.
fn lower_csharp_else_chain(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    // Unwrap `else_clause` if present (older grammars).
    let unwrapped = if node.kind() == "else_clause" {
        node.named_children().next()
    } else {
        Some(node)
    };
    let inner_node = match unwrapped {
        Some(n) => n,
        None => return SyntaxTree::Unknown { kind: "else_clause(empty)".to_string(), range, span },
    };
    if inner_node.kind() == "if_statement" {
        let cond = inner_node.child_by_field_name("condition")
            .map(|n| Box::new(lower_node(n, source).wrap_expression()));
        let body = inner_node.child_by_field_name("consequence")
            .map(|n| Box::new(lower_csharp_consequence(n, source)));
        let else_node = inner_node.child_by_field_name("alternative").or_else(|| {
            let r = inner_node.named_children().find(|c| c.kind() == "else_clause");
            r
        });
        let else_branch = else_node.map(|a| Box::new(lower_csharp_else_chain(a, source)));
        match (cond, body) {
            (Some(c), Some(b)) => SyntaxTree::ElseIf {
                condition: c, body: b, else_branch, range, span,
            },
            _ => SyntaxTree::Unknown { kind: "else_if(missing)".to_string(), range, span },
        }
    } else {
        SyntaxTree::Else {
            body: Box::new(lower_csharp_consequence(inner_node, source)),
            range, span,
        }
    }
}

/// Walk up the CST to find the enclosing type declaration kind
/// (class/struct/interface/record). Used to pick the correct default
/// access modifier for members (interface members default to Public,
/// class/struct/record members to Private).
///
/// Returns a `'static str` because the result is always one of four
/// fixed kind names (matched on the parent's `kind()`); we re-emit the
/// literal rather than borrowing from the parent.
fn enclosing_type_kind(node: &RawNode) -> Option<&'static str> {
    PARENT_MAP.with(|p| {
        let map = p.borrow();
        let mut cur_id = node.id();
        while let Some(&parent_ptr) = map.get(&cur_id) {
            // Safety: pointers populated by `populate_parent_map`
            // refer to nodes owned by the `RawNode` tree passed to
            // `lower_csharp_root`, which lives for the duration of
            // that call. The map is cleared on exit.
            let parent = unsafe { &*parent_ptr };
            match parent.kind() {
                "class_declaration" => return Some("class_declaration"),
                "struct_declaration" => return Some("struct_declaration"),
                "interface_declaration" => return Some("interface_declaration"),
                "record_declaration" => return Some("record_declaration"),
                _ => cur_id = parent.id(),
            }
        }
        None
    })
}

/// Lower a keyword-prefixed simple statement / expression
/// (`yield`, `lock`, `goto`, `typeof`, etc.) to
/// `SyntaxTree::SimpleStatement`. Children are the CST's named children
/// lowered recursively, with field-aware wrapping that mirrors
/// the imperative pipeline's `apply_field_wrappings` pass.
/// Modifiers are extracted from any `modifier` child nodes —
/// declaration-shaped kinds (delegate/event/indexer/destructor)
/// need them; statement kinds simply have none.
/// Lower C# `throw expr;` / `throw expr` (expression form) so the
/// thrown expression sits under an `<expression>` host
/// (Principle #5 — matches return/raise/yield/throw across
/// languages). Bare `throw;` (rethrow inside a catch) keeps an
/// empty `<throw/>` so the empty-element pass folds it to a
/// marker.
fn lower_csharp_throw(node: &RawNode, source: &str) -> SyntaxTree {
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

fn simple_statement(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = lower_csharp_modifiers(node, source, None);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.children() {
        if c.is_named() && c.kind() != "modifier" {
            let field_name = c.field_name();
            let inner = lower_node(c, source);
            children.push(maybe_wrap_field(field_name, inner));
        }
    }
    SyntaxTree::SimpleStatement { element_name, modifiers, extra_markers: Vec::new(), children, range, span }
}

/// Lower `event_field_declaration` as `SyntaxTree::Variable` with
/// `element_name: "event"` so the event's type + name render as
/// direct children of `<event>` (just like `<field>` /
/// `<property>` / `<parameter>` do). Without this, the CST's
/// nested `variable_declaration` leaks as
/// `<event><variable><type>…</type><name>…</name></variable></event>`
/// — the inner `<variable>` is an abstract supertype wrapper the
/// developer never queries for (Principle #11), and the shape
/// diverges from `<field>` for the same conceptual role
/// (Principle #5).
fn lower_event_field_declaration(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = lower_csharp_modifiers(node, source, Some(Access::Private));
    // Pull type + name out of the nested `variable_declaration`.
    let mut type_ir: Option<Box<SyntaxTree>> = None;
    let mut name_ir: Option<Box<SyntaxTree>> = None;
    for c in node.named_children() {
        if c.kind() != "variable_declaration" { continue; }
        for sub in c.named_children() {
            match sub.kind() {
                "variable_declarator" => {
                    if let Some(n) = sub.child_by_field_name("name") {
                        name_ir = Some(Box::new(name_of(n, source)));
                    }
                }
                _ => {
                    // First non-declarator child is the type.
                    if type_ir.is_none() {
                        type_ir = Some(Box::new(lower_node(sub, source)));
                    }
                }
            }
        }
    }
    let name = name_ir.unwrap_or_else(|| Box::new(SyntaxTree::Unknown {
        kind: "event(missing name)".to_string(),
        range, span,
    }));
    SyntaxTree::Event {
        modifiers,
        decorators: Vec::new(),
        type_ann: type_ir,
        name,
        value: None,
        range,
        span,
    }
}

/// Like `simple_statement` but adds explicit `<marker/>` siblings.
/// Used for pattern combinators (`and`/`or`) and keyword-bearing
/// elements (`stackalloc`, `ref`) where the imperative pipeline
/// emits a marker matching the anonymous keyword in the text.
fn simple_statement_marked(
    node: &RawNode,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let modifiers = lower_csharp_modifiers(node, source, None);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.children() {
        if c.is_named() && c.kind() != "modifier" {
            let field_name = c.field_name();
            let inner = lower_node(c, source);
            children.push(maybe_wrap_field(field_name, inner));
        }
    }
    SyntaxTree::SimpleStatement { element_name, modifiers, extra_markers, children, range, span }
}

/// Merge `where T : ...` clauses into the matching `<generic>` item.
/// Each `where` clause is a `SimpleStatement::where` whose first
/// `SyntaxTree::Name` child names the target type parameter; remaining
/// `SimpleStatement::constraint` children are translated into markers
/// and `<extends>` wrappers and appended to the matching item's
/// children. Mirrors the (now-removed) `attach_ir_where_clauses` post
/// transform, but operates on typed tree before xot rendering — so the
/// generic param renders with its constraints already attached.
fn fold_csharp_where_clauses_into_generics(
    mut generics: Vec<SyntaxTree>,
    where_clauses: Vec<SyntaxTree>,
    source: &str,
) -> Vec<SyntaxTree> {
    if where_clauses.is_empty() || generics.is_empty() { return generics; }
    for clause in where_clauses {
        let SyntaxTree::SimpleStatement { children: clause_children, .. } = clause else { continue };
        // First Name child is the target generic-param name.
        let target_name = clause_children.iter().find_map(|c| match c {
            SyntaxTree::Name { text, .. } => Some(text.clone()),
            _ => None,
        });
        let Some(target_name) = target_name else { continue };
        // Find the matching generic item by its first Name child.
        let target_idx = generics.iter().position(|i| {
            csharp_generic_item_name(i, source).as_deref() == Some(target_name.as_str())
        });
        let Some(target_idx) = target_idx else { continue };
        // Anchor every synthesized marker's range at the generic
        // item's `range.end` so `render_with_gaps` doesn't emit
        // gap text from the item's last child to a marker placed
        // anywhere outside the item's own range.
        let anchor = generics[target_idx].range().end;
        // Translate each constraint into a child of the generic item.
        for c in clause_children {
            let SyntaxTree::SimpleStatement { element_name: "constraint", children, range, span, .. } = c else { continue };
            if let Some(translated) = translate_csharp_constraint(children, range, anchor, span, source) {
                if let SyntaxTree::SimpleStatement { children: target_children, .. } = &mut generics[target_idx] {
                    target_children.push(translated);
                }
            }
        }
    }
    generics
}

/// Extract the source text of the first `SyntaxTree::Name` child of a
/// `<generic>` item (which is a `SimpleStatement::generic` produced
/// by `type_parameter`).
fn csharp_generic_item_name(item: &SyntaxTree, source: &str) -> Option<String> {
    let SyntaxTree::SimpleStatement { children, .. } = item else { return None };
    children.iter().find_map(|c| match c {
        SyntaxTree::Name { text, .. } => Some(text.clone()),
        _ => None,
    })
}

/// Translate one `<constraint>`'s children into the corresponding
/// generic-parameter child:
/// - `new()` (a `SimpleStatement::new` child) → empty `<new/>` marker
/// - type bound (a `<type>`-shaped child: `GenericType`,
///   `FieldWrap("type", _)`, or `SimpleStatement::type`) →
///   `<extends>type</extends>` (no double-wrap if already typed)
/// - bare keyword (`class` / `struct` / `notnull` / `unmanaged` —
///   detected by source text) → empty marker by that name
///
/// Returned tree uses zero-width ranges anchored at `range.start` so the
/// markers contribute no source text — `render_tree_class` emits the
/// where-clause source bytes as gap text under `<class>` (see the
/// `CSlot::Where` branch), and these merged markers add structure
/// without duplicating bytes.
fn translate_csharp_constraint(
    children: Vec<SyntaxTree>,
    constraint_range: ByteRange,
    anchor: u32,
    span: Span,
    source: &str,
) -> Option<SyntaxTree> {
    let zero = ByteRange::empty_at(anchor);
    let has_new = children.iter().any(|c| matches!(c, SyntaxTree::SimpleStatement { element_name: "new", .. }));
    if has_new {
        return Some(empty_csharp_marker("new", zero, span));
    }
    let type_idx = children.iter().position(|c| matches!(c,
        SyntaxTree::GenericType { .. }
            | SyntaxTree::SimpleStatement { element_name: "type", .. }
            | SyntaxTree::FieldWrap { wrapper: "type", .. }
    ));
    if let Some(idx) = type_idx {
        let mut children = children;
        let type_ir = children.swap_remove(idx);
        // `<extends>` is structural-only inside `<generic>`. Its
        // inner `type_ir` keeps its original range so `<name>`
        // / `<type>` leaves can carry the bound's text — the
        // bound's source bytes also flow through the class-level
        // where-clause gap text, accepting a minor duplication
        // bounded to the bound type's identifier in exchange for
        // a queryable structural shape.
        return Some(SyntaxTree::SimpleStatement {
            element_name: "extends",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![type_ir],
            range: zero,
            span,
        });
    }
    let trimmed = constraint_range.slice(source).trim();
    match trimmed {
        "class" | "struct" | "notnull" | "unmanaged" => {
            let name: &'static str = match trimmed {
                "class"     => "class",
                "struct"    => "struct",
                "notnull"   => "notnull",
                "unmanaged" => "unmanaged",
                _ => unreachable!(),
            };
            Some(empty_csharp_marker(name, zero, span))
        }
        _ => None,
    }
}

fn empty_csharp_marker(element_name: &'static str, range: ByteRange, span: Span) -> SyntaxTree {
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children: Vec::new(),
        range,
        span,
    }
}

/// Wrap an tree node in `SyntaxTree::FieldWrap` if its tree-sitter field name
/// has an entry in the C# field-wrapping table. Mirrors the
/// imperative pipeline's `apply_field_wrappings` pass, but applied
/// at lowering time so the tree is already correctly nested.
fn maybe_wrap_field(field_name: Option<&str>, inner: SyntaxTree) -> SyntaxTree {
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
    // Skip the wrap when it would produce nested same-name elements
    // (e.g. `<name><name>Foo</name></name>` for a `name=identifier`
    // field). The inner tree already renders as the wrapper name, so
    // wrapping again is pure noise — appears in JSON as the
    // `"name": {"name": "Foo"}` shape that the user explicitly
    // flagged as forbidden.
    let already_wrapped = matches!(
        (wrapper, &inner),
        ("name", SyntaxTree::Name { .. })
            | ("type", SyntaxTree::SimpleStatement { element_name: "type", .. })
            | ("type", SyntaxTree::GenericType { .. })
            | ("body", SyntaxTree::Body { .. })
    );
    if already_wrapped {
        return inner;
    }
    let r = inner.range();
    let s = inner.span();
    SyntaxTree::FieldWrap { wrapper, inner: Box::new(inner), range: r, span: s }
}

/// Lower a C# `catch_clause` to `SyntaxTree::ExceptHandler` with kind="catch".
/// Children:
///   - optional `catch_declaration` containing type and optional binding
///   - optional `catch_filter_clause` (the `when (...)` form)
///   - `block` (the handler body)
fn lower_csharp_catch_clause(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut type_target: Option<Box<SyntaxTree>> = None;
    let mut binding: Option<Box<SyntaxTree>> = None;
    let mut filter: Option<Box<SyntaxTree>> = None;
    let mut body: Option<Box<SyntaxTree>> = None;
    for c in node.named_children() {
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
                    binding = Some(Box::new(name_of(n, source).wrap_expression()));
                }
            }
            "catch_filter_clause" => {
                // First named child is the filter expression.
                let inner = c.named_children().next();
                if let Some(i) = inner {
                    filter = Some(Box::new(lower_node(i, source).wrap_expression()));
                }
            }
            "block" if body.is_none() => {
                body = Some(Box::new(lower_block_like(c, source)));
            }
            _ => {}
        }
    }
    SyntaxTree::Catch {
        type_target,
        binding,
        filter,
        body: body.unwrap_or_else(|| Box::new(SyntaxTree::Body { children: Vec::new(), pass_only: false, block_wrap: false, range: ByteRange::empty_at(range.end), span })),
        range, span,
    }
}

/// Lower a `block` or `declaration_list` into `SyntaxTree::Body`. C# wraps
/// block contents in an inner `<block>` element matching the
/// imperative pipeline's apply_field_wrappings output: a `block`
/// CST node with `field=body` becomes `<body><block>{stmts}</block></body>`.
fn lower_block_like(node: &RawNode, source: &str) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node.named_children()
        .map(|c| lower_node(c, source))
        .collect();
    let children = merge_adjacent_line_comments(children, source);
    let block_wrap = node.kind() == "block";
    SyntaxTree::Body {
        children,
        pass_only: false,
        block_wrap,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Recursively collect identifier segments from a `qualified_name`
/// CST node, producing a flat list of `SyntaxTree::Name`. The grammar nests
/// `qualified_name(qualified_name(a, b), c)` for `a.b.c`; we want
/// flat `[a, b, c]`.
fn collect_qualified_name_segments(node: &RawNode, source: &str, out: &mut Vec<SyntaxTree>) {
    for c in node.named_children() {
        match c.kind() {
            "qualified_name" => collect_qualified_name_segments(c, source, out),
            "identifier" => out.push(name_of(c, source)),
            _ => out.push(lower_node(c, source)),
        }
    }
}

/// Group consecutive `SyntaxTree::Comment` children that are line comments on
/// adjacent lines into a single comment whose range spans them all,
/// then classify each comment as `trailing` (same line as preceding
/// code), `leading` (immediately precedes a non-comment sibling on
/// the next line), or floating (neither). Mirrors the imperative
/// pipeline's `classify_and_group` behaviour.
fn merge_adjacent_line_comments(children: Vec<SyntaxTree>, source: &str) -> Vec<SyntaxTree> {
    // Phase 1: merge runs of adjacent line comments. Don't merge
    // across a "trailing" boundary — a `// trailing` line comment
    // attached to preceding code shouldn't absorb the next leading
    // group.
    let mut out: Vec<SyntaxTree> = Vec::with_capacity(children.len());
    for child in children {
        if let SyntaxTree::Comment { leading, trailing, range, span } = child {
            // Determine if this new comment would be a trailing one
            // given the *previous non-comment* sibling — needed to
            // avoid merging a trailing single-line comment with the
            // following leading group.
            let prev_non_comment = out.iter().rev()
                .find(|c| !matches!(c, SyntaxTree::Comment { .. }));
            let curr_is_trailing = prev_non_comment.map_or(false, |prev| {
                let prev_end = prev.range().end as usize;
                let between = &source[prev_end..range.start as usize];
                // Trailing iff no newline between the previous code
                // and this comment.
                !between.contains('\n')
            });

            if let Some(SyntaxTree::Comment { range: prev_range, .. }) = out.last() {
                let gap = &source[prev_range.end as usize..range.start as usize];
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_is_line_comment = source[prev_range.start as usize..prev_range.end as usize]
                    .trim_start().starts_with("//");
                let curr_is_line_comment = source[range.start as usize..range.end as usize]
                    .trim_start().starts_with("//");
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: Flag::On { .. }, .. }));
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

    // Phase 2: mark a comment as `leading = true` iff the next
    // non-comment sibling starts on the very next line (no blank
    // line in between) AND it isn't already classified as trailing.
    let n = out.len();
    for i in 0..n {
        if let SyntaxTree::Comment { trailing, range, span, .. } = &out[i] {
            if trailing.is_set() { continue; }
            let comment_end = range.end as usize;
            let comment_range = *range;
            let comment_span = *span;
            // Find next non-comment sibling.
            let next = out.iter().skip(i + 1).find(|c| !matches!(c, SyntaxTree::Comment { .. }));
            if let Some(next_ir) = next {
                let next_start = next_ir.range().start as usize;
                let between = &source[comment_end..next_start];
                let newlines = between.chars().filter(|&c| c == '\n').count();
                if newlines == 1 && between.chars().all(|c| c.is_whitespace()) {
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

/// Lower a `variable_declarator` into `SyntaxTree::Variable`. The type
/// annotation comes from the parent's `type` field (variable_declaration
/// holds the type for the whole declarator group).
///
/// Falls back to `SyntaxTree::Unknown` for tuple-deconstruction forms like
/// `var (a, b) = …` because tree-sitter-c-sharp gives those a `name`
/// field whose byte range *overlaps* with the implicit_type's range
/// (it spans `var (a, b)` rather than just `(a, b)`). Source-text
/// recovery would emit "var" twice. The Unknown fallback preserves
/// the round-trip invariant; structural support for tuple
/// deconstruction is a future enhancement.
fn lower_variable_declarator(
    declarator: &RawNode,
    type_node: Option<&RawNode>,
    source: &str,
    range: ByteRange,
    span: Span,
    element_name: &'static str,
    modifiers: Modifiers,
    decorators: Vec<SyntaxTree>,
) -> SyntaxTree {
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
        let direct = declarator.named_children()
            .filter(|c| {
                Some(c.id()) != name_id
                    && c.kind() != "bracketed_argument_list"
                    && c.kind() != "equals_value_clause"
            })
            .last();
        if direct.is_some() { return direct; }
        // Fallback: older grammars wrap the value in equals_value_clause.
        let eqv = declarator.named_children()
            .find(|c| c.kind() == "equals_value_clause");
        eqv.and_then(|e| {
            let r = e.named_children().next();
            r
        })
    });

    // For tuple deconstruction (`var (a, b) = pair;`) the declarator
    // has no `name` field — the binding side is the `tuple_pattern`
    // child. Lower as SyntaxTree::Variable with the pattern in the `name`
    // slot so the whole declaration is one statement-level element
    // (`<variable><type>…</type><pattern>…</pattern><value>…</value>`)
    // instead of the previous Inline-flatten that scattered the
    // `<name>var</name>` / `<pattern>` / value siblings directly
    // under the enclosing block (Principle #5 — deconstructed and
    // named declarations are the same conceptual role).
    let Some(n) = name_node else {
        let pattern_node = declarator.named_children()
            .find(|c| matches!(
                c.kind(),
                "tuple_pattern" | "declaration_pattern" | "list_pattern"
                    | "var_pattern" | "discard_pattern"
            ));
        match pattern_node {
            Some(p) => {
                let pattern_ir = Box::new(lower_node(p, source));
                let value_ir = value_node
                    .filter(|v| v.id() != p.id())
                    .map(|v| crate::tree::Expression::wrap(lower_node(v, source)));
                let type_ir = type_node.map(|t| Box::new(lower_node(t, source)));
                return SyntaxTree::variable_or_field(
                    element_name,
                    modifiers,
                    decorators,
                    type_ir,
                    pattern_ir,
                    value_ir,
                    range,
                    span,
                );
            }
            None => {
                let mut children: Vec<SyntaxTree> = Vec::new();
                if let Some(t) = type_node {
                    children.push(lower_node(t, source));
                }
                children.extend(declarator.named_children().map(|c| lower_node(c, source)));
                return SyntaxTree::Inline { children, list_name: None, range, span };
            }
        }
    };
    if let Some(t) = type_node {
        if n.byte_range().start < t.byte_range().end {
            let mut children: Vec<SyntaxTree> = vec![lower_node(t, source)];
            children.extend(declarator.named_children().map(|c| lower_node(c, source)));
            return SyntaxTree::Inline { children, list_name: None, range, span };
        }
    }

    let name_ir = name_of(n, source);
    let type_ir = type_node.map(|t| Box::new(lower_node(t, source)));
    let value_ir = value_node.map(|v| crate::tree::Expression::wrap(lower_node(v, source)));
    SyntaxTree::variable_or_field(
        element_name,
        modifiers,
        decorators,
        type_ir,
        Box::new(name_ir),
        value_ir,
        range,
        span,
    )
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
fn lower_binding_to_segments(node: &RawNode, source: &str, optional_first: bool) -> Vec<AccessSegment> {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        "member_binding_expression" => {
            // Single member segment from this binding.
            let inner = node.named_children().next();
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
            let arg_list = node.child_by_field_name("subscript");
            let indices = match arg_list {
                Some(a) => {
                    a.named_children().map(|n| {
                        if n.kind() == "argument" {
                            let inner = n.named_children().next();
                            inner.map(|i| lower_node(i, source))
                                .unwrap_or_else(|| SyntaxTree::Inline { children: Vec::new(), list_name: None, range: range_of(n), span: span_of(n) })
                        } else { lower_node(n, source) }
                    }).collect()
                }
                None => Vec::new(),
            };
            // Index segment doesn't currently support optional — but
            // we tag the eventual tree variant with optionality on the
            // PARENT chain. For this slice we wire it through a
            // future `optional` field on Index when we add it. For
            // now, mark Member-style optional only.
            // Future work: extend AccessSegment::Index with an optional flag.
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
            let subscript_node = node.child_by_field_name("subscript");
            let mut segments = match object_node {
                Some(o) => lower_binding_to_segments(o, source, optional_first),
                None => Vec::new(),
            };
            let indices = match subscript_node {
                Some(s) => {
                    s.named_children().map(|n| {
                        if n.kind() == "argument" {
                            let inner = n.named_children().next();
                            inner.map(|i| lower_node(i, source))
                                .unwrap_or_else(|| SyntaxTree::Inline { children: Vec::new(), list_name: None, range: range_of(n), span: span_of(n) })
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

fn lower_children(parent: &RawNode, source: &str) -> Vec<SyntaxTree> {
    parent
        .named_children()
        .map(|c| lower_node(c, source))
        .collect()
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

/// C# binary / logical operators → typed [`OperatorKind`]. Unary-only
/// (`!`, `~`, `++`, `--`) handled by the Unary lowering path.
fn op_kind(op: &str) -> Option<OperatorKind> {
    Some(match op {
        "+" => OperatorKind::Plus,
        "-" => OperatorKind::Minus,
        "*" => OperatorKind::Multiply,
        "/" => OperatorKind::Divide,
        "%" => OperatorKind::Modulo,
        "==" => OperatorKind::Equal,
        "!=" => OperatorKind::NotEqual,
        "<" => OperatorKind::Less,
        "<=" => OperatorKind::LessOrEqual,
        ">" => OperatorKind::Greater,
        ">=" => OperatorKind::GreaterOrEqual,
        "&&" => OperatorKind::And,
        "||" => OperatorKind::Or,
        "&" => OperatorKind::BitwiseAnd,
        "|" => OperatorKind::BitwiseOr,
        "^" => OperatorKind::BitwiseXor,
        "<<" => OperatorKind::ShiftLeft,
        ">>" => OperatorKind::ShiftRight,
        ">>>" => OperatorKind::ShiftRightUnsigned,
        "??" => OperatorKind::NullCoalesce,
        _ => return None,
    })
}
