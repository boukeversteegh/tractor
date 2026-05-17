//! TypeScript / JavaScript / TSX / JSX tree-sitter CST → tree lowering.
//!
//! Single lower function handles all four flavours — the TS / JS / TSX
//! grammars share most node kinds (TS is a superset, TSX adds JSX-only
//! kinds). Per-kind arms recursively lower children; the renderer in
//! `crate::tree::to_xot` is shared.
//!
//! Production parser routes ts/js/tsx/jsx through this lowering
//! end-to-end (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/typescript/{rules,transformations,transform}.rs`
//! modules were retired alongside this migration.
//!
//! Coverage is incremental: each unhandled kind falls through to
//! `SyntaxTree::Unknown`. Diagnostic test
//! `tests/ir_typescript_missing_kinds.rs` lists kinds the corpus
//! exercises that aren't yet typed.


use crate::raw::RawNode;

use crate::tree::lower_helpers::{
    false_of, name_of, null_of, range_of, span_of, string_of, text_of, true_of,
};
use crate::tree::types::{Access, AccessSegment, ByteRange, Flag, SyntaxTree, Modifiers, Marker, OperatorKind, ParamKind};

/// Lower a TypeScript tree-sitter root node to [`SyntaxTree`].
pub fn lower_typescript_root(root: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => SyntaxTree::Module {
            children: merge_ts_line_comments(lower_children(root, source), source),
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

pub fn lower_typescript_node(node: &RawNode, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

fn lower_node(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms -----------------------------------------------------
        "identifier" | "type_identifier" | "property_identifier" | "shorthand_property_identifier"
        | "shorthand_property_identifier_pattern" => name_of(node, source),
        // TS `number` is dual-purpose (int + float); the imperative
        // pipeline emits `<number>` (not `<int>` like C#/Python).
        // Use SimpleStatement with element_name="number" to preserve
        // source bytes as text.
        "number" => SyntaxTree::SimpleStatement {
            element_name: "number",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: Vec::new(),
            range,
            span,
        },
        "string" => string_of(node, source),
        // Template literals contain `template_substitution` (`${...}`)
        // children. Lower as `<template>` with interpolation children
        // so XPath can address `template[interpolation/name='x']`.
        // Plain templates (no substitutions) still emit `<template>`.
        "template_string" => {
            let has_subs = node.named_children()
                .any(|c| c.kind() == "template_substitution");
            if !has_subs {
                string_of(node, source)
            } else {
                let children: Vec<SyntaxTree> = node
                    .named_children()
                    .map(|c| lower_node(c, source))
                    .collect();
                SyntaxTree::SimpleStatement {
                    element_name: "template",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children,
                    range,
                    span,
                }
            }
        }
        "template_substitution" => simple_statement(node, "interpolation", source),
        "true" => true_of(node, source),
        "false" => false_of(node, source),
        "null" => null_of(node, source),
        "undefined" => name_of(node, source),
        "this" => SyntaxTree::SimpleStatement {
            element_name: "this",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: Vec::new(),
            range,
            span,
        },
        "super" => SyntaxTree::SimpleStatement {
            element_name: "super",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: Vec::new(),
            range,
            span,
        },

        // Predefined types.
        "predefined_type" | "void_type" => name_of(node, source),

        // ----- Containers / declarations ---------------------------------
        "class_declaration" | "abstract_class_declaration" | "class" | "interface_declaration" => {
            let kind: &'static str = if node.kind() == "interface_declaration" {
                "interface"
            } else {
                "class"
            };
            let name_node = node.child_by_field_name("name");
            let body_node = node.child_by_field_name("body");
            let type_param_list = node
                .named_children()
                .find(|c| c.kind() == "type_parameters");
            // Extends / implements clauses.
            let mut bases: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                match c.kind() {
                    "class_heritage" => {
                        for inner in c.named_children() {
                            match inner.kind() {
                                "extends_clause" => {
                                    for t in inner.named_children() {
                                        bases.push(lower_node(t, source).wrap_extends());
                                    }
                                }
                                "implements_clause" => {
                                    for t in inner.named_children() {
                                        let inner_ir = lower_node(t, source);
                                        let already_typed = matches!(
                                            inner_ir,
                                            SyntaxTree::GenericType { .. }
                                                | SyntaxTree::SimpleStatement { element_name: "type", .. }
                                        );
                                        let typed = if already_typed {
                                            inner_ir
                                        } else {
                                            SyntaxTree::SimpleStatement {
                                                element_name: "type",
                                                modifiers: Modifiers::default(),
                                                extra_markers: Vec::new(),
                                                children: vec![inner_ir],
                                                range: range_of(t),
                                                span: span_of(t),
                                            }
                                        };
                                        bases.push(SyntaxTree::SimpleStatement {
                                            element_name: "implements",
                                            modifiers: Modifiers::default(),
                                            extra_markers: Vec::new(),
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
                        for t in c.named_children() {
                            bases.push(lower_node(t, source));
                        }
                    }
                    _ => {}
                }
            }
            let modifiers = lower_ts_modifiers(node, source, None);
            let generics: Vec<SyntaxTree> = match type_param_list {
                Some(tpl) => tpl.named_children().map(|c| lower_node(c, source)).collect(),
                None => Vec::new(),
            };
            let decorators = extract_ts_decorators(node, source);
            let name = Box::new(match name_node {
                Some(n) => name_of(n, source),
                None => SyntaxTree::Unknown {
                    kind: format!("{}(missing name)", kind),
                    range,
                    span,
                },
            });
            let body = Box::new(match body_node {
                Some(b) => lower_block_like(b, source),
                None => SyntaxTree::Body {
                    children: Vec::new(),
                    pass_only: false,
                    block_wrap: false,
                    range: ByteRange::empty_at(range.end),
                    span,
                },
            });
            let where_clauses = Vec::new();
            match kind {
                "interface" => SyntaxTree::Interface {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
                _ => SyntaxTree::Class {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
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
            let parameters: Vec<SyntaxTree> = match params_node {
                Some(p) => {
                    p.named_children()
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            let returns = return_type_node.map(|t| {
                // return_type wraps an inner type.
                let inner = t.named_children().next().unwrap_or(t);
                Box::new(SyntaxTree::Returns {
                    type_ann: Box::new(lower_node(inner, source)),
                    range: range_of(t),
                    span: span_of(t),
                })
            });
            let body: Option<Box<SyntaxTree>> = body_node.map(|b| Box::new(lower_block_like(b, source)));
            let element_name: &'static str =
                if matches!(node.kind(), "method_definition" | "method_signature" | "abstract_method_signature") {
                    "method"
                } else {
                    "function"
                };
            // Type parameters: `<T, U>` lower to flat `<generic>` siblings.
            let type_params_node = node
                .named_children()
                .find(|c| c.kind() == "type_parameters");
            let generics: Vec<SyntaxTree> = match type_params_node {
                Some(tp) => tp.named_children().map(|c| lower_node(c, source)).collect(),
                None => Vec::new(),
            };
            let decorators = extract_ts_decorators(node, source);
            let name = Box::new(match name_node {
                Some(n) => name_of(n, source),
                None => SyntaxTree::Unknown {
                    kind: format!("{}(missing name)", element_name),
                    range,
                    span,
                },
            });
            SyntaxTree::function_or_method(
                element_name, modifiers, decorators, name, generics, parameters,
                returns, Vec::new(), body, range, span,
            )
        }

        "arrow_function" => {
            // Arrow has a parameter list (or single bare identifier) and
            // a body which is either a `<block>` or an expression.
            // Emits `<arrow>` with parameter children + `<body>` (block)
            // or `<value><expression>...</expression></value>` (expr).
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            let return_type_node = node.child_by_field_name("return_type");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(p) = params_node {
                for c in p.named_children() {
                    children.push(lower_node(c, source));
                }
            } else {
                // Single bare identifier — `x => ...`. Tree-sitter
                // exposes the parameter as the `parameter` field
                // (single identifier).
                if let Some(p) = node.child_by_field_name("parameter") {
                    // Wrap as a Parameter so shape stays uniform.
                    children.push(SyntaxTree::Parameter {
                        kind: ParamKind::Regular,
                        extra_markers: vec![Marker::implicit("required")],
                        modifiers: Modifiers::default(),
                        name: Box::new(name_of(p, source)),
                        type_ann: None,
                        default: None,
                        range: range_of(p),
                        span: span_of(p),
                    });
                }
            }
            if let Some(rt) = return_type_node {
                let inner = rt.named_children().next().unwrap_or(rt);
                children.push(SyntaxTree::Returns {
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
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "value",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![SyntaxTree::SimpleStatement {
                            element_name: "expression",
                            modifiers: Modifiers::default(),
                            extra_markers: Vec::new(),
                            children: vec![inner],
                            range: range_of(b),
                            span: span_of(b),
                        }],
                        range: range_of(b),
                        span: span_of(b),
                    });
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "arrow",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
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
            let extra_markers: Vec<crate::tree::types::Marker> = if node.kind() == "optional_parameter" {
                vec![Marker::implicit("optional")]
            } else {
                vec![Marker::implicit("required")]
            };
            let modifiers = lower_ts_modifiers(node, source, None);
            // Tree-sitter labeled-tuple elements (`[head: number]`) emit
            // a required/optional parameter without a `pattern` field —
            // the label-identifier becomes the FIRST named child instead.
            // Fall back to the first named identifier child for the name.
            let name_fallback = if pattern.is_none() {
                let found = node.named_children()
                    .find(|c| matches!(c.kind(), "identifier" | "type_identifier" | "property_identifier"));
                found
            } else {
                None
            };
            SyntaxTree::Parameter {
                kind: ParamKind::Regular,
                extra_markers,
                modifiers,
                name: Box::new(match pattern.or(name_fallback) {
                    Some(p) => lower_node(p, source),
                    None => SyntaxTree::Unknown {
                        kind: "parameter(missing pattern)".to_string(),
                        range,
                        span,
                    },
                }),
                type_ann: type_node.map(|t| {
                    let inner = t.named_children().next().unwrap_or(t);
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
            let declarators: Vec<&RawNode> = node
                .named_children()
                .filter(|c| c.kind() == "variable_declarator")
                .collect();
            // Detect let/const/var keyword as a marker.
            let leading = source[range.start as usize..range.end as usize].trim_start();
            let kw_marker: Vec<crate::tree::types::Marker> = if leading.starts_with("const") {
                vec![Marker::implicit("const")]
            } else if leading.starts_with("let") {
                vec![Marker::implicit("let")]
            } else if leading.starts_with("var") {
                vec![Marker::implicit("var")]
            } else {
                Vec::new()
            };
            if declarators.is_empty() {
                return SyntaxTree::Unknown {
                    kind: "lexical_declaration(no declarators)".to_string(),
                    range,
                    span,
                };
            }
            // Single-declarator: build the typed `SyntaxTree::Variable`
            // directly so name/type_ann/value become real fields on
            // the variant (visible to field projection) instead of a
            // flat `<children>` Vec under SimpleStatement.
            //
            // Multi-declarator (`let x = 1, y = 2;`) keeps the
            // SimpleStatement wrapper for now — no typed
            // multi-declarator variant exists, and each declarator
            // still becomes a `<declarator>` SimpleStatement inside.
            if declarators.len() == 1 {
                let d = declarators[0];
                let parts = lower_ts_declarator_parts(d, source);
                let name = parts.name.unwrap_or_else(|| {
                    Box::new(SyntaxTree::Unknown {
                        kind: "variable_declarator(missing name)".to_string(),
                        range,
                        span,
                    })
                });
                SyntaxTree::Variable {
                    modifiers,
                    decorators: Vec::new(),
                    extra_markers: kw_marker,
                    type_ann: parts.type_ann,
                    name,
                    value: parts.value,
                    range,
                    span,
                }
            } else {
                let mut children: Vec<SyntaxTree> = Vec::new();
                for d in declarators {
                    let parts = lower_ts_declarator_parts(d, source);
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "declarator",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: ts_declarator_parts_flat(parts),
                        range: range_of(d),
                        span: span_of(d),
                    });
                }
                SyntaxTree::SimpleStatement {
                    element_name: "variable",
                    modifiers,
                    extra_markers: kw_marker,
                    children,
                    range,
                    span,
                }
            }
        }

        // Public / private / class field.
        "public_field_definition" => {
            let name_node = node.child_by_field_name("name");
            let type_node = node.child_by_field_name("type");
            let value_node = node.child_by_field_name("value");
            // TS class fields default to `public`.
            let modifiers = lower_ts_modifiers(node, source, Some(Access::Public));
            let value_ir = value_node.map(|v| crate::tree::Expression::wrap(lower_node(v, source)));
            SyntaxTree::Field {
                modifiers,
                decorators: extract_ts_decorators(node, source),
                type_ann: type_node.map(|t| {
                    let inner = t.named_children().next().unwrap_or(t);
                    Box::new(lower_node(inner, source))
                }),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown {
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
        "object_type" => simple_statement_marked(node, "type", vec![Marker::implicit("object")], source),

        // Statements.
        "expression_statement" => {
            let inner = node.named_children().next();
            match inner {
                Some(n) => SyntaxTree::Expression {
                    inner: Box::new(lower_node(n, source)),
                    marker: None,
                    range,
                    span,
                },
                None => SyntaxTree::Unknown {
                    kind: "expression_statement(empty)".to_string(),
                    range,
                    span,
                },
            }
        }

        "return_statement" => {
            let value = node.named_children().next();
            SyntaxTree::Return {
                value: value.map(|v| Box::new(lower_node(v, source).wrap_expression_inline_aware())),
                range,
                span,
            }
        }

        "if_statement" => {
            let cond = node
                .child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source).wrap_expression()));
            let body = node
                .child_by_field_name("consequence")
                .map(|n| Box::new(lower_block_like(n, source)));
            let alt = node.child_by_field_name("alternative");
            let else_branch = alt.map(|a| Box::new(lower_ts_else_chain(a, source)));
            match (cond, body) {
                (Some(c), Some(b)) => SyntaxTree::If {
                    condition: c,
                    body: b,
                    else_branch,
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "if_statement(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        "while_statement" => {
            let cond = node
                .child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source).wrap_expression()));
            let body = node
                .child_by_field_name("body")
                .map(|n| Box::new(lower_block_like(n, source)));
            match (cond, body) {
                (Some(c), Some(b)) => SyntaxTree::While {
                    condition: c,
                    body: b,
                    else_body: None,
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
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
            let updates: Vec<SyntaxTree> = update_node
                .map(|u| {
                    if u.kind() == "sequence_expression" {
                        // Comma-separated updates `j--, i++` — flatten.
                        u.named_children()
                            .map(|c| lower_node(c, source))
                            .collect()
                    } else {
                        vec![lower_node(u, source)]
                    }
                })
                .unwrap_or_default();
            match body {
                Some(b) => SyntaxTree::CFor {
                    initializer: init.map(|i| Box::new(lower_node(i, source))),
                    condition: cond_node.map(|c| Box::new(lower_node(c, source))),
                    updates,
                    body: b,
                    range,
                    span,
                },
                None => SyntaxTree::Unknown {
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
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(l) = left_node {
                let inner = lower_node(l, source);
                let inner_range = range_of(l);
                let inner_span = span_of(l);
                let expr = SyntaxTree::Expression {
                    inner: Box::new(inner),
                    marker: None,
                    range: inner_range,
                    span: inner_span,
                };
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "left",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![expr],
                    range: inner_range,
                    span: inner_span,
                });
            }
            if let Some(r) = right_node {
                let inner = lower_node(r, source);
                let inner_range = range_of(r);
                let inner_span = span_of(r);
                let expr = SyntaxTree::Expression {
                    inner: Box::new(inner),
                    marker: None,
                    range: inner_range,
                    span: inner_span,
                };
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "right",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![expr],
                    range: inner_range,
                    span: inner_span,
                });
            }
            if let Some(b) = body_node {
                children.push(lower_block_like(b, source));
            }
            SyntaxTree::SimpleStatement {
                element_name: "for",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
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
                (Some(b), Some(c)) => SyntaxTree::DoWhile {
                    body: b,
                    condition: c,
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "do_statement(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        "break_statement" => SyntaxTree::Break { range, span },
        "continue_statement" => SyntaxTree::Continue { range, span },

        // Try.
        "try_statement" => simple_statement(node, "try", source),
        "catch_clause" => simple_statement(node, "catch", source),
        "finally_clause" => simple_statement(node, "finally", source),
        "throw_statement" => lower_typescript_throw(node, source),

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
        "import_statement" => lower_ts_import_statement(node, source),
        "export_statement" => simple_statement(node, "export", source),
        // Import-clause / export-clause / namespace-import / named-imports
        // are wrapper grammar nodes — flatten their children into the
        // parent <import>/<export>. The post-pass `typescript_restructure_import`
        // restructures the resulting flat children into the canonical
        // shape (default/spec/path).
        "import_clause" | "export_clause" | "named_imports" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        // `* as ns` namespace-import — emit as `<namespace><name>ns</name></namespace>`.
        "namespace_import" => simple_statement(node, "namespace", source),
        // `import_specifier` / `export_specifier` — `{ namedA }` or `{ namedA as aliasedB }`.
        "import_specifier" | "export_specifier" => {
            let name_node = node.child_by_field_name("name");
            let alias_node = node.child_by_field_name("alias");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            if let Some(a) = alias_node {
                children.push(name_of(a, source));
            }
            SyntaxTree::SimpleStatement {
                element_name: "spec",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range,
                span,
            }
        }

        // Decorators.
        "decorator" => {
            let inner = node.named_children().next();
            match inner {
                Some(i) => SyntaxTree::Decorator {
                    inner: Box::new(lower_node(i, source)),
                    range,
                    span,
                },
                None => SyntaxTree::Unknown { kind: "decorator(empty)".to_string(), range, span },
            }
        }

        // Binary / unary / assignment.
        "binary_expression" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (left, right, op_kind(&op_text)) {
                (Some(l), Some(r), Some(kind)) => {
                    kind.build_binary(l, r, op_text, op_range, range, span)
                }
                _ => SyntaxTree::Unknown {
                    kind: "binary_expression(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        "unary_expression" | "update_expression" => {
            let mut op_node = None;
            for c in node.children() {
                if !c.is_named() {
                    op_node = Some(c);
                    break;
                }
            }
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let mut operand_node = None;
            for c in node.named_children() {
                operand_node = Some(c);
                break;
            }
            match operand_node {
                Some(o) => {
                    let is_postfix = op_range.start >= range_of(o).end;
                    let extra_markers: Vec<crate::tree::types::Marker> = if is_postfix {
                        vec![Marker::implicit("postfix")]
                    } else if matches!(op_text.as_str(), "++" | "--") {
                        vec![Marker::implicit("prefix")]
                    } else {
                        Vec::new()
                    };
                    match op_marker(&op_text) {
                        Some(marker) => SyntaxTree::Unary {
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
                (Some(l), Some(r)) => SyntaxTree::Assign {
                    targets: vec![lower_node(l, source).wrap_expression()],
                    type_annotation: None,
                    op_text,
                    op_range,
                    op_markers: Vec::new(),
                    values: vec![lower_node(r, source).wrap_expression()],
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
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
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }

        // `import.meta` / `new.target` — JS meta-properties are
        // atomic identifiers. We render them as a single <name> leaf
        // covering the dot-spanning text so the chain receiver shape
        // is `<name>import.meta</name>` (matches Python's __file__
        // precedent).
        "meta_property" => name_of(node, source),

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
                        SyntaxTree::ObjectAccess { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::ObjectAccess {
                                receiver,
                                segments,
                                range,
                                span,
                            }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "super"]),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => SyntaxTree::Unknown { kind: "member_expression(missing)".to_string(), range, span },
            }
        }

        "call_expression" => {
            // Check if function is itself a member chain — fold.
            let function_node = node.child_by_field_name("function");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<SyntaxTree> = match args_node {
                Some(a) => {
                    a.named_children()
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            match function_node {
                Some(f) => {
                    let callee = lower_node(f, source);
                    let callee_range = callee.range();
                    if let SyntaxTree::ObjectAccess { receiver, mut segments, .. } = callee {
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
                        return SyntaxTree::ObjectAccess { receiver, segments, range, span };
                    }
                    SyntaxTree::Call {
                        callee: Box::new(callee),
                        arguments,
                        range,
                        span,
                    }
                }
                None => SyntaxTree::Unknown { kind: "call_expression(missing)".to_string(), range, span },
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
                        SyntaxTree::ObjectAccess { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::ObjectAccess { receiver, segments, range, span }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "super"]),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => SyntaxTree::Unknown { kind: "subscript_expression(missing)".to_string(), range, span },
            }
        }

        "new_expression" => {
            let constructor_node = node.child_by_field_name("constructor");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<SyntaxTree> = match args_node {
                Some(a) => {
                    a.named_children()
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            SyntaxTree::ObjectCreation {
                type_target: constructor_node.map(|t| Box::new(lower_node(t, source))),
                arguments,
                initializer: None,
                range,
                span,
            }
        }

        // Generic types.
        "generic_type" => {
            let kids: Vec<&RawNode> = node.named_children().collect();
            if let Some(name) = kids.first() {
                let mut params: Vec<SyntaxTree> = Vec::new();
                for c in kids.iter().skip(1) {
                    if c.kind() == "type_arguments" {
                        params.extend(c.named_children().map(|n| lower_node(n, source)));
                    } else {
                        params.push(lower_node(*c, source));
                    }
                }
                SyntaxTree::GenericType {
                    name: Box::new(lower_node(*name, source)),
                    params,
                    range,
                    span,
                }
            } else {
                SyntaxTree::Unknown { kind: "generic_type(empty)".to_string(), range, span }
            }
        }

        "type_parameter" => {
            // TS: `T extends Foo = Default` — name + constraint + default.
            // The default branch (`= Default`) appears as a `default_type`
            // sibling in the CST.
            let mut name_node: Option<&RawNode> = None;
            let mut constraint_node: Option<&RawNode> = None;
            let mut default_node: Option<&RawNode> = None;
            for c in node.named_children() {
                match c.kind() {
                    "constraint" => constraint_node = Some(c),
                    "default_type" => default_node = Some(c),
                    _ if name_node.is_none() => name_node = Some(c),
                    _ => {}
                }
            }
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            if let Some(cn) = constraint_node {
                let inner = cn.named_children().next();
                let inner_ir = match inner {
                    Some(t) => lower_node(t, source),
                    None => SyntaxTree::Unknown { kind: "constraint(empty)".to_string(), range: range_of(cn), span: span_of(cn) },
                };
                let already_typed = matches!(
                    inner_ir,
                    SyntaxTree::GenericType { .. } | SyntaxTree::SimpleStatement { element_name: "type", .. }
                );
                let typed = if already_typed {
                    inner_ir
                } else {
                    let r = inner_ir.range();
                    let s = inner_ir.span();
                    SyntaxTree::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner_ir],
                        range: r,
                        span: s,
                    }
                };
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "extends",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![typed],
                    range: range_of(cn),
                    span: span_of(cn),
                });
            }
            // `<T = Default>` default-type — emit as `<type[default]>`
            // containing the lowered default type. Principle #14 — every
            // type-reference slot carries a `<type>` child; here the
            // outer `<type[default]>` names the default-type role on
            // the type parameter, and the inner lowered type carries
            // the actual type expression (which renders its own
            // `<type>` wrapper when it's a name leaf).
            if let Some(dn) = default_node {
                let inner = dn.named_children().next();
                let inner_ir = match inner {
                    Some(t) => lower_node(t, source),
                    None => SyntaxTree::Unknown {
                        kind: "default_type(empty)".to_string(),
                        range: range_of(dn),
                        span: span_of(dn),
                    },
                };
                let already_typed = matches!(
                    inner_ir,
                    SyntaxTree::GenericType { .. }
                        | SyntaxTree::SimpleStatement { element_name: "type", .. }
                );
                let typed = if already_typed {
                    inner_ir
                } else {
                    let r = inner_ir.range();
                    let s = inner_ir.span();
                    SyntaxTree::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner_ir],
                        range: r,
                        span: s,
                    }
                };
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "type",
                    modifiers: Modifiers::default(),
                    extra_markers: vec![Marker::implicit("default")],
                    children: vec![typed],
                    range: range_of(dn),
                    span: span_of(dn),
                });
            }
            SyntaxTree::SimpleStatement {
                element_name: "generic",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range,
                span,
            }
        }

        "array_type" => simple_statement_marked(node, "type", vec![Marker::implicit("array")], source),
        "tuple_type" => lower_ts_tuple_type(node, source),
        "union_type" => simple_statement_marked(node, "type", vec![Marker::implicit("union")], source),
        "intersection_type" => simple_statement_marked(node, "type", vec![Marker::implicit("intersection")], source),
        "literal_type" => simple_statement_marked(node, "type", vec![Marker::implicit("literal")], source),
        "function_type" => lower_ts_function_type(node, source),
        "readonly_type" => simple_statement_marked(node, "type", vec![Marker::implicit("readonly")], source),
        "constructor_type" => simple_statement_marked(node, "type", vec![Marker::implicit("constructor")], source),
        "type_query" => simple_statement_marked(node, "type", vec![Marker::implicit("typeof")], source),
        "index_type_query" => simple_statement_marked(node, "type", vec![Marker::implicit("keyof")], source),
        "lookup_type" => simple_statement_marked(node, "type", vec![Marker::implicit("lookup")], source),
        "conditional_type" => lower_ts_conditional_type(node, source),
        "mapped_type_clause" => simple_statement_marked(node, "type", vec![Marker::implicit("mapped")], source),
        "template_literal_type" => simple_statement_marked(node, "type", vec![Marker::implicit("template")], source),
        // `x is number` — a type-predicate wrapper. Inlines its
        // children (`<name>x</name><type><name>number</name></type>`)
        // into the parent so naked `<predicate>` and `asserts_annotation`
        // both end up with flat children. The standalone case (no
        // `asserts` keyword) is wrapped at lowering time at the call
        // site (we wrap with simple_statement so it still emits
        // `<predicate>`).
        "type_predicate" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
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
                                return name_of(c, source);
                            }
                        }
                        // Otherwise treat as type leaf — wrap in <type><name/></type>.
                        return SyntaxTree::SimpleStatement {
                            element_name: "type",
                            modifiers: Modifiers::default(),
                            extra_markers: Vec::new(),
                            children: vec![name_of(c, source)],
                            range: range_of(c),
                            span: span_of(c),
                        };
                    }
                    lower_node(c, source)
                })
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "predicate",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range,
                span,
            }
        }
        "asserts_annotation" | "asserts" => {
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                if c.kind() == "type_predicate" {
                    // Inline type_predicate's children directly.
                    for inner in c.named_children() {
                        if inner.kind() == "type_identifier" || inner.kind() == "predefined_type"
                            || inner.kind() == "identifier"
                        {
                            if let Some(name_n) = c.child_by_field_name("name") {
                                if name_n.id() == inner.id() {
                                    children.push(name_of(inner, source));
                                    continue;
                                }
                            }
                            children.push(SyntaxTree::SimpleStatement {
                                element_name: "type",
                                modifiers: Modifiers::default(),
                                extra_markers: Vec::new(),
                                children: vec![name_of(inner, source)],
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
            SyntaxTree::SimpleStatement {
                element_name: "predicate",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("asserts")],
                children,
                range,
                span,
            }
        }
        "infer_type" => simple_statement_marked(node, "type", vec![Marker::implicit("infer")], source),
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
            for c in node.children() {
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
            let extra_markers: Vec<crate::tree::types::Marker> = match markers.as_slice() {
                [] => Vec::new(),
                ["readonly"] => vec![Marker::implicit("readonly")],
                ["optional"] => vec![Marker::implicit("optional")],
                ["readonly", "optional"] => vec![Marker::implicit("readonly"), Marker::implicit("optional")],
                _ => Vec::new(),
            };
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            if let Some(t) = type_node {
                let inner = t.named_children().next().unwrap_or(t);
                let inner_ir = lower_node(inner, source);
                let already_typed = matches!(
                    &inner_ir,
                    SyntaxTree::GenericType { .. }
                        | SyntaxTree::SimpleStatement { element_name: "type", .. }
                );
                if already_typed {
                    children.push(inner_ir);
                } else {
                    let r = inner_ir.range();
                    let s = inner_ir.span();
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner_ir],
                        range: r,
                        span: s,
                    });
                }
            }
            SyntaxTree::SimpleStatement {
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
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(f) = function_node { children.push(lower_node(f, source)); }
            if let Some(ta) = type_args_node {
                for c in ta.named_children() {
                    let inner_ir = lower_node(c, source);
                    let already_typed = matches!(
                        &inner_ir,
                        SyntaxTree::GenericType { .. }
                            | SyntaxTree::SimpleStatement { element_name: "type", .. }
                    );
                    if already_typed {
                        children.push(inner_ir);
                    } else {
                        let r = inner_ir.range();
                        let s = inner_ir.span();
                        children.push(SyntaxTree::SimpleStatement {
                            element_name: "type",
                            modifiers: Modifiers::default(),
                            extra_markers: Vec::new(),
                            children: vec![inner_ir],
                            range: r,
                            span: s,
                        });
                    }
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("generic")],
                children,
                range,
                span,
            }
        }
        // `<T>` after a function/instantiation — flatten as type-children
        // siblings into the parent.
        "type_arguments" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| {
                    let inner_ir = lower_node(c, source);
                    let already_typed = matches!(
                        &inner_ir,
                        SyntaxTree::GenericType { .. }
                            | SyntaxTree::SimpleStatement { element_name: "type", .. }
                    );
                    if already_typed {
                        inner_ir
                    } else {
                        let r = inner_ir.range();
                        let s = inner_ir.span();
                        SyntaxTree::SimpleStatement {
                            element_name: "type",
                            modifiers: Modifiers::default(),
                            extra_markers: Vec::new(),
                            children: vec![inner_ir],
                            range: r,
                            span: s,
                        }
                    }
                })
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        // `label: stmt` — labeled statement.
        "labeled_statement" => simple_statement(node, "label", source),
        // `module M { ... }` (TS namespace module body).
        "internal_module" => simple_statement(node, "namespace", source),
        // `import M = require(...)`.
        "import_require_clause" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        // `class { static { ... } }`.
        "class_static_block" => simple_statement_marked(node, "block", vec![Marker::implicit("static")], source),
        // `[a, b = 1]` / `{ a = 1 }` — assignment-pattern with default
        // value. Inline the children into the parent (the parent
        // pair_pattern / array_pattern provides the role-element);
        // wrap the right side in `<value><expression>...` so XPath
        // can address the default consistently.
        "assignment_pattern" => {
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(l) = left {
                children.push(lower_node(l, source));
            }
            if let Some(r) = right {
                let inner = lower_node(r, source);
                let r_range = range_of(r);
                let r_span = span_of(r);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "value",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: r_range,
                        span: r_span,
                    }],
                    range: r_range,
                    span: r_span,
                });
            }
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        // `{ a: aa = 1 }` in a pattern — `object_assignment_pattern` has a
        // pair-with-default. Inline to flatten.
        "object_assignment_pattern_unused" => simple_statement(node, "pair", source),
        // Regex literal.
        "regex" => string_of(node, source),
        // Label name in `outer: for (...) { break outer; }`.
        "statement_identifier" => name_of(node, source),
        // `yield x` / `yield* x` — emits `<yield>x</yield>`.
        "yield_expression" => simple_statement(node, "yield", source),
        // `T?` short-form optional — emit as `<type>` with `optional` marker
        // wrapping the inner type.
        "opting_type_annotation" => simple_statement_marked(node, "type", vec![Marker::implicit("optional")], source),
        // `{ x = 1 }` in destructure — assignment pattern; emit as `<pair>` with the inner.
        "object_assignment_pattern" => simple_statement(node, "pair", source),
        // `x!` — non-null assertion, render as `<nonnull>x</nonnull>`.
        "non_null_expression" => {
            let inner = node.named_children().next();
            let children: Vec<SyntaxTree> = match inner {
                Some(i) => vec![lower_node(i, source)],
                None => Vec::new(),
            };
            SyntaxTree::SimpleStatement {
                element_name: "nonnull",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range,
                span,
            }
        }
        // `this` as a type expression. Render as `<type><name>this</name></type>` shape.
        "this_type" => name_of(node, source),
        // `...string[]` rest type — lower as `<type><rest/><type[array]>...` shape.
        "rest_type" => simple_statement_marked(node, "type", vec![Marker::implicit("rest")], source),
        // Template literal type segments — lower transparent.
        "template_type" => name_of(node, source),
        "string_fragment" => string_of(node, source),
        // `enum E { A = "a" }` — `enum_assignment` is `name = value`.
        "enum_assignment" => simple_statement(node, "constant", source),
        // `(x: T)` — formal_parameters used in function-type RHS. Inline.
        "formal_parameters" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        // `declare` ambient declarations.
        "ambient_declaration" => simple_statement(node, "declare", source),
        // Switch body wraps the cases — lower transparent.
        "switch_body" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }

        "type_alias_declaration" => {
            let name_node = node.child_by_field_name("name");
            let value_node = node.child_by_field_name("value");
            let type_params_node = node.child_by_field_name("type_parameters");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            if let Some(tp) = type_params_node {
                for c in tp.named_children() {
                    children.push(lower_node(c, source));
                }
            }
            if let Some(v) = value_node {
                let inner = lower_node(v, source);
                let already_typed = matches!(
                    &inner,
                    SyntaxTree::GenericType { .. }
                        | SyntaxTree::SimpleStatement { element_name: "type", .. }
                        | SyntaxTree::SimpleStatement { element_name: "predicate", .. }
                );
                if already_typed {
                    children.push(inner);
                } else {
                    let r = inner.range();
                    let s = inner.span();
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: r,
                        span: s,
                    });
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "alias",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range,
                span,
            }
        }
        "enum_declaration" => simple_statement(node, "enum", source),

        // Comments.
        "comment" => SyntaxTree::Comment {
            leading: Flag::Off,
            trailing: Flag::Off,
            range,
            span,
        },

        // Parenthesized.
        "parenthesized_expression" => {
            let inner = node.named_children().next();
            match inner {
                Some(n) => lower_node(n, source),
                None => SyntaxTree::Unknown { kind: "paren(empty)".to_string(), range, span },
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
                (Some(c), Some(t), Some(f)) => SyntaxTree::Ternary {
                    condition: Box::new(c.wrap_expression()),
                    if_true: Box::new(t.wrap_expression()),
                    if_false: Box::new(f.wrap_expression()),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "ternary(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Object / array literal.
        "object" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "object",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("literal")],
                children,
                range,
                span,
            }
        }
        "array" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::List { children, range, span }
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

        "spread_element" => SyntaxTree::ListSplat {
            inner: {
                let inner = node.named_children().next();
                Box::new(match inner {
                    Some(i) => lower_node(i, source),
                    None => SyntaxTree::Unknown { kind: "spread(empty)".to_string(), range, span },
                })
            },
            range,
            span,
        },

        // Array/object destructuring patterns. Lower to `<pattern>`
        // with an `<array/>`/`<object/>` shape marker. Children are
        // the inner names / pair patterns.
        "array_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("array")], source),
        "object_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("object")], source),
        // `shorthand_property_identifier_pattern` is handled earlier
        // in this match (atom arm) — SyntaxTree::Name. Comment kept for
        // navigation.
        // `{ x: a }` in an object pattern — `<pair>`.
        "pair_pattern" => simple_statement(node, "pair", source),

        // `...rest` pattern in a parameter list. Tree-sitter wraps the
        // identifier in a `rest_pattern`. We lower to `<rest><name>rest</name></rest>`
        // so the enclosing `<parameter>` element ends up with the shape
        // `<parameter><required/><rest/><rest><name>rest</name></rest></parameter>`
        // (the outer parameter is responsible for the `<rest/>` marker).
        "rest_pattern" => {
            let inner = node.named_children().next();
            let children: Vec<SyntaxTree> = match inner {
                Some(i) => vec![lower_node(i, source)],
                None => Vec::new(),
            };
            SyntaxTree::SimpleStatement {
                element_name: "rest",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
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
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }

        // Fallback ------------------------------------------------------
        other => SyntaxTree::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

fn lower_ts_else_chain(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    // tree-sitter-typescript's `if_statement.alternative` is an
    // `else_clause` wrapping a body or another if.
    if node.kind() == "else_clause" {
        let inner = node.named_children().next();
        match inner {
            Some(i) if i.kind() == "if_statement" => {
                let cond = i.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source).wrap_expression()));
                let body = i.child_by_field_name("consequence").map(|n| Box::new(lower_block_like(n, source)));
                let alt = i.child_by_field_name("alternative");
                let else_branch = alt.map(|a| Box::new(lower_ts_else_chain(a, source)));
                match (cond, body) {
                    (Some(c), Some(b)) => SyntaxTree::ElseIf {
                        condition: c,
                        body: b,
                        else_branch,
                        range,
                        span,
                    },
                    _ => SyntaxTree::Unknown { kind: "ts_else_if(missing)".to_string(), range, span },
                }
            }
            Some(b) => SyntaxTree::Else {
                body: Box::new(lower_block_like(b, source)),
                range,
                span,
            },
            None => SyntaxTree::Unknown { kind: "else_clause(empty)".to_string(), range, span },
        }
    } else if node.kind() == "if_statement" {
        let cond = node.child_by_field_name("condition").map(|n| Box::new(lower_node(n, source).wrap_expression()));
        let body = node.child_by_field_name("consequence").map(|n| Box::new(lower_block_like(n, source)));
        let alt = node.child_by_field_name("alternative");
        let else_branch = alt.map(|a| Box::new(lower_ts_else_chain(a, source)));
        match (cond, body) {
            (Some(c), Some(b)) => SyntaxTree::ElseIf { condition: c, body: b, else_branch, range, span },
            _ => SyntaxTree::Unknown { kind: "ts_else_if(missing)".to_string(), range, span },
        }
    } else {
        SyntaxTree::Else {
            body: Box::new(lower_block_like(node, source)),
            range,
            span,
        }
    }
}

fn lower_block_like(node: &RawNode, source: &str) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| lower_node(c, source))
        .collect();
    let children = merge_ts_line_comments(children, source);
    let block_wrap = node.kind() == "statement_block";
    SyntaxTree::Body {
        children,
        pass_only: false,
        block_wrap,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Structured parts of a TS `variable_declarator`. Lets the
/// variable-declaration lowering build a typed `SyntaxTree::Variable`
/// directly (with `type_ann` / `name` / `value` as explicit fields)
/// instead of stuffing everything under a SimpleStatement's flat
/// `children` Vec.
struct TsDeclaratorParts {
    type_ann: Option<Box<SyntaxTree>>,
    name: Option<Box<SyntaxTree>>,
    value: Option<crate::tree::Expression>,
}

/// Lower a TS `variable_declarator` into structured parts.
fn lower_ts_declarator_parts(d: &RawNode, source: &str) -> TsDeclaratorParts {
    let type_ann = d.child_by_field_name("type").map(|t| {
        let inner = t.named_children().next().unwrap_or(t);
        let inner_ir = lower_node(inner, source);
        if matches!(
            inner_ir,
            SyntaxTree::GenericType { .. }
                | SyntaxTree::SimpleStatement { element_name: "type", .. }
        ) {
            Box::new(inner_ir)
        } else {
            Box::new(SyntaxTree::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner_ir],
                range: range_of(t),
                span: span_of(t),
            })
        }
    });
    let name = d.child_by_field_name("name").map(|n| Box::new(lower_node(n, source)));
    let value = d
        .child_by_field_name("value")
        .map(|v| crate::tree::Expression::wrap(lower_node(v, source)));
    TsDeclaratorParts { type_ann, name, value }
}

