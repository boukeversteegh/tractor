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


use std::cell::RefCell;
use std::collections::HashMap;

use crate::raw::RawNode;

use crate::tree::lower_helpers::{
    false_of, float_of, int_of, name_of, null_of, range_of, span_of, string_of, text_of, true_of,
};
use crate::tree::types::{Access, AccessSegment, ByteRange, Flag, SyntaxTree, Modifiers, Marker, ParamKind, Span};

// Parent-map context (see csharp's lower.rs for the rationale).
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

/// Lower a Java tree-sitter root node to [`SyntaxTree`].
pub fn lower_java_root(root: &RawNode, source: &str) -> SyntaxTree {
    PARENT_MAP.with(|p| {
        let mut map = HashMap::new();
        populate_parent_map(root, &mut map);
        *p.borrow_mut() = map;
    });
    let result = lower_java_root_inner(root, source);
    PARENT_MAP.with(|p| p.borrow_mut().clear());
    result
}

fn lower_java_root_inner(root: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => SyntaxTree::Module {
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
pub fn lower_java_node(node: &RawNode, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

fn lower_node(node: &RawNode, source: &str) -> SyntaxTree {
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
            extra_markers: vec![Marker::implicit("void")],
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
                let mut found = None;
                for c in node.named_children() {
                    if matches!(c.kind(), "class_body" | "interface_body" | "enum_body" | "record_body") {
                        found = Some(c);
                        break;
                    }
                }
                found
            });
            let type_param_list = node
                .named_children()
                .find(|c| c.kind() == "type_parameters");
            let superclass = node
                .named_children()
                .find(|c| c.kind() == "superclass");
            let interfaces = node
                .named_children()
                .find(|c| matches!(c.kind(), "super_interfaces" | "extends_interfaces"));
            let decorators: Vec<SyntaxTree> = node
                .named_children()
                .filter(|c| c.kind() == "modifiers")
                .flat_map(|m| {
                    m.named_children()
                        .filter(|c| matches!(c.kind(), "annotation" | "marker_annotation"))
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            let modifiers = lower_java_modifiers(node, source, /*default_access*/ Some(Access::Package));
            let generics: Vec<SyntaxTree> = match type_param_list {
                Some(tpl) => tpl.named_children().map(|c| lower_node(c, source)).collect(),
                None => Vec::new(),
            };
            let mut bases: Vec<SyntaxTree> = Vec::new();
            // Superclass (`extends Foo`) — bare type, gets wrapped in
            // `<extends>` by the Class render's default base path.
            if let Some(sc) = superclass {
                bases.extend(sc.named_children().map(|c| lower_node(c, source).wrap_extends()));
            }
            // Interfaces (`implements Bar, Baz`) — each wrapped in
            // a `<implements>` SimpleStatement so the Class render
            // emits `<implements><type>...</type></implements>`
            // sibling under `<class>` (matches imperative shape).
            if let Some(ifs) = interfaces {
                for c in ifs.named_children() {
                    if c.kind() == "type_list" {
                        for n in c.named_children() {
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
                                    extra_markers: Vec::new(),
                                    children: vec![inner],
                                    range: range_of(n),
                                    span: span_of(n),
                                }
                            };
                            bases.push(SyntaxTree::SimpleStatement {
                                element_name: "implements",
                                modifiers: Modifiers::default(),
                                extra_markers: Vec::new(),
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
                "record" => SyntaxTree::Record {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
                _ => SyntaxTree::Class {
                    modifiers, decorators, name, generics, bases, where_clauses, body, range, span,
                },
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
            let type_param_list = node
                .named_children()
                .find(|c| c.kind() == "type_parameters");
            let generics: Vec<SyntaxTree> = match type_param_list {
                Some(tpl) => tpl.named_children().map(|c| lower_node(c, source)).collect(),
                None => Vec::new(),
            };
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
            let decorators: Vec<SyntaxTree> = node
                .named_children()
                .filter(|c| c.kind() == "modifiers")
                .flat_map(|m| {
                    m.named_children()
                        .filter(|c| matches!(c.kind(), "annotation" | "marker_annotation"))
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            let parameters: Vec<SyntaxTree> = match params_node {
                Some(p) => {
                    p.named_children()
                        .map(|c| lower_node(c, source))
                        .collect()
                }
                None => Vec::new(),
            };
            let returns = returns_node.map(|t| {
                Box::new(SyntaxTree::Returns {
                    type_ann: Box::new(lower_node(t, source).wrap_type()),
                    range: range_of(t),
                    span: span_of(t),
                })
            });
            // `throws E1, E2` — each exception-type target becomes one
            // `<throws>/<type>/<name>` sibling on the method (Principle
            // #18 — name the relationship after the operator).
            let throws: Vec<SyntaxTree> = node
                .named_children()
                .filter(|c| c.kind() == "throws")
                .flat_map(|tc| {
                    tc.named_children()
                        .map(|t| build_throws_target(t, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            // Body is None for abstract / interface methods — the
            // Function render skips emitting `<body>` when None
            // (matches imperative shape `<method[abstract]>` only).
            let body: Option<Box<SyntaxTree>> = body_node.map(|b| Box::new(lower_block_like(b, source)));
            let is_constructor = node.kind() == "constructor_declaration";
            let element_kind = if is_constructor { "constructor" } else { "method" };
            let name = Box::new(match name_node {
                Some(n) => name_of(n, source),
                None => SyntaxTree::Unknown {
                    kind: format!("{}(missing name)", element_kind),
                    range,
                    span,
                },
            });
            if is_constructor {
                let body = body.map(|b| *b).unwrap_or_else(|| SyntaxTree::Body {
                    children: Vec::new(),
                    pass_only: false,
                    block_wrap: false,
                    range: ByteRange::empty_at(range.end),
                    span,
                });
                SyntaxTree::Constructor {
                    modifiers,
                    decorators,
                    name,
                    parameters,
                    body: Box::new(body),
                    range,
                    span,
                }
            } else {
                SyntaxTree::Method {
                    modifiers,
                    decorators,
                    name,
                    generics,
                    parameters,
                    returns,
                    throws,
                    body,
                    range,
                    span,
                }
            }
        }

        "formal_parameter" | "spread_parameter" => {
            // For spread_parameter, the type and name come from
            // different positions: first named child = type, last
            // named child has the binding (variable_declarator → name).
            let is_spread = node.kind() == "spread_parameter";
            let type_node = node.child_by_field_name("type").or_else(|| {
                if is_spread {
                    let mut first = None;
                    for n in node.named_children() {
                        first = Some(n);
                        break;
                    }
                    first
                } else { None }
            });
            let name_node = node.child_by_field_name("name").or_else(|| {
                if is_spread {
                    // variable_declarator → identifier[name]
                    for n in node.named_children() {
                        if n.kind() == "variable_declarator" {
                            return n.child_by_field_name("name");
                        }
                    }
                }
                None
            });
            let extra_markers: Vec<crate::tree::types::Marker> = if is_spread {
                vec![Marker::implicit("variadic")]
            } else {
                Vec::new()
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
                type_ann: type_node.map(|t| Box::new(lower_node(t, source).wrap_type())),
                default: None,
                range,
                span,
            }
        }

        // Field.
        "field_declaration" => {
            let modifiers = lower_java_modifiers(node, source, Some(Access::Package));
            let type_node = node.child_by_field_name("type");
            let decorators: Vec<SyntaxTree> = node
                .named_children()
                .filter(|c| c.kind() == "modifiers")
                .flat_map(|m| {
                    m.named_children()
                        .filter(|c| matches!(c.kind(), "annotation" | "marker_annotation"))
                        .map(|c| lower_node(c, source))
                        .collect::<Vec<_>>()
                })
                .collect();
            // Java has multiple variable_declarators per field.
            let declarators: Vec<&RawNode> = node
                .named_children()
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
            let declarators: Vec<&RawNode> = node
                .named_children()
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
                .map(|n| Box::new(lower_node(n, source).wrap_slot("condition")));
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
                .map(|n| Box::new(lower_node(n, source).wrap_slot("condition")));
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
                let mut out = Vec::new();
                for (i, c) in node.children().enumerate() {
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
                        extra_markers: Vec::new(),
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
                    extra_markers: Vec::new(),
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

        // Try/catch/finally.
        "try_statement" | "try_with_resources_statement" => {
            let mut try_body: Option<Box<SyntaxTree>> = None;
            let mut handlers: Vec<SyntaxTree> = Vec::new();
            let mut finally_body: Option<Box<SyntaxTree>> = None;
            for c in node.named_children() {
                match c.kind() {
                    "block" => {
                        if try_body.is_none() {
                            try_body = Some(Box::new(lower_block_like(c, source)));
                        }
                    }
                    "catch_clause" => handlers.push(lower_java_catch_clause(c, source)),
                    "finally_clause" => {
                        let inner = c.named_children().find(|n| n.kind() == "block");
                        if let Some(b) = inner {
                            finally_body = Some(Box::new(lower_block_like(b, source).wrap_clause("finally")));
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
                simple_statement_marked(node, "import", vec![Marker::implicit("static")], source)
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
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                match c.kind() {
                    "annotation_argument_list" => {
                        for a in c.named_children() {
                            children.push(lower_node(a, source));
                        }
                    }
                    _ => children.push(lower_node(c, source)),
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "annotation",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
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
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l.wrap_slot("left")),
                    right: Box::new(r.wrap_slot("right")),
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

        "assignment_expression" => {
            // Pretend like Python's: <assign><left>{lhs}</left><op>=</op><right>{rhs}</right></assign>.
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
                    a.named_children()
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
                        SyntaxTree::ObjectAccess { receiver, mut segments, .. } => {
                            segments.push(call_segment);
                            SyntaxTree::ObjectAccess {
                                receiver,
                                segments,
                                range,
                                span,
                            }
                        }
                        other => SyntaxTree::ObjectAccess {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["this", "super"]),
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
                    a.named_children()
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
            let kids: Vec<&RawNode> = node.named_children().collect();
            if let Some(name) = kids.first() {
                let params: Vec<SyntaxTree> = node
                    .named_children()
                    .filter(|c| c.kind() == "type_arguments")
                    .flat_map(|c| {
                        c.named_children()
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
            let mut name_node: Option<&RawNode> = None;
            let mut bound_node: Option<&RawNode> = None;
            for c in node.named_children() {
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
                let inner = tb.named_children().next();
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
                    range: range_of(tb),
                    span: span_of(tb),
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

        // Comments.
        "line_comment" | "block_comment" => SyntaxTree::Comment {
            leading: Flag::Off,
            trailing: Flag::Off,
            range,
            span,
        },

        // Parenthesized expression — unwrap.
        "parenthesized_expression" => {
            let inner = node.named_children().next();
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
                    type_ann: Box::new(lower_node(t, source).wrap_type()),
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
                    condition: Box::new(c.wrap_slot("condition")),
                    if_true: Box::new(t.wrap_slot("then")),
                    if_false: Box::new(f.wrap_slot("else")),
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
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "arm",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
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
            let children: Vec<SyntaxTree> = node
                .named_children()
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
            let children: Vec<SyntaxTree> = node
                .named_children()
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
            let marker: Vec<crate::tree::types::Marker> = if leading.starts_with("this") {
                vec![Marker::implicit("this")]
            } else if leading.starts_with("super") {
                vec![Marker::implicit("super")]
            } else {
                Vec::new()
            };
            // Walk the argument_list children directly so they sit as
            // call args (matches the imperative `<call>` shape).
            // Skip the leading `this` / `super` keyword child — the
            // `[this]` / `[super]` marker on `<call>` already conveys
            // that fact; keeping the `<this>this</this>` /
            // `<super>super</super>` text leaf duplicates the marker
            // and leaks the keyword as text (Principle #2).
            let mut children: Vec<SyntaxTree> = Vec::new();
            // Track the end of the keyword child so we can shrink
            // the SimpleStatement's `range` past it. Without this
            // shrink, an empty `super();` ends up rendering as
            // `<call[super]>super();</call>` — gap text covers the
            // whole source range because there are no children to
            // anchor the gap calculation.
            let mut start = range.start;
            for c in node.named_children() {
                if c.kind() == "argument_list" {
                    for a in c.named_children() {
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
            let children: Vec<SyntaxTree> = node
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SyntaxTree::SimpleStatement {
                element_name: "array",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("literal")],
                children,
                range,
                span,
            }
        }
        "assert_statement" => simple_statement(node, "assert", source),
        "catch_type" => {
            // `IOException | IllegalStateException` — multi-type catch.
            let children: Vec<SyntaxTree> = node
                .named_children()
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
            let body = body.map(|b| *b).unwrap_or_else(|| SyntaxTree::Body {
                children: Vec::new(),
                pass_only: false,
                block_wrap: false,
                range: ByteRange::empty_at(range.end),
                span,
            });
            SyntaxTree::Constructor {
                modifiers,
                decorators: Vec::new(),
                name,
                parameters: Vec::new(),
                body: Box::new(body),
                range,
                span,
            }
        }
        "dimensions" => simple_statement(node, "type", source),
        "enum_body_declarations" => {
            let children: Vec<SyntaxTree> = node
                .named_children()
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
            let mut children: Vec<SyntaxTree> = Vec::new();
            for c in node.named_children() {
                if c.kind() == "block" {
                    children.extend(c.named_children().map(|n| lower_node(n, source)));
                }
            }
            SyntaxTree::SimpleStatement {
                element_name: "block",
                modifiers: Modifiers::default(),
                extra_markers: vec![Marker::implicit("static")],
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
        "wildcard" => simple_statement(node, "type", source),

        // Java pattern matching constructs.
        // tree-sitter wraps the actual pattern in a `pattern` node;
        // unwrap and recurse so `type_pattern` etc. surface directly.
        "pattern" => {
            let inner = node.named_children().next();
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
            let mut type_node: Option<&RawNode> = None;
            let mut name_node: Option<&RawNode> = None;
            for c in node.named_children() {
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
                        extra_markers: Vec::new(),
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
                extra_markers: Vec::new(),
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
            let children: Vec<SyntaxTree> = node
                .named_children()
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

fn lower_java_else_chain(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    // Java `if_statement.alternative` is either another `if_statement`
    // (else-if chain) or a `block` / single statement (plain else).
    if node.kind() == "if_statement" {
        let cond = node
            .child_by_field_name("condition")
            .map(|n| Box::new(lower_node(n, source).wrap_slot("condition")));
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

fn lower_java_catch_clause(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut type_target: Option<Box<SyntaxTree>> = None;
    let mut binding: Option<Box<SyntaxTree>> = None;
    let mut body: Option<Box<SyntaxTree>> = None;
    for c in node.named_children() {
        match c.kind() {
            "catch_formal_parameter" => {
                // Java: `catch (Type name)` — has `type` and `name` children.
                for inner in c.named_children() {
                    match inner.kind() {
                        "catch_type" | "type" | "type_identifier" | "scoped_type_identifier" => {
                            type_target = Some(Box::new(lower_node(inner, source).wrap_type()));
                        }
                        "identifier" => {
                            binding = Some(Box::new(name_of(inner, source).wrap_slot("as")));
                        }
                        _ => {}
                    }
                }
            }
            "block" => body = Some(Box::new(lower_block_like(c, source))),
            _ => {}
        }
    }
    SyntaxTree::Catch {
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

/// Wrap a Java exception-type target in `<throws>/<type>/{lowered
/// target}`. The outer `<throws>` is the operator-named relationship
/// (Principle #18); the inner `<type>` is the namespace marker
/// (Principle #14 — every type-reference slot carries a `<type>`
/// child).
fn build_throws_target(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let inner = lower_node(node, source);
    // Wrap the lowered target in a `<type>` slot (FieldWrap renders as
    // `<type>{inner}</type>`), then wrap that in `<throws>`.
    let type_slot = SyntaxTree::FieldWrap {
        wrapper: "type",
        inner: Box::new(inner),
        range,
        span,
    };
    SyntaxTree::SimpleStatement {
        element_name: "throws",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children: vec![type_slot],
        range,
        span,
    }
}

fn lower_block_like(node: &RawNode, source: &str) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node
        .named_children()
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
    parent: &RawNode,
    type_node: Option<&RawNode>,
    declarators: &[&RawNode],
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
                extra_markers: Vec::new(),
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
                extra_markers: Vec::new(),
                children: vec![*inner.inner],
                range: range_of(v),
                span: span_of(v),
            });
        }
        children.push(SyntaxTree::SimpleStatement {
            element_name: "declarator",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
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
        extra_markers: Vec::new(),
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
    let value_node = declarator.child_by_field_name("value");
    let Some(n) = name_node else {
        let children: Vec<SyntaxTree> = declarator
            .named_children()
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
    let type_ir = type_node.map(|t| Box::new(lower_node(t, source).wrap_type()));
    // Wrap the value in a `<value>` SimpleStatement so the post-pass'
    // `wrap_expression_positions` finds it (it scans for `<value>`,
    // `<condition>` etc. and adds the `<expression>` host inside).
    // Java tests assert `value/expression/int='1'`; without the
    // wrapper the value renders as a bare child of `<field>`.
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

fn lower_children(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    node.named_children()
        .map(|c| lower_node(c, source))
        .collect()
}

/// Walk Java `modifiers` block and accumulate flags into the
/// `Modifiers` struct. `default_access` applies when no explicit
/// access modifier (public/private/protected) is found — e.g.
/// package-private fields and methods get `Access::Internal`.
fn lower_java_modifiers(
    node: &RawNode,
    source: &str,
    default_access: Option<Access>,
) -> Modifiers {
    let mut m = Modifiers::default();
    if let Some(da) = default_access {
        m.access = Some(da);
    }
    use crate::tree::types::Flag;
    for c in node.named_children() {
        if c.kind() != "modifiers" {
            continue;
        }
        for tok in c.children() {
            let text = text_of(tok, source);
            let flag = Flag::anchored(range_of(tok), span_of(tok));
            match text.as_str() {
                "public" => m.access = Some(Access::Public),
                "private" => m.access = Some(Access::Private),
                "protected" => m.access = Some(Access::Protected),
                "static" => m.static_ = flag,
                "final" => m.final_ = flag,
                "abstract" => m.abstract_ = flag,
                "synchronized" => m.synchronized_ = flag,
                "native" => m.native = flag,
                "volatile" => m.volatile = flag,
                "transient" => m.transient = flag,
                "strictfp" => m.strictfp = flag,
                "default" => m.default = flag,
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
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: Flag::On { .. }, .. }));
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
            let trailing = if trailing.is_set() || curr_is_trailing {
                Flag::implicit_at(span.line, span.column)
            } else {
                Flag::Off
            };
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
        if let SyntaxTree::Comment { trailing, range, span, .. } = &out[i] {
            if trailing.is_set() {
                continue;
            }
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

/// Walk a (possibly nested) `scoped_identifier` / `scoped_type_identifier`
/// subtree, emitting `SyntaxTree::Name` for each terminal identifier. Mirrors
/// the imperative `flatten_nested_paths` post-walk and the Java post-
/// pass `java_unwrap_type_in_path` (which dropped `<type>` wrappers
/// on path segments) — both are encoded here at lowering time.
fn collect_scoped_segments<'a>(node: &'a RawNode, source: &str, out: &mut Vec<SyntaxTree>) {
    for c in node.named_children() {
        match c.kind() {
            "scoped_identifier" | "scoped_type_identifier" => {
                collect_scoped_segments(c, source, out);
            }
            _ => out.push(lower_node(c, source)),
        }
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

/// Lower Java `throw expr;` so the thrown expression sits under an
/// `<expression>` host (Principle #5, matches Python yield/raise
/// and the existing `<return>` shape — same conceptual role:
/// throw-a-value vs return-a-value).
fn lower_java_throw(node: &RawNode, source: &str) -> SyntaxTree {
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

#[allow(dead_code)]
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
fn enclosing_type_kind(node: &RawNode) -> Option<String> {
    PARENT_MAP.with(|p| {
        let map = p.borrow();
        let mut cur_id = node.id();
        while let Some(&parent_ptr) = map.get(&cur_id) {
            // Safety: parent pointers populated by `populate_parent_map`
            // refer to nodes owned by the `RawNode` tree passed to
            // `lower_java_root`, which outlives this lookup.
            let parent = unsafe { &*parent_ptr };
            match parent.kind() {
                "class_declaration"
                | "interface_declaration"
                | "record_declaration"
                | "enum_declaration"
                | "annotation_type_declaration" => return Some(parent.kind().to_string()),
                _ => cur_id = parent.id(),
            }
        }
        None
    })
}



