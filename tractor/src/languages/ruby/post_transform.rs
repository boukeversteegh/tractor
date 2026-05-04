//! Ruby post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply chain inversion (with flat-
//! call pre-pass), conditional collapse, expression-position wrap,
//! body-value wrap (Ruby has no `expression_statement`), block-body
//! retag, lambda-body collapse, pair-key extract, brace strip, list
//! distribution, and the case/when list-tag pass.
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 333 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::{collapse_conditionals, collect_named_elements};

/// Ruby post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
///
/// Ruby's tree-sitter grammar has no `expression_statement` analog —
/// expressions appear directly under `<body>`. So statement-level
/// host migration handles two layers:
/// 1. slot-level hosts (`left`/`right`/`condition`/`value`/`return`)
/// 2. body-level: walk `<body>` / `<then>` / `<else>` children and
///    wrap value-producing kinds in `<expression>`. Ruby has implicit
///    return — every method body's last expression IS the return
///    value — so value-producing children of body containers are
///    real expression positions and should carry the host.
pub fn ruby_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Ruby uses Java's flat call shape (`<call><object/>NAME...</call>`).
    // Wrap object+name into canonical `<member>` callee, then invert.
    crate::transform::chain_inversion::wrap_flat_call_member(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    // Ruby destructured params `proc { |(x, y)| ... }` produce a
    // `<parameter[destructured]>` with multiple `<name>` siblings.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("parameter", "name"),
            // Alternative patterns `1 | 2 | 3` produce
            // `<pattern[alternative]>` with multiple `<int>` (or
            // `<string>`/`<name>`) siblings. Per Principle #19
            // they're role-uniform — each is one alternative
            // option. Tag so JSON renders e.g. `ints: [...]`
            // instead of overflowing to `children`. Cardinality
            // discriminator (>=2) keeps singleton patterns alone.
            ("pattern", "int"),
            ("pattern", "string"),
            ("pattern", "name"),
            // Ruby interpolated strings: `<string>` parent with one or
            // more `<interpolation>` chunks. Bulk-distribute on
            // `"string"` (removed iter 309) was wrapping single-interp
            // cases in 1-elem JSON arrays.
            ("string", "interpolation"),
            // Ruby concatenated strings `"a" "b" "c"` —
            // `<string[concatenated]>` parent with multiple
            // `<string>` children. (Ruby has no
            // `tag_multi_same_name_children` call; cover this case
            // with the targeted role tag.)
            ("string", "string"),
            // Ruby array literals `[1, 2, 3]` etc. Iter 323 dropped
            // `"array"` from the bulk distribute config (was wrapping
            // singleton spread `[*items]` cases in 1-elem JSON arrays).
            // These targeted role tags cover the multi-cardinality
            // element types that exist in the blueprint.
            ("array", "int"),
            ("array", "name"),
            ("array", "object"),
        ],
    )?;
    crate::transform::wrap_body_value_children(
        xot,
        root,
        &["body", "then", "else"],
        RUBY_VALUE_KINDS,
    )?;
    ruby_retag_singleton_block_body(xot, root)?;
    ruby_collapse_lambda_body(xot, root)?;
    ruby_extract_pair_keys(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    ruby_tag_case_when_lists(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        // `"array"` removed iter 323 — was wrapping singleton `<spread>`
        // children of `[*items]` arrays in 1-elem JSON arrays.
        // Targeted role tags above cover the multi-cardinality cases
        // (int/name/object).
        xot, root, &["body", "program", "tuple", "list", "dict", "hash", "repetition"],
    )?;
    Ok(())
}