/// Flatten declarator parts into a `Vec<SyntaxTree>` for the
/// multi-declarator case (where each declarator becomes a
/// `<declarator>` SimpleStatement). Uses the legacy
/// `<type>`/`<name>`/`<value>` SimpleStatement wrappers because
/// there's no typed multi-declarator variant yet.
fn ts_declarator_parts_flat(parts: TsDeclaratorParts) -> Vec<SyntaxTree> {
    let mut out: Vec<SyntaxTree> = Vec::new();
    if let Some(t) = parts.type_ann {
        out.push(*t);
    }
    if let Some(n) = parts.name {
        out.push(*n);
    }
    if let Some(v) = parts.value {
        let range = v.inner.range();
        let span = v.inner.span();
        out.push(SyntaxTree::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![*v.inner],
            range,
            span,
        });
    }
    out
}

fn lower_children(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    node.named_children()
        .map(|c| lower_node(c, source))
        .collect()
}

fn extract_ts_decorators(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    node.named_children()
        .filter(|c| c.kind() == "decorator")
        .map(|c| lower_node(c, source))
        .collect()
}

fn lower_ts_modifiers(
    node: &RawNode,
    source: &str,
    default_access: Option<Access>,
) -> Modifiers {
    let mut m = Modifiers::default();
    if let Some(da) = default_access {
        m.access = Some(da);
    }
    use crate::tree::types::Flag;
    for c in node.children() {
        let kind = c.kind();
        let flag = Flag::anchored(range_of(c), span_of(c));
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
            "static" => m.static_ = flag,
            "abstract" => m.abstract_ = flag,
            "readonly" => m.readonly = flag,
            "async" => m.async_ = flag,
            "override" | "override_modifier" => m.override_ = flag,
            _ => {
                // Token-level keyword detection (some modifiers are unnamed children).
                if !c.is_named() {
                    let txt = text_of(c, source);
                    match txt.as_str() {
                        "static" => m.static_ = flag,
                        "abstract" => m.abstract_ = flag,
                        "readonly" => m.readonly = flag,
                        "async" => m.async_ = flag,
                        "override" => m.override_ = flag,
                        "public" => m.access = Some(Access::Public),
                        "private" => m.access = Some(Access::Private),
                        "protected" => m.access = Some(Access::Protected),
                        "get" => m.getter = flag,
                        "set" => m.setter = flag,
                        "*" => m.generator = flag,
                        _ => {}
                    }
                }
            }
        }
    }
    m
}

