//! Go post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply chain inversion, conditional
//! collapse, closure-body retag, expression-position wrap, multi-
//! target tagging, role tagging, path flattening, brace strip, list
//! distribution.
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 331 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::collapse_conditionals;

/// Go post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
pub fn go_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Invert right-deep `<member>`/`<call>` chains. Go's tree
    // matches the canonical input shape exactly (same as Python),
    // so no normalization step is needed. Run early so subsequent
    // passes see the post-inversion shape.
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    go_retag_singleton_closure_body(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // Go struct fields and shared-type parameters can declare
    // multiple names with one type:
    //   `x, y int` (struct field) → `<field>` with two `<name>` + `<type>`.
    //   `func f(x, y int)` (param) → `<parameter>` with two `<name>` + `<type>`.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("field", "name"),
            ("parameter", "name"),
            // Go multi-return functions: `func f() (int, error)`
            // produce `<returns>` with multiple `<type>` siblings.
            ("returns", "type"),
            // Go multi-target var: `var x, y = 1, 2` produces
            // `<var>` with multiple `<name>` siblings.
            ("var", "name"),
            // Go struct fields: `type T struct { A int; B string }`
            // produces `<struct>` with multiple `<field>` siblings
            // (each a member declaration, role-uniform per
            // Principle #19). Tag so JSON renders `fields: [...]`.
            ("struct", "field"),
            // Go generics with shared constraint — `func F[T, U any]`
            // produces `<generic>` with multiple `<name>` siblings
            // (one per type-parameter name) plus a singleton `<type>`
            // constraint. Tag the names; the type stays singleton.
            ("generic", "name"),
            // Go type switch with multiple types per case —
            // `case int, int32, int64:` produces `<case>` with
            // multiple `<type>` siblings (one per alternative).
            // Role-uniform alternatives per Principle #19.
            ("case", "type"),
            // Go interfaces with multiple methods + type-set elements.
            ("interface", "method"),
            ("interface", "type"),
            // Go multi-value return: `return x, err` produces
            // `<return>` with multiple `<expression>` siblings
            // (after `wrap_expression_positions`). Tag so JSON
            // renders `expressions: [...]` instead of overflowing
            // to `children`. Mirrors Python iter 265.
            ("return", "expression"),
            // Go multi-value var declaration `name, age = "alice", 30`
            // produces `<value>` with multiple `<expression>`
            // siblings. Same archetype as multi-return, scoped to
            // var declarations.
            ("value", "expression"),
            // Go select with multiple cases `select { case ... }`.
            // Multiple `<case>` siblings under `<select>` —
            // role-uniform per Principle #19.
            ("select", "case"),
            // Go switch with multiple cases `switch x { case ... }`.
            // Targeted role tag replaces the bulk-distribute entry on
            // `"switch"` (removed iter 304 — that entry was wrapping
            // the singleton subject `<value>` in a 1-elem array).
            ("switch", "case"),
            // Go if-then with multi-statement body `if cond {
            // stmt1; stmt2 }` produces `<then>` with multiple
            // `<assign>` (or other statement) siblings. Role-
            // uniform.
            ("then", "assign"),
            ("else", "assign"),
            // IR-pipeline additions for Go (cover multi-cardinality
            // children that overflow $children otherwise):
            ("import", "spec"),
            ("var", "value"),
            ("interface", "name"),
            ("type", "name"),
            ("call", "name"),
            ("call", "object"),
            ("method", "name"),
            ("function", "name"),
            ("expression", "name"),
            ("literal", "int"),
        ],
    )?;
    // Go's `if x { ... }` has `<then>` body; strip braces there too.
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::distribute_member_list_attrs(
        // `"array"` removed iter 311 — Go's `<array>` is the array
        // TYPE spec `[5]int` (singleton size + singleton element
        // type). Bulk distribute was creating 1-elem JSON arrays
        // on both. Go has no multi-cardinality `<array>` cases in
        // the blueprint (literals go inside `<literal>/<array>+<body>`
        // — the body holds elements). No targeted tags needed.
        xot, root, &["body", "file", "tuple", "list", "dict", "repetition"],
    )?;
    Ok(())
}

/// Re-tag a `<closure>`'s `<body>` wrapper as `<value>` for
/// single-statement bodies so Go closures match the closure
/// archetype unification (Rust closure / TS arrow / C# lambda /
/// PHP arrow / Python lambda / Ruby Block / Ruby Lambda from iters
/// 161-174). Multi-statement bodies keep `<body>`.
///
/// Runs as a post-pass (not a per-kind Custom handler) because
/// Go's `block` rule is Pure Flatten, which runs DURING the walk —
/// at FuncLiteral-handler time, body still wraps the unflattened
/// block. By post-transform time, body's element children are
/// the actual statements.
///
/// Run BEFORE `wrap_expression_positions` so the new `<value>`
/// slot's first child gets wrapped in `<expression>` automatically
/// (closing iter 174's "all 8 PLs" claim — Go was missed).
fn go_retag_singleton_closure_body(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut closures: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("closure")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut closures);

    let value_id = xot.add_name("value");
    for closure in closures {
        let body = xot.children(closure)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
        let body = match body { Some(b) => b, None => continue };
        let elem_count = xot.children(body)
            .filter(|&c| xot.element(c).is_some())
            .count();
        if elem_count != 1 { continue; }
        if let Some(elem) = xot.element_mut(body) {
            elem.set_name(value_id);
        }
        // Strip stray `{`/`}` text leaves: `strip_body_braces` later
        // in the pipeline only fires on `<body>`-named containers;
        // we just renamed body→value, so handle it here.
        let text_targets: Vec<XotNode> = xot.children(body)
            .filter(|&c| {
                xot.text_str(c)
                    .map(|s| matches!(s.trim(), "{" | "}"))
                    .unwrap_or(false)
            })
            .collect();
        for t in text_targets {
            xot.detach(t)?;
        }
    }
    Ok(())
}
