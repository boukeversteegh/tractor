//! TypeScript / JavaScript / TSX / JSX tree-sitter CST → IR lowering.
//!
//! Single lower function handles all four flavours — the TS / JS / TSX
//! grammars share most node kinds (TS is a superset, TSX adds JSX-only
//! kinds). Per-kind arms recursively lower children; the renderer in
//! `crate::ir::render` is shared.
//!
//! Production parser routes ts/js/tsx/jsx through this lowering
//! end-to-end (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/typescript/{rules,transformations,transform}.rs`
//! modules were retired alongside this migration.
//!
//! Coverage is incremental: each unhandled kind falls through to
//! `Ir::Unknown`. Diagnostic test
//! `tests/ir_typescript_missing_kinds.rs` lists kinds the corpus
//! exercises that aren't yet typed.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{Access, AccessSegment, ByteRange, Ir, Modifiers, ParamKind, Span};

/// Lower a TypeScript tree-sitter root node to [`Ir`].
pub fn lower_typescript_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => Ir::Module {
            element_name: "program",
            children: merge_ts_line_comments(lower_children(root, source), source),
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

pub fn lower_typescript_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms -----------------------------------------------------
        "identifier" | "type_identifier" | "property_identifier" | "shorthand_property_identifier"
        | "shorthand_property_identifier_pattern" => Ir::Name { range, span },
        // TS `number` is dual-purpose (int + float); the imperative
        // pipeline emits `<number>` (not `<int>` like C#/Python).
        // Use SimpleStatement with element_name="number" to preserve
        // source bytes as text.
        "number" => Ir::SimpleStatement {
            element_name: "number",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range,
            span,
        },
        "string" => Ir::String { range, span },
        // Template literals contain `template_substitution` (`${...}`)
        // children. Lower as `<template>` with interpolation children
        // so XPath can address `template[interpolation/name='x']`.
        // Plain templates (no substitutions) still emit `<template>`.
        "template_string" => {
            let mut cursor = node.walk();
            let has_subs = node.named_children(&mut cursor)
                .any(|c| c.kind() == "template_substitution");
            if !has_subs {
                Ir::String { range, span }
            } else {
                let mut cursor2 = node.walk();
                let children: Vec<Ir> = node
                    .named_children(&mut cursor2)
                    .map(|c| lower_node(c, source))
                    .collect();
                Ir::SimpleStatement {
                    element_name: "template",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children,
                    range,
                    span,
                }
            }
        }
        "template_substitution" => simple_statement(node, "interpolation", source),
        "true" => Ir::True { range, span },
        "false" => Ir::False { range, span },
        "null" => Ir::Null { range, span },
        "undefined" => Ir::Name { range, span },
        "this" => Ir::SimpleStatement {
            element_name: "this",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range,
            span,
        },
        "super" => Ir::SimpleStatement {
            element_name: "super",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range,
            span,
        },

        // Predefined types.
        "predefined_type" | "void_type" => Ir::Name { range, span },

        // ----- Containers / declarations ---------------------------------
        "class_declaration" | "abstract_class_declaration" | "class" | "interface_declaration" => {
            let kind: &'static str = if node.kind() == "interface_declaration" {
                "interface"
            } else {
                "class"
            };
            let name_node = node.child_by_field_name("name");
            let body_node = node.child_by_field_name("body");
            let mut tpc = node.walk();
            let type_param_list = node
                .named_children(&mut tpc)
                .find(|c| c.kind() == "type_parameters");
            // Extends / implements clauses.
            let mut bases: Vec<Ir> = Vec::new();
            let mut hcc = node.walk();
            for c in node.named_children(&mut hcc) {
                match c.kind() {
                    "class_heritage" => {
                        let mut chc = c.walk();
                        for inner in c.named_children(&mut chc) {
                            match inner.kind() {
                                "extends_clause" => {
                                    let mut ec = inner.walk();
                                    for t in inner.named_children(&mut ec) {
                                        bases.push(lower_node(t, source));
                                    }
                                }
                                "implements_clause" => {
                                    let mut ic = inner.walk();
                                    for t in inner.named_children(&mut ic) {
                                        let inner_ir = lower_node(t, source);
                                        let already_typed = matches!(
                                            inner_ir,
                                            Ir::GenericType { .. }
                                                | Ir::SimpleStatement { element_name: "type", .. }
                                        );
                                        let typed = if already_typed {
                                            inner_ir
                                        } else {
                                            Ir::SimpleStatement {
                                                element_name: "type",
                                                modifiers: Modifiers::default(),
                                                extra_markers: &[],
                                                children: vec![inner_ir],
                                                range: range_of(t),
                                                span: span_of(t),
                                            }
                                        };
                                        bases.push(Ir::SimpleStatement {
                                            element_name: "implements",
                                            modifiers: Modifiers::default(),
                                            extra_markers: &[],
                                            children: vec![typed],
                                            range: range_of(t),
                                            span: span_of(t),
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "extends_type_clause" => {
                        // Interface `extends Foo, Bar` — multiple bases.
                        let mut ec = c.walk();
                        for t in c.named_children(&mut ec) {
                            bases.push(lower_node(t, source));
                        }
                    }
                    _ => {}
                }
            }
            let modifiers = lower_ts_modifiers(node, source, None);
            let generics = type_param_list.map(|tpl| {
                let mut tplc = tpl.walk();
                let items: Vec<Ir> = tpl
                    .named_children(&mut tplc)
                    .map(|c| lower_node(c, source))
                    .collect();
                Box::new(Ir::Generic {
                    items,
                    range: range_of(tpl),
                    span: span_of(tpl),
                })
            });
            Ir::Class {
                kind,
                modifiers,
                decorators: extract_ts_decorators(node, source),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown {
                        kind: format!("{}(missing name)", kind),
                        range,
                        span,
                    },
                }),
                generics,
                bases,
                where_clauses: Vec::new(),
                body: Box::new(match body_node {
                    Some(b) => lower_block_like(b, source),
                    None => Ir::Body {
                        children: Vec::new(),
                        pass_only: false,
                        block_wrap: false,
                        range: ByteRange::empty_at(range.end),
                        span,
                    },
                }),
                range,
                span,
            }
        }

        // Function / method.
        "function_declaration" | "function_signature" | "method_definition"
        | "method_signature" | "abstract_method_signature" | "function_expression"
        | "generator_function_declaration" => {
            let name_node = node.child_by_field_name("name");
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            let return_type_node = node.child_by_field_name("return_type");
            // TS class methods default to `public`; standalone
            // functions have no access modifier.
            let is_class_member = matches!(
                node.kind(),
                "method_definition" | "method_signature" | "abstract_method_signature"
            );
            let default_access = if is_class_member {
                Some(Access::Public)
            } else {
                None
            };
            let modifiers = lower_ts_modifiers(node, source, default_access);
            let parameters: Vec<Ir> = match params_node {
                Some(p) => {
                    let mut pc = p.walk();
                    p.named_children(&mut pc)
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            let returns = return_type_node.map(|t| {
                // return_type wraps an inner type.
                let mut tc = t.walk();
                let inner = t.named_children(&mut tc).next().unwrap_or(t);
                Box::new(Ir::Returns {
                    type_ann: Box::new(lower_node(inner, source)),
                    range: range_of(t),
                    span: span_of(t),
                })
            });
            let body: Option<Box<Ir>> = body_node.map(|b| Box::new(lower_block_like(b, source)));
            let element_name: &'static str =
                if matches!(node.kind(), "method_definition" | "method_signature" | "abstract_method_signature") {
                    "method"
                } else {
                    "function"
                };
            // Type parameters: `<T, U>` lower to flat `<generic>` siblings.
            let mut tpc = node.walk();
            let type_params_node = node
                .named_children(&mut tpc)
                .find(|c| c.kind() == "type_parameters");
            let generics: Option<Box<Ir>> = type_params_node.map(|tp| {
                let mut tc = tp.walk();
                let items: Vec<Ir> = tp
                    .named_children(&mut tc)
                    .map(|c| lower_node(c, source))
                    .collect();
                Box::new(Ir::Generic {
                    items,
                    range: range_of(tp),
                    span: span_of(tp),
                })
            });
            Ir::Function {
                element_name,
                modifiers,
                decorators: extract_ts_decorators(node, source),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown {
                        kind: format!("{}(missing name)", element_name),
                        range,
                        span,
                    },
                }),
                generics,
                parameters,
                returns,
                body,
                range,
                span,
            }
        }

        "arrow_function" => {
            // Arrow has a parameter list (or single bare identifier) and
            // a body which is either a `<block>` or an expression.
            // Emits `<arrow>` with parameter children + `<body>` (block)
            // or `<value><expression>...</expression></value>` (expr).
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            let return_type_node = node.child_by_field_name("return_type");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(p) = params_node {
                let mut pc = p.walk();
                for c in p.named_children(&mut pc) {
                    children.push(lower_node(c, source));
                }
            } else {
                // Single bare identifier — `x => ...`. Tree-sitter
                // exposes the parameter as the `parameter` field
                // (single identifier).
                if let Some(p) = node.child_by_field_name("parameter") {
                    // Wrap as a Parameter so shape stays uniform.
                    children.push(Ir::Parameter {
                        kind: ParamKind::Regular,
                        extra_markers: &["required"],
                        modifiers: Modifiers::default(),
                        name: Box::new(Ir::Name { range: range_of(p), span: span_of(p) }),
                        type_ann: None,
                        default: None,
                        range: range_of(p),
                        span: span_of(p),
                    });
                }
            }
            if let Some(rt) = return_type_node {
                let mut tc = rt.walk();
                let inner = rt.named_children(&mut tc).next().unwrap_or(rt);
                children.push(Ir::Returns {
                    type_ann: Box::new(lower_node(inner, source)),
                    range: range_of(rt),
                    span: span_of(rt),
                });
            }
            if let Some(b) = body_node {
                if b.kind() == "statement_block" {
                    children.push(lower_block_like(b, source));
                } else {
                    // Expression body — wrap in <value><expression>.
                    let inner = lower_node(b, source);
                    children.push(Ir::SimpleStatement {
                        element_name: "value",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![Ir::SimpleStatement {
                            element_name: "expression",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![inner],
                            range: range_of(b),
                            span: span_of(b),
                        }],
                        range: range_of(b),
                        span: span_of(b),
                    });
                }
            }
            Ir::SimpleStatement {
                element_name: "arrow",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }

        // Required / optional / rest parameter.
        "required_parameter" | "optional_parameter" => {
            let pattern = node.child_by_field_name("pattern");
            let type_node = node.child_by_field_name("type");
            let value_node = node.child_by_field_name("value");
            let extra_markers: &'static [&'static str] = if node.kind() == "optional_parameter" {
                &["optional"]
            } else {
                &["required"]
            };
            let modifiers = lower_ts_modifiers(node, source, None);
            // Tree-sitter labeled-tuple elements (`[head: number]`) emit
            // a required/optional parameter without a `pattern` field —
            // the label-identifier becomes the FIRST named child instead.
            // Fall back to the first named identifier child for the name.
            let name_fallback = if pattern.is_none() {
                let mut cursor = node.walk();
                let found = node.named_children(&mut cursor)
                    .find(|c| matches!(c.kind(), "identifier" | "type_identifier" | "property_identifier"));
                found
            } else {
                None
            };
            Ir::Parameter {
                kind: ParamKind::Regular,
                extra_markers,
                modifiers,
                name: Box::new(match pattern.or(name_fallback) {
                    Some(p) => lower_node(p, source),
                    None => Ir::Unknown {
                        kind: "parameter(missing pattern)".to_string(),
                        range,
                        span,
                    },
                }),
                type_ann: type_node.map(|t| {
                    let mut tc = t.walk();
                    let inner = t.named_children(&mut tc).next().unwrap_or(t);
                    Box::new(lower_node(inner, source))
                }),
                default: value_node.map(|v| Box::new(lower_node(v, source))),
                range,
                span,
            }
        }

        // Variable / lexical declaration.
        "lexical_declaration" | "variable_declaration" => {
            // `let x = 1, y = 2;` — multiple variable_declarators.
            let modifiers = lower_ts_modifiers(node, source, None);
            let mut vc = node.walk();
            let declarators: Vec<TsNode> = node
                .named_children(&mut vc)
                .filter(|c| c.kind() == "variable_declarator")
                .collect();
            // Detect let/const/var keyword as a marker.
            let leading = source[range.start as usize..range.end as usize].trim_start();
            let kw_marker: &'static [&'static str] = if leading.starts_with("const") {
                &["const"]
            } else if leading.starts_with("let") {
                &["let"]
            } else if leading.starts_with("var") {
                &["var"]
            } else {
                &[]
            };
            if declarators.is_empty() {
                return Ir::Unknown {
                    kind: "lexical_declaration(no declarators)".to_string(),
                    range,
                    span,
                };
            }
            // Always emit `<variable[let|const|var]>` with markers
            // controlled here. Single-declarator inlines its
            // type/name/value as flat children of `<variable>`;
            // multi-declarator keeps a `<declarator>` wrapper per
            // entry. Type comes from individual declarators in TS
            // (not the parent statement, unlike Java).
            let mut children: Vec<Ir> = Vec::new();
            if declarators.len() == 1 {
                let d = declarators[0];
                let parts = lower_ts_declarator_parts(d, source);
                children.extend(parts);
            } else {
                for d in declarators {
                    children.push(Ir::SimpleStatement {
                        element_name: "declarator",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: lower_ts_declarator_parts(d, source),
                        range: range_of(d),
                        span: span_of(d),
                    });
                }
            }
            Ir::SimpleStatement {
                element_name: "variable",
                modifiers,
                extra_markers: kw_marker,
                children,
                range,
                span,
            }
        }

        // Public / private / class field.
        "public_field_definition" => {
            let name_node = node.child_by_field_name("name");
            let type_node = node.child_by_field_name("type");
            let value_node = node.child_by_field_name("value");
            // TS class fields default to `public`.
            let modifiers = lower_ts_modifiers(node, source, Some(Access::Public));
            let value_ir = value_node.map(|v| {
                Box::new(Ir::SimpleStatement {
                    element_name: "value",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![lower_node(v, source)],
                    range: range_of(v),
                    span: span_of(v),
                })
            });
            Ir::Variable {
                element_name: "field",
                modifiers,
                decorators: extract_ts_decorators(node, source),
                type_ann: type_node.map(|t| {
                    let mut tc = t.walk();
                    let inner = t.named_children(&mut tc).next().unwrap_or(t);
                    Box::new(lower_node(inner, source))
                }),
                name: Box::new(match name_node {
                    Some(n) => Ir::Name { range: range_of(n), span: span_of(n) },
                    None => Ir::Unknown {
                        kind: "field(missing name)".to_string(),
                        range,
                        span,
                    },
                }),
                value: value_ir,
                range,
                span,
            }
        }

        // Block / body.
        "statement_block" | "class_body" | "interface_body" | "enum_body" => lower_block_like(node, source),
        "object_type" => simple_statement_marked(node, "type", &["object"], source),

        // Statements.
        "expression_statement" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(n) => Ir::Expression {
                    inner: Box::new(lower_node(n, source)),
                    marker: None,
                    range,
                    span,
                },
                None => Ir::Unknown {
                    kind: "expression_statement(empty)".to_string(),
                    range,
                    span,
                },
            }
        }

        "return_statement" => {
            let mut cursor = node.walk();
            let value = node.named_children(&mut cursor).next();
            Ir::Return {
                value: value.map(|v| Box::new(lower_node(v, source))),
                range,
                span,
            }
        }

        "if_statement" => {
            let cond = node
                .child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source)));
            let body = node
                .child_by_field_name("consequence")
                .map(|n| Box::new(lower_block_like(n, source)));
            let alt = node.child_by_field_name("alternative");
            let else_branch = alt.map(|a| Box::new(lower_ts_else_chain(a, source)));
            match (cond, body) {
                (Some(c), Some(b)) => Ir::If {
                    condition: c,
                    body: b,
                    else_branch,
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "if_statement(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        "while_statement" => {
            let cond = node
                .child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source)));
            let body = node
                .child_by_field_name("body")
                .map(|n| Box::new(lower_block_like(n, source)));
            match (cond, body) {
                (Some(c), Some(b)) => Ir::While {
                    condition: c,
                    body: b,
                    else_body: None,
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "while_statement(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        "for_statement" => {
            let init = node.child_by_field_name("initializer");
            let cond_node = node.child_by_field_name("condition");
            let update_node = node.child_by_field_name("increment");
            let body = node
                .child_by_field_name("body")
                .map(|n| Box::new(lower_block_like(n, source)));
            let updates: Vec<Ir> = update_node
                .map(|u| {
                    if u.kind() == "sequence_expression" {
                        // Comma-separated updates `j--, i++` — flatten.
                        let mut cursor = u.walk();
                        u.named_children(&mut cursor)
                            .map(|c| lower_node(c, source))
                            .collect()
                    } else {
                        vec![lower_node(u, source)]
                    }
                })
                .unwrap_or_default();
            match body {
                Some(b) => Ir::CFor {
                    initializer: init.map(|i| Box::new(lower_node(i, source))),
                    condition: cond_node.map(|c| Box::new(lower_node(c, source))),
                    updates,
                    body: b,
                    range,
                    span,
                },
                None => Ir::Unknown {
                    kind: "for_statement(no body)".to_string(),
                    range,
                    span,
                },
            }
        }

        "for_in_statement" => {
            // `for (k in obj)` or `for (item of items)` — TS shape:
            // `<for><left><expression>{binding}</expression></left><right><expression>{iter}</expression></right><body>...</body></for>`.
            let left_node = node.child_by_field_name("left");
            let right_node = node.child_by_field_name("right");
            let body_node = node.child_by_field_name("body");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(l) = left_node {
                let inner = lower_node(l, source);
                let inner_range = range_of(l);
                let inner_span = span_of(l);
                let expr = Ir::Expression {
                    inner: Box::new(inner),
                    marker: None,
                    range: inner_range,
                    span: inner_span,
                };
                children.push(Ir::SimpleStatement {
                    element_name: "left",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![expr],
                    range: inner_range,
                    span: inner_span,
                });
            }
            if let Some(r) = right_node {
                let inner = lower_node(r, source);
                let inner_range = range_of(r);
                let inner_span = span_of(r);
                let expr = Ir::Expression {
                    inner: Box::new(inner),
                    marker: None,
                    range: inner_range,
                    span: inner_span,
                };
                children.push(Ir::SimpleStatement {
                    element_name: "right",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![expr],
                    range: inner_range,
                    span: inner_span,
                });
            }
            if let Some(b) = body_node {
                children.push(lower_block_like(b, source));
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

        "do_statement" => {
            let body = node
                .child_by_field_name("body")
                .map(|n| Box::new(lower_block_like(n, source)));
            let cond = node
                .child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source)));
            match (body, cond) {
                (Some(b), Some(c)) => Ir::DoWhile {
                    body: b,
                    condition: c,
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "do_statement(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        "break_statement" => Ir::Break { range, span },
        "continue_statement" => Ir::Continue { range, span },

        // Try.
        "try_statement" => simple_statement(node, "try", source),
        "catch_clause" => simple_statement(node, "catch", source),
        "finally_clause" => simple_statement(node, "finally", source),
        "throw_statement" => simple_statement(node, "throw", source),

        // JSX / TSX. The imperative pipeline mapped these as plain
        // renames (no Custom handlers); SimpleStatement is enough.
        // - jsx_element / jsx_self_closing_element → <element>
        // - jsx_opening_element → <opening>
        // - jsx_closing_element → <closing>
        // - jsx_attribute → <prop>
        // - jsx_expression → <value>
        // - jsx_text → <text>
        // The opening/closing element nodes hold the tag name as their
        // own child (an identifier), which lowers to <name>; queries
        // like `//element[opening/name='div']` therefore work without
        // any extra rewiring.
        "jsx_element" => simple_statement(node, "element", source),
        "jsx_self_closing_element" => simple_statement(node, "element", source),
        "jsx_opening_element" => simple_statement(node, "opening", source),
        "jsx_closing_element" => simple_statement(node, "closing", source),
        "jsx_attribute" => simple_statement(node, "prop", source),
        "jsx_expression" => simple_statement(node, "value", source),
        "jsx_text" => simple_statement(node, "text", source),
        "jsx_fragment" => simple_statement(node, "element", source),
        "jsx_namespace_name" => simple_statement(node, "name", source),

        // Imports / exports.
        "import_statement" => simple_statement(node, "import", source),
        "export_statement" => simple_statement(node, "export", source),
        // Import-clause / export-clause / namespace-import / named-imports
        // are wrapper grammar nodes — flatten their children into the
        // parent <import>/<export>. The post-pass `typescript_restructure_import`
        // restructures the resulting flat children into the canonical
        // shape (default/spec/path).
        "import_clause" | "export_clause" | "named_imports" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }
        // `* as ns` namespace-import — emit as `<namespace><name>ns</name></namespace>`.
        "namespace_import" => simple_statement(node, "namespace", source),
        // `import_specifier` / `export_specifier` — `{ namedA }` or `{ namedA as aliasedB }`.
        "import_specifier" | "export_specifier" => {
            let name_node = node.child_by_field_name("name");
            let alias_node = node.child_by_field_name("alias");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(n) = name_node {
                children.push(Ir::Name { range: range_of(n), span: span_of(n) });
            }
            if let Some(a) = alias_node {
                children.push(Ir::Name { range: range_of(a), span: span_of(a) });
            }
            Ir::SimpleStatement {
                element_name: "spec",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }

        // Decorators.
        "decorator" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => Ir::Decorator {
                    inner: Box::new(lower_node(i, source)),
                    range,
                    span,
                },
                None => Ir::Unknown { kind: "decorator(empty)".to_string(), range, span },
            }
        }

        // Binary / unary / assignment.
        "binary_expression" => {
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
                    kind: "binary_expression(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        "unary_expression" | "update_expression" => {
            let mut cursor1 = node.walk();
            let mut op_node = None;
            for c in node.children(&mut cursor1) {
                if !c.is_named() {
                    op_node = Some(c);
                    break;
                }
            }
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let mut cursor2 = node.walk();
            let mut operand_node = None;
            for c in node.named_children(&mut cursor2) {
                operand_node = Some(c);
                break;
            }
            match operand_node {
                Some(o) => {
                    let is_postfix = op_range.start >= range_of(o).end;
                    let extra_markers: &'static [&'static str] = if is_postfix {
                        &["postfix"]
                    } else if matches!(op_text.as_str(), "++" | "--") {
                        &["prefix"]
                    } else {
                        &[]
                    };
                    match op_marker(&op_text) {
                        Some(marker) => Ir::Unary {
                            op_text,
                            op_marker: marker,
                            op_range,
                            operand: Box::new(lower_node(o, source)),
                            extra_markers,
                            range,
                            span,
                        },
                        None => simple_statement(node, "unary", source),
                    }
                }
                None => simple_statement(node, "unary", source),
            }
        }

        "assignment_expression" | "augmented_assignment_expression" => {
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (left, right) {
                (Some(l), Some(r)) => Ir::Assign {
                    targets: vec![lower_node(l, source)],
                    type_annotation: None,
                    op_text,
                    op_range,
                    op_markers: Vec::new(),
                    values: vec![lower_node(r, source)],
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "assignment(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // `a, b, c` — sequence/comma-expression. Flattens via Inline so
        // the parent (e.g. for-loop update) sees the inner expressions
        // directly.
        "sequence_expression" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }

        // `import.meta` / `new.target` — JS meta-properties are
        // atomic identifiers. We render them as a single <name> leaf
        // covering the dot-spanning text so the chain receiver shape
        // is `<name>import.meta</name>` (matches Python's __file__
        // precedent).
        "meta_property" => Ir::Name { range, span },

        // Member / call chains.
        "member_expression" => {
            let object_node = node.child_by_field_name("object");
            let property_node = node.child_by_field_name("property");
            let optional = source[range.start as usize..range.end as usize].contains("?.");
            match (object_node, property_node) {
                (Some(object), Some(attr)) => {
                    let object_ir = lower_node(object, source);
                    let property_range = range_of(attr);
                    let property_span = span_of(attr);
                    let segment_range = ByteRange::new(object_ir.range().end, property_range.end);
                    let segment = AccessSegment::Member {
                        property_range,
                        property_span,
                        optional,
                        range: segment_range,
                        span,
                    };
                    match object_ir {
                        Ir::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            Ir::Access {
                                receiver,
                                segments,
                                range,
                                span,
                            }
                        }
                        other => Ir::Access {
                            receiver: Box::new(other),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => Ir::Unknown { kind: "member_expression(missing)".to_string(), range, span },
            }
        }

        "call_expression" => {
            // Check if function is itself a member chain — fold.
            let function_node = node.child_by_field_name("function");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<Ir> = match args_node {
                Some(a) => {
                    let mut ac = a.walk();
                    a.named_children(&mut ac)
                        .map(|c| lower_node(c, source))
                        .collect()
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
                        let call_segment = if let Some((property_range, property_span)) = last_member {
                            segments.pop();
                            AccessSegment::Call {
                                name: Some(property_range),
                                name_span: Some(property_span),
                                arguments,
                                range: ByteRange::new(property_range.start, range.end),
                                span,
                            }
                        } else {
                            AccessSegment::Call {
                                name: None,
                                name_span: None,
                                arguments,
                                range: ByteRange::new(callee_range.end, range.end),
                                span,
                            }
                        };
                        segments.push(call_segment);
                        return Ir::Access { receiver, segments, range, span };
                    }
                    Ir::Call {
                        callee: Box::new(callee),
                        arguments,
                        range,
                        span,
                    }
                }
                None => Ir::Unknown { kind: "call_expression(missing)".to_string(), range, span },
            }
        }

        "subscript_expression" => {
            let object_node = node.child_by_field_name("object");
            let index_node = node.child_by_field_name("index");
            match (object_node, index_node) {
                (Some(o), Some(i)) => {
                    let object_ir = lower_node(o, source);
                    let segment = AccessSegment::Index {
                        indices: vec![lower_node(i, source)],
                        range: ByteRange::new(object_ir.range().end, range.end),
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
                            range,
                            span,
                        },
                    }
                }
                _ => Ir::Unknown { kind: "subscript_expression(missing)".to_string(), range, span },
            }
        }

        "new_expression" => {
            let constructor_node = node.child_by_field_name("constructor");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<Ir> = match args_node {
                Some(a) => {
                    let mut ac = a.walk();
                    a.named_children(&mut ac)
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            Ir::ObjectCreation {
                type_target: constructor_node.map(|t| Box::new(lower_node(t, source))),
                arguments,
                initializer: None,
                range,
                span,
            }
        }

        // Generic types.
        "generic_type" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            if let Some(name) = kids.first() {
                let mut params: Vec<Ir> = Vec::new();
                for c in kids.iter().skip(1) {
                    if c.kind() == "type_arguments" {
                        let mut tc = c.walk();
                        params.extend(c.named_children(&mut tc).map(|n| lower_node(n, source)));
                    } else {
                        params.push(lower_node(*c, source));
                    }
                }
                Ir::GenericType {
                    name: Box::new(lower_node(*name, source)),
                    params,
                    range,
                    span,
                }
            } else {
                Ir::Unknown { kind: "generic_type(empty)".to_string(), range, span }
            }
        }

        "type_parameter" => {
            // TS: `T extends Foo = Default` — name + constraint + default.
            let mut cursor = node.walk();
            let mut name_node: Option<TsNode> = None;
            let mut constraint_node: Option<TsNode> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "constraint" => constraint_node = Some(c),
                    _ if name_node.is_none() => name_node = Some(c),
                    _ => {}
                }
            }
            let mut children: Vec<Ir> = Vec::new();
            if let Some(n) = name_node {
                children.push(Ir::Name {
                    range: range_of(n),
                    span: span_of(n),
                });
            }
            if let Some(cn) = constraint_node {
                let mut cc = cn.walk();
                let inner = cn.named_children(&mut cc).next();
                let inner_ir = match inner {
                    Some(t) => lower_node(t, source),
                    None => Ir::Unknown { kind: "constraint(empty)".to_string(), range: range_of(cn), span: span_of(cn) },
                };
                let already_typed = matches!(
                    inner_ir,
                    Ir::GenericType { .. } | Ir::SimpleStatement { element_name: "type", .. }
                );
                let typed = if already_typed {
                    inner_ir
                } else {
                    let r = inner_ir.range();
                    let s = inner_ir.span();
                    Ir::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner_ir],
                        range: r,
                        span: s,
                    }
                };
                children.push(Ir::SimpleStatement {
                    element_name: "extends",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![typed],
                    range: range_of(cn),
                    span: span_of(cn),
                });
            }
            Ir::SimpleStatement {
                element_name: "generic",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }

        "array_type" => simple_statement_marked(node, "type", &["array"], source),
        "tuple_type" => simple_statement_marked(node, "type", &["tuple"], source),
        "union_type" => simple_statement_marked(node, "type", &["union"], source),
        "intersection_type" => simple_statement_marked(node, "type", &["intersection"], source),
        "literal_type" => simple_statement_marked(node, "type", &["literal"], source),
        "function_type" => simple_statement_marked(node, "type", &["function"], source),
        "readonly_type" => simple_statement_marked(node, "type", &["readonly"], source),
        "constructor_type" => simple_statement_marked(node, "type", &["constructor"], source),
        "type_query" => simple_statement_marked(node, "type", &["typeof"], source),
        "index_type_query" => simple_statement_marked(node, "type", &["keyof"], source),
        "lookup_type" => simple_statement_marked(node, "type", &["lookup"], source),
        "conditional_type" => simple_statement_marked(node, "type", &["conditional"], source),
        "mapped_type_clause" => simple_statement_marked(node, "type", &["mapped"], source),
        "template_literal_type" => simple_statement_marked(node, "type", &["template"], source),
        // `x is number` — a type-predicate wrapper. Inlines its
        // children (`<name>x</name><type><name>number</name></type>`)
        // into the parent so naked `<predicate>` and `asserts_annotation`
        // both end up with flat children. The standalone case (no
        // `asserts` keyword) is wrapped at lowering time at the call
        // site (we wrap with simple_statement so it still emits
        // `<predicate>`).
        "type_predicate" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| {
                    // The right side is the type; wrap it in `<type>`
                    // if it's a leaf identifier so consumers see
                    // `<type><name>...</name></type>`.
                    if c.kind() == "type_identifier" || c.kind() == "predefined_type"
                        || c.kind() == "identifier"
                    {
                        // Decide whether this is the LHS (variable name) or RHS (type).
                        // The first identifier is the LHS, second is RHS. We rely on the
                        // `name` field for LHS detection.
                        if let Some(name_n) = node.child_by_field_name("name") {
                            if name_n.id() == c.id() {
                                return Ir::Name { range: range_of(c), span: span_of(c) };
                            }
                        }
                        // Otherwise treat as type leaf — wrap in <type><name/></type>.
                        return Ir::SimpleStatement {
                            element_name: "type",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![Ir::Name { range: range_of(c), span: span_of(c) }],
                            range: range_of(c),
                            span: span_of(c),
                        };
                    }
                    lower_node(c, source)
                })
                .collect();
            Ir::SimpleStatement {
                element_name: "predicate",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }
        "asserts_annotation" | "asserts" => {
            let mut cursor = node.walk();
            let mut children: Vec<Ir> = Vec::new();
            for c in node.named_children(&mut cursor) {
                if c.kind() == "type_predicate" {
                    // Inline type_predicate's children directly.
                    let mut tc = c.walk();
                    for inner in c.named_children(&mut tc) {
                        if inner.kind() == "type_identifier" || inner.kind() == "predefined_type"
                            || inner.kind() == "identifier"
                        {
                            if let Some(name_n) = c.child_by_field_name("name") {
                                if name_n.id() == inner.id() {
                                    children.push(Ir::Name { range: range_of(inner), span: span_of(inner) });
                                    continue;
                                }
                            }
                            children.push(Ir::SimpleStatement {
                                element_name: "type",
                                modifiers: Modifiers::default(),
                                extra_markers: &[],
                                children: vec![Ir::Name { range: range_of(inner), span: span_of(inner) }],
                                range: range_of(inner),
                                span: span_of(inner),
                            });
                        } else {
                            children.push(lower_node(inner, source));
                        }
                    }
                } else {
                    children.push(lower_node(c, source));
                }
            }
            Ir::SimpleStatement {
                element_name: "predicate",
                modifiers: Modifiers::default(),
                extra_markers: &["asserts"],
                children,
                range,
                span,
            }
        }
        "infer_type" => simple_statement_marked(node, "type", &["infer"], source),
        // `type X = Y;` — alias with a name and a value type. The
        // value gets wrapped in `<type>` if it's a leaf identifier
        // (predefined_type / type_identifier) so XPath can address
        // `alias/type[name='Y']`. Already-typed inner (generic_type,
        // SimpleStatement<type>) passes through unchanged.
        // Interface property: `readonly id: string`, `label?: string`,
        // `value: T`. Lower as `<property>` with markers.
        "property_signature" => {
            let name_node = node.child_by_field_name("name");
            let type_node = node.child_by_field_name("type");
            // Detect `readonly` and `?` (optional) markers via unnamed children.
            let mut readonly = false;
            let mut optional = false;
            let mut tcursor = node.walk();
            for c in node.children(&mut tcursor) {
                if !c.is_named() {
                    let txt = text_of(c, source);
                    match txt.as_str() {
                        "readonly" => readonly = true,
                        "?" => optional = true,
                        _ => {}
                    }
                }
            }
            let mut markers: Vec<&'static str> = Vec::new();
            if readonly { markers.push("readonly"); }
            if optional { markers.push("optional"); }
            let extra_markers: &'static [&'static str] = match markers.as_slice() {
                [] => &[],
                ["readonly"] => &["readonly"],
                ["optional"] => &["optional"],
                ["readonly", "optional"] => &["readonly", "optional"],
                _ => &[],
            };
            let mut children: Vec<Ir> = Vec::new();
            if let Some(n) = name_node {
                children.push(Ir::Name { range: range_of(n), span: span_of(n) });
            }
            if let Some(t) = type_node {
                let mut tc = t.walk();
                let inner = t.named_children(&mut tc).next().unwrap_or(t);
                let inner_ir = lower_node(inner, source);
                let already_typed = matches!(
                    &inner_ir,
                    Ir::GenericType { .. }
                        | Ir::SimpleStatement { element_name: "type", .. }
                );
                if already_typed {
                    children.push(inner_ir);
                } else {
                    let r = inner_ir.range();
                    let s = inner_ir.span();
                    children.push(Ir::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner_ir],
                        range: r,
                        span: s,
                    });
                }
            }
            Ir::SimpleStatement {
                element_name: "property",
                modifiers: Modifiers::default(),
                extra_markers,
                children,
                range,
                span,
            }
        }
        // `[K in keyof T]: T[K]` — index signature. Lower as `<indexer>`.
        "index_signature" => simple_statement(node, "indexer", source),
        // `await x` — emits `<await>x</await>`.
        "await_expression" => simple_statement(node, "await", source),
        // `Foo<T>` as expression position (instantiation_expression).
        "instantiation_expression" => {
            let function_node = node.child_by_field_name("function");
            let type_args_node = node.child_by_field_name("type_arguments");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(f) = function_node { children.push(lower_node(f, source)); }
            if let Some(ta) = type_args_node {
                let mut tc = ta.walk();
                for c in ta.named_children(&mut tc) {
                    let inner_ir = lower_node(c, source);
                    let already_typed = matches!(
                        &inner_ir,
                        Ir::GenericType { .. }
                            | Ir::SimpleStatement { element_name: "type", .. }
                    );
                    if already_typed {
                        children.push(inner_ir);
                    } else {
                        let r = inner_ir.range();
                        let s = inner_ir.span();
                        children.push(Ir::SimpleStatement {
                            element_name: "type",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![inner_ir],
                            range: r,
                            span: s,
                        });
                    }
                }
            }
            Ir::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers: &["generic"],
                children,
                range,
                span,
            }
        }
        // `<T>` after a function/instantiation — flatten as type-children
        // siblings into the parent.
        "type_arguments" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| {
                    let inner_ir = lower_node(c, source);
                    let already_typed = matches!(
                        &inner_ir,
                        Ir::GenericType { .. }
                            | Ir::SimpleStatement { element_name: "type", .. }
                    );
                    if already_typed {
                        inner_ir
                    } else {
                        let r = inner_ir.range();
                        let s = inner_ir.span();
                        Ir::SimpleStatement {
                            element_name: "type",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![inner_ir],
                            range: r,
                            span: s,
                        }
                    }
                })
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }
        // `label: stmt` — labeled statement.
        "labeled_statement" => simple_statement(node, "label", source),
        // `module M { ... }` (TS namespace module body).
        "internal_module" => simple_statement(node, "namespace", source),
        // `import M = require(...)`.
        "import_require_clause" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }
        // `class { static { ... } }`.
        "class_static_block" => simple_statement_marked(node, "block", &["static"], source),
        // `[a, b = 1]` / `{ a = 1 }` — assignment-pattern with default
        // value. Inline the children into the parent (the parent
        // pair_pattern / array_pattern provides the role-element);
        // wrap the right side in `<value><expression>...` so XPath
        // can address the default consistently.
        "assignment_pattern" => {
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(l) = left {
                children.push(lower_node(l, source));
            }
            if let Some(r) = right {
                let inner = lower_node(r, source);
                let r_range = range_of(r);
                let r_span = span_of(r);
                children.push(Ir::SimpleStatement {
                    element_name: "value",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: r_range,
                        span: r_span,
                    }],
                    range: r_range,
                    span: r_span,
                });
            }
            Ir::Inline { children, list_name: None, range, span }
        }
        // `{ a: aa = 1 }` in a pattern — `object_assignment_pattern` has a
        // pair-with-default. Inline to flatten.
        "object_assignment_pattern_unused" => simple_statement(node, "pair", source),
        // Regex literal.
        "regex" => Ir::String { range, span },
        // Label name in `outer: for (...) { break outer; }`.
        "statement_identifier" => Ir::Name { range, span },
        // `yield x` / `yield* x` — emits `<yield>x</yield>`.
        "yield_expression" => simple_statement(node, "yield", source),
        // `T?` short-form optional — emit as `<type>` with `optional` marker
        // wrapping the inner type.
        "opting_type_annotation" => simple_statement_marked(node, "type", &["optional"], source),
        // `{ x = 1 }` in destructure — assignment pattern; emit as `<pair>` with the inner.
        "object_assignment_pattern" => simple_statement(node, "pair", source),
        // `x!` — non-null assertion, render as `<nonnull>x</nonnull>`.
        "non_null_expression" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            let children: Vec<Ir> = match inner {
                Some(i) => vec![lower_node(i, source)],
                None => Vec::new(),
            };
            Ir::SimpleStatement {
                element_name: "nonnull",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }
        // `this` as a type expression. Render as `<type><name>this</name></type>` shape.
        "this_type" => Ir::Name { range, span },
        // `...string[]` rest type — lower as `<type><rest/><type[array]>...` shape.
        "rest_type" => simple_statement_marked(node, "type", &["rest"], source),
        // Template literal type segments — lower transparent.
        "template_type" => Ir::Name { range, span },
        "string_fragment" => Ir::String { range, span },
        // `enum E { A = "a" }` — `enum_assignment` is `name = value`.
        "enum_assignment" => simple_statement(node, "constant", source),
        // `(x: T)` — formal_parameters used in function-type RHS. Inline.
        "formal_parameters" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }
        // `declare` ambient declarations.
        "ambient_declaration" => simple_statement(node, "declare", source),
        // Switch body wraps the cases — lower transparent.
        "switch_body" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }

        "type_alias_declaration" => {
            let name_node = node.child_by_field_name("name");
            let value_node = node.child_by_field_name("value");
            let type_params_node = node.child_by_field_name("type_parameters");
            let mut children: Vec<Ir> = Vec::new();
            if let Some(n) = name_node {
                children.push(Ir::Name { range: range_of(n), span: span_of(n) });
            }
            if let Some(tp) = type_params_node {
                let mut tc = tp.walk();
                for c in tp.named_children(&mut tc) {
                    children.push(lower_node(c, source));
                }
            }
            if let Some(v) = value_node {
                let inner = lower_node(v, source);
                let already_typed = matches!(
                    &inner,
                    Ir::GenericType { .. }
                        | Ir::SimpleStatement { element_name: "type", .. }
                        | Ir::SimpleStatement { element_name: "predicate", .. }
                );
                if already_typed {
                    children.push(inner);
                } else {
                    let r = inner.range();
                    let s = inner.span();
                    children.push(Ir::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: r,
                        span: s,
                    });
                }
            }
            Ir::SimpleStatement {
                element_name: "alias",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }
        "enum_declaration" => simple_statement(node, "enum", source),

        // Comments.
        "comment" => Ir::Comment {
            leading: false,
            trailing: false,
            range,
            span,
        },

        // Parenthesized.
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(n) => lower_node(n, source),
                None => Ir::Unknown { kind: "paren(empty)".to_string(), range, span },
            }
        }

        // Type assertion / as expression / satisfies — use `<as>` to match
        // the imperative pipeline shape (allows nested `<as>` for stacked
        // assertions like `<T><U>x` — old-style cast inside another).
        "as_expression" | "type_assertion" | "satisfies_expression" => {
            simple_statement(node, "as", source)
        }

        // Ternary.
        "ternary_expression" => {
            let cond = node.child_by_field_name("condition").map(|n| lower_node(n, source));
            let if_true = node.child_by_field_name("consequence").map(|n| lower_node(n, source));
            let if_false = node.child_by_field_name("alternative").map(|n| lower_node(n, source));
            match (cond, if_true, if_false) {
                (Some(c), Some(t), Some(f)) => Ir::Ternary {
                    condition: Box::new(c),
                    if_true: Box::new(t),
                    if_false: Box::new(f),
                    range,
                    span,
                },
                _ => Ir::Unknown {
                    kind: "ternary(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Object / array literal.
        "object" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::SimpleStatement {
                element_name: "object",
                modifiers: Modifiers::default(),
                extra_markers: &["literal"],
                children,
                range,
                span,
            }
        }
        "array" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::List { children, range, span }
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

        "spread_element" => Ir::ListSplat {
            inner: {
                let mut cursor = node.walk();
                let inner = node.named_children(&mut cursor).next();
                Box::new(match inner {
                    Some(i) => lower_node(i, source),
                    None => Ir::Unknown { kind: "spread(empty)".to_string(), range, span },
                })
            },
            range,
            span,
        },

        // Array/object destructuring patterns. Lower to `<pattern>`
        // with an `<array/>`/`<object/>` shape marker. Children are
        // the inner names / pair patterns.
        "array_pattern" => simple_statement_marked(node, "pattern", &["array"], source),
        "object_pattern" => simple_statement_marked(node, "pattern", &["object"], source),
        // Shorthand `{ x }` in an object pattern — just a name.
        "shorthand_property_identifier_pattern" => Ir::Name { range, span },
        // `{ x: a }` in an object pattern — `<pair>`.
        "pair_pattern" => simple_statement(node, "pair", source),

        // `...rest` pattern in a parameter list. Tree-sitter wraps the
        // identifier in a `rest_pattern`. We lower to `<rest><name>rest</name></rest>`
        // so the enclosing `<parameter>` element ends up with the shape
        // `<parameter><required/><rest/><rest><name>rest</name></rest></parameter>`
        // (the outer parameter is responsible for the `<rest/>` marker).
        "rest_pattern" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            let children: Vec<Ir> = match inner {
                Some(i) => vec![lower_node(i, source)],
                None => Vec::new(),
            };
            Ir::SimpleStatement {
                element_name: "rest",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }

        // Switch.
        "switch_statement" => simple_statement(node, "switch", source),
        "switch_case" | "switch_default" => simple_statement(node, "arm", source),

        // Argument list (rare standalone).
        "arguments" => {
            let mut cursor = node.walk();
            let children: Vec<Ir> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            Ir::Inline { children, list_name: None, range, span }
        }

        // Fallback ------------------------------------------------------
        other => Ir::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

fn lower_ts_else_chain(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    // tree-sitter-typescript's `if_statement.alternative` is an
    // `else_clause` wrapping a body or another if.
    if node.kind() == "else_clause" {
        let mut cursor = node.walk();
        let inner = node.named_children(&mut cursor).next();
        match inner {
            Some(i) if i.kind() == "if_statement" => {
                let cond = i.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source)));
                let body = i.child_by_field_name("consequence").map(|n| Box::new(lower_block_like(n, source)));
                let alt = i.child_by_field_name("alternative");
                let else_branch = alt.map(|a| Box::new(lower_ts_else_chain(a, source)));
                match (cond, body) {
                    (Some(c), Some(b)) => Ir::ElseIf {
                        condition: c,
                        body: b,
                        else_branch,
                        range,
                        span,
                    },
                    _ => Ir::Unknown { kind: "ts_else_if(missing)".to_string(), range, span },
                }
            }
            Some(b) => Ir::Else {
                body: Box::new(lower_block_like(b, source)),
                range,
                span,
            },
            None => Ir::Unknown { kind: "else_clause(empty)".to_string(), range, span },
        }
    } else if node.kind() == "if_statement" {
        let cond = node.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source)));
        let body = node.child_by_field_name("consequence").map(|n| Box::new(lower_block_like(n, source)));
        let alt = node.child_by_field_name("alternative");
        let else_branch = alt.map(|a| Box::new(lower_ts_else_chain(a, source)));
        match (cond, body) {
            (Some(c), Some(b)) => Ir::ElseIf { condition: c, body: b, else_branch, range, span },
            _ => Ir::Unknown { kind: "ts_else_if(missing)".to_string(), range, span },
        }
    } else {
        Ir::Else {
            body: Box::new(lower_block_like(node, source)),
            range,
            span,
        }
    }
}