fn merge_ts_line_comments(children: Vec<SyntaxTree>, source: &str) -> Vec<SyntaxTree> {
    let mut out: Vec<SyntaxTree> = Vec::with_capacity(children.len());
    for child in children {
        if let SyntaxTree::Comment { leading, trailing, range, span } = child {
            let prev_non_comment = out.iter().rev().find(|c| !matches!(c, SyntaxTree::Comment { .. }));
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
                    .trim_start().starts_with("//");
                let curr_is_line_comment = source[range.start as usize..range.end as usize]
                    .trim_start().starts_with("//");
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: Flag::On { .. }, .. }));
                if only_one_newline && prev_is_line_comment && curr_is_line_comment
                    && !prev_was_trailing && !curr_is_trailing {
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
    let n = out.len();
    for i in 0..n {
        if let SyntaxTree::Comment { trailing, range, span, .. } = &out[i] {
            if trailing.is_set() { continue; }
            let comment_end = range.end as usize;
            let comment_range = *range;
            let comment_span = *span;
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

/// Lower `throw expr` so the thrown expression sits under an
/// `<expression>` host (Principle #5 — matches Python yield/raise,
/// Java/C# throw, and the existing `<return>` shape across the
/// codebase).
fn lower_typescript_throw(node: &RawNode, source: &str) -> SyntaxTree {
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

/// Lower a TS `tuple_type` (`[A, B, C]`) so each element is wrapped
/// in its own `<type>` element. Principle #14 — every type-reference
/// slot carries a `<type>` child; tuple elements are type-references
/// regardless of whether they're leaf names or compound expressions.
fn lower_ts_tuple_type(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| ts_wrap_in_type(lower_node(c, source)))
        .collect();
    SyntaxTree::SimpleStatement {
        element_name: "type",
        modifiers: Modifiers::default(),
        extra_markers: vec![Marker::implicit("tuple")],
        children,
        range,
        span,
    }
}

/// Lower a TS `function_type` (`(x: T) => U`) so the return-type slot
/// carries a `<returns>/<type>` wrapper, matching the `<returns>` slot
/// on regular function declarations. Without the wrapper the return
/// type is an anonymous trailing child indistinguishable from a
/// parameter or another structural child (Principle #19 — role-mixed
/// children wrap in role-named slots).
fn lower_ts_function_type(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let return_node = node.child_by_field_name("return_type");
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if let Some(rt) = return_node {
            if c.id() == rt.id() {
                let inner = lower_node(c, source);
                let typed = ts_wrap_in_type(inner);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "returns",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![typed],
                    range: range_of(c),
                    span: span_of(c),
                });
                continue;
            }
        }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name: "type",
        modifiers: Modifiers::default(),
        extra_markers: vec![Marker::implicit("function")],
        children,
        range,
        span,
    }
}

