//! Ruby tree-sitter CST → tree lowering.
//!
//! Production parser routes Ruby through this lowering end-to-end
//! (see `parser::use_ir_pipeline`). The legacy imperative
//! `languages/ruby/{rules,transformations,transform}.rs` modules
//! were retired alongside this migration.


use crate::raw::RawNode;

use crate::tree::lower_helpers::{
    false_of, float_of, int_of, name_of, range_of, span_of, string_of, text_of, true_of,
};
use crate::tree::types::{AccessSegment, ByteRange, SyntaxTree, Modifiers, Marker};

pub fn lower_ruby_root(root: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => SyntaxTree::Module {
            element_name: "program",
            children: merge_ruby_line_comments(lower_children(root, source), source),
            range, span,
        },
        other => SyntaxTree::Unknown { kind: other.to_string(), range, span },
    }
}

/// Classify Ruby comments. Ruby uses `#` for line comments. Tree-sitter
/// ruby includes the trailing \n in the comment range like rust/go.
fn merge_ruby_line_comments(children: Vec<SyntaxTree>, source: &str) -> Vec<SyntaxTree> {
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
                // Ruby's tree-sitter comment range excludes the trailing \n,
                // so adjacent line comments separated by exactly one newline
                // (the line terminator) should merge.
                let only_one_newline = gap.chars().filter(|&c| c == '\n').count() <= 1
                    && gap.chars().all(|c| c.is_whitespace());
                let prev_text = &source[prev_range.start as usize..prev_range.end as usize];
                let curr_text = &source[range.start as usize..range.end as usize];
                let prev_is_line_comment = prev_text.trim_start().starts_with('#');
                let curr_is_line_comment = curr_text.trim_start().starts_with('#');
                let prev_was_trailing = matches!(out.last(), Some(SyntaxTree::Comment { trailing: true, .. }));
                if only_one_newline
                    && prev_is_line_comment
                    && curr_is_line_comment
                    && !prev_was_trailing
                    && !curr_is_trailing
                {
                    if let Some(SyntaxTree::Comment { range: r, span: s, .. }) = out.last_mut() {
                        r.end = range.end;
                        s.end_line = span.end_line;
                        s.end_column = span.end_column;
                    }
                    continue;
                }
            }
            let trailing = trailing || curr_is_trailing;
            out.push(SyntaxTree::Comment { leading, trailing, range, span });
        } else {
            out.push(child);
        }
    }
    let n = out.len();
    for i in 0..n {
        if let SyntaxTree::Comment { trailing, range, .. } = &out[i] {
            if *trailing { continue; }
            let comment_end = range.end as usize;
            let next = out.iter().skip(i + 1).find(|c| !matches!(c, SyntaxTree::Comment { .. }));
            if let Some(next_ir) = next {
                let next_start = next_ir.range().start as usize;
                let between = &source[comment_end..next_start];
                let newlines = between.chars().filter(|&c| c == '\n').count();
                if newlines <= 1 && between.chars().all(|c| c.is_whitespace()) {
                    if let SyntaxTree::Comment { leading, .. } = &mut out[i] {
                        *leading = true;
                    }
                }
            }
        }
    }
    out
}

pub fn lower_ruby_node(node: &RawNode, source: &str) -> SyntaxTree {
    lower_node(node, source)
}