fn lower_block_like(node: TsNode<'_>, source: &str) -> Ir {
    let mut cursor = node.walk();
    let children: Vec<Ir> = node
        .named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
    let children = merge_ts_line_comments(children, source);
    let block_wrap = node.kind() == "statement_block";
    Ir::Body {
        children,
        pass_only: false,
        block_wrap,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Lower a TS `variable_declarator`'s children into a flat list:
/// `<type>` (when annotated), `<name>`, and `<value>` (when
/// initialized). Used for both single- and multi-declarator
/// statements.
fn lower_ts_declarator_parts(d: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut parts: Vec<Ir> = Vec::new();
    if let Some(t) = d.child_by_field_name("type") {
        let mut tc = t.walk();
        let inner = t.named_children(&mut tc).next().unwrap_or(t);
        let inner_ir = lower_node(inner, source);
        let already_typed = matches!(
            inner_ir,
            Ir::GenericType { .. } | Ir::SimpleStatement { element_name: "type", .. }
        );
        if already_typed {
            parts.push(inner_ir);
        } else {
            parts.push(Ir::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![inner_ir],
                range: range_of(t),
                span: span_of(t),
            });
        }
    }
    if let Some(n) = d.child_by_field_name("name") {
        parts.push(lower_node(n, source));
    }
    if let Some(v) = d.child_by_field_name("value") {
        parts.push(Ir::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![lower_node(v, source)],
            range: range_of(v),
            span: span_of(v),
        });
    }
    parts
}