/// Wrap a lowered TypeScript type-expression in a `<type>` element if
/// it isn't already type-shaped. Keeps the Principle #14 invariant
/// that every type-reference slot carries a `<type>` child.
fn ts_wrap_in_type(inner: SyntaxTree) -> SyntaxTree {
    match &inner {
        SyntaxTree::GenericType { .. }
        | SyntaxTree::SimpleStatement { element_name: "type", .. } => inner,
        _ => {
            let r = inner.range();
            let s = inner.span();
            SyntaxTree::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner],
                range: r,
                span: s,
            }
        }
    }
}

/// Lower a TypeScript `conditional_type` (`T extends X ? Y : Z`) into
/// `<type[conditional]>` with four role-named slot children:
///
///   - `<left>` — the type being tested (`T`)
///   - `<right>` — the type being matched against (`X`)
///   - `<then>` — the result when the test holds (`Y`)
///   - `<else>` — the result when the test fails (`Z`)
///
/// Each slot wraps a `<type>` element (Principle #14). Without these
/// role-named slots the four children are bare same-shape siblings
/// (Principle #19 violation: positional disambiguation only) and
/// queries like `//type[conditional]/then` no longer locate the
/// true-branch.
fn lower_ts_conditional_type(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let left_node = node.child_by_field_name("left");
    let right_node = node.child_by_field_name("right");
    let consequence_node = node.child_by_field_name("consequence");
    let alternative_node = node.child_by_field_name("alternative");

    let mut children: Vec<SyntaxTree> = Vec::new();
    for (slot, src_node) in [
        ("left", left_node),
        ("right", right_node),
        ("then", consequence_node),
        ("else", alternative_node),
    ] {
        if let Some(n) = src_node {
            let inner = lower_node(n, source);
            let type_slot = ts_wrap_in_type(inner);
            children.push(SyntaxTree::SimpleStatement {
                element_name: slot,
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![type_slot],
                range: range_of(n),
                span: span_of(n),
            });
        }
    }

    SyntaxTree::SimpleStatement {
        element_name: "type",
        modifiers: Modifiers::default(),
        extra_markers: vec![Marker::implicit("conditional")],
        children,
        range,
        span,
    }
}

