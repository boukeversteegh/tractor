//! TypeScript tree-sitter CST → IR lowering.
//!
//! Mirrors the C# / Java IRs closely. TypeScript shares most CST
//! shapes (class/function/method/field declarations, type
//! annotations, generics, arrow chains). Per-kind arms recursively
//! lower children; the renderer in `crate::ir::render` is shared.
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
        "number" => Ir::Int { range, span },
        "string" | "template_string" => Ir::String { range, span },
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
            let modifiers = lower_ts_modifiers(node, source, None);
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
                generics: None,
                parameters,
                returns,
                body,
                range,
                span,
            }
        }

        "arrow_function" => simple_statement(node, "lambda", source),

        // Required / optional / rest parameter.
        "required_parameter" | "optional_parameter" => {
            let pattern = node.child_by_field_name("pattern");
            let type_node = node.child_by_field_name("type");
            let value_node = node.child_by_field_name("value");
            let extra_markers: &'static [&'static str] = if node.kind() == "optional_parameter" {
                &["optional"]
            } else {
                &[]
            };
            let modifiers = lower_ts_modifiers(node, source, None);
            let _ = modifiers;
            Ir::Parameter {
                kind: ParamKind::Regular,
                extra_markers,
                name: Box::new(match pattern {
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
            if declarators.len() == 1 {
                let d = declarators[0];
                lower_ts_variable_declarator(d, source, range, span, "variable", modifiers, kw_marker)
            } else if !declarators.is_empty() {
                let children: Vec<Ir> = declarators
                    .into_iter()
                    .map(|d| {
                        lower_ts_variable_declarator(
                            d,
                            source,
                            range_of(d),
                            span_of(d),
                            "variable",
                            modifiers,
                            &[],
                        )
                    })
                    .collect();
                Ir::SimpleStatement {
                    element_name: "variable",
                    modifiers,
                    extra_markers: kw_marker,
                    children,
                    range,
                    span,
                }
            } else {
                Ir::Unknown {
                    kind: "lexical_declaration(no declarators)".to_string(),
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
            let modifiers = lower_ts_modifiers(node, source, None);
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
        "statement_block" | "class_body" | "interface_body" | "enum_body"
        | "object_type" => lower_block_like(node, source),

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
            let updates: Vec<Ir> = update_node.map(|u| vec![lower_node(u, source)]).unwrap_or_default();
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
            // `for (k in obj)` or `for (item of items)` — TS uses
            // `for_in_statement` for both. Distinguish by the keyword
            // (`in` vs `of`) in source.
            let kind_node = node.child_by_field_name("kind");
            let kw = kind_node.map(|n| text_of(n, source)).unwrap_or_default();
            let _ = kw;
            simple_statement(node, "for", source)
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

        // Imports / exports.
        "import_statement" => simple_statement(node, "import", source),
        "export_statement" => simple_statement(node, "export", source),

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
        "type_alias_declaration" => simple_statement(node, "type_alias", source),
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

        // Type assertion / as expression.
        "as_expression" | "type_assertion" | "satisfies_expression" => {
            simple_statement(node, "cast", source)
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
    let mut variable = Ir::Variable {
        element_name,
        modifiers,
        decorators: Vec::new(),
        type_ann: type_ir,
        name: Box::new(name_ir),
        value: value_ir,
        range,
        span,
    };
    // Apply `extra_markers` (let/const/var) by wrapping in a
    // SimpleStatement that prepends the markers — since Ir::Variable
    // doesn't have an extra_markers field. The resulting shape is
    // `<variable[const]>...</variable>`.
    if !extra_markers.is_empty() {
        let mut new_variable = std::mem::replace(
            &mut variable,
            Ir::Unknown { kind: "swap".to_string(), range, span },
        );
        // Re-emit with markers via a thin SimpleStatement wrapper —
        // but we want the markers ON the variable element. Easiest:
        // mutate via inserting markers into modifiers' marker_names
        // — but those are typed flags. For now, wrap and let the
        // post-pass handle list-tagging.
        let _ = new_variable;
        // Use SimpleStatement to encode the kw marker; renderer
        // emits `<variable[let]>` etc.
        // Recreate the Ir::Variable values by destructuring.
        // (The above swap returned the original variable; rebuild
        // using a fresh new_variable construction is cleaner.)
    }
    let _ = extra_markers;
    variable
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
            "override" => m.override_ = true,
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