fn lower_ts_variable_declarator(
    declarator: TsNode<'_>,
    source: &str,
    range: ByteRange,
    span: Span,
    element_name: &'static str,
    modifiers: Modifiers,
    extra_markers: &'static [&'static str],
) -> Ir {
    let name_node = declarator.child_by_field_name("name");
    let type_node = declarator.child_by_field_name("type");
    let value_node = declarator.child_by_field_name("value");
    let Some(n) = name_node else {
        let mut cursor = declarator.walk();
        let children: Vec<Ir> = declarator
            .named_children(&mut cursor)
            .map(|c| lower_node(c, source))
            .collect();
        return Ir::Inline {
            children,
            list_name: None,
            range,
            span,
        };
    };
    let name_ir = lower_node(n, source);
    let type_ir = type_node.map(|t| {
        let mut tc = t.walk();
        let inner = t.named_children(&mut tc).next().unwrap_or(t);
        Box::new(lower_node(inner, source))
    });
    let value_ir = value_node.map(|v| {
        let inner = lower_node(v, source);
        Box::new(Ir::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![inner],
            range: range_of(v),
            span: span_of(v),
        })
    });
    // For now ignore extra_markers (let/const/var). The
    // imperative pipeline emits these as `<variable[const]>` etc.
    // — we'll add this as Ir::Variable.extra_markers later if
    // tests require it; for now, omit and let the post-pass
    // populate `list=` if needed.
    let _ = extra_markers;
    Ir::Variable {
        element_name,
        modifiers,
        decorators: Vec::new(),
        type_ann: type_ir,
        name: Box::new(name_ir),
        value: value_ir,
        range,
        span,
    }
}