fn lower_node(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    match node.kind() {
        // ----- Atoms ---------------------------------------------------
        "identifier" | "constant" | "global_variable" | "instance_variable"
        | "class_variable" | "method_identifier"
        | "encoding" | "file" | "line" | "setter" | "subshell"
        | "uninterpreted" => name_of(node, source),

        // Implicit-receiver keywords get their own element name so
        // queries like `//self` and `//super` find every site without
        // colliding with regular identifier references (Principle #5
        // — unified concept per language; the chain-inversion doc's
        // implicit-receiver pattern in design.md).
        "self" => SyntaxTree::Atom {
            element_name: "self",
            text: text_of(node, source),
            range, span,
        },
        "super" => SyntaxTree::Atom {
            element_name: "super",
            text: text_of(node, source),
            range, span,
        },

        "integer" => int_of(node, source),
        "float" | "complex" | "rational" => float_of(node, source),
        "string" => {
            // Ruby strings can have `interpolation` children — for those
            // emit `<string>` with interpolation children. Plain strings
            // stay as a leaf.
            let has_interp = node.named_children()
                .any(|c| c.kind() == "interpolation");
            if !has_interp {
                string_of(node, source)
            } else {
                let children: Vec<SyntaxTree> = node
                    .named_children()
                    .map(|c| lower_node(c, source))
                    .collect();
                SyntaxTree::SimpleStatement {
                    element_name: "string",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children,
                    range, span,
                }
            }
        }
        "character" => string_of(node, source),
        "regex" => simple_statement(node, "regex", source),
        "true" => true_of(node, source),
        "false" => false_of(node, source),
        "nil" => SyntaxTree::SimpleStatement {
            element_name: "nil",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: Vec::new(),
            range, span,
        },
        "comment" => SyntaxTree::Comment { leading: false, trailing: false, range, span },

        // ----- Symbols -------------------------------------------------
        "simple_symbol" | "hash_key_symbol" => simple_statement(node, "symbol", source),
        "delimited_symbol" => simple_statement_marked(node, "symbol", vec![Marker::implicit("delimited")], source),

        // ----- Module / class / method ---------------------------------
        "module" => ruby_method(node, "module", Vec::new(), source),
        "class" => ruby_method(node, "class", Vec::new(), source),
        "singleton_class" => ruby_method(node, "class", vec![Marker::implicit("singleton")], source),
        "method" => ruby_method(node, "method", Vec::new(), source),
        "singleton_method" => ruby_method(node, "method", vec![Marker::implicit("singleton")], source),
        "lambda" => simple_statement(node, "lambda", source),

        // ----- Parameters ----------------------------------------------
        "method_parameters" | "block_parameters" | "lambda_parameters" => {
            // Wrap bare identifier children in `<parameter>` so the
            // shape is uniform with `keyword_parameter` / `optional_parameter`.
            let kids: Vec<SyntaxTree> = node.named_children()
                .map(|c| {
                    if matches!(c.kind(), "identifier") {
                        SyntaxTree::SimpleStatement {
                            element_name: "parameter",
                            modifiers: Modifiers::default(),
                            extra_markers: Vec::new(),
                            children: vec![name_of(c, source)],
                            range: range_of(c),
                            span: span_of(c),
                        }
                    } else {
                        lower_node(c, source)
                    }
                })
                .collect();
            SyntaxTree::Inline {
                children: kids,
                list_name: Some("parameters"),
                range, span,
            }
        }
        "block_parameter" => simple_statement_marked(node, "parameter", vec![Marker::implicit("block")], source),
        "splat_parameter" => simple_statement_marked(node, "parameter", vec![Marker::implicit("splat")], source),
        "hash_splat_parameter" => simple_statement_marked(node, "parameter", vec![Marker::implicit("kwsplat")], source),
        "keyword_parameter" => ruby_param_with_value(node, vec![Marker::implicit("keyword")], source),
        "optional_parameter" => ruby_param_with_value(node, vec![Marker::implicit("default")], source),
        "forward_parameter" => simple_statement_marked(node, "parameter", vec![Marker::implicit("forward")], source),
        "destructured_parameter" => simple_statement_marked(node, "parameter", vec![Marker::implicit("destructured")], source),

        // ----- Control flow --------------------------------------------
        // Ruby `if cond then x elsif c2 then y else z end` is nested in
        // the CST (`if -> alternative=elsif -> alternative=else`). Flatten
        // at lowering time so the tree carries a flat `<if>[else_if][else]`
        // shape — replaces the `collapse_conditionals` post-walk for Ruby.
        "if" | "if_modifier" => lower_ruby_if(node, "if", source),
        "unless" | "unless_modifier" => lower_ruby_if(node, "unless", source),
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
        "begin_block" => simple_statement_marked(node, "block", vec![Marker::implicit("begin")], source),
        "end_block" => simple_statement_marked(node, "block", vec![Marker::implicit("end")], source),
        "do_block" => simple_statement_marked(node, "block", vec![Marker::implicit("do")], source),
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
        "assignment" => lower_ruby_assignment(node, source),
        "operator_assignment" => lower_ruby_operator_assignment(node, source),
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
                (Some(l), Some(r)) if !marker.is_empty() => SyntaxTree::Binary {
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
            let mut op_node = None;
            for c in node.children() {
                if !c.is_named() {
                    op_node = Some(c);
                    break;
                }
            }
            let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
            let op_byte_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
            let operand = node.named_children().next();
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
                Some(o) if !marker.is_empty() => SyntaxTree::Unary {
                    op_text,
                    op_marker: marker,
                    op_range: op_byte_range,
                    operand: Box::new(lower_node(o, source)),
                    extra_markers: Vec::new(),
                    range, span,
                },
                _ => simple_statement(node, "unary", source),
            }
        }
        // Ruby `.foo` and `&.foo` (safe-nav) — every member access is a
        // call. Fold into SyntaxTree::Access mirroring TS/Rust/Go/PHP.
        "call" => {
            let receiver_node = node.child_by_field_name("receiver");
            let method_node = node.child_by_field_name("method");
            let args_node = node.child_by_field_name("arguments");
            let arguments: Vec<SyntaxTree> = match args_node {
                Some(a) => {
                    a.named_children().map(|c| lower_node(c, source)).collect()
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
                        SyntaxTree::Access { receiver, mut segments, .. } => {
                            segments.push(segment);
                            SyntaxTree::Access { receiver, segments, range, span }
                        }
                        other => SyntaxTree::Access {
                            receiver: crate::tree::types::AccessReceiver::from_tree(other, &["self", "super"]),
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
            let kids: Vec<&RawNode> = node.named_children().collect();
            let mut children: Vec<SyntaxTree> = Vec::new();
            for (i, c) in kids.iter().enumerate() {
                let slot = match i {
                    0 => "condition",
                    1 => "then",
                    2 => "else",
                    _ => "expression",
                };
                let inner = lower_node(*c, source);
                children.push(SyntaxTree::SimpleStatement {
                    element_name: slot,
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: vec![SyntaxTree::SimpleStatement {
                        element_name: "expression",
                        modifiers: Modifiers::default(),
                        extra_markers: Vec::new(),
                        children: vec![inner],
                        range: range_of(*c),
                        span: span_of(*c),
                    }],
                    range: range_of(*c),
                    span: span_of(*c),
                });
            }
            SyntaxTree::SimpleStatement {
                element_name: "ternary",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children,
                range, span,
            }
        }
        "range" => lower_ruby_range(node, source),
        "array" => simple_statement(node, "array", source),
        "hash" => simple_statement(node, "hash", source),
        "pair" => simple_statement(node, "pair", source),
        "string_array" => simple_statement_marked(node, "array", vec![Marker::implicit("string")], source),
        "symbol_array" => simple_statement_marked(node, "array", vec![Marker::implicit("symbol")], source),
        "chained_string" => simple_statement_marked(node, "string", vec![Marker::implicit("concatenated")], source),
        "interpolation" => simple_statement(node, "interpolation", source),
        "element_reference" => simple_statement(node, "index", source),
        "scope_resolution" => simple_statement_marked(node, "member", vec![Marker::implicit("static")], source),

        // ----- Patterns ------------------------------------------------
        "pattern" => simple_statement(node, "pattern", source),
        "alternative_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("alternative")], source),
        "array_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("array")], source),
        "as_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("as")], source),
        "expression_reference_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("expression")], source),
        "find_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("find")], source),
        "hash_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("hash")], source),
        "keyword_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("keyword")], source),
        "match_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("match")], source),
        "test_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("test")], source),
        "variable_reference_pattern" => simple_statement_marked(node, "pattern", vec![Marker::implicit("variable")], source),
        "parenthesized_pattern" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // ----- Arguments / spread --------------------------------------
        "argument_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: Some("arguments"),
            range, span,
        },
        "splat_argument" => simple_statement_marked(node, "spread", vec![Marker::implicit("list")], source),
        "hash_splat_argument" => simple_statement_marked(node, "spread", vec![Marker::implicit("dict")], source),
        "block_argument" => simple_statement_marked(node, "argument", vec![Marker::implicit("block")], source),
        "forward_argument" => simple_statement_marked(node, "argument", vec![Marker::implicit("forward")], source),
        "hash_splat_nil" => simple_statement_marked(node, "spread", vec![Marker::implicit("nil")], source),

        // ----- Structural wrappers (flatten) ---------------------------
        "body_statement" | "block_body" | "parenthesized_statements"
        | "string_content" | "escape_sequence" | "bare_string" | "bare_symbol"
        | "heredoc_body" | "heredoc_content" | "heredoc_end"
        | "in" | "left_assignment_list" => SyntaxTree::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },

        // `<<-TEXT` / `<<~TEXT` heredoc opener — the marker that
        // appears in the source where a string literal would go. The
        // body of the heredoc is parsed as a sibling node in
        // tree-sitter-ruby's CST. We lower the opener as a `<string>`
        // leaf so the assignment's right-hand side preserves the
        // source marker text (Goal #7 — Source Reversibility — and
        // the stable-expression-host decision, which says every
        // value-position operand wraps in an `<expression>` host
        // wrapping a concrete leaf).
        "heredoc_beginning" => string_of(node, source),
        "empty_statement" => SyntaxTree::Inline {
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
        "destructured_left_assignment" => simple_statement_marked(node, "left", vec![Marker::implicit("destructured")], source),
        // `operator` is a tree-sitter node for `def +(other)` style
        // operator method names. Lower as `<name>` so the method's name
        // child stays a name leaf.
        "operator" => name_of(node, source),

        other => SyntaxTree::Unknown { kind: other.to_string(), range, span },
    }
}

/// Lower a Ruby keyword/optional parameter with `<name>` + `<value>` slots.
fn ruby_param_with_value(
    node: &RawNode,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let name_node = node.child_by_field_name("name");
    let value_node = node.child_by_field_name("value");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(n) = name_node {
        children.push(name_of(n, source));
    }
    if let Some(v) = value_node {
        let inner = lower_node(v, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![inner],
            range: range_of(v),
            span: span_of(v),
        });
    }
    SyntaxTree::SimpleStatement {
        element_name: "parameter",
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range, span,
    }
}