/// Lower a TypeScript `import_statement` with an explicit kind marker
/// on `<import>` (Principle #9 — Exhaustive Markers for Mutually
/// Exclusive Variations). The marker names a closed-set classification
/// of the import shape so consumer queries can scope precisely:
///
///   - `import "x"`                 → `<import[sideeffect]>`
///   - `import { a, b } from "x"`   → `<import[group]>`
///   - `import * as ns from "x"`    → `<import[namespace]>`
///   - `import x from "x"`          → `<import[default]>`
///
/// Mixed forms (`import x, { a, b } from "x"`) carry multiple markers
/// in source order. The inner shape (specifiers, namespace, path)
/// flows through as in the bare `simple_statement` path.
fn lower_ts_import_statement(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| {
            // Detect the import path string and lower it to
            // `<path>./x</path>` (Atom leaf, quotes stripped) instead of
            // `<string>"./x"</string>` — the path is a module reference,
            // not a value-namespace string (Z1 + Principle #14).
            if c.kind() == "string" {
                let raw_range = range_of(&c);
                let raw = text_of_str(&c, source);
                let stripped = strip_string_quotes(raw).to_string();
                // Shrink the range by the leading/trailing quote so the
                // anchored-source slicer emits the stripped text, not the
                // quoted form.
                let bytes = raw.as_bytes();
                let has_quotes = bytes.len() >= 2
                    && matches!(bytes[0], b'"' | b'\'' | b'`')
                    && bytes[0] == bytes[bytes.len() - 1];
                let inner_range = if has_quotes {
                    ByteRange { start: raw_range.start + 1, end: raw_range.end - 1, anchored: true }
                } else {
                    raw_range
                };
                SyntaxTree::Atom {
                    element_name: "path",
                    text: stripped,
                    range: inner_range,
                    span: span_of(&c),
                }
            } else {
                lower_node(c, source)
            }
        })
        .collect();
    let kind = classify_ts_import(node);
    SyntaxTree::SimpleStatement {
        element_name: "import",
        modifiers: Modifiers::default(),
        extra_markers: kind,
        children,
        range,
        span,
    }
}

