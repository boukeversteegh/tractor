//! # Proof of concept: derive an AST-level patch from a minimal pair
//!
//! Given a rule's `valid` and `invalid` example snippets, compute the set of
//! divergence points between the two ASTs. Each divergence point is a
//! proto-SetOp: an xpath-like location in the invalid tree plus the
//! replacement text sliced from the valid tree's source.
//!
//! This POC is built on tractor-core's existing parse pipeline
//! (`parser::parse_string_to_xot`) and the xot helper API
//! (`xot_transform::helpers`) — the same building blocks the set/render
//! machinery already uses. No new parsing, no custom tree abstraction.
//!
//! Later phases:
//!   - Turn the element-name-and-index path into an XPath expression.
//!   - Abstract identifier leaves into $1, $2, … placeholders.
//!   - Plug the derived patch into `xpath_upsert::upsert_typed` as a
//!     structural-replacement SetOp.
//!
//! Run:
//!   cargo test -p tractor-core --test ast_diff_poc -- --nocapture

use tractor_core::parser::{parse_string_to_xot, XotParseResult};
use tractor_core::tree_mode::TreeMode;
use tractor_core::xot_transform::helpers;

use xot::{Node as XotNode, Xot};

// ---------------------------------------------------------------------------
// Diff model
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DiffPoint {
    /// Element-name trail from the root to the divergence node, with the
    /// named-child index at each step. Example:
    ///   compilation_unit/class_declaration[0]/declaration_list[2]
    ///      /method_declaration[1]/block[3]/if_statement[0]/return_statement
    pub path: String,
    pub before_kind: String,
    pub after_kind: String,
    pub before_text: String,
    pub after_text: String,
}

fn diff_csharp(invalid_src: &str, valid_src: &str) -> Vec<DiffPoint> {
    let inv = parse_string_to_xot(invalid_src, "csharp", "<invalid>".into(), Some(TreeMode::Raw))
        .expect("parse invalid");
    let val = parse_string_to_xot(valid_src, "csharp", "<valid>".into(), Some(TreeMode::Raw))
        .expect("parse valid");

    let mut out = Vec::new();
    let mut trail = Vec::new();
    walk(&inv, inv.root, &val, val.root, &mut trail, &mut out);
    out
}

fn walk(
    inv: &XotParseResult,
    a: XotNode,
    val: &XotParseResult,
    b: XotNode,
    trail: &mut Vec<String>,
    out: &mut Vec<DiffPoint>,
) {
    // The xot root is the document node. Descend to its single element
    // child so comparison starts at the real root (compilation_unit for C#).
    let (a, b) = normalize_roots(inv, a, val, b);

    let a_kind = helpers::get_element_name(&inv.xot, a);
    let b_kind = helpers::get_element_name(&val.xot, b);

    // Text / non-element nodes: compare their rendered text content.
    let (a_is_elem, b_is_elem) = (a_kind.is_some(), b_kind.is_some());
    if !a_is_elem || !b_is_elem {
        return; // walker only drives element recursion; text deltas surface
                // as changed leaf elements' source slices above
    }

    let a_kind = a_kind.unwrap();
    let b_kind = b_kind.unwrap();

    let a_children = helpers::get_element_children(&inv.xot, a);
    let b_children = helpers::get_element_children(&val.xot, b);

    if a_kind != b_kind || a_children.len() != b_children.len() {
        let step_kind = if a_kind == b_kind {
            a_kind.clone()
        } else {
            format!("{}|{}", a_kind, b_kind)
        };
        trail.push(step_kind);
        out.push(DiffPoint {
            path: trail.join("/"),
            before_kind: a_kind,
            after_kind: b_kind,
            before_text: source_slice(inv, a).unwrap_or_default(),
            after_text: source_slice(val, b).unwrap_or_default(),
        });
        trail.pop();
        return;
    }

    // Same kind + child count → check leaf text before recursing.
    if a_children.is_empty() {
        let at = full_text(&inv.xot, a);
        let bt = full_text(&val.xot, b);
        if at != bt {
            trail.push(format!("{}[leaf]", a_kind));
            out.push(DiffPoint {
                path: trail.join("/"),
                before_kind: a_kind.clone(),
                after_kind: b_kind.clone(),
                before_text: at,
                after_text: bt,
            });
            trail.pop();
        }
        return;
    }

    // Recurse.
    for (i, (ac, bc)) in a_children.iter().zip(b_children.iter()).enumerate() {
        trail.push(format!("{}[{}]", a_kind, i));
        walk(inv, *ac, val, *bc, trail, out);
        trail.pop();
    }
}