/// Lower a Ruby `if`/`unless` to a flat tree shape:
/// `<if> condition body* <else_if/>* <else/>? </if>`. Tree-sitter
/// nests the alternatives (`if.alternative = elsif.alternative = else`);
/// this fn walks that chain and emits siblings of the outer `<if>`.
fn lower_ruby_if(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        match c.kind() {
            "elsif" => {
                // Lower the elsif's own non-alternative children as the
                // <else_if>'s children, then continue the chain by
                // recursing into its alternative.
                children.push(flatten_ruby_elsif_chain(c, source));
                if let Some(alt) = ruby_alternative_child(c) {
                    append_ruby_alternative_chain(alt, source, &mut children);
                }
            }
            "else" => {
                children.push(simple_statement(c, "else", source));
            }
            _ => children.push(lower_node(c, source)),
        }
    }
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range,
        span,
    }
}

/// Lower an `elsif` node, EXCLUDING any nested `elsif`/`else`
/// alternative — the caller appends those as siblings.
fn flatten_ruby_elsif_chain(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if matches!(c.kind(), "elsif" | "else") {
            continue;
        }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name: "else_if",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range,
        span,
    }
}

/// Walk a Ruby `if`-alternative chain (an `elsif` or terminal `else`)
/// and append the flattened sequence to `out`.
fn append_ruby_alternative_chain(node: &RawNode, source: &str, out: &mut Vec<SyntaxTree>) {
    match node.kind() {
        "elsif" => {
            out.push(flatten_ruby_elsif_chain(node, source));
            if let Some(alt) = ruby_alternative_child(node) {
                append_ruby_alternative_chain(alt, source, out);
            }
        }
        "else" => {
            out.push(simple_statement(node, "else", source));
        }
        _ => {}
    }
}

