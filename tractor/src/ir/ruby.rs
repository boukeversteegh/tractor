//! Ruby tree-sitter CST → IR lowering.
//!
//! Production parser routes Ruby through this lowering end-to-end
//! (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/ruby/{rules,transformations,transform}.rs` modules
//! were retired alongside this migration.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::lower_helpers::{range_of, span_of, text_of};
use super::types::{AccessSegment, ByteRange, Ir, Modifiers};

pub fn lower_ruby_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => Ir::Module {
            element_name: "program",
            children: merge_ruby_line_comments(lower_children(root, source), source),
            range, span,
        },
        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

/// Classify Ruby comments. Ruby uses `#` for line comments. Tree-sitter
/// ruby includes the trailing \n in the comment range like rust/go.
fn merge_ruby_line_comments(children: Vec<Ir>, source: &str) -> Vec<Ir> {
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
                // Ruby's tree-sitter comment range excludes the trailing \n,
                // so adjacent line comments separated by exactly one newline
                // (the line terminator) should merge.
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_text = &source[prev_range.start as usize..prev_range.end as usize];
                let curr_text = &source[range.start as usize..range.end as usize];
                let prev_is_line_comment = prev_text.trim_start().starts_with('#');
                let curr_is_line_comment = curr_text.trim_start().starts_with('#');
                let prev_was_trailing = matches!(out.last(), Some(Ir::Comment { trailing: true, .. }));
                if only_one_newline
                    && prev_is_line_comment
                    && curr_is_line_comment
                    && !prev_was_trailing
                    && !curr_is_trailing
                {
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

pub fn lower_ruby_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "identifier" | "constant" | "global_variable" | "instance_variable"
        | "class_variable" | "self" | "method_identifier"
        | "encoding" | "file" | "line" | "setter" | "subshell"
        | "super" | "uninterpreted" => Ir::Name { range, span },

        "integer" => Ir::Int { range, span },
        "float" | "complex" | "rational" => Ir::Float { range, span },
        "string" => {
            // Ruby strings can have `interpolation` children — for those
            // emit `<string>` with interpolation children. Plain strings
            // stay as a leaf.
            let mut cursor = node.walk();
            let has_interp = node.named_children(&mut cursor)
                .any(|c| c.kind() == "interpolation");
            if !has_interp {
                Ir::String { range, span }
            } else {
                let mut cursor2 = node.walk();
                let children: Vec<Ir> = node
                    .named_children(&mut cursor2)
                    .map(|c| lower_node(c, source))
                    .collect();
                Ir::SimpleStatement {
                    element_name: "string",
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children,
                    range, span,
                }
            }
        }
        "character" => Ir::String { range, span },
        "regex" => simple_statement(node, "regex", source),
        "true" => Ir::True { range, span },
        "false" => Ir::False { range, span },
        "nil" => Ir::SimpleStatement {
            element_name: "nil",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range, span,
        },
        "comment" => Ir::Comment { leading: false, trailing: false, range, span },

        // ----- Symbols -------------------------------------------------
        "simple_symbol" | "hash_key_symbol" => simple_statement(node, "symbol", source),
        "delimited_symbol" => simple_statement_marked(node, "symbol", &["delimited"], source),

        // ----- Module / class / method ---------------------------------
        "module" => ruby_method(node, "module", &[], source),
        "class" => ruby_method(node, "class", &[], source),
        "singleton_class" => ruby_method(node, "class", &["singleton"], source),
        "method" => ruby_method(node, "method", &[], source),
        "singleton_method" => ruby_method(node, "method", &["singleton"], source),
        "lambda" => simple_statement(node, "lambda", source),

        // ----- Parameters ----------------------------------------------
        "method_parameters" | "block_parameters" | "lambda_parameters" => {
            // Wrap bare identifier children in `<parameter>` so the
            // shape is uniform with `keyword_parameter` / `optional_parameter`.
            let mut cursor = node.walk();
            let kids: Vec<Ir> = node.named_children(&mut cursor)
                .map(|c| {
                    if matches!(c.kind(), "identifier") {
                        Ir::SimpleStatement {
                            element_name: "parameter",
                            modifiers: Modifiers::default(),
                            extra_markers: &[],
                            children: vec![Ir::Name { range: range_of(c), span: span_of(c) }],
                            range: range_of(c),
                            span: span_of(c),
                        }
                    } else {
                        lower_node(c, source)
                    }
                })
                .collect();
            Ir::Inline {
                children: kids,
                list_name: Some("parameters"),
                range, span,
            }
        }
        "block_parameter" => simple_statement_marked(node, "parameter", &["block"], source),
        "splat_parameter" => simple_statement_marked(node, "parameter", &["splat"], source),
        "hash_splat_parameter" => simple_statement_marked(node, "parameter", &["kwsplat"], source),
        "keyword_parameter" => ruby_param_with_value(node, &["keyword"], source),
        "optional_parameter" => ruby_param_with_value(node, &["default"], source),
        "forward_parameter" => simple_statement_marked(node, "parameter", &["forward"], source),
        "destructured_parameter" => simple_statement_marked(node, "parameter", &["destructured"], source),

        // ----- Control flow --------------------------------------------
        "if" | "if_modifier" => simple_statement(node, "if", source),
        "unless" | "unless_modifier" => simple_statement(node, "unless", source),
        "elsif" => simple_statement(node, "else_if", source),
        "else" => simple_statement(node, "else", source),
        "for" => ruby_for(node, source),
        "while" | "while_modifier" => ruby_while_until(node, "while", source),
        "until" | "until_modifier" => ruby_while_until(node, "until", source),
        "case" => simple_statement(node, "case", source),
        "when" => simple_statement(node, "when", source),
        "case_match" => simple_statement(node, "match", source),
        "in_clause" => simple_statement(node, "in", source),
        "if_guard" => simple_statement(node, "if", source),
        "unless_guard" => simple_statement(node, "unless", source),
        "begin" => simple_statement(node, "begin", source),
        "begin_block" => simple_statement_marked(node, "block", &["begin"], source),
        "end_block" => simple_statement_marked(node, "block", &["end"], source),
        "do_block" => simple_statement_marked(node, "block", &["do"], source),
        "rescue" => simple_statement(node, "rescue", source),
        "rescue_modifier" => simple_statement(node, "rescue", source),
        "ensure" => simple_statement(node, "ensure", source),
        "exception_variable" => simple_statement(node, "variable", source),
        "exceptions" => simple_statement(node, "exceptions", source),
        "return" => simple_statement(node, "return", source),
        "break" => simple_statement(node, "break", source),
        "next" => simple_statement(node, "next", source),
        "redo" => simple_statement(node, "redo", source),
        "retry" => simple_statement(node, "retry", source),
        "yield" => simple_statement(node, "yield", source),
        "block" => simple_statement(node, "block", source),
        "do" => simple_statement(node, "do", source),
        "then" => simple_statement(node, "then", source),

        // ----- Expressions ---------------------------------------------
        "assignment" => simple_statement(node, "assign", source),
        "operator_assignment" => simple_statement(node, "assign", source),
        "binary" => {
            let left = node.child_by_field_name("left").map(|n| lower_node(n, source));
            let right = node.child_by_field_name("right").map(|n| lower_node(n, source));
            let op_node = node.child_by_field_name("operator");
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let marker = match op_text.as_str() {
                "+" => "plus",
                "-" => "minus",
                "*" => "multiply",
                "/" => "divide",
                "%" => "modulo",
                "**" => "power",
                "==" => "equal",
                "!=" => "not_equal",
                "<" => "less",
                "<=" => "less_or_equal",
                ">" => "greater",
                ">=" => "greater_or_equal",
                "<=>" => "spaceship",
                "&&" | "and" => "and",
                "||" | "or" => "or",
                "&" => "bitwise_and",
                "|" => "bitwise_or",
                "^" => "bitwise_xor",
                "<<" => "shift_left",
                ">>" => "shift_right",
                "===" => "case_equal",
                _ => "",
            };
            match (left, right) {
                (Some(l), Some(r)) if !marker.is_empty() => Ir::Binary {
                    element_name: if matches!(op_text.as_str(), "&&" | "||" | "and" | "or") { "logical" } else { "binary" },
                    op_text,
                    op_marker: marker,
                    op_range,
                    left: Box::new(l),
                    right: Box::new(r),
                    range, span,
                },
                _ => simple_statement(node, "binary", source),
            }
        }
        "unary" => {
            // Ruby unary covers `defined? x`, `!x`, `~x`, `-x`, `+x`.
            // Detect the operator from the first unnamed token.
            let mut cursor = node.walk();
            let mut op_node = None;
            for c in node.children(&mut cursor) {
                if !c.is_named() {
                    op_node = Some(c);
                    break;
                }
            }
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let mut cursor2 = node.walk();
            let operand = node.named_children(&mut cursor2).next();
            let marker = match op_text.as_str() {
                "+" => "plus",
                "-" => "minus",
                "!" => "not",
                "~" => "bitwise_not",
                "defined?" => "defined",
                "not" => "not",
                _ => "",
            };
            match operand {
                Some(o) if !marker.is_empty() => Ir::Unary {
                    op_text,
                    op_marker: marker,
                    op_range: op_byte_range,
                    operand: Box::new(lower_node(o, source)),
                    extra_markers: &[],
                    range, span,
                },
                _ => simple_statement(node, "unary", source),
            }
        }
        // Ruby `.foo` and `&.foo` (safe-nav) — every member access is a
        // call. Fold into Ir::Access mirroring TS/Rust/Go/PHP.
        "call" => {
            let receiver_node = node.child_by_field_name("receiver");
            let method_node = node.child_by_field_name("method");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<Ir> = match args_node {
                Some(a) => {
                    let mut ac = a.walk();
                    a.named_children(&mut ac).map(|c| lower_node(c, source)).collect()
                }
                None => Vec::new(),
            };
            // `obj&.method` — safe nav optional marker.
            let optional = source[range.start as usize..range.end as usize].contains("&.");
            match (receiver_node, method_node) {
                (Some(recv), Some(m)) => {
                    let object_ir = lower_node(recv, source);
                    let method_range = range_of(m);
                    let method_span = span_of(m);
                    // Ruby treats every `.foo` as a method call, even
                    // without parens. Always emit AccessSegment::Call.
                    let _ = optional;
                    let segment = AccessSegment::Call {
                        name: Some(method_range),
                        name_span: Some(method_span),
                        arguments,
                        range: ByteRange::new(method_range.start, range.end),
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
                _ => simple_statement(node, "call", source),
            }
        }
        "conditional" => {
            // `cond ? a : b` — emit `<ternary>` with condition/then/else
            // slot wrappers around `<expression>` hosts.
            let mut cursor = node.walk();
            let kids: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
            let mut children: Vec<Ir> = Vec::new();
            for (i, c) in kids.iter().enumerate() {
                let slot = match i {
                    0 => "condition",
                    1 => "then",
                    2 => "else",
                    _ => "expression",
                };
                let inner = lower_node(*c, source);
                children.push(Ir::SimpleStatement {
                    element_name: slot,
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![Ir::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: &[],
                        children: vec![inner],
                        range: range_of(*c),
                        span: span_of(*c),
                    }],
                    range: range_of(*c),
                    span: span_of(*c),
                });
            }
            Ir::SimpleStatement {
                element_name: "ternary",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children,
                range, span,
            }
        }
        "range" => simple_statement(node, "range", source),
        "array" => simple_statement(node, "array", source),
        "hash" => simple_statement(node, "hash", source),
        "pair" => simple_statement(node, "pair", source),
        "string_array" => simple_statement_marked(node, "array", &["string"], source),
        "symbol_array" => simple_statement_marked(node, "array", &["symbol"], source),
        "chained_string" => simple_statement_marked(node, "string", &["concatenated"], source),
        "interpolation" => simple_statement(node, "interpolation", source),
        "element_reference" => simple_statement(node, "index", source),
        "scope_resolution" => simple_statement_marked(node, "member", &["static"], source),

        // ----- Patterns ------------------------------------------------
        "pattern" => simple_statement(node, "pattern", source),
        "alternative_pattern" => simple_statement_marked(node, "pattern", &["alternative"], source),
        "array_pattern" => simple_statement_marked(node, "pattern", &["array"], source),
        "as_pattern" => simple_statement_marked(node, "pattern", &["as"], source),
        "expression_reference_pattern" => simple_statement_marked(node, "pattern", &["expression"], source),
        "find_pattern" => simple_statement_marked(node, "pattern", &["find"], source),
        "hash_pattern" => simple_statement_marked(node, "pattern", &["hash"], source),
        "keyword_pattern" => simple_statement_marked(node, "pattern", &["keyword"], source),
        "match_pattern" => simple_statement_marked(node, "pattern", &["match"], source),
        "test_pattern" => simple_statement_marked(node, "pattern", &["test"], source),
        "variable_reference_pattern" => simple_statement_marked(node, "pattern", &["variable"], source),
        "parenthesized_pattern" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Arguments / spread --------------------------------------
        "argument_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },
        "splat_argument" => simple_statement_marked(node, "spread", &["list"], source),
        "hash_splat_argument" => simple_statement_marked(node, "spread", &["dict"], source),
        "block_argument" => simple_statement_marked(node, "argument", &["block"], source),
        "forward_argument" => simple_statement_marked(node, "argument", &["forward"], source),
        "hash_splat_nil" => simple_statement_marked(node, "spread", &["nil"], source),

        // ----- Structural wrappers (flatten) ---------------------------
        "body_statement" | "block_body" | "parenthesized_statements"
        | "string_content" | "escape_sequence" | "bare_string" | "bare_symbol"
        | "heredoc_beginning" | "heredoc_body" | "heredoc_content" | "heredoc_end"
        | "in" | "left_assignment_list" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "empty_statement" => Ir::Inline {
            children: Vec::new(),
            list_name: None,
            range, span,
        },

        // ----- Inheritance ---------------------------------------------
        "superclass" => simple_statement(node, "extends", source),

        // ----- Aliasing ------------------------------------------------
        "alias" => simple_statement(node, "alias", source),
        "undef" => simple_statement(node, "undef", source),

        // ----- Misc ----------------------------------------------------
        "rest_assignment" => simple_statement(node, "spread", source),
        "right_assignment_list" => simple_statement(node, "right", source),
        "destructured_left_assignment" => simple_statement_marked(node, "left", &["destructured"], source),
        // `operator` is a tree-sitter node for `def +(other)` style
        // operator method names. Lower as `<name>` so the method's name
        // child stays a name leaf.
        "operator" => Ir::Name { range, span },

        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

/// Lower a Ruby keyword/optional parameter with `<name>` + `<value>` slots.
fn ruby_param_with_value(
    node: TsNode<'_>,
    extra_markers: &'static [&'static str],
    source: &str,
) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let name_node = node.child_by_field_name("name");
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
            children: vec![inner],
            range: range_of(v),
            span: span_of(v),
        });
    }
    Ir::SimpleStatement {
        element_name: "parameter",
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range, span,
    }
}

