//! Rust tree-sitter CST → IR lowering.
//!
//! Mirrors the C#/Java/TypeScript IR pipeline patterns. Each per-kind
//! arm recursively lowers children; unhandled kinds fall through to
//! `Ir::Unknown`. The renderer in `crate::ir::to_xot` is shared with
//! the other IR languages.
//!
//! Production parser routes Rust through this lowering end-to-end
//! (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/rust_lang/{rules,transformations,transform}.rs`
//! modules were retired alongside this migration.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{Access, AccessSegment, ByteRange, Ir, Modifiers, ParamKind, Span};

/// Lower a Rust tree-sitter root node to [`Ir`].
pub fn lower_rust_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "source_file" => Ir::Module {
            element_name: "file",
            children: merge_rust_line_comments(lower_children(root, source), source),
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

/// Detect the comment prefix (`//!`, `///`, `//`). Used to decide
/// whether two adjacent line comments should merge — only same-prefix
/// runs merge.
fn comment_prefix(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("//!") {
        Some("//!")
    } else if trimmed.starts_with("///") {
        Some("///")
    } else if trimmed.starts_with("//") {
        Some("//")
    } else {
        None
    }
}

/// Classify Rust comments into leading/trailing/floating. Mirrors
/// merge_java_line_comments — same `//` line-comment grammar applies.
fn merge_rust_line_comments(children: Vec<Ir>, source: &str) -> Vec<Ir> {
    let mut out: Vec<Ir> = Vec::with_capacity(children.len());
    for child in children {
        if let Ir::Comment {
            leading,
            trailing,
            range,
            span,
        } = child
        {
            let prev_non_comment = out.iter().rev().find(|c| !matches!(c, Ir::Comment { .. }));
            let curr_is_trailing = prev_non_comment.map_or(false, |prev| {
                let prev_end = prev.range().end as usize;
                let between = &source[prev_end..range.start as usize];
                !between.contains('\n')
            });
            if let Some(Ir::Comment { range: prev_range, .. }) = out.last() {
                let gap = &source[prev_range.end as usize..range.start as usize];
                // Tree-sitter rust line_comment ranges include the
                // line-terminator, so a "blank line" gap is just
                // 1 \n (the blank line's own char). Treat zero-newline
                // gap as adjacency only.
                let no_newline = !gap.contains('\n') && gap.chars().all(|c| c.is_whitespace());
                let prev_text = &source[prev_range.start as usize..prev_range.end as usize];
                let curr_text = &source[range.start as usize..range.end as usize];
                let prev_trim = prev_text.trim_start();
                let curr_trim = curr_text.trim_start();
                let prev_prefix = comment_prefix(prev_trim);
                let curr_prefix = comment_prefix(curr_trim);
                let same_prefix = prev_prefix == curr_prefix && prev_prefix.is_some();
                let prev_was_trailing = matches!(out.last(), Some(Ir::Comment { trailing: true, .. }));
                if no_newline
                    && same_prefix
                    && !prev_was_trailing
                    && !curr_is_trailing
                {
                    if let Some(Ir::Comment {
                        range: r,
                        span: s,
                        ..
                    }) = out.last_mut()
                    {
                        r.end = range.end;
                        s.end_line = span.end_line;
                        s.end_column = span.end_column;
                    }
                    continue;
                }
            }
            let trailing = trailing || curr_is_trailing;
            out.push(Ir::Comment {
                leading,
                trailing,
                range,
                span,
            });
        } else {
            out.push(child);
        }
    }
    let n = out.len();
    for i in 0..n {
        if let Ir::Comment { trailing, range, .. } = &out[i] {
            if *trailing {
                continue;
            }
            let comment_end = range.end as usize;
            let next = out.iter().skip(i + 1).find(|c| !matches!(c, Ir::Comment { .. }));
            if let Some(next_ir) = next {
                let next_start = next_ir.range().start as usize;
                let between = &source[comment_end..next_start];
                let newlines = between.chars().filter(|&c| c == '\n').count();
                // tree-sitter rust line_comment includes trailing \n,
                // so adjacency to the next item can have 0 or 1
                // newline in the gap (not the strict 1 used by C#/Java
                // where line_comment excludes the \n).
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

/// Public entry for lowering a single Rust CST node — useful for tests.
pub fn lower_rust_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "identifier" | "type_identifier" | "field_identifier"
        | "shorthand_field_identifier" | "primitive_type" | "self"
        | "super" | "super_" | "metavariable" | "label" => Ir::Name { range, span },

        // `_` wildcard pattern (unnamed token, but reachable via the
        // pattern field of let_declaration / match_arm / etc.).
        "_" => simple_statement_marked(node, "pattern", &["wildcard"], source),

        // `'a` lifetime — emit `<lifetime>` with `<name>` leaf inside.
        // The inner identifier text is the lifetime name without the
        // apostrophe.
        "lifetime" => {
            let mut cursor = node.walk();
            let identifier = node.named_children(&mut cursor).find(|c| c.kind() == "identifier");
            let children = match identifier {
                Some(id) => vec![Ir::Name { range: range_of(id), span: span_of(id) }],
                None => Vec::new(),
            };
            Ir::SimpleStatement {
                element_name: "lifetime",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }

        // Path-form identifiers (`std::collections::HashMap`).
        "scoped_identifier" | "scoped_type_identifier" => {
            simple_statement(node, "path", source)
        }

        // Literals.
        "integer_literal" => Ir::Int { range, span },
        "float_literal" => Ir::Float { range, span },
        "string_literal" => Ir::String { range, span },
        "raw_string_literal" => simple_statement_marked(node, "string", &["raw"], source),
        "char_literal" => simple_statement(node, "char", source),
        "byte_literal" => Ir::String { range, span },
        "boolean_literal" => Ir::SimpleStatement {
            element_name: "bool",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range,
            span,
        },
        "negative_literal" => simple_statement_marked(node, "literal", &["negative"], source),
        "unit_expression" => Ir::SimpleStatement {
            element_name: "literal",
            modifiers: Modifiers::default(),
            extra_markers: &["unit"],
            children: Vec::new(),
            range,
            span,
        },

        // ----- Comments ------------------------------------------------
        "line_comment" | "block_comment" | "doc_comment" => Ir::Comment {
            leading: false,
            trailing: false,
            range,
            span,
        },

        // ----- Module / file structure ---------------------------------
        // mod_item handled below in declaration block (rust_decl)

        // `use std::collections::HashMap;` — use declaration.
        "use_declaration" => simple_statement(node, "use", source),
        "use_as_clause" | "use_list" | "scoped_use_list" | "use_wildcard"
        | "use_bounds" => {
            // Wrapper grammar — flatten children into the parent.
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }
        // `extern crate alloc;`.
        "extern_crate_declaration" => simple_statement_marked(node, "use", &["extern"], source),

        // ----- Items: struct / enum / trait / impl / function / type --
        "struct_item" => rust_decl(node, "struct", source),
        "enum_item" => rust_decl(node, "enum", source),
        "enum_variant" => simple_statement(node, "variant", source),
        "enum_variant_list" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }
        "union_item" => rust_decl(node, "union", source),
        "trait_item" => {
            // `trait Foo { ... }` — wrap declaration_list body in `<body>`.
            let body_node = node.child_by_field_name("body");
            let mut cursor = node.walk();
            let mut is_pub = false;
            let mut children: Vec<Ir> = Vec::new();
            for c in node.named_children(&mut cursor) {
                if c.kind() == "visibility_modifier" {
                    if text_of(c, source).starts_with("pub") { is_pub = true; }
                    continue;
                }
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
                            children: merge_rust_line_comments(body_children, source),
                            range: range_of(c),
                            span: span_of(c),
                        });
                        continue;
                    }
                }
                children.push(lower_node(c, source));
            }
            let extra_markers: &'static [&'static str] = if is_pub { &["pub"] } else { &["private"] };
            Ir::SimpleStatement {
                element_name: "trait",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }
        "impl_item" => {
            // `impl Trait for Type { ... }` — wrap trait in `<implements>`,
            // wrap target type in `<type>` if leaf, body block to `<body>`.
            let trait_node = node.child_by_field_name("trait");
            let type_node = node.child_by_field_name("type");
            let body_node = node.child_by_field_name("body");
            let mut cursor = node.walk();
            let mut children: Vec<Ir> = Vec::new();
            for c in node.named_children(&mut cursor) {
                if let Some(t) = trait_node {
                    if c.id() == t.id() {
                        let inner = lower_node(c, source);
                        let typed = wrap_in_type_if_leaf(inner, range_of(c), span_of(c));
                        children.push(Ir::SimpleStatement {
                            element_name: "implements",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![typed],
                            range: range_of(c),
                            span: span_of(c),
                        });
                        continue;
                    }
                }
                if let Some(t) = type_node {
                    if c.id() == t.id() {
                        let inner = lower_node(c, source);
                        children.push(wrap_in_type_if_leaf(inner, range_of(c), span_of(c)));
                        continue;
                    }
                }
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
                            children: merge_rust_line_comments(body_children, source),
                            range: range_of(c),
                            span: span_of(c),
                        });
                        continue;
                    }
                }
                children.push(lower_node(c, source));
            }
            Ir::SimpleStatement {
                element_name: "impl",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "type_item" => {
            // `type Id = u32;` — alias. Detect visibility, wrap target type in `<type>`.
            let name_node = node.child_by_field_name("name");
            let type_node = node.child_by_field_name("type");
            let mut cursor = node.walk();
            let mut is_pub = false;
            for c in node.named_children(&mut cursor) {
                if c.kind() == "visibility_modifier" && text_of(c, source).starts_with("pub") {
                    is_pub = true;
                }
            }
            let mut children: Vec<Ir> = Vec::new();
            // Iterate in source order: name, type_parameters, type.
            let mut cursor2 = node.walk();
            for c in node.named_children(&mut cursor2) {
                match c.kind() {
                    "visibility_modifier" => {} // skip
                    _ => {
                        if let Some(t) = type_node {
                            if c.id() == t.id() {
                                let inner_ir = lower_node(c, source);
                                children.push(wrap_in_type_if_leaf(inner_ir, range_of(c), span_of(c)));
                                continue;
                            }
                        }
                        if let Some(n) = name_node {
                            if c.id() == n.id() {
                                children.push(Ir::Name { range: range_of(c), span: span_of(c) });
                                continue;
                            }
                        }
                        children.push(lower_node(c, source));
                    }
                }
            }
            let extra_markers: &'static [&'static str] = if is_pub { &["pub"] } else { &["private"] };
            Ir::SimpleStatement {
                element_name: "alias",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }
        "const_item" => rust_decl(node, "const", source),
        "static_item" => rust_decl(node, "static", source),
        "function_item" | "function_signature_item" => rust_function(node, source),
        "mod_item" => {
            // `mod outer { ... }` — wrap declaration_list body in `<body>`.
            let body_node = node.child_by_field_name("body");
            let mut cursor = node.walk();
            let mut is_pub = false;
            let mut pub_qual: Option<Ir> = None;
            for c in node.named_children(&mut cursor) {
                if c.kind() == "visibility_modifier" {
                    if let RustVis::PubQualified(q) = classify_rust_visibility(node, source) {
                        pub_qual = Some(q);
                        break;
                    }
                    if text_of(c, source).starts_with("pub") { is_pub = true; }
                }
            }
            let mut children: Vec<Ir> = Vec::new();
            if let Some(q) = &pub_qual {
                children.push(Ir::SimpleStatement {
                    element_name: "pub",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![q.clone()],
                    range, span,
                });
            }
            let mut cursor2 = node.walk();
            for c in node.named_children(&mut cursor2) {
                if c.kind() == "visibility_modifier" {
                    continue;
                }
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
                            children: merge_rust_line_comments(body_children, source),
                            range: range_of(c),
                            span: span_of(c),
                        });
                        continue;
                    }
                }
                children.push(lower_node(c, source));
            }
            let extra_markers: &'static [&'static str] = match (&pub_qual, is_pub) {
                (Some(_), _) => &[],
                (None, true) => &["pub"],
                (None, false) => &["private"],
            };
            Ir::SimpleStatement {
                element_name: "mod",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }
        "macro_definition" => simple_statement_marked(node, "macro", &["definition"], source),
        "macro_rule" => simple_statement(node, "arm", source),
        "macro_invocation" => simple_statement(node, "macro", source),
        "associated_type" => simple_statement_marked(node, "type", &["associated"], source),
        "type_binding" => simple_statement_marked(node, "type", &["associated"], source),

        // Visibility modifier — handled inside rust_decl. If we see one
        // standalone (orphaned), emit Inline so source bytes survive.
        "visibility_modifier" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },
        // `mut` / `const` modifiers.
        "mutable_specifier" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range,
            span,
        },
        "function_modifiers" | "extern_modifier" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },

        // ----- Field declarations / initializers -----------------------
        "field_declaration" => {
            // `pub x: T` — emit `<field>` with optional `<pub/>`/<private/>
            // marker, name, type wrapped in `<type>` if leaf.
            let name_node = node.child_by_field_name("name");
            let type_node = node.child_by_field_name("type");
            let mut cursor = node.walk();
            let mut is_pub = false;
            for c in node.named_children(&mut cursor) {
                if c.kind() == "visibility_modifier" && text_of(c, source).starts_with("pub") {
                    is_pub = true;
                }
            }
            let mut children: Vec<Ir> = Vec::new();
            let mut cursor2 = node.walk();
            for c in node.named_children(&mut cursor2) {
                match c.kind() {
                    "visibility_modifier" => {} // skipped — marker on parent
                    _ => {
                        if let Some(n) = name_node {
                            if c.id() == n.id() {
                                children.push(Ir::Name { range: range_of(c), span: span_of(c) });
                                continue;
                            }
                        }
                        if let Some(t) = type_node {
                            if c.id() == t.id() {
                                let inner = lower_node(c, source);
                                children.push(wrap_in_type_if_leaf(inner, range_of(c), span_of(c)));
                                continue;
                            }
                        }
                        children.push(lower_node(c, source));
                    }
                }
            }
            let extra_markers: &'static [&'static str] = if is_pub { &["pub"] } else { &[] };
            Ir::SimpleStatement {
                element_name: "field",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }
        "field_declaration_list" | "ordered_field_declaration_list" => {
            let mut cursor = node.walk();
            let body_children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::SimpleStatement {
                element_name: "body",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: merge_rust_line_comments(body_children, source),
                range, span,
            }
        }
        "field_initializer" => {
            // `name: value` — emit `<field><name>name</name><value><expression>value</expression></value></field>`.
            // tree-sitter rust uses `field` as the name field.
            let name_node = node.child_by_field_name("field");
            let value_node = node.child_by_field_name("value");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(n) = name_node {
                children.push(Ir::Name { range: range_of(n), span: span_of(n) });
            }
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
            Ir::SimpleStatement {
                element_name: "field",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "shorthand_field_initializer" => simple_statement(node, "field", source),
        "field_initializer_list" => simple_statement(node, "body", source),
        "base_field_initializer" => simple_statement_marked(node, "field", &["base"], source),

        // ----- Parameters ----------------------------------------------
        "parameter" => {
            // `name: T` — emit `<parameter><name>name</name><type><name>T</name></type></parameter>`.
            let pattern_node = node.child_by_field_name("pattern");
            let type_node = node.child_by_field_name("type");
            let mut children: Vec<Ir> = Vec::new();
            let mut cursor = node.walk();
            for c in node.named_children(&mut cursor) {
                if let Some(t) = type_node {
                    if c.id() == t.id() {
                        let inner = lower_node(c, source);
                        children.push(wrap_in_type_if_leaf(inner, range_of(c), span_of(c)));
                        continue;
                    }
                }
                if let Some(p) = pattern_node {
                    if c.id() == p.id() {
                        children.push(lower_node(c, source));
                        continue;
                    }
                }
                children.push(lower_node(c, source));
            }
            Ir::SimpleStatement {
                element_name: "parameter",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "self_parameter" => simple_statement_marked(node, "parameter", &["self"], source),
        "variadic_parameter" => simple_statement_marked(node, "parameter", &["variadic"], source),
        "const_parameter" => simple_statement_marked(node, "parameter", &["const"], source),
        "parameters" | "closure_parameters" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("parameters"),
            range,
            span,
        },

        // ----- Generics ------------------------------------------------
        "type_parameter" => simple_statement(node, "generic", source),
        "lifetime_parameter" => {
            // Tree-sitter wraps the lifetime in a parameter node; we
            // unwrap to a single `<lifetime>` element instead of
            // producing `<lifetime><lifetime>...</lifetime></lifetime>`.
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).find(|c| c.kind() == "lifetime");
            match inner {
                Some(l) => lower_node(l, source),
                None => Ir::SimpleStatement {
                    element_name: "lifetime",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: Vec::new(),
                    range, span,
                },
            }
        }
        "type_parameters" | "type_arguments" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range,
            span,
        },
        "where_clause" => simple_statement(node, "where", source),
        "where_predicate" => simple_statement(node, "bound", source),
        "trait_bounds" => {
            // `: Trait1 + Trait2 + 'a` — emit `<extends>` with each
            // bound wrapped in `<type>` if leaf-shaped so XPath sees
            // `extends/type[name='Trait1']`.
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| {
                    let inner = lower_node(c, source);
                    wrap_in_type_if_leaf(inner, range_of(c), span_of(c))
                })
                .collect();
            Ir::SimpleStatement {
                element_name: "extends",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "higher_ranked_trait_bound" => simple_statement_marked(node, "bound", &["higher"], source),
        "removed_trait_bound" => simple_statement_marked(node, "bound", &["optional"], source),
        "constrained_type_parameter" => simple_statement(node, "generic", source),
        "optional_type_parameter" => simple_statement(node, "generic", source),

        // ----- Type-shape grammar --------------------------------------
        "abstract_type" => simple_statement_marked(node, "type", &["abstract"], source),
        "array_type" => simple_statement_marked(node, "type", &["array"], source),
        "tuple_type" => simple_statement_marked(node, "type", &["tuple"], source),
        "unit_type" => simple_statement_marked(node, "type", &["unit"], source),
        "never_type" => simple_statement_marked(node, "type", &["never"], source),
        "function_type" => simple_statement_marked(node, "type", &["function"], source),
        "dynamic_type" => simple_statement_marked(node, "type", &["dynamic"], source),
        "pointer_type" => simple_statement_marked(node, "type", &["pointer"], source),
        "reference_type" => {
            // `&T` / `&mut T` / `&'a T` — emit `<type[borrowed]>` (with
            // `<mut/>` marker if `mut` keyword present) and wrap the
            // inner type in `<type>` if it's a leaf identifier.
            let mut cursor = node.walk();
            let mut has_mut = false;
            for c in node.children(&mut cursor) {
                if !c.is_named() && text_of(c, source) == "mut" {
                    has_mut = true;
                }
                if c.kind() == "mutable_specifier" {
                    has_mut = true;
                }
            }
            let mut cursor2 = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor2)
                .filter(|c| c.kind() != "mutable_specifier")
                .map(|c| {
                    let inner_ir = lower_node(c, source);
                    wrap_in_type_if_leaf(inner_ir, range_of(c), span_of(c))
                })
                .collect();
            let extra_markers: &'static [&'static str] =
                if has_mut { &["borrowed", "mut"] } else { &["borrowed"] };
            Ir::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }
        "bounded_type" => simple_statement_marked(node, "type", &["bounded"], source),
        "generic_type" => {
            // `Foo<T, U>` / `std::collections::HashMap<String, T>` —
            // collapse the type name into a single `<name>full text</name>`
            // (matches imperative `rewrite_generic_type` behavior). Inline
            // type_arguments with each leaf wrapped in `<type>`.
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| {
                    match c.kind() {
                        "type_identifier" | "identifier" | "scoped_type_identifier"
                        | "scoped_identifier" => {
                            // Collapse to single <name>FULL_TEXT</name>.
                            Ir::Name { range: range_of(c), span: span_of(c) }
                        }
                        "type_arguments" => {
                            let mut tc = c.walk();
                            let typed: Vec<Ir> = c
                                .named_children(&mut tc)
                                .map(|t| {
                                    let inner = lower_node(t, source);
                                    wrap_in_type_if_leaf(inner, range_of(t), span_of(t))
                                })
                                .collect();
                            Ir::Inline {
                                children: typed,
                                list_name: None,
                                range: range_of(c),
                                span: span_of(c),
                            }
                        }
                        _ => lower_node(c, source),
                    }
                })
                .collect();
            Ir::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers: &["generic"],
                children,
                range, span,
            }
        }
        "generic_type_with_turbofish" => {
            simple_statement_marked(node, "type", &["turbofish"], source)
        }
        "qualified_type" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },
        "bracketed_type" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },

        // ----- Statements ----------------------------------------------
        "let_declaration" => {
            // `let pat: T = value;` — emit
            // `<let>{mut?}<name>pat</name>{type}?<value><expression>value</expression></value></let>`.
            let pattern_node = node.child_by_field_name("pattern");
            let type_node = node.child_by_field_name("type");
            let value_node = node.child_by_field_name("value");
            let mut cursor = node.walk();
            let mut has_mut = false;
            for c in node.named_children(&mut cursor) {
                if c.kind() == "mutable_specifier" { has_mut = true; }
            }
            let mut children: Vec<Ir> = Vec::new();
            if let Some(p) = pattern_node {
                children.push(lower_node(p, source));
            }
            if let Some(t) = type_node {
                let inner = lower_node(t, source);
                children.push(wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
            }
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
            let extra_markers: &'static [&'static str] = if has_mut { &["mut"] } else { &[] };
            Ir::SimpleStatement {
                element_name: "let",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }
        "expression_statement" => {
            // Control-flow and declaration expressions surface as direct
            // siblings of the parent <body>/<block> (no <expression>
            // host). Other expressions wrap in `<expression>` to host
            // the inner expression.
            //
            // try_expression (`expr?`) gets special treatment: the `?`
            // marker rides as `<try/>` on the surrounding expression
            // host instead of producing a nested `<try><inner/></try>`
            // element (avoids double-wrap per the imperative shape).
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => {
                    let bare = matches!(
                        i.kind(),
                        "for_expression" | "while_expression" | "loop_expression"
                            | "if_expression" | "match_expression" | "block"
                            | "async_block" | "const_block" | "try_block"
                            | "gen_block" | "unsafe_block" | "labeled_expression"
                            | "macro_invocation"
                    );
                    if bare {
                        return Ir::Inline {
                            children: vec![lower_node(i, source)],
                            list_name: None,
                            range, span,
                        };
                    }
                    // Detect try_expression: pull the `?` operand up
                    // and put `<try/>` marker on the expression host.
                    if i.kind() == "try_expression" {
                        let mut tcursor = i.walk();
                        let operand = i.named_children(&mut tcursor).next();
                        if let Some(o) = operand {
                            return Ir::SimpleStatement {
                                element_name: "expression",
                                modifiers: Modifiers::default(),
                                extra_markers: &["try"],
                                children: vec![lower_node(o, source)],
                                range, span,
                            };
                        }
                    }
                    Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![lower_node(i, source)],
                        range, span,
                    }
                }
                None => Ir::Inline {
                    children: Vec::new(),
                    list_name: None,
                    range, span,
                },
            }
        }
        "empty_statement" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range,
            span,
        },
        "block" => simple_statement(node, "block", source),
        // `async { ... }` / `const { ... }` / `try { ... }` / `gen { ... }`
        // wrap an inner block — inline the inner block's children
        // directly under the marker-tagged outer block to avoid
        // `<block[async]><block>...</block></block>` nesting.
        "async_block" => rust_marked_block(node, "block", &["async"], source),
        "const_block" => rust_marked_block(node, "block", &["const"], source),
        "try_block" => rust_marked_block(node, "block", &["try"], source),
        "gen_block" => rust_marked_block(node, "block", &["gen"], source),
        "unsafe_block" => simple_statement(node, "unsafe", source),
        "declaration_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },

        // ----- Control flow --------------------------------------------
        "if_expression" => rust_if_expression(node, source),
        "else_clause" => simple_statement(node, "else", source),
        "let_chain" | "let_condition" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },
        "match_expression" => {
            let value_node = node.child_by_field_name("value");
            let body_node = node.child_by_field_name("body");
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
            if let Some(b) = body_node {
                let mut bc = b.walk();
                let body_children: Vec<Ir> = b
                    .named_children(&mut bc)
                    .map(|s| lower_node(s, source))
                    .collect();
                children.push(Ir::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: body_children,
                    range: range_of(b),
                    span: span_of(b),
                });
            }
            Ir::SimpleStatement {
                element_name: "match",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "match_arm" => {
            let pattern_node = node.child_by_field_name("pattern");
            let value_node = node.child_by_field_name("value");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(p) = pattern_node {
                children.push(lower_node(p, source));
            }
            if let Some(v) = value_node {
                let inner = lower_node(v, source);
                children.push(Ir::SimpleStatement {
                    element_name: "value",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![inner],
                    range: range_of(v),
                    span: span_of(v),
                });
            }
            Ir::SimpleStatement {
                element_name: "arm",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "match_block" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },
        "match_pattern" => {
            // Wraps inner pattern + optional `if cond` guard.
            let cond_node = node.child_by_field_name("condition");
            let mut cursor = node.walk();
            let mut children: Vec<Ir> = Vec::new();
            for c in node.named_children(&mut cursor) {
                if let Some(cn) = cond_node {
                    if c.id() == cn.id() {
                        children.push(Ir::SimpleStatement {
                            element_name: "condition",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![lower_node(c, source)],
                            range: range_of(c),
                            span: span_of(c),
                        });
                        continue;
                    }
                }
                children.push(lower_node(c, source));
            }
            Ir::SimpleStatement {
                element_name: "pattern",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "for_expression" => {
            // `for pat in iter { body }` — emit
            // `<for>{label?} <name>pat</name> <value><expression>iter</expression></value> <body>{block}</body></for>`.
            let pattern_node = node.child_by_field_name("pattern");
            let value_node = node.child_by_field_name("value");
            let body_node = node.child_by_field_name("body");
            let mut cursor = node.walk();
            let mut children: Vec<Ir> = Vec::new();
            for c in node.named_children(&mut cursor) {
                let mut handled = false;
                if let Some(p) = pattern_node {
                    if c.id() == p.id() {
                        children.push(lower_node(c, source));
                        handled = true;
                    }
                }
                if !handled {
                    if let Some(v) = value_node {
                        if c.id() == v.id() {
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
                            handled = true;
                        }
                    }
                }
                if !handled {
                    if let Some(b) = body_node {
                        if c.id() == b.id() {
                            children.push(rename_block_as_body(c, source));
                            handled = true;
                        }
                    }
                }
                if !handled {
                    children.push(lower_node(c, source));
                }
            }
            Ir::SimpleStatement {
                element_name: "for",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "while_expression" => {
            let cond_node = node.child_by_field_name("condition");
            let body_node = node.child_by_field_name("body");
            let mut cursor = node.walk();
            let mut children: Vec<Ir> = Vec::new();
            for c in node.named_children(&mut cursor) {
                let mut handled = false;
                if let Some(cn) = cond_node {
                    if c.id() == cn.id() {
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
                        handled = true;
                    }
                }
                if !handled {
                    if let Some(b) = body_node {
                        if c.id() == b.id() {
                            children.push(rename_block_as_body(c, source));
                            handled = true;
                        }
                    }
                }
                if !handled {
                    children.push(lower_node(c, source));
                }
            }
            Ir::SimpleStatement {
                element_name: "while",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "loop_expression" => {
            let body_node = node.child_by_field_name("body");
            let mut cursor = node.walk();
            let mut children: Vec<Ir> = Vec::new();
            for c in node.named_children(&mut cursor) {
                if let Some(b) = body_node {
                    if c.id() == b.id() {
                        children.push(rename_block_as_body(c, source));
                        continue;
                    }
                }
                children.push(lower_node(c, source));
            }
            Ir::SimpleStatement {
                element_name: "loop",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "return_expression" => simple_statement(node, "return", source),
        "break_expression" => simple_statement(node, "break", source),
        "continue_expression" => simple_statement(node, "continue", source),
        "yield_expression" => simple_statement(node, "yield", source),

        // ----- Expressions ---------------------------------------------
        // ----- Chain inversion -----------------------------------------
        // `obj.field` — fold into Ir::Access if obj is itself a chain.
        "field_expression" => {
            let value_node = node.child_by_field_name("value");
            let field_node = node.child_by_field_name("field");
            match (value_node, field_node) {
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
                _ => Ir::Unknown { kind: "field_expression(missing)".to_string(), range, span },
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
                        } else {
                            None
                        };
                        let call_segment = if let Some((pr, ps)) = last_member {
                            segments.pop();
                            AccessSegment::Call {
                                name: Some(pr),
                                name_span: Some(ps),
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
        "generic_function" => simple_statement_marked(node, "call", &["generic"], source),
        "index_expression" => {
            // `expr[index]` — fold into Ir::Access if expr is a chain.
            let mut cursor = node.walk();
            let kids: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
            if let (Some(obj), Some(idx)) = (kids.first(), kids.get(1)) {
                let object_ir = lower_node(*obj, source);
                let index_ir = lower_node(*idx, source);
                let segment_range = ByteRange::new(object_ir.range().end, range.end);
                let segment = AccessSegment::Index {
                    indices: vec![index_ir],
                    range: segment_range,
                    span,
                };
                return match object_ir {
                    Ir::Access { receiver, mut segments, .. } => {
                        segments.push(segment);
                        Ir::Access { receiver, segments, range, span }
                    }
                    other => Ir::Access {
                        receiver: Box::new(other),
                        segments: vec![segment],
                        range, span,
                    },
                };
            }
            simple_statement(node, "index", source)
        }
        "tuple_expression" => simple_statement(node, "tuple", source),
        "array_expression" => simple_statement(node, "array", source),
        "struct_expression" => simple_statement(node, "literal", source),
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
        "assignment_expression" => simple_statement(node, "assign", source),
        "compound_assignment_expr" => simple_statement(node, "assign", source),
        "type_cast_expression" => {
            // `expr as Type` — emit
            // `<cast><value><expression>expr</expression></value><type>Type</type></cast>`.
            // tree-sitter rust uses fields `value` and `type`.
            let value_node = node.child_by_field_name("value");
            let type_node = node.child_by_field_name("type");
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
            if let Some(t) = type_node {
                let inner = lower_node(t, source);
                children.push(wrap_in_type_if_leaf(inner, range_of(t), span_of(t)));
            }
            Ir::SimpleStatement {
                element_name: "cast",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "reference_expression" => simple_statement(node, "ref", source),
        "await_expression" => simple_statement(node, "await", source),
        "try_expression" => simple_statement(node, "try", source),
        "range_expression" => {
            // `a..b` (exclusive) / `a..=b` (inclusive). Text-detect the
            // operator. Wrap LHS in <from>, RHS in <to>.
            let text = &source[range.start as usize..range.end as usize];
            let inclusive = text.contains("..=");
            let mut cursor = node.walk();
            let kids: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
            let mut children: Vec<Ir> = Vec::new();
            // Find the operator position to split lhs/rhs.
            let mut op_byte: Option<u32> = None;
            let mut cursor2 = node.walk();
            for c in node.children(&mut cursor2) {
                if !c.is_named() {
                    let t = text_of(c, source);
                    if t == ".." || t == "..=" {
                        op_byte = Some(c.start_byte() as u32);
                        break;
                    }
                }
            }
            for c in &kids {
                let inner = lower_node(*c, source);
                let ir = match op_byte {
                    Some(op) if (c.start_byte() as u32) < op => {
                        Ir::SimpleStatement {
                            element_name: "from",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![inner],
                            range: range_of(*c),
                            span: span_of(*c),
                        }
                    }
                    Some(_) => Ir::SimpleStatement {
                        element_name: "to",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(*c),
                        span: span_of(*c),
                    },
                    None => inner,
                };
                children.push(ir);
            }
            let extra_markers: &'static [&'static str] = if inclusive { &["inclusive"] } else { &["exclusive"] };
            Ir::SimpleStatement {
                element_name: "range",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }
        "closure_expression" => simple_statement(node, "closure", source),
        "parenthesized_expression" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },

        // ----- Patterns ------------------------------------------------
        "captured_pattern" => simple_statement_marked(node, "pattern", &["capture"], source),
        "generic_pattern" => simple_statement_marked(node, "pattern", &["generic"], source),
        "reference_pattern" => simple_statement_marked(node, "pattern", &["ref"], source),
        "remaining_field_pattern" => simple_statement_marked(node, "pattern", &["rest"], source),
        "slice_pattern" => simple_statement_marked(node, "pattern", &["slice"], source),
        "tuple_pattern" => simple_statement_marked(node, "pattern", &["tuple"], source),
        "tuple_struct_pattern" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },
        "token_binding_pattern" => simple_statement_marked(node, "pattern", &["binding"], source),
        "field_pattern" => simple_statement_marked(node, "pattern", &["field"], source),
        "or_pattern" => simple_statement_marked(node, "pattern", &["or"], source),
        "mut_pattern" => simple_statement_marked(node, "pattern", &["mut"], source),
        "ref_pattern" => simple_statement_marked(node, "pattern", &["ref"], source),
        "struct_pattern" => simple_statement_marked(node, "pattern", &["struct"], source),
        "range_pattern" => {
            // `0..=9` / `0..9` inside a pattern.
            let text = &source[range.start as usize..range.end as usize];
            let inclusive = text.contains("..=");
            let mut cursor = node.walk();
            let kids: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
            let mut op_byte: Option<u32> = None;
            let mut cursor2 = node.walk();
            for c in node.children(&mut cursor2) {
                if !c.is_named() {
                    let t = text_of(c, source);
                    if t == ".." || t == "..=" || t == "..." {
                        op_byte = Some(c.start_byte() as u32);
                        break;
                    }
                }
            }
            let mut children: Vec<Ir> = Vec::new();
            for c in &kids {
                let inner = lower_node(*c, source);
                let ir = match op_byte {
                    Some(op) if (c.start_byte() as u32) < op => Ir::SimpleStatement {
                        element_name: "from",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(*c),
                        span: span_of(*c),
                    },
                    Some(_) => Ir::SimpleStatement {
                        element_name: "to",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(*c),
                        span: span_of(*c),
                    },
                    None => inner,
                };
                children.push(ir);
            }
            let extra_markers: &'static [&'static str] = if inclusive { &["inclusive"] } else { &["exclusive"] };
            Ir::SimpleStatement {
                element_name: "range",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range, span,
            }
        }

        // ----- Argument / parameter wrappers ---------------------------
        "arguments" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range,
            span,
        },
        "fragment_specifier" => simple_statement(node, "fragment", source),
        "token_repetition" | "token_repetition_pattern" => {
            simple_statement(node, "repetition", source)
        }
        "token_tree" | "token_tree_pattern" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },
        "string_content" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range,
            span,
        },
        "escape_sequence" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range,
            span,
        },

        // ----- Attributes ----------------------------------------------
        "attribute_item" => simple_statement(node, "attribute", source),
        "inner_attribute_item" => simple_statement_marked(node, "attribute", &["inner"], source),
        "attribute" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range,
            span,
        },

        // ----- Foreign / extern blocks ---------------------------------
        "foreign_mod_item" => simple_statement_marked(node, "mod", &["foreign", "extern"], source),

        // ----- Crate marker --------------------------------------------
        "crate" => Ir::Name { range, span },

        // Default: surface as <unknown> so coverage diagnostics show it.
        other => Ir::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
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

/// Wrap a leaf-like Ir (identifier-shaped) in `<type>` so it surfaces
/// as `<type><name>X</name></type>`. Already-typed (Ir::Access,
/// Ir::SimpleStatement<element_name="type"|"path"|"alias">, etc.) pass
/// through unchanged.
fn wrap_in_type_if_leaf(inner: Ir, range: ByteRange, span: Span) -> Ir {
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

/// Lower a marker-prefixed block (`async { ... }`, `const { ... }`,
/// `try { ... }`, `gen { ... }`) — produce `<block[marker]>` whose
/// children are the inner block's children (NOT a nested `<block>`).
fn rust_marked_block(
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers: &'static [&'static str],
    source: &str,
) -> Ir {
    let mut cursor = node.walk();
    let mut children: Vec<Ir> = Vec::new();
    for c in node.named_children(&mut cursor) {
        if c.kind() == "block" {
            // Inline the inner block's children.
            let mut bc = c.walk();
            for inner in c.named_children(&mut bc) {
                children.push(lower_node(inner, source));
            }
        } else {
            children.push(lower_node(c, source));
        }
    }
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children: merge_rust_line_comments(children, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// Rename a `block` node to `<body>`, lowering its inner statements
/// directly into the body (no nested `<block>` wrapper). Comments
/// inside the body are classified via `merge_rust_line_comments`.
fn rename_block_as_body(block: TsNode<'_>, source: &str) -> Ir {
    let mut cursor = block.walk();
    let children: Vec<Ir> = block
        .named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
    Ir::SimpleStatement {
        element_name: "body",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children: merge_rust_line_comments(children, source),
        range: range_of(block),
        span: span_of(block),
    }
}

/// Lower a Rust if_expression to the canonical `<if>` shape:
/// `<if><condition>...</condition><then>{block}</then><else>...</else></if>`.
/// `<else>` becomes `<else_if>` when its body is itself an if_expression
/// (chain collapse, matching the imperative pipeline shape).
fn rust_if_expression(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let consequence_node = node.child_by_field_name("consequence");
    let alternative_node = node.child_by_field_name("alternative");
    let mut children: Vec<Ir> = Vec::new();

    if let Some(c) = cond_node {
        children.push(Ir::SimpleStatement {
            element_name: "condition",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![lower_node(c, source)],
            range: range_of(c),
            span: span_of(c),
        });
    }
    if let Some(c) = consequence_node {
        children.push(Ir::SimpleStatement {
            element_name: "then",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![lower_node(c, source)],
            range: range_of(c),
            span: span_of(c),
        });
    }
    // Collapse else-if chain into flat <else_if>/<else> siblings.
    let mut cur_alt = alternative_node;
    while let Some(a) = cur_alt {
        let mut cursor = a.walk();
        let inner = a.named_children(&mut cursor).next();
        match inner {
            Some(i) if i.kind() == "if_expression" => {
                // Inner if_expression — render as <else_if> with its
                // own condition/then, then continue chain.
                let inner_cond = i.child_by_field_name("condition");
                let inner_cons = i.child_by_field_name("consequence");
                let inner_alt = i.child_by_field_name("alternative");
                let mut else_if_children: Vec<Ir> = Vec::new();
                if let Some(c) = inner_cond {
                    else_if_children.push(Ir::SimpleStatement {
                        element_name: "condition",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![lower_node(c, source)],
                        range: range_of(c),
                        span: span_of(c),
                    });
                }
                if let Some(c) = inner_cons {
                    else_if_children.push(Ir::SimpleStatement {
                        element_name: "then",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![lower_node(c, source)],
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
            }
            Some(i) => {
                children.push(Ir::SimpleStatement {
                    element_name: "else",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![lower_node(i, source)],
                    range: range_of(a),
                    span: span_of(a),
                });
                cur_alt = None;
            }
            None => {
                cur_alt = None;
            }
        }
    }

    Ir::SimpleStatement {
        element_name: "if",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range,
        span,
    }
}

/// Classify a Rust visibility_modifier text:
/// - `pub` → simple marker; emit `<pub/>` on parent
/// - `pub(crate)` → `<pub><crate/></pub>` element child
/// - `pub(super)` → `<pub><super/></pub>`
/// - `pub(self)` → `<pub><self/></pub>`
/// - `pub(in path)` → `<pub><in>path</in></pub>`
/// - missing → `<private/>` marker
enum RustVis {
    /// No modifier — emit `<private/>` extra-marker.
    Private,
    /// Simple `pub` — emit `<pub/>` extra-marker.
    Pub,
    /// `pub(qualifier)` — emit a `<pub>` child element containing the
    /// qualifier element. No marker.
    PubQualified(Ir),
}

fn classify_rust_visibility(node: TsNode<'_>, source: &str) -> RustVis {
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if c.kind() == "visibility_modifier" {
            let txt = text_of(c, source);
            if !txt.starts_with("pub") {
                continue;
            }
            // Look at unnamed children for `(`, qualifier, `)`.
            let mut tcursor = c.walk();
            let mut qualifier: Option<TsNode<'_>> = None;
            for ch in c.children(&mut tcursor) {
                if !ch.is_named() {
                    continue;
                }
                qualifier = Some(ch);
                break;
            }
            if qualifier.is_none() && !txt.contains('(') {
                return RustVis::Pub;
            }
            // Look for the qualifier kind: `crate` / `super` / `self` / `scoped_identifier`.
            // tree-sitter rust gives `crate`, `super`, `self` as named atoms.
            let qual_text = txt.trim_start_matches("pub").trim().trim_start_matches('(').trim_end_matches(')').trim();
            let q_inner = if qual_text.starts_with("in ") {
                // `pub(in path)` — emit `<in>path</in>` element.
                let path_text = qual_text["in ".len()..].trim();
                let _ = path_text;
                // Use a SimpleStatement<in> with a name leaf for path text.
                // Source-bytes-preserving fallback.
                Ir::SimpleStatement {
                    element_name: "in",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![],
                    range: range_of(c),
                    span: span_of(c),
                }
            } else {
                // Render the qualifier as a marker-only element
                // (`<crate/>`, `<super/>`, `<self/>`).
                let element_name: &'static str = match qual_text {
                    "crate" => "crate",
                    "super" => "super",
                    "self" => "self",
                    _ => "crate",
                };
                Ir::SimpleStatement {
                    element_name,
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![],
                    range: range_of(c),
                    span: span_of(c),
                }
            };
            return RustVis::PubQualified(q_inner);
        }
    }
    RustVis::Private
}

/// Lower a Rust function_item with proper <returns> wrapping.
/// Tree-sitter rust uses `return_type` as a FIELD on function_item
/// pointing at the type child (no wrapper element). We detect it via
/// child_by_field_name and wrap in Ir::Returns. The body block is
/// renamed to `<body>` so XPath sees `function/body/...`.
fn rust_function(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let return_type_node = node.child_by_field_name("return_type");
    let body_node = node.child_by_field_name("body");
    let vis = classify_rust_visibility(node, source);
    let mut cursor = node.walk();
    let mut children: Vec<Ir> = Vec::new();
    // For PubQualified, push `<pub>` element first.
    if let RustVis::PubQualified(q) = &vis {
        let pub_inner = q.clone();
        children.push(Ir::SimpleStatement {
            element_name: "pub",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![pub_inner],
            range,
            span,
        });
    }
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "visibility_modifier" => {
                // Already handled by extra_markers/PubQualified.
            }
            _ => {
                if let Some(rt) = return_type_node {
                    if c.id() == rt.id() {
                        children.push(Ir::Returns {
                            type_ann: Box::new(lower_node(c, source)),
                            range: range_of(c),
                            span: span_of(c),
                        });
                        continue;
                    }
                }
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
                            children: merge_rust_line_comments(body_children, source),
                            range: range_of(c),
                            span: span_of(c),
                        });
                        continue;
                    }
                }
                children.push(lower_node(c, source));
            }
        }
    }
    let extra_markers: &'static [&'static str] = match &vis {
        RustVis::Private => &["private"],
        RustVis::Pub => &["pub"],
        RustVis::PubQualified(_) => &[],
    };
    Ir::SimpleStatement {
        element_name: "function",
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range,
        span,
    }
}

/// Rust uses `<pub/>` / `<private/>` markers (not `<public/>`).
/// Detect visibility and emit appropriate extra_markers. Also detects
/// `mut` modifier (for `static mut COUNTER` etc.).
fn rust_decl(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let mut cursor = node.walk();
    let mut is_pub = false;
    let mut has_mut = false;
    for c in node.named_children(&mut cursor) {
        if c.kind() == "visibility_modifier" {
            let txt = text_of(c, source);
            if txt.starts_with("pub") {
                is_pub = true;
            }
        }
        if c.kind() == "mutable_specifier" {
            has_mut = true;
        }
    }
    let extra_markers: &'static [&'static str] = match (is_pub, has_mut) {
        (true, true) => &["pub", "mut"],
        (true, false) => &["pub"],
        (false, true) => &["private", "mut"],
        (false, false) => &["private"],
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

#[allow(dead_code)]
fn unused_modifiers() -> Modifiers {
    let mut m = Modifiers::default();
    m.access = Some(Access::Private);
    m
}

#[allow(dead_code)]
fn unused_param_kind() -> ParamKind {
    ParamKind::Regular
}