/// Tag the multi-instance role children of Ruby's pattern-match
/// constructs (`<case>`, `<when>`, `<match>`) with `list=` so JSON
/// consumers see them as arrays:
/// - `<case>` → `<when>` children → `list="when"` (case branches; multi).
/// - `<when>` → `<pattern>` children → `list="pattern"` (multi-pattern
///   `when X, Y` lifts each as a sibling).
/// - `<match>` → `<in>` children → `list="in"` (Ruby 3.0+
///   pattern-match `case x in ... in ... end`; multi-arm).
///
/// `distribute_member_list_attrs` would over-tag siblings that are
/// role-MIXED (e.g. `<case>`'s `<value>` discriminant and `<else>`,
/// which are singletons). Per Principle #19, we hand-pick the
/// roles that genuinely repeat.
fn ruby_tag_case_when_lists(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_attr, get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    fn collect(xot: &Xot, node: XotNode, name: &str, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some(name)
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, name, out);
        }
    }

    // `<case>` → tag `<when>` children with list="when".
    let mut cases: Vec<XotNode> = Vec::new();
    collect(xot, root, "case", &mut cases);
    for case in cases {
        let whens: Vec<XotNode> = xot.children(case)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("when")
            })
            .collect();
        for w in whens {
            if get_attr(xot, w, "list").is_none() {
                xot.with_attr(w, "list", "whens");
            }
        }
    }

    // `<when>` → tag `<pattern>` children with list="pattern".
    let mut whens: Vec<XotNode> = Vec::new();
    collect(xot, root, "when", &mut whens);
    for w in whens {
        let patterns: Vec<XotNode> = xot.children(w)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("pattern")
            })
            .collect();
        for p in patterns {
            if get_attr(xot, p, "list").is_none() {
                xot.with_attr(p, "list", "patterns");
            }
        }
    }

    // `<match>` → tag `<in>` children with list="in" (Ruby 3.0+
    // `case x in pat1 then ... in pat2 then ... end` pattern-match).
    let mut matches: Vec<XotNode> = Vec::new();
    collect(xot, root, "match", &mut matches);
    for m in matches {
        let ins: Vec<XotNode> = xot.children(m)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("in")
            })
            .collect();
        for i in ins {
            if get_attr(xot, i, "list").is_none() {
                xot.with_attr(i, "list", "ins");
            }
        }
    }

    Ok(())
}

/// Re-tag a `<body>` wrapper as `<value>` when its parent is a
/// `<block>` (call-attached closure: `arr.each { |x| ... }`,
/// `proc { ... }`, `arr.each do |x| ... end`) AND the body has
/// exactly one element child. This brings call-attached closures
/// into the iter 161/162/167/168 closure-body archetype:
/// `block/value/expression/...` for single-statement bodies;
/// multi-statement bodies keep `<body>` so per-statement `list=`
/// distribution remains visible.
///
/// Runs as a post-pass (not in a per-kind Custom handler) because
/// the count must be taken AFTER `block_body` / `body_statement`
/// flatten and AFTER `wrap_body_value_children` wraps value-
/// producing kids in `<expression>`. Doing this at walk-time would
/// always see "1 element child" (the unflattened block_body
/// wrapper), retagging multi-statement blocks too — bug fixed in
/// this iter (was iter 169).
///
/// Lambda's outer `<body>` (whose parent is `<lambda>`, not
/// `<block>`) is NOT touched here — see backlog item for Lambda
/// outer-body collapse.
fn ruby_retag_singleton_block_body(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("block")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut targets);

    let value_id = xot.add_name("value");
    let expr_id = xot.add_name("expression");
    for block in targets {
        let body_child = xot.children(block)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
        let body = match body_child { Some(b) => b, None => continue };
        let elem_children: Vec<XotNode> = xot.children(body)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if elem_children.len() != 1 { continue; }
        let only_child = elem_children[0];
        if let Some(elem) = xot.element_mut(body) {
            elem.set_name(value_id);
        }
        // wrap_body_value_children handled value-producing kinds at step 3
        // (they're already inside `<expression>`). Non-value-producing
        // single statements (`<if>`, `<break>`, `<while>`, …) need an
        // `<expression>` host now that they live in a `<value>` slot
        // (Principle #15). Idempotent: skip when already an expression.
        if get_element_name(xot, only_child).as_deref() != Some("expression") {
            let host = xot.new_element(expr_id);
            xot.with_wrap_child(only_child, host)?;
        }
    }
    Ok(())
}