fn lower_children(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect()
}

fn extract_ts_decorators(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| c.kind() == "decorator")
        .map(|c| lower_node(c, source))
        .collect()
}

fn lower_ts_modifiers(
    node: TsNode<'_>,
    source: &str,
    default_access: Option<Access>,
) -> Modifiers {
    let mut m = Modifiers::default();
    if let Some(da) = default_access {
        m.access = Some(da);
    }
    let mut cursor = node.walk();
    for c in node.children(&mut cursor) {
        let kind = c.kind();
        match kind {
            "accessibility_modifier" => {
                let txt = text_of(c, source);
                match txt.as_str() {
                    "public" => m.access = Some(Access::Public),
                    "private" => m.access = Some(Access::Private),
                    "protected" => m.access = Some(Access::Protected),
                    _ => {}
                }
            }
            "static" => m.static_ = true,
            "abstract" => m.abstract_ = true,
            "readonly" => m.readonly = true,
            "async" => m.async_ = true,
            "override" | "override_modifier" => m.override_ = true,
            _ => {
                // Token-level keyword detection (some modifiers are unnamed children).
                if !c.is_named() {
                    let txt = text_of(c, source);
                    match txt.as_str() {
                        "static" => m.static_ = true,
                        "abstract" => m.abstract_ = true,
                        "readonly" => m.readonly = true,
                        "async" => m.async_ = true,
                        "override" => m.override_ = true,
                        "public" => m.access = Some(Access::Public),
                        "private" => m.access = Some(Access::Private),
                        "protected" => m.access = Some(Access::Protected),
                        "get" => m.getter = true,
                        "set" => m.setter = true,
                        "*" => m.generator = true,
                        _ => {}
                    }
                }
            }
        }
    }
    m
}