fn normalize_roots(
    inv: &XotParseResult,
    mut a: XotNode,
    val: &XotParseResult,
    mut b: XotNode,
) -> (XotNode, XotNode) {
    if helpers::get_element_name(&inv.xot, a).is_none() {
        if let Some(child) = helpers::get_element_children(&inv.xot, a).into_iter().next() {
            a = child;
        }
    }
    if helpers::get_element_name(&val.xot, b).is_none() {
        if let Some(child) = helpers::get_element_children(&val.xot, b).into_iter().next() {
            b = child;
        }
    }
    (a, b)
}

// ---------------------------------------------------------------------------
// Source slicing (using line/column attributes the builder attaches)
// ---------------------------------------------------------------------------

fn source_slice(pr: &XotParseResult, node: XotNode) -> Option<String> {
    let sl = helpers::get_line(&pr.xot, node, "line")?;
    let sc = helpers::get_attr(&pr.xot, node, "column")?.parse::<usize>().ok()?;
    let el = helpers::get_line(&pr.xot, node, "end_line")?;
    let ec = helpers::get_attr(&pr.xot, node, "end_column")?.parse::<usize>().ok()?;

    // tree-sitter reports 0-based rows; tractor stores them 1-based.
    // Column attrs are stored 1-based as well (see xot_builder).
    let sl = sl.saturating_sub(1);
    let el = el.saturating_sub(1);
    let sc = sc.saturating_sub(1);
    let ec = ec.saturating_sub(1);

    let lines = &pr.source_lines;
    if sl >= lines.len() {
        return None;
    }

    if sl == el {
        let line = &lines[sl];
        let s = sc.min(line.len());
        let e = ec.min(line.len());
        return Some(line[s..e].to_string());
    }

    let mut out = String::new();
    let first = &lines[sl];
    out.push_str(&first[sc.min(first.len())..]);
    for i in (sl + 1)..el.min(lines.len()) {
        out.push('\n');
        out.push_str(&lines[i]);
    }
    if el < lines.len() {
        out.push('\n');
        let last = &lines[el];
        out.push_str(&last[..ec.min(last.len())]);
    }
    Some(out)
}

/// Fallback for leaf elements whose line/col yields empty text — just read
/// whatever text children the node holds.
fn full_text(xot: &Xot, node: XotNode) -> String {
    helpers::get_text_content(xot, node).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Pretty printing
// ---------------------------------------------------------------------------

fn print_report(name: &str, invalid: &str, valid: &str, diffs: &[DiffPoint]) {
    println!("\n========== {} ==========", name);
    println!("--- INVALID ---\n{}", invalid.trim());
    println!("--- VALID ---\n{}", valid.trim());
    println!("--- DERIVED PATCH ({} diff point(s)) ---", diffs.len());
    for (i, d) in diffs.iter().enumerate() {
        println!("[{}] path: {}", i + 1, d.path);
        println!("    kind: {} -> {}", d.before_kind, d.after_kind);
        println!("    BEFORE: {}", one_line(&d.before_text));
        println!("    AFTER : {}", one_line(&d.after_text));
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn poc_always_use_braces() {
    let invalid = "class C { void M(int x) { if (x > 0) return; } }";
    let valid   = "class C { void M(int x) { if (x > 0) { return; } } }";

    let diffs = diff_csharp(invalid, valid);
    print_report("always-use-braces", invalid, valid, &diffs);

    // Expect one divergence: the if-statement's body child is a
    // return_statement on invalid, a block on valid.
    assert_eq!(diffs.len(), 1, "expected a single localized divergence");
    assert_eq!(diffs[0].before_kind, "return_statement");
    assert_eq!(diffs[0].after_kind, "block");
    assert!(diffs[0].after_text.contains('{'));
}

#[test]
fn poc_no_null_comparison() {
    let invalid =
        "class C { void M(object foo) { if (foo == null) throw new System.Exception(); } }";
    let valid =
        "class C { void M(object foo) { foo.IsNotNullOrThrow(); } }";

    let diffs = diff_csharp(invalid, valid);
    print_report("no-null-comparison", invalid, valid, &diffs);

    // Expect one divergence at statement level:
    //   if_statement  ->  expression_statement
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].before_kind, "if_statement");
    assert_eq!(diffs[0].after_kind, "expression_statement");
    assert!(diffs[0].after_text.contains("IsNotNullOrThrow"));
}

#[test]
fn poc_primary_constructor_is_coarse() {
    let invalid = "class C { private readonly Bar _bar; public C(Bar bar) { _bar = bar; } }";
    let valid   = "class C(Bar bar) { }";

    let diffs = diff_csharp(invalid, valid);
    print_report("primary-constructor", invalid, valid, &diffs);

    // Stress case. Multiple coordinated changes that don't factor into
    // clean localized edits — the walker is expected to bail out high in
    // the tree (at or near class_declaration) and emit a coarse patch.
    // We pin "at least one diff" rather than a specific shape.
    assert!(!diffs.is_empty());
    println!(
        "NOTE: primary-constructor produced {} diff point(s) — documented limit case.",
        diffs.len()
    );
}