/// First named `elsif` / `else` child of an `if` or `elsif` node — the
/// continuation of the alternative chain in tree-sitter-ruby's nested
/// CST shape.
fn ruby_alternative_child<'a>(node: &'a RawNode) -> Option<&'a RawNode> {
    let r = node.named_children()
        .find(|n| matches!(n.kind(), "elsif" | "else"));
    r
}

/// Lower a Ruby while/until loop with `<condition><expression>` and
/// `<body>` slot wrapping.
fn ruby_while_until(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let cond_node = node.child_by_field_name("condition");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(c) = cond_node {
        let inner = lower_node(c, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "condition",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![SyntaxTree::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner],
                range: range_of(c),
                span: span_of(c),
            }],
            range: range_of(c),
            span: span_of(c),
        });
    }
    if let Some(b) = body_node {
        let body_children: Vec<SyntaxTree> = b
            .named_children()
            .map(|s| lower_node(s, source))
            .collect();
        children.push(SyntaxTree::SimpleStatement {
            element_name: "body",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: body_children,
            range: range_of(b),
            span: span_of(b),
        });
    }
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower a Ruby for-in: `for X in items do ... end`.
/// pattern → bare name; value (the `in items`) → `<value><expression>items</expression></value>`;
/// body → `<body>` (do block contents).
fn ruby_for(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let pattern_node = node.child_by_field_name("pattern");
    let value_node = node.child_by_field_name("value");
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(p) = pattern_node {
        children.push(lower_node(p, source));
    }
    if let Some(v) = value_node {
        // The value is an `in` clause — drill into it for the actual iterable.
        let inner = v.named_children().next().unwrap_or(v);
        let inner_ir = lower_node(inner, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "value",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![SyntaxTree::SimpleStatement {
                element_name: "expression",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: vec![inner_ir],
                range: range_of(inner),
                span: span_of(inner),
            }],
            range: range_of(v),
            span: span_of(v),
        });
    }
    if let Some(b) = body_node {
        let body_children: Vec<SyntaxTree> = b
            .named_children()
            .map(|s| lower_node(s, source))
            .collect();
        children.push(SyntaxTree::SimpleStatement {
            element_name: "body",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: body_children,
            range: range_of(b),
            span: span_of(b),
        });
    }
    SyntaxTree::SimpleStatement {
        element_name: "for",
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range, span,
    }
}

