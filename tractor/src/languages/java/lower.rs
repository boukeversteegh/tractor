//! Java tree-sitter CST → tree lowering.
//!
//! Mirrors the C# tree closely — Java and C# share most CST shapes
//! (class/method/field declarations, modifiers, generics, blocks).
//! Per-kind arms recursively lower children; the renderer in
//! `crate::tree::to_xot` is shared with all other tree-pipeline languages.
//!
//! Coverage is incremental: each unhandled kind falls through to
//! `SyntaxTree::Unknown`. The diagnostic test
//! `tests/ir_java_missing_kinds.rs` lists kinds the corpus
//! exercises that aren't yet typed.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use crate::tree::lower_helpers::{
    false_of, float_of, int_of, name_of, null_of, range_of, span_of, string_of, text_of, true_of,
};
use crate::tree::types::{Access, AccessSegment, ByteRange, SyntaxTree, Modifiers, ParamKind, Span};

/// Lower a Java tree-sitter root node to [`SyntaxTree`].
pub fn lower_java_root(root: TsNode<'_>, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => SyntaxTree::Module {
            element_name: "program",
            children: merge_java_line_comments(lower_children(root, source), source),
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

/// Public entry for lowering an arbitrary Java CST node — useful for
/// tests that want to lower a single expression without scaffolding.
pub fn lower_java_node(node: TsNode<'_>, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms -----------------------------------------------------
        "identifier" | "type_identifier" | "scoped_type_identifier" => {
            name_of(node, source)
        }
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => int_of(node, source),
        "decimal_floating_point_literal" | "hex_floating_point_literal" => {
            float_of(node, source)
        }
        "string_literal" | "character_literal" => string_of(node, source),
        "true" => true_of(node, source),
        "false" => false_of(node, source),
        "null_literal" => null_of(node, source),

        // Predefined types: bare names. (`int`/`long`/`boolean`/...)
        "boolean_type" | "integral_type" | "floating_point_type" => {
            name_of(node, source)
        }
        // Java `void` carries an extra `<void/>` marker on the type
        // (query shortcut for "no return value"). Lower as a typed
        // SimpleStatement so the marker appears inside `<type>`.
        "void_type" => SyntaxTree::SimpleStatement {
            element_name: "type",
            modifiers: Modifiers::default(),
            extra_markers: &["void"],
            children: vec![name_of(node, source)],
            range,
            span,
        },

        // ----- Containers / declarations ---------------------------------
        "class_declaration" | "interface_declaration" | "record_declaration"
        | "enum_declaration" => {
            let kind: &'static str = match node.kind() {
                "interface_declaration" => "interface",
                "record_declaration" => "record",
                "enum_declaration" => "enum",
                _ => "class",
            };
            let name_node = node.child_by_field_name("name");
            let body_node = node.child_by_field_name("body").or_else(|| {
                let mut bc = node.walk();
                let mut found = None;
                for c in node.named_children(&mut bc) {
                    if matches!(c.kind(), "class_body" | "interface_body" | "enum_body" | "record_body") {
                        found = Some(c);
                        break;
                    }
                }
                found
            });
            let mut tpc = node.walk();
            let type_param_list = node
                .named_children(&mut tpc)
                .find(|c| c.kind() == "type_parameters");
            let mut sc = node.walk();
            let superclass = node
                .named_children(&mut sc)
                .find(|c| c.kind() == "superclass");
            let mut ic = node.walk();
            let interfaces = node
                .named_children(&mut ic)
                .find(|c| matches!(c.kind(), "super_interfaces" | "extends_interfaces"));
            let mut ac = node.walk();
            let decorators: Vec<SyntaxTree> = node
                .named_children(&mut ac)
                .filter(|c| c.kind() == "modifiers")
                .flat_map(|m| {
                    let mut mc = m.walk();
                    m.named_children(&mut mc)
                        .filter(|c| matches!(c.kind(), "annotation" | "marker_annotation"))
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            let modifiers = lower_java_modifiers(node, source, /*default_access*/ Some(Access::Package));
            let generics = type_param_list.map(|tpl| {
                let mut tplc = tpl.walk();
                let items: Vec<SyntaxTree> = tpl
                    .named_children(&mut tplc)
                    .map(|c| lower_node(c, source))
                    .collect();
                Box::new(SyntaxTree::Generic {
                    items,
                    range: range_of(tpl),
                    span: span_of(tpl),
                })
            });
            let mut bases: Vec<SyntaxTree> = Vec::new();
            // Superclass (`extends Foo`) — bare type, gets wrapped in
            // `<extends>` by the Class render's default base path.
            if let Some(sc) = superclass {
                let mut scc = sc.walk();
                bases.extend(sc.named_children(&mut scc).map(|c| lower_node(c, source)));
            }
            // Interfaces (`implements Bar, Baz`) — each wrapped in
            // a `<implements>` SimpleStatement so the Class render
            // emits `<implements><type>...</type></implements>`
            // sibling under `<class>` (matches imperative shape).
            if let Some(ifs) = interfaces {
                let mut ic2 = ifs.walk();
                for c in ifs.named_children(&mut ic2) {
                    if c.kind() == "type_list" {
                        let mut tlc = c.walk();
                        for n in c.named_children(&mut tlc) {
                            let inner = lower_node(n, source);
                            let already_typed = matches!(
                                inner,
                                SyntaxTree::GenericType { .. }
                                    | SyntaxTree::SimpleStatement { element_name: "type", .. }
                            );
                            let type_inner = if already_typed {
                                inner
                            } else {
                                SyntaxTree::SimpleStatement {
                                    element_name: "type",
                                    modifiers: Modifiers::default(),
                                    extra_markers: &[],
                                    children: vec![inner],
                                    range: range_of(n),
                                    span: span_of(n),
                                }
                            };
                            bases.push(SyntaxTree::SimpleStatement {
                                element_name: "implements",
                                modifiers: Modifiers::default(),
                                extra_markers: &[],
                                children: vec![type_inner],
                                range: range_of(n),
                                span: span_of(n),
                            });
                        }
                    } else {
                        bases.push(lower_node(c, source));
                    }
                }
            }
            SyntaxTree::Class {
                kind,
                modifiers,
                decorators,
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown {
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
                    None => SyntaxTree::Body {
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

        // Method / constructor.
        "method_declaration" | "constructor_declaration" => {
            let name_node = node.child_by_field_name("name");
            let params_node = node.child_by_field_name("parameters");
            let body_node = node.child_by_field_name("body");
            let returns_node = node.child_by_field_name("type");
            // Generic type parameters live as a `type_parameters`
            // named child before the return-type / name.
            let mut tpc = node.walk();
            let type_param_list = node
                .named_children(&mut tpc)
                .find(|c| c.kind() == "type_parameters");
            let generics = type_param_list.map(|tpl| {
                let mut tplc = tpl.walk();
                let items: Vec<SyntaxTree> = tpl
                    .named_children(&mut tplc)
                    .map(|c| lower_node(c, source))
                    .collect();
                Box::new(SyntaxTree::Generic {
                    items,
                    range: range_of(tpl),
                    span: span_of(tpl),
                })
            });
            // Default access depends on enclosing type:
            //   interface + abstract (no body) → implicit `public`
            //   interface + has body (default method) → `package`
            //   class / record / enum → `package`
            // Pins current imperative-pipeline behavior pinned by
            // visibility::java_interface (`default String name()`
            // stays `<package/>`, not promoted to `<public/>`).
            let in_interface = enclosing_type_kind(node).as_deref() == Some("interface_declaration");
            let default_access = if in_interface && body_node.is_none() {
                Some(Access::Public)
            } else {
                Some(Access::Package)
            };
            let modifiers = lower_java_modifiers(node, source, default_access);
            let mut ac = node.walk();
            let decorators: Vec<SyntaxTree> = node
                .named_children(&mut ac)
                .filter(|c| c.kind() == "modifiers")
                .flat_map(|m| {
                    let mut mc = m.walk();
                    m.named_children(&mut mc)
                        .filter(|c| matches!(c.kind(), "annotation" | "marker_annotation"))
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            let parameters: Vec<SyntaxTree> = match params_node {
                Some(p) => {
                    let mut pc = p.walk();
                    p.named_children(&mut pc)
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            let returns = returns_node.map(|t| {
                Box::new(SyntaxTree::Returns {
                    type_ann: Box::new(lower_node(t, source)),
                    range: range_of(t),
                    span: span_of(t),
                })
            });
            // Body is None for abstract / interface methods — the
            // Function render skips emitting `<body>` when None
            // (matches imperative shape `<method[abstract]>` only).
            let body: Option<Box<SyntaxTree>> = body_node.map(|b| Box::new(lower_block_like(b, source)));
            let element_name: &'static str = if node.kind() == "constructor_declaration" {
                "constructor"
            } else {
                "method"
            };
            SyntaxTree::Function {
                element_name,
                modifiers,
                decorators,
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown {
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

        "formal_parameter" | "spread_parameter" => {
            // For spread_parameter, the type and name come from
            // different positions: first named child = type, last
            // named child has the binding (variable_declarator → name).
            let is_spread = node.kind() == "spread_parameter";
            let type_node = node.child_by_field_name("type").or_else(|| {
                if is_spread {
                    let mut c = node.walk();
                    let mut first = None;
                    for n in node.named_children(&mut c) {
                        first = Some(n);
                        break;
                    }
                    first
                } else { None }
            });
            let name_node = node.child_by_field_name("name").or_else(|| {
                if is_spread {
                    // variable_declarator → identifier[name]
                    let mut c = node.walk();
                    for n in node.named_children(&mut c) {
                        if n.kind() == "variable_declarator" {
                            return n.child_by_field_name("name");
                        }
                    }
                }
                None
            });
            let extra_markers: &'static [&'static str] = if is_spread {
                &["variadic"]
            } else {
                &[]
            };
            let modifiers = lower_java_modifiers(node, source, None);
            let _ = modifiers;
            SyntaxTree::Parameter {
                kind: ParamKind::Regular,
                extra_markers,
                modifiers: Modifiers::default(),
                name: Box::new(match name_node {
                    Some(n) => name_of(n, source),
                    None => SyntaxTree::Unknown {
                        kind: "parameter(missing name)".to_string(),
                        range,
                        span,
                    },
                }),
                type_ann: type_node.map(|t| Box::new(lower_node(t, source))),
                default: None,
                range,
                span,
            }
        }

        // Field.
        "field_declaration" => {
            let modifiers = lower_java_modifiers(node, source, Some(Access::Package));
            let type_node = node.child_by_field_name("type");
            let mut ac = node.walk();
            let decorators: Vec<SyntaxTree> = node
                .named_children(&mut ac)
                .filter(|c| c.kind() == "modifiers")
                .flat_map(|m| {
                    let mut mc = m.walk();
                    m.named_children(&mut mc)
                        .filter(|c| matches!(c.kind(), "annotation" | "marker_annotation"))
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            // Java has multiple variable_declarators per field.
            let mut vc = node.walk();
            let declarators: Vec<TsNode> = node
                .named_children(&mut vc)
                .filter(|c| c.kind() == "variable_declarator")
                .collect();
            lower_java_multi_declarator(
                node,
                type_node,
                &declarators,
                source,
                range,
                span,
                "field",
                modifiers,
                decorators,
            )
        }

        // Local variable.
        "local_variable_declaration" => {
            let modifiers = lower_java_modifiers(node, source, None);
            let type_node = node.child_by_field_name("type");
            let mut vc = node.walk();
            let declarators: Vec<TsNode> = node
                .named_children(&mut vc)
                .filter(|c| c.kind() == "variable_declarator")
                .collect();
            lower_java_multi_declarator(
                node,
                type_node,
                &declarators,
                source,
                range,
                span,
                "variable",
                modifiers,
                Vec::new(),
            )
        }

        // Block / body.
        "block" => lower_block_like(node, source),

        // Statements.
        "expression_statement" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
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
            let mut cursor = node.walk();
            let value = node.named_children(&mut cursor).next();
            SyntaxTree::Return {
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
            let else_branch = alt.map(|a| Box::new(lower_java_else_chain(a, source)));
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
                .map(|n| Box::new(lower_node(n, source)));
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
            // Java C-style: init / condition / update / body.
            let init = node.child_by_field_name("init");
            let cond = node
                .child_by_field_name("condition")
                .map(|n| Box::new(lower_node(n, source)));
            let updates: Vec<SyntaxTree> = {
                let mut cursor = node.walk();
                let mut out = Vec::new();
                for (i, c) in node.children(&mut cursor).enumerate() {
                    if node.field_name_for_child(i as u32) == Some("update") {
                        out.push(lower_node(c, source));
                    }
                }
                out
            };
            let body = node
                .child_by_field_name("body")
                .map(|n| Box::new(lower_block_like(n, source)));
            match body {
                Some(b) => SyntaxTree::CFor {
                    initializer: init.map(|i| Box::new(lower_node(i, source))),
                    condition: cond,
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

        "enhanced_for_statement" => {
            // `for (T x : xs) body` — Java foreach. Imperative shape:
            // `<foreach><type>...</type><name>x</name><value><expression>xs</expression></value><body>...</body></foreach>`
            // (no `<left>`/`<right>` wrappers like C#'s).
            let type_node = node.child_by_field_name("type");
            let name_node = node.child_by_field_name("name");
            let value_node = node.child_by_field_name("value");
            let body_node = node.child_by_field_name("body");
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(t) = type_node {
                let inner = lower_node(t, source);
                let already_typed = matches!(
                    inner,
                    SyntaxTree::GenericType { .. } | SyntaxTree::SimpleStatement { element_name: "type", .. }
                );
                if already_typed {
                    children.push(inner);
                } else {
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(t),
                        span: span_of(t),
                    });
                }
            }
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            if let Some(v) = value_node {
                let expr = crate::tree::Expression::wrap(lower_node(v, source));
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "value",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![*expr.inner],
                    range: range_of(v),
                    span: span_of(v),
                });
            }
            if let Some(b) = body_node {
                children.push(lower_block_like(b, source));
            }
            SyntaxTree::SimpleStatement {
                element_name: "foreach",
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

        // Try/catch/finally.
        "try_statement" | "try_with_resources_statement" => {
            let mut cursor = node.walk();
            let mut try_body: Option<Box<SyntaxTree>> = None;
            let mut handlers: Vec<SyntaxTree> = Vec::new();
            let mut finally_body: Option<Box<SyntaxTree>> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "block" => {
                        if try_body.is_none() {
                            try_body = Some(Box::new(lower_block_like(c, source)));
                        }
                    }
                    "catch_clause" => handlers.push(lower_java_catch_clause(c, source)),
                    "finally_clause" => {
                        let mut fc = c.walk();
                        let inner = c.named_children(&mut fc).find(|n| n.kind() == "block");
                        if let Some(b) = inner {
                            finally_body = Some(Box::new(lower_block_like(b, source)));
                        }
                    }
                    _ => {}
                }
            }
            SyntaxTree::Try {
                try_body: try_body.unwrap_or_else(|| {
                    Box::new(SyntaxTree::Body {
                        children: Vec::new(),
                        pass_only: false,
                        block_wrap: false,
                        range: ByteRange::empty_at(range.start),
                        span,
                    })
                }),
                handlers,
                else_body: None,
                finally_body,
                range,
                span,
            }
        }

        // Throw statement → simple statement so it carries source bytes.
        "throw_statement" => lower_java_throw(node, source),

        // Imports & package.
        "import_declaration" => {
            // `import static java.util.Foo;` carries a `<static/>`
            // marker so the keyword doesn't leak as text. Detect by
            // source-prefix scan ("import static ...").
            let leading = source[range.start as usize..range.end as usize].trim_start();
            let prefix = leading.trim_start_matches("import").trim_start();
            if prefix.starts_with("static") {
                simple_statement_marked(node, "import", &["static"], source)
            } else {
                simple_statement(node, "import", source)
            }
        }
        "package_declaration" => simple_statement(node, "package", source),

        // Annotations / decorators.
        "annotation" | "marker_annotation" => {
            // Java's `@Override`, `@SuppressWarnings("foo")` — render
            // as `<annotation>` (matches imperative pipeline; C# uses
            // `<attribute>` for its corresponding construct, hence
            // the name divergence).
            let mut cursor = node.walk();
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "annotation_argument_list" => {
                        let mut alc = c.walk();
                        for a in c.named_children(&mut alc) {
                            children.push(lower_node(a, source));
                        }
                    }
                    _ => children.push(lower_node(c, source)),
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "annotation",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
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
                (Some(l), Some(r), Some(marker)) => SyntaxTree::Binary {
                    element_name: "binary",
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l),
                    right: Box::new(r),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
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

        "assignment_expression" => {
            // Pretend like Python's: <assign><left>{lhs}</left><op>=</op><right>{rhs}</right></assign>.
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            match (left, right) {
                (Some(l), Some(r)) => SyntaxTree::Assign {
                    targets: vec![lower_node(l, source)],
                    type_annotation: None,
                    op_text,
                    op_range,
                    op_markers: Vec::new(),
                    values: vec![lower_node(r, source)],
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "assignment_expression(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Member / call chains.
        "field_access" => {
            let object_node = node.child_by_field_name("object");
            let field_node = node.child_by_field_name("field");
            match (object_node, field_node) {
                (Some(object), Some(attr)) => {
                    let object_ir = lower_node(object, source);
                    let property_range = range_of(attr);
                    let property_span = span_of(attr);
                    let segment_range = ByteRange::new(object_ir.range().end, property_range.end);
                    let segment = AccessSegment::Member {
                        property_range,
                        property_span,
                        optional: false,
                        range: segment_range,
                        span,
                    };
                    match object_ir {
                        SyntaxTree::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::Access {
                                receiver,
                                segments,
                                range,
                                span,
                            }
                        }
                        other => SyntaxTree::Access {
                            receiver: Box::new(other),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => SyntaxTree::Unknown {
                    kind: "field_access(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        "method_invocation" => {
            // `obj.method(args)` — fold name into the chain's Call
            // segment so the rendered shape is
            // `<object><name>obj</name><call><name>method</name>...</call></object>`.
            // For bare `name(args)` (no object), produce SyntaxTree::Call.
            let object_node = node.child_by_field_name("object");
            let name_node = node.child_by_field_name("name");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<SyntaxTree> = match args_node {
                Some(a) => {
                    let mut ac = a.walk();
                    a.named_children(&mut ac)
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            match (object_node, name_node) {
                (Some(o), Some(n)) => {
                    let object_ir = lower_node(o, source);
                    let property_range = range_of(n);
                    let property_span = span_of(n);
                    let call_segment = AccessSegment::Call {
                        name: Some(property_range),
                        name_span: Some(property_span),
                        arguments,
                        range: ByteRange::new(object_ir.range().end, range.end),
                        span,
                    };
                    match object_ir {
                        SyntaxTree::Access { receiver, mut segments, .. } => {
                            segments.push(call_segment);
                            SyntaxTree::Access {
                                receiver,
                                segments,
                                range,
                                span,
                            }
                        }
                        other => SyntaxTree::Access {
                            receiver: Box::new(other),
                            segments: vec![call_segment],
                            range,
                            span,
                        },
                    }
                }
                (None, Some(n)) => {
                    // Bare invocation `name(args)`.
                    let callee = name_of(n, source);
                    SyntaxTree::Call {
                        callee: Box::new(callee),
                        arguments,
                        range,
                        span,
                    }
                }
                _ => SyntaxTree::Unknown {
                    kind: "method_invocation(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        "array_access" => {
            // `arr[0]` — fold into Access chain.
            let array_node = node.child_by_field_name("array");
            let index_node = node.child_by_field_name("index");
            match (array_node, index_node) {
                (Some(a), Some(i)) => {
                    let array_ir = lower_node(a, source);
                    let segment = AccessSegment::Index {
                        indices: vec![lower_node(i, source)],
                        range: ByteRange::new(array_ir.range().end, range.end),
                        span,
                    };
                    match array_ir {
                        SyntaxTree::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::Access { receiver, segments, range, span }
                        }
                        other => SyntaxTree::Access {
                            receiver: Box::new(other),
                            segments: vec![segment],
                            range,
                            span,
                        },
                    }
                }
                _ => SyntaxTree::Unknown {
                    kind: "array_access(missing field)".to_string(),
                    range,
                    span,
                },
            }
        }

        "object_creation_expression" => {
            let type_node = node.child_by_field_name("type");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<SyntaxTree> = match args_node {
                Some(a) => {
                    let mut ac = a.walk();
                    a.named_children(&mut ac)
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            SyntaxTree::ObjectCreation {
                type_target: type_node.map(|t| Box::new(lower_node(t, source))),
                arguments,
                initializer: None,
                range,
                span,
            }
        }

        // Generic types `List<String>`.
        "generic_type" => {
            let mut cursor = node.walk();
            let kids: Vec<TsNode> = node.named_children(&mut cursor).collect();
            if let Some(name) = kids.first() {
                let params: Vec<SyntaxTree> = node
                    .named_children(&mut node.walk())
                    .filter(|c| c.kind() == "type_arguments")
                    .flat_map(|c| {
                        let mut tc = c.walk();
                        c.named_children(&mut tc)
                            .map(|n| lower_node(n, source))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                SyntaxTree::GenericType {
                    name: Box::new(lower_node(*name, source)),
                    params,
                    range,
                    span,
                }
            } else {
                SyntaxTree::Unknown {
                    kind: "generic_type(empty)".to_string(),
                    range,
                    span,
                }
            }
        }

        "type_parameter" => {
            // Java type parameter: `T` or `T extends Bound1 & Bound2`.
            // tree-sitter exposes a `type_identifier` (the name) + an
            // optional `type_bound` child whose named children are
            // the bound types. Render as `<generic>` (matches
            // imperative shape) with `<name>T</name>` and an
            // `<extends><type>Bound</type></extends>` child.
            let mut cursor = node.walk();
            let mut name_node: Option<TsNode> = None;
            let mut bound_node: Option<TsNode> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "type_bound" => bound_node = Some(c),
                    _ if name_node.is_none() => name_node = Some(c),
                    _ => {}
                }
            }
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            if let Some(tb) = bound_node {
                let mut bc = tb.walk();
                let inner = tb.named_children(&mut bc).next();
                let inner_ir = match inner {
                    Some(t) => lower_node(t, source),
                    None => SyntaxTree::Unknown {
                        kind: "type_bound(empty)".to_string(),
                        range: range_of(tb),
                        span: span_of(tb),
                    },
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
                        extra_markers: &[],
                        children: vec![inner_ir],
                        range: r,
                        span: s,
                    }
                };
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "extends",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![typed],
                    range: range_of(tb),
                    span: span_of(tb),
                });
            }
            SyntaxTree::SimpleStatement {
                element_name: "generic",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }

        "array_type" => simple_statement_marked(node, "type", &["array"], source),

        // Comments.
        "line_comment" | "block_comment" => SyntaxTree::Comment {
            leading: false,
            trailing: false,
            range,
            span,
        },

        // Parenthesized expression — unwrap.
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(n) => lower_node(n, source),
                None => SyntaxTree::Unknown {
                    kind: "parenthesized_expression(empty)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Cast `(T) expr`.
        "cast_expression" => {
            let type_node = node.child_by_field_name("type");
            let value_node = node.child_by_field_name("value");
            match (type_node, value_node) {
                (Some(t), Some(v)) => SyntaxTree::Cast {
                    type_ann: Box::new(lower_node(t, source)),
                    value: Box::new(lower_node(v, source)),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "cast_expression(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Ternary `cond ? a : b`.
        "ternary_expression" => {
            let cond = node.child_by_field_name("condition").map(|n| lower_node(n, source));
            let if_true = node.child_by_field_name("consequence").map(|n| lower_node(n, source));
            let if_false = node.child_by_field_name("alternative").map(|n| lower_node(n, source));
            match (cond, if_true, if_false) {
                (Some(c), Some(t), Some(f)) => SyntaxTree::Ternary {
                    condition: Box::new(c),
                    if_true: Box::new(t),
                    if_false: Box::new(f),
                    range,
                    span,
                },
                _ => SyntaxTree::Unknown {
                    kind: "ternary_expression(missing)".to_string(),
                    range,
                    span,
                },
            }
        }

        // Switch.
        "switch_expression" | "switch_statement" => simple_statement(node, "switch", source),
        "switch_rule" => {
            // Java 14+ arrow-form switch: `case X -> result;` or
            // `default -> result;`. Lower as `<arm>` with the labels
            // and result as flat children.
            let mut cursor = node.walk();
            let children: Vec<SyntaxTree> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "arm",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }
        "annotation_type_element_declaration" => {
            // `String value() default "x";` — annotation method.
            // Lower as a method-shaped SimpleStatement so it surfaces.
            simple_statement(node, "method", source)
        }
        "guard" => {
            // Java 21 record-pattern guard `case R(int x) when x > 0`.
            simple_statement(node, "guard", source)
        }
        "annotation_type_body" => {
            // Body of `@interface Foo { ... }` — lower like a block
            // with no extra wrapping; the parent annotation_type_decl
            // handles its own shape.
            let mut cursor = node.walk();
            let children: Vec<SyntaxTree> = node
                .named_children(&mut cursor)
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
        "switch_block" => {
            let mut cursor = node.walk();
            let children: Vec<SyntaxTree> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        "switch_block_statement_group" => simple_statement(node, "section", source),
        "switch_label" => simple_statement(node, "case", source),

        // Scoped identifier `java.util.List` — flatten to a path of names.
        // (`scoped_type_identifier` is also handled by the atom arm
        // above as `SyntaxTree::Name` for the simple cases; this arm is the
        // path-shaped catch-all for `scoped_identifier` proper.)
        "scoped_identifier" => {
            // Flatten recursively at lowering time so `java.util.List`
            // produces `<path><name>java</name><name>util</name><name>List</name></path>`
            // instead of nested `<path><path>...</path>...</path>`. Replaces
            // the imperative `flatten_nested_paths` post-walk.
            let mut segments: Vec<SyntaxTree> = Vec::new();
            collect_scoped_segments(node, source, &mut segments);
            SyntaxTree::Path { segments, range, span }
        }

        "enum_constant" => {
            // `RED`, `BLUE(0xff)` — has identifier name + optional arguments.
            let name_node = node.child_by_field_name("name");
            let name = match name_node {
                Some(n) => Box::new(name_of(n, source)),
                None => Box::new(SyntaxTree::Unknown {
                    kind: "enum_constant(no name)".to_string(),
                    range,
                    span,
                }),
            };
            SyntaxTree::EnumMember {
                decorators: Vec::new(),
                name,
                value: None,
                range,
                span,
            }
        }

        "explicit_constructor_invocation" => {
            // `this(args)` or `super(args)` — emit `<call[this]>` or
            // `<call[super]>` with the args as flat children. tree-
            // sitter exposes the keyword as the first unnamed child;
            // detect by source-text prefix.
            let leading = source[range.start as usize..range.end as usize].trim_start();
            let marker: &'static [&'static str] = if leading.starts_with("this") {
                &["this"]
            } else if leading.starts_with("super") {
                &["super"]
            } else {
                &[]
            };
            // Walk the argument_list children directly so they sit as
            // call args (matches the imperative `<call>` shape).
            // Skip the leading `this` / `super` keyword child — the
            // `[this]` / `[super]` marker on `<call>` already conveys
            // that fact; keeping the `<this>this</this>` /
            // `<super>super</super>` text leaf duplicates the marker
            // and leaks the keyword as text (Principle #2).
            let mut cursor = node.walk();
            let mut children: Vec<SyntaxTree> = Vec::new();
            // Track the end of the keyword child so we can shrink
            // the SimpleStatement's `range` past it. Without this
            // shrink, an empty `super();` ends up rendering as
            // `<call[super]>super();</call>` — gap text covers the
            // whole source range because there are no children to
            // anchor the gap calculation.
            let mut start = range.start;
            for c in node.named_children(&mut cursor) {
                if c.kind() == "argument_list" {
                    let mut ac = c.walk();
                    for a in c.named_children(&mut ac) {
                        children.push(lower_node(a, source));
                    }
                } else if matches!(c.kind(), "this" | "super") {
                    let kw_end = c.end_byte() as u32;
                    if kw_end > start {
                        start = kw_end;
                    }
                    continue;
                } else {
                    children.push(lower_node(c, source));
                }
            }
            let trimmed_range = ByteRange::new(start, range.end);
            SyntaxTree::SimpleStatement {
                element_name: "call",
                modifiers: Modifiers::default(),
                extra_markers: marker,
                children,
                range: trimmed_range,
                span,
            }
        }
        "annotation_type_declaration" => simple_statement(node, "interface", source),
        "array_initializer" => {
            // Java's `{1, 2, 3}` array literal — render as `<array>`
            // (matches imperative shape; no `<list>` element in
            // Java's vocabulary).
            let mut cursor = node.walk();
            let children: Vec<SyntaxTree> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "array",
                modifiers: Modifiers::default(),
                extra_markers: &["literal"],
                children,
                range,
                span,
            }
        }
        "assert_statement" => simple_statement(node, "assert", source),
        "catch_type" => {
            // `IOException | IllegalStateException` — multi-type catch.
            let mut cursor = node.walk();
            let children: Vec<SyntaxTree> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        "class_literal" => simple_statement(node, "type", source),
        "compact_constructor_declaration" => {
            // record's compact ctor: `record R(int x) { }` — body only.
            let body_node = node.child_by_field_name("body");
            let body: Option<Box<SyntaxTree>> = body_node.map(|b| Box::new(lower_block_like(b, source)));
            let modifiers = lower_java_modifiers(node, source, Some(Access::Public));
            let name_node = node.child_by_field_name("name");
            let name = match name_node {
                Some(n) => Box::new(name_of(n, source)),
                None => Box::new(SyntaxTree::Unknown {
                    kind: "compact_ctor(no name)".to_string(),
                    range,
                    span,
                }),
            };
            SyntaxTree::Function {
                element_name: "constructor",
                modifiers,
                decorators: Vec::new(),
                name,
                generics: None,
                parameters: Vec::new(),
                returns: None,
                body,
                range,
                span,
            }
        }
        "dimensions" => simple_statement(node, "type", source),
        "enum_body_declarations" => {
            let mut cursor = node.walk();
            let children: Vec<SyntaxTree> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline { children, list_name: None, range, span }
        }
        "instanceof_expression" => {
            // `expr instanceof Type [name]` — SyntaxTree::Is.
            let value_node = node.child_by_field_name("left");
            let type_node = node.child_by_field_name("right");
            match (value_node, type_node) {
                (Some(v), Some(t)) => SyntaxTree::Is {
                    value: Box::new(lower_node(v, source)),
                    type_target: Box::new(lower_node(t, source)),
                    range,
                    span,
                },
                _ => simple_statement(node, "is", source),
            }
        }
        "labeled_statement" => simple_statement(node, "label", source),
        "method_reference" => simple_statement(node, "member", source),
        "static_initializer" => {
            // `static { ... }` — render the WHOLE static_initializer
            // (including the `static` keyword) as a `<block[static]>`
            // SimpleStatement so the keyword doesn't leak into the
            // class body's gap text.
            let mut cursor = node.walk();
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children(&mut cursor) {
                if c.kind() == "block" {
                    let mut bc = c.walk();
                    children.extend(c.named_children(&mut bc).map(|n| lower_node(n, source)));
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "block",
                modifiers: Modifiers::default(),
                extra_markers: &["static"],
                children,
                range,
                span,
            }
        }
        "synchronized_statement" => simple_statement(node, "lock", source),
        // `this` and `super` keywords appear as access-chain receivers
        // and as direct expressions. Render as `<this>this</this>` /
        // `<super>super</super>` text leaves so XPath `[. = 'super']`
        // matches the imperative pipeline shape.
        "this" => SyntaxTree::SimpleStatement {
            element_name: "this",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range,
            span,
        },
        "super" => SyntaxTree::SimpleStatement {
            element_name: "super",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range,
            span,
        },
        "wildcard" => simple_statement(node, "type", source),

        // Java pattern matching constructs.
        // tree-sitter wraps the actual pattern in a `pattern` node;
        // unwrap and recurse so `type_pattern` etc. surface directly.
        "pattern" => {
            let mut cursor = node.walk();
            let inner = node.named_children(&mut cursor).next();
            match inner {
                Some(i) => lower_node(i, source),
                None => SyntaxTree::Unknown { kind: "pattern(empty)".to_string(), range, span },
            }
        }
        "type_pattern" => {
            // `Integer i` — has a `type` field and a `name` field.
            // Render as `<pattern><type><name>Integer</name></type><name>i</name></pattern>`
            // (the imperative shape; `<type>` is structural here, not
            // a marker — Type is dual-use in Java's vocabulary).
            let mut cursor = node.walk();
            let mut type_node: Option<TsNode> = None;
            let mut name_node: Option<TsNode> = None;
            for c in node.named_children(&mut cursor) {
                match c.kind() {
                    "type_identifier" | "scoped_type_identifier" | "generic_type" | "boolean_type"
                    | "integral_type" | "floating_point_type" | "void_type" | "array_type" => {
                        if type_node.is_none() {
                            type_node = Some(c);
                        }
                    }
                    "identifier" => {
                        if name_node.is_none() {
                            name_node = Some(c);
                        }
                    }
                    _ => {}
                }
            }
            let mut children: Vec<SyntaxTree> = Vec::new();
            if let Some(t) = type_node {
                let inner = lower_node(t, source);
                let already_typed = matches!(
                    inner,
                    SyntaxTree::GenericType { .. } | SyntaxTree::SimpleStatement { element_name: "type", .. }
                );
                if already_typed {
                    children.push(inner);
                } else {
                    children.push(SyntaxTree::SimpleStatement {
                        element_name: "type",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(t),
                        span: span_of(t),
                    });
                }
            }
            if let Some(n) = name_node {
                children.push(name_of(n, source));
            }
            SyntaxTree::SimpleStatement {
                element_name: "pattern",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range,
                span,
            }
        }

        // Yield.
        "yield_statement" => simple_statement(node, "yield", source),

        // Lambda.
        "lambda_expression" => simple_statement(node, "lambda", source),

        // Argument list (rare standalone).
        "argument_list" | "block_statements" => {
            let mut cursor = node.walk();
            let children: Vec<SyntaxTree> = node
                .named_children(&mut cursor)
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::Inline {
                children,
                list_name: None,
                range,
                span,
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

fn lower_java_else_chain(node: TsNode<'_>, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    // Java `if_statement.alternative` is either another `if_statement`
    // (else-if chain) or a `block` / single statement (plain else).
    if node.kind() == "if_statement" {
        let cond = node
            .child_by_field_name("condition")
            .map(|n| Box::new(lower_node(n, source)));
        let body = node
            .child_by_field_name("consequence")
            .map(|n| Box::new(lower_block_like(n, source)));
        let alt = node.child_by_field_name("alternative");
        let else_branch = alt.map(|a| Box::new(lower_java_else_chain(a, source)));
        match (cond, body) {
            (Some(c), Some(b)) => SyntaxTree::ElseIf {
                condition: c,
                body: b,
                else_branch,
                range,
                span,
            },
            _ => SyntaxTree::Unknown {
                kind: "java_else_if(missing)".to_string(),
                range,
                span,
            },
        }
    } else {
        // Plain else — wrap in <else>.
        SyntaxTree::Else {
            body: Box::new(lower_block_like(node, source)),
            range,
            span,
        }
    }
}

fn lower_java_catch_clause(node: TsNode<'_>, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let mut type_target: Option<Box<SyntaxTree>> = None;
    let mut binding: Option<Box<SyntaxTree>> = None;
    let mut body: Option<Box<SyntaxTree>> = None;
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "catch_formal_parameter" => {
                // Java: `catch (Type name)` — has `type` and `name` children.
                let mut cc = c.walk();
                for inner in c.named_children(&mut cc) {
                    match inner.kind() {
                        "catch_type" | "type" | "type_identifier" | "scoped_type_identifier" => {
                            type_target = Some(Box::new(lower_node(inner, source)));
                        }
                        "identifier" => {
                            binding = Some(Box::new(name_of(inner, source)));
                        }
                        _ => {}
                    }
                }
            }
            "block" => body = Some(Box::new(lower_block_like(c, source))),
            _ => {}
        }
    }
    SyntaxTree::ExceptHandler {
        kind: "catch",
        type_target,
        binding,
        filter: None,
        body: body.unwrap_or_else(|| {
            Box::new(SyntaxTree::Body {
                children: Vec::new(),
                pass_only: false,
                block_wrap: false,
                range: ByteRange::empty_at(range.start),
                span,
            })
        }),
        range,
        span,
    }
}

fn lower_block_like(node: TsNode<'_>, source: &str) -> SyntaxTree {
    let mut cursor = node.walk();
    let children: Vec<SyntaxTree> = node
        .named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
    let children = merge_java_line_comments(children, source);
    // Java method bodies render as bare `<body>` (no inner `<block>`
    // wrapper) — matches the imperative pipeline's shape and lets
    // tests assert `//method/body/<stmt>` directly.
    SyntaxTree::Body {
        children,
        pass_only: false,
        block_wrap: false,
        range: range_of(node),
        span: span_of(node),
    }
}

/// Lower a Java multi-declarator field/local statement. Single-
/// declarator: produce a flat `SyntaxTree::Variable` so the post-pass'
/// `flatten_single_declarator_children` keeps it bare. Multi-
/// declarator: produce one outer `<field>`/`<variable>` wrapping
/// `<declarator>` children — matches the imperative pipeline's
/// shape for `int x = 1, y = 2;`.
fn lower_java_multi_declarator(
    parent: TsNode<'_>,
    type_node: Option<TsNode<'_>>,
    declarators: &[TsNode<'_>],
    source: &str,
    range: ByteRange,
    span: Span,
    element_name: &'static str,
    modifiers: Modifiers,
    decorators: Vec<SyntaxTree>,
) -> SyntaxTree {
    let _ = parent;
    if declarators.is_empty() {
        return SyntaxTree::Unknown {
            kind: format!("{}(no declarators)", element_name),
            range,
            span,
        };
    }
    if declarators.len() == 1 {
        let d = declarators[0];
        return lower_variable_declarator(
            d,
            type_node,
            source,
            range,
            span,
            element_name,
            modifiers,
            decorators,
        );
    }
    // Multi-declarator: wrap each in a `<declarator>` SimpleStatement.
    // Type stays at the outer `<field>` level — wrap in `<type>` if
    // the inner doesn't already produce a type-shaped element.
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(t) = type_node {
        let inner = lower_node(t, source);
        let already_typed = matches!(
            inner,
            SyntaxTree::GenericType { .. }
                | SyntaxTree::SimpleStatement { element_name: "type", .. }
        );
        if already_typed {
            children.push(inner);
        } else {
            children.push(SyntaxTree::SimpleStatement {
                element_name: "type",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![inner],
                range: range_of(t),
                span: span_of(t),
            });
        }
    }
    for d in declarators {
        let name_node = d.child_by_field_name("name");
        let value_node = d.child_by_field_name("value");
        let mut decl_children: Vec<SyntaxTree> = Vec::new();
        if let Some(n) = name_node {
            decl_children.push(name_of(n, source));
        }
        if let Some(v) = value_node {
            // <value><expression>...</expression></value> — value-position
            // expression host (Principle #15) encoded at lowering time.
            let inner = crate::tree::Expression::wrap(lower_node(v, source));
            decl_children.push(SyntaxTree::SimpleStatement {
                element_name: "value",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![*inner.inner],
                range: range_of(v),
                span: span_of(v),
            });
        }
        children.push(SyntaxTree::SimpleStatement {
            element_name: "declarator",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: decl_children,
            range: range_of(*d),
            span: span_of(*d),
        });
    }
    // Use SyntaxTree::Variable as the outer wrapper but with a SimpleStatement
    // hack: we need the `<field>` element with mixed children
    // (type + declarator + declarator). Easiest is a SimpleStatement
    // with `element_name`.
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers,
        extra_markers: &[],
        children: {
            let mut all: Vec<SyntaxTree> = decorators;
            all.extend(children);
            all
        },
        range,
        span,
    }
}

fn lower_variable_declarator(
    declarator: TsNode<'_>,
    type_node: Option<TsNode<'_>>,
    source: &str,
    range: ByteRange,
    span: Span,
    element_name: &'static str,
    modifiers: Modifiers,
    decorators: Vec<SyntaxTree>,
) -> SyntaxTree {
    let name_node = declarator.child_by_field_name("name");
    let value_node = declarator.child_by_field_name("value");
    let Some(n) = name_node else {
        let mut cursor = declarator.walk();
        let children: Vec<SyntaxTree> = declarator
            .named_children(&mut cursor)
            .map(|c| lower_node(c, source))
            .collect();
        return SyntaxTree::Inline {
            children,
            list_name: None,
            range,
            span,
        };
    };
    let name_ir = name_of(n, source);
    let type_ir = type_node.map(|t| Box::new(lower_node(t, source)));
    // Wrap the value in a `<value>` SimpleStatement so the post-pass'
    // `wrap_expression_positions` finds it (it scans for `<value>`,
    // `<condition>` etc. and adds the `<expression>` host inside).
    // Java tests assert `value/expression/int='1'`; without the
    // wrapper the value renders as a bare child of `<field>`.
    let value_ir = value_node.map(|v| crate::tree::Expression::wrap(lower_node(v, source)));
    SyntaxTree::Variable {
        element_name,
        modifiers,
        decorators,
        type_ann: type_ir,
        name: Box::new(name_ir),
        value: value_ir,
        range,
        span,
    }
}

fn lower_children(node: TsNode<'_>, source: &str) -> Vec<SyntaxTree> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect()
}

/// Walk Java `modifiers` block and accumulate flags into the
/// `Modifiers` struct. `default_access` applies when no explicit
/// access modifier (public/private/protected) is found — e.g.
/// package-private fields and methods get `Access::Internal`.
fn lower_java_modifiers(
    node: TsNode<'_>,
    source: &str,
    default_access: Option<Access>,
) -> Modifiers {
    let mut m = Modifiers::default();
    if let Some(da) = default_access {
        m.access = Some(da);
    }
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        if c.kind() != "modifiers" {
            continue;
        }
        let mut mc = c.walk();
        for tok in c.children(&mut mc) {
            let text = text_of(tok, source);
            match text.as_str() {
                "public" => m.access = Some(Access::Public),
                "private" => m.access = Some(Access::Private),
                "protected" => m.access = Some(Access::Protected),
                "static" => m.static_ = true,
                "final" => m.final_ = true,
                "abstract" => m.abstract_ = true,
                "synchronized" => m.synchronized_ = true,
                "native" => m.native = true,
                "volatile" => m.volatile = true,
                "transient" => m.transient = true,
                "strictfp" => m.strictfp = true,
                "default" => m.default = true,
                _ => {}
            }
        }
    }
    m
}

fn merge_java_line_comments(children: Vec<SyntaxTree>, source: &str) -> Vec<SyntaxTree> {
    // Java line comments use `//` like C#; reuse the same merge logic.
    // Reuse the C# implementation by inlining the algorithm here so we
    // don't cross-reference a feature-gated module.
    let mut out: Vec<SyntaxTree> = Vec::with_capacity(children.len());
    for child in children {
        if let SyntaxTree::Comment {
            leading,
            trailing,
            range,
            span,
        } = child
        {
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
                let prev_is_line_comment = source
                    [prev_range.start as usize..prev_range.end as usize]
                    .trim_start()
                    .starts_with("//");
                let curr_is_line_comment = source[range.start as usize..range.end as usize]
                    .trim_start()
                    .starts_with("//");
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: true, .. }));
                if only_one_newline
                    && prev_is_line_comment
                    && curr_is_line_comment
                    && !prev_was_trailing
                    && !curr_is_trailing
                {
                    if let Some(SyntaxTree::Comment {
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
            out.push(SyntaxTree::Comment {
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
        if let SyntaxTree::Comment { trailing, range, .. } = &out[i] {
            if *trailing {
                continue;
            }
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

/// Walk a (possibly nested) `scoped_identifier` / `scoped_type_identifier`
/// subtree, emitting `SyntaxTree::Name` for each terminal identifier. Mirrors
/// the imperative `flatten_nested_paths` post-walk and the Java post-
/// pass `java_unwrap_type_in_path` (which dropped `<type>` wrappers
/// on path segments) — both are encoded here at lowering time.
fn collect_scoped_segments<'a>(node: TsNode<'a>, source: &str, out: &mut Vec<SyntaxTree>) {
    let mut cursor = node.walk();
    for c in node.named_children(&mut cursor) {
        match c.kind() {
            "scoped_identifier" | "scoped_type_identifier" => {
                collect_scoped_segments(c, source, out);
            }
            _ => out.push(lower_node(c, source)),
        }
    }
}

fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let children: Vec<SyntaxTree> = node
        .named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range,
        span,
    }
}

/// Lower Java `throw expr;` so the thrown expression sits under an
/// `<expression>` host (Principle #5, matches Python yield/raise
/// and the existing `<return>` shape — same conceptual role:
/// throw-a-value vs return-a-value).
fn lower_java_throw(node: TsNode<'_>, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let inner: Vec<SyntaxTree> = node.named_children(&mut cursor)
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
            extra_markers: &[],
            children: inner,
            range: expr_range,
            span,
        }]
    };
    SyntaxTree::SimpleStatement {
        element_name: "throw",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range,
        span,
    }
}

#[allow(dead_code)]
fn simple_statement_marked(
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers: &'static [&'static str],
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut cursor = node.walk();
    let children: Vec<SyntaxTree> = node
        .named_children(&mut cursor)
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
        "instanceof" => "instanceof",
        _ => return None,
    })
}

/// Walk up the CST to find the nearest enclosing type declaration
/// kind (class/interface/record/enum). Used for default-access
/// derivation: interface members default to `public`, others to
/// `package`-private.
fn enclosing_type_kind(node: TsNode<'_>) -> Option<String> {
    let mut current = node.parent();
    while let Some(p) = current {
        match p.kind() {
            "class_declaration"
            | "interface_declaration"
            | "record_declaration"
            | "enum_declaration"
            | "annotation_type_declaration" => return Some(p.kind().to_string()),
            _ => current = p.parent(),
        }
    }
    None
}