/// Collapse the doubled-body shape produced by `->(x) { ... }` /
/// `->(x) do ... end`-style stabby lambdas into the
/// closure-archetype shape used by other languages
/// (`lambda/value/expression/...` for single-stmt; `lambda/body/...`
/// multi-stmt).
///
/// Tree-sitter's grammar nests two `<body>` levels for stabby
/// lambdas: one from field-wrapping `lambda.body` (outer), one from
/// field-wrapping `block.body` (inner). The inner block element
/// (`<block>` from `RubyKind::Block` Passthrough) sits between them,
/// carrying the literal `{` `}` text leaves. After
/// `ruby_retag_singleton_block_body`, the inner block contains
/// either `<value>` (single-stmt) or `<body>` (multi-stmt).
///
/// This pass lifts that inner element up to replace the outer
/// `body/block` chain, producing:
/// - single-stmt `->(x) { x + 1 }` → `lambda/value/expression/binary/...`
///   (matches Rust closure / TS arrow / C# lambda / PHP arrow / Python
///    lambda from iters 161/162/167/168).
/// - multi-stmt `->(x) { puts x; x + 1 }` → `lambda/body/expression: [..., ...]`
///   (mirrors Ruby Block multi-stmt shape from iter 173).
///
/// Note: `lambda do ... end` is parsed as a `call` to the `lambda`
/// method with an attached `<do_block>`, NOT as a `<lambda>`
/// element — handled by the iter-173 call-attached path.
fn ruby_collapse_lambda_body(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut lambdas: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("lambda")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut lambdas);

    for lambda in lambdas {
        let outer_body = xot.children(lambda)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
        let outer_body = match outer_body { Some(b) => b, None => continue };

        let body_elem_children: Vec<XotNode> = xot.children(outer_body)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if body_elem_children.len() != 1 { continue; }
        let block = body_elem_children[0];
        if get_element_name(xot, block).as_deref() != Some("block") { continue; }

        let block_elem_children: Vec<XotNode> = xot.children(block)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if block_elem_children.len() != 1 { continue; }
        let inner = block_elem_children[0];
        let inner_name = get_element_name(xot, inner);
        if !matches!(inner_name.as_deref(), Some("value") | Some("body")) { continue; }

        // Lift: detach inner from block, insert before outer_body, detach outer_body.
        // The block element (and any text leaves it contained, like `{` / `}`)
        // is dropped — this is structural, source-text fidelity is advisory.
        xot.detach(inner)?;
        xot.insert_before(outer_body, inner)?;
        xot.detach(outer_body)?;
    }
    Ok(())
}

/// Within-language Principle #5: every Ruby pair should expose its
/// key as a structured child (not bare text). Three source forms:
///   1. `id: value`     — key is shorthand symbol; tree-sitter emits
///                         `"id:"` as a single text leaf.
///   2. `'k' => value`  — key is a string literal; the `=>` is a
///                         bare text leaf between key and value.
///   3. `:foo => value` — key is an explicit symbol; tree-sitter
///                         emits `":foo =>"` as a single text leaf.
///
/// Extract the key into a proper `<name>` (form 1) or `<symbol>`
/// (form 3) element, and strip the `=>` text (form 2). Source-text
/// preservation is given up for queryability — Ruby pairs become
/// uniformly `<pair><name|symbol|string>K</...><value>V</value></pair>`.
fn ruby_extract_pair_keys(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let mut pairs: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "pair", &mut pairs);

    for pair in pairs {
        // Inspect text leaves and strip arrow-only ones.
        let children: Vec<XotNode> = xot.children(pair).collect();
        for child in &children {
            let trimmed = match xot.text_str(*child) {
                Some(t) => t.trim().to_string(),
                None => continue,
            };
            // Form 2: bare `=>` text — strip.
            if trimmed == "=>" {
                xot.detach(*child)?;
                continue;
            }
            // Form 3: ":foo =>" — extract symbol foo, strip arrow.
            if trimmed.starts_with(':') && trimmed.ends_with("=>") {
                let key_part = trimmed
                    .trim_start_matches(':')
                    .trim_end_matches("=>")
                    .trim()
                    .to_string();
                if !key_part.is_empty() {
                    let symbol_elt = xot.add_name("symbol");
                    let symbol_node = xot.new_element(symbol_elt);
                    let key_text = xot.new_text(&key_part);
                    xot.append(symbol_node, key_text)?;
                    xot.insert_before(*child, symbol_node)?;
                }
                xot.detach(*child)?;
                continue;
            }
            // Form 1: "id:" — extract bare name, strip trailing `:`.
            if trimmed.ends_with(':') && !trimmed.starts_with(':') {
                let key_part = trimmed
                    .trim_end_matches(':')
                    .trim()
                    .to_string();
                if !key_part.is_empty() && !key_part.contains(char::is_whitespace) {
                    let name_elt = xot.add_name("name");
                    let name_node = xot.new_element(name_elt);
                    let key_text = xot.new_text(&key_part);
                    xot.append(name_node, key_text)?;
                    xot.insert_before(*child, name_node)?;
                }
                xot.detach(*child)?;
                continue;
            }
        }
        let _ = get_element_name;
    }
    Ok(())
}

/// Element names that are value-producing in Ruby and should be
/// wrapped in `<expression>` when they appear as direct children of
/// a body-level container (`<body>`, `<then>`, `<else>`). Names NOT in
/// this list are statement-only (declarations, control flow, jump
/// statements, comments) and are left bare.
const RUBY_VALUE_KINDS: &[&str] = &[
    // Calls / member access / indexing — function results are values.
    "call", "member", "index", "lambda", "yield",
    // Operator expressions
    "binary", "unary", "conditional", "range", "match",
    // Literals
    "string", "symbol", "int", "float", "regex",
    "true", "false", "nil", "self",
    "array", "hash", "pair",
    // Identifiers / references
    "name",
];