/// Lower a Ruby while/until loop with `<condition><expression>` and
/// `<body>` slot wrapping.
fn ruby_while_until(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(c) = cond_node {
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
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower a Ruby for-in: `for X in items do ... end`.
/// pattern → bare name; value (the `in items`) → `<value><expression>items</expression></value>`;
/// body → `<body>` (do block contents).
fn ruby_for(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let pattern_node = node.child_by_field_name("pattern");
    let value_node = node.child_by_field_name("value");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<Ir> = Vec::new();
    if let Some(p) = pattern_node {
        children.push(lower_node(p, source));
    }
    if let Some(v) = value_node {
        // The value is an `in` clause — drill into it for the actual iterable.
        let mut vc = v.walk();
        let inner = v.named_children(&mut vc).next().unwrap_or(v);
        let inner_ir = lower_node(inner, source);
        children.push(Ir::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: vec![Ir::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: &[],
                children: vec![inner_ir],
                range: range_of(inner),
                span: span_of(inner),
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
        element_name: "for",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

/// Lower a Ruby method/singleton_method, wrapping body_statement in `<body>`.
fn ruby_method(
    node: TsNode<'_>,
    element_name: &'static str,
    extra_markers: &'static [&'static str],
    source: &str,
) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut cursor = node.walk();
    let mut children: Vec<Ir> = Vec::new();
    for c in node.named_children(&mut cursor) {
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
                    children: merge_ruby_line_comments(body_children, source),
                    range: range_of(c),
                    span: span_of(c),
                });
                continue;
            }
        }
        children.push(lower_node(c, source));
    }
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range, span,
    }
}

fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    let mut cursor = node.walk();
    let children: Vec<Ir> = node.named_children(&mut cursor).map(|c| lower_node(c, source)).collect();
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
    let children: Vec<Ir> = node.named_children(&mut cursor).map(|c| lower_node(c, source)).collect();
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
    node.named_children(&mut cursor).map(|c| lower_node(c, source)).collect()
}