/// Lower a Ruby method/singleton_method, wrapping body_statement in `<body>`.
fn ruby_method(
    node: &RawNode,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let body_node = node.child_by_field_name("body");
    let mut children: Vec<SyntaxTree> = Vec::new();
    for c in node.named_children() {
        if let Some(b) = body_node {
            if c.id() == b.id() {
                let body_children: Vec<SyntaxTree> = c
                    .named_children()
                    .map(|s| lower_node(s, source))
                    .collect();
                children.push(SyntaxTree::SimpleStatement {
                    element_name: "body",
                    modifiers: Modifiers::default(),
                    extra_markers: Vec::new(),
                    children: merge_ruby_line_comments(body_children, source),
                    range: range_of(c),
                    span: span_of(c),
                });
                continue;
            }
        }
        children.push(lower_node(c, source));
    }
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range, span,
    }
}

fn simple_statement(node: &RawNode, element_name: &'static str, source: &str) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node.named_children().map(|c| lower_node(c, source)).collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: Vec::new(),
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

fn simple_statement_marked(
    node: &RawNode,
    element_name: &'static str,
    extra_markers: Vec<crate::tree::types::Marker>,
    source: &str,
) -> SyntaxTree {
    let children: Vec<SyntaxTree> = node.named_children().map(|c| lower_node(c, source)).collect();
    SyntaxTree::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers,
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

fn lower_children(node: &RawNode, source: &str) -> Vec<SyntaxTree> {
    node.named_children().map(|c| lower_node(c, source)).collect()
}

/// Ruby `x = expr` — plain assignment. The `=` token is anonymous in
/// tree-sitter-ruby's CST; we locate it by scanning the source between
/// the `left` and `right` fields. Maps to `SyntaxTree::Assign` so the
/// shared `render_tree_assign` projection emits the canonical
/// `<assign>/<left>/<expression>…<op>=</op>…<right>/<expression>…`
/// shape (Principle #19 — role-mixed children wrap in role-named
/// slots; stable-expression-host decision — each value position is
/// wrapped in `<expression>`).
fn lower_ruby_assignment(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let (op_text, op_range) = ruby_locate_assign_eq(node, source, left, right);
    SyntaxTree::Assign {
        targets: left.map(|n| vec![lower_node(n, source)]).unwrap_or_default(),
        type_annotation: None,
        op_text,
        op_range,
        op_markers: Vec::new(),
        values: right.map(|n| vec![lower_node(n, source).wrap_expression()]).unwrap_or_default(),
        range,
        span,
    }
}

/// Ruby `x += expr` and friends. The operator IS a named field here.
fn lower_ruby_operator_assignment(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let op_node = node.child_by_field_name("operator");
    let op_text = op_node.map(|n| text_of(n, source)).unwrap_or_default();
    let op_range = op_node.map(range_of).unwrap_or(ByteRange::empty_at(range.start));
    SyntaxTree::Assign {
        targets: left.map(|n| vec![lower_node(n, source)]).unwrap_or_default(),
        type_annotation: None,
        op_text,
        op_range,
        op_markers: Vec::new(),
        values: right.map(|n| vec![lower_node(n, source).wrap_expression()]).unwrap_or_default(),
        range,
        span,
    }
}

/// Lower a Ruby `range` (1..10 / 1...10) with explicit kind marker
/// + `<from>`/`<to>` slot wrappers. The two anchors play *different*
/// roles (range start vs range end), so role-named slots are required
/// (Principle #19). The kind marker — `<inclusive/>` for `..`,
/// `<exclusive/>` for `...` — exhausts the mutually-exclusive variant
/// set (Principle #9).
fn lower_ruby_range(node: &RawNode, source: &str) -> SyntaxTree {
    let span = span_of(node);
    let range = range_of(node);
    // Operator token is anonymous (`..` or `...`); inclusivity is
    // distinguished by the operator length.
    let kind: Vec<crate::tree::types::Marker> = match node
        .children()
        .find(|c| !c.is_named() && matches!(c.utf8_text(source), ".." | "..."))
        .map(|c| c.utf8_text(source))
    {
        Some("...") => vec![Marker::implicit("exclusive")],
        Some("..") | _ => vec![Marker::implicit("inclusive")],
    };
    let begin = node.child_by_field_name("begin");
    let end = node.child_by_field_name("end");
    let mut children: Vec<SyntaxTree> = Vec::new();
    if let Some(b) = begin {
        let inner = lower_node(b, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "from",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![inner],
            range: range_of(b),
            span: span_of(b),
        });
    }
    if let Some(e) = end {
        let inner = lower_node(e, source);
        children.push(SyntaxTree::SimpleStatement {
            element_name: "to",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![inner],
            range: range_of(e),
            span: span_of(e),
        });
    }
    SyntaxTree::SimpleStatement {
        element_name: "range",
        modifiers: Modifiers::default(),
        extra_markers: kind,
        children,
        range,
        span,
    }
}

/// Locate the `=` token inside a plain Ruby assignment by scanning the
/// source between the `left` and `right` fields.
fn ruby_locate_assign_eq(
    node: &RawNode,
    source: &str,
    left: Option<&RawNode>,
    right: Option<&RawNode>,
) -> (String, ByteRange) {
    let after_left = left.map(|l| l.end_byte()).unwrap_or(node.start_byte());
    let until = right.map(|r| r.start_byte()).unwrap_or(node.end_byte());
    if after_left <= until {
        if let Some(rel) = source[after_left..until].find('=') {
            let abs = after_left + rel;
            return ("=".to_string(), ByteRange::new(abs as u32, (abs + 1) as u32));
        }
    }
    ("".to_string(), ByteRange::empty_at(after_left as u32))
}