fn text_of_str<'a>(node: &RawNode, source: &'a str) -> &'a str {
    let r = range_of(node);
    &source[r.start as usize..r.end as usize]
}

fn strip_string_quotes(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    let n = bytes.len();
    if n >= 2 {
        let first = bytes[0];
        let last = bytes[n - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') || (first == b'`' && last == b'`') {
            return &raw[1..n - 1];
        }
    }
    raw
}

fn classify_ts_import(node: &RawNode) -> Vec<crate::tree::types::Marker> {
    let import_clause = node
        .named_children()
        .find(|c| c.kind() == "import_clause");
    let Some(clause) = import_clause else {
        // No clause → bare `import "x"` side-effect-only import.
        return vec![Marker::implicit("sideeffect")];
    };
    let mut has_default = false;
    let mut has_namespace = false;
    let mut has_group = false;
    for c in clause.named_children() {
        match c.kind() {
            "identifier" => has_default = true,
            "namespace_import" => has_namespace = true,
            "named_imports" => has_group = true,
            _ => {}
        }
    }
    match (has_default, has_namespace, has_group) {
        (false, false, false) => vec![Marker::implicit("sideeffect")],
        (true, false, false) => vec![Marker::implicit("default")],
        (false, true, false) => vec![Marker::implicit("namespace")],
        (false, false, true) => vec![Marker::implicit("group")],
        (true, false, true) => vec![Marker::implicit("default"), Marker::implicit("group")],
        (true, true, false) => vec![Marker::implicit("default"), Marker::implicit("namespace")],
        // Mixed/other combinations: keep both flags.
        _ => vec![Marker::implicit("default"), Marker::implicit("group"), Marker::implicit("namespace")],
    }
}

fn simple_statement(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| lower_node(c, source))
        .collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range,
        span,
    }
}

fn simple_statement_marked(
    node: &RawNode,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let children: Vec<SyntaxTree> = node
        .named_children()
        .map(|c| lower_node(c, source))
        .collect();
    SyntaxTree::SimpleStatement {
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

/// TS binary / logical operators → typed [`OperatorKind`]. Unary-only
/// (`!`, `~`, `++`, `--`, `typeof`, `void`, `delete`) handled by the
/// Unary lowering path; not included here. Comparison ops still flow
/// through Binary lowering until `Comparison` is restructured.
fn op_kind(op: &str) -> Option<OperatorKind> {
    Some(match op {
        "+" => OperatorKind::Plus,
        "-" => OperatorKind::Minus,
        "*" => OperatorKind::Multiply,
        "/" => OperatorKind::Divide,
        "%" => OperatorKind::Modulo,
        "**" => OperatorKind::Power,
        "==" | "===" => OperatorKind::Equal,
        "!=" | "!==" => OperatorKind::NotEqual,
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
        "instanceof" => OperatorKind::Instanceof,
        "in" => OperatorKind::In,
        _ => return None,
    })
}