fn merge_ts_line_comments(children: Vec<Ir>, source: &str) -> Vec<Ir> {
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
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_is_line_comment = source[prev_range.start as usize..prev_range.end as usize]
                    .trim_start().starts_with("//");
                let curr_is_line_comment = source[range.start as usize..range.end as usize]
                    .trim_start().starts_with("//");
                let prev_was_trailing = matches!(out.last(), Some(Ir::Comment { trailing: true, .. }));
                if only_one_newline && prev_is_line_comment && curr_is_line_comment
                    && !prev_was_trailing && !curr_is_trailing {
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

fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
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
        range,
        span,
    }
}

fn simple_statement_marked(
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers: &'static [&'static str],
    source: &str,
) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
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
        range,
        span,
    }
}

fn op_marker(op: &str) -> Option<&'static str> {
    Some(match op {
        "+" => "plus",
        "-" => "minus",
        "*" => "multiply",
        "/" => "divide",
        "%" => "modulo",
        "**" => "power",
        "==" | "===" => "equal",
        "!=" | "!==" => "not_equal",
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
        "instanceof" => "instanceof",
        "in" => "in",
        "typeof" => "typeof",
        "void" => "void",
        "delete" => "delete",
        _ => return None,
    })
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

fn range_of(node: TsNode<'_>) -> ByteRange {
    let r = node.byte_range();
    ByteRange::new(r.start as u32, r.end as u32)
}

fn text_of(node: TsNode<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .map(|s| s.to_string())
        .unwrap_or_default()
}
