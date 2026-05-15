//! Parity test for the experimental typed-tree pipeline (Python slice).
//!
//! Approach: take a Python source string, run *both* pipelines, compare
//! the structural shape AND the verbatim text recovery of the produced
//! Xot trees.
//!
//! ## What "parity" means here
//!
//! 1. **Structural parity.** Element-name + nesting + text leaves match
//!    the existing pipeline. Attributes (line/column/kind/field/list)
//!    are intentionally out of scope for this first cut.
//!
//! 2. **Text-content recovery (XPath `string(.)` parity).** For every
//!    test source `s`, the tree-rendered root element's
//!    text-concatenation equals `s`. This is the
//!    `[.='foo()']`-by-source-text invariant: a query like
//!    `//call[.='f(x)']` works on the tree-rendered tree.
//!
//! 3. **Round-trip identity.** `to_source(tree, source) == source` for
//!    every test input — every byte of source is recoverable from the
//!    tree via its byte range.

#![cfg(feature = "native")]

use tractor::tree::{render_to_xot, to_source};
use tractor::languages::python::lower_python_root;
use tractor::{parse, ParseInput, ParseOptions};
use xot::{Node as XotNode, Xot};

/// Render a Xot subtree to a structural string: one line per element,
/// indented by depth, showing element name and *leaf text only* (text
/// on elements that have no element children).
///
/// Inter-child gap text (parens, dots, commas, whitespace, etc.) is
/// hidden in this view because that is exactly the place where the
/// tree pipeline and the existing pipeline diverge: the tree pipeline
/// preserves all gaps for source recovery; the existing pipeline is
/// lossy on chain-inversion punctuation. Hiding gaps lets us assert
/// structural parity on what both pipelines agree about (element
/// names, nesting, leaf text), while the separate text-content
/// invariant verifies that tree's preservation works.
fn structural_view(xot: &Xot, root: XotNode) -> String {
    let mut out = String::new();
    render_structural(xot, root, 0, &mut out);
    out
}

fn render_structural(xot: &Xot, node: XotNode, depth: usize, out: &mut String) {
    if let Some(elem) = xot.element(node) {
        let name = xot.local_name_str(elem.name());
        for _ in 0..depth { out.push_str("  "); }
        out.push_str(name);
        let has_element_child = xot.children(node).any(|c| xot.element(c).is_some());
        if !has_element_child {
            // Pure leaf — show its text. Both pipelines agree here.
            let direct_text: String = xot
                .children(node)
                .filter_map(|c| xot.text_str(c).map(|s| s.to_string()))
                .collect();
            if !direct_text.is_empty() {
                out.push_str(" text=");
                out.push_str(&format!("{:?}", direct_text));
            }
        }
        out.push('\n');
        for child in xot.children(node) {
            if xot.element(child).is_some() {
                render_structural(xot, child, depth + 1, out);
            }
        }
    }
}

/// Concatenate all descendant text of `node` in document order.
/// Equivalent to XPath `string(.)`.
fn text_content(xot: &Xot, node: XotNode) -> String {
    let mut out = String::new();
    walk_text(xot, node, &mut out);
    out
}

fn walk_text(xot: &Xot, node: XotNode, out: &mut String) {
    for child in xot.children(node) {
        if let Some(s) = xot.text_str(child) {
            out.push_str(s);
        }
        if xot.element(child).is_some() {
            walk_text(xot, child, out);
        }
    }
}

/// Run the existing pipeline and return the structural view of its root.
fn current_pipeline_view(source: &str) -> String {
    let result = parse(
        ParseInput::Inline { content: source, file_label: "<test>" },
        ParseOptions { language: Some("python"), ..Default::default() },
    )
    .expect("current pipeline parse");
    let xot = result.documents.xot();
    let doc_node = result.documents.document_node(result.doc_handle).expect("doc");
    let root = if xot.is_document(doc_node) {
        xot.document_element(doc_node).expect("doc element")
    } else {
        doc_node
    };
    structural_view(xot, root)
}

struct XotResult {
    xot: Xot,
    root: XotNode,
}

/// Run the tree pipeline. Returns (structural_view, xot_result).
fn ir_pipeline_view(source: &str) -> (String, XotResult) {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("set python lang");
    let cst = parser.parse(source, None).expect("ts parse");
    let tree = lower_python_root(&tractor::raw::RawNode::from_tree_sitter(cst.root_node(), source), source);

    // Round-trip invariant — assert at every test call.
    let recovered = to_source(&tree, source);
    assert_eq!(
        recovered, source,
        "round-trip identity broken: to_source(tree, s) != s",
    );

    let mut xot = Xot::new();
    let doc_root_name = xot.add_name("_doc_root");
    let doc_root = xot.new_element(doc_root_name);
    render_to_xot(&mut xot, doc_root, &tree, source).expect("render");
    // The structural view starts at the tree-emitted root.
    let ir_root = xot
        .children(doc_root)
        .find(|&c| xot.element(c).is_some())
        .expect("tree root present");
    let view = structural_view(&xot, ir_root);
    (view, XotResult { xot, root: ir_root })
}

fn assert_parity(source: &str, label: &str) {
    let cur = current_pipeline_view(source);
    let (new, ir_xot) = ir_pipeline_view(source);

    // tree invariant 1: lossless source recovery. XPath string(.) on the
    // tree root must equal the source verbatim. This is the
    // `[.='foo()']`-by-source-text contract the user asked for.
    let ir_text = text_content(&ir_xot.xot, ir_xot.root);
    if ir_text != source {
        panic!(
            "tree text-content invariant broken for {label}\n\
             expected (source): {source:?}\n\
             got     (string.):  {ir_text:?}\n\
             ----- tree view -----\n{new}\
             -------------------"
        );
    }

    // Structural parity (leaf-text view, gap-text hidden): tree and the
    // existing pipeline must agree on element names, nesting, and
    // leaf-text contents. Gap text divergence (where tree preserves and
    // existing drops) is *outside* this view by design.
    if cur != new {
        panic!(
            "structural parity divergence for {label}\n\
             ----- source -----\n{source}\n\
             ----- current pipeline -----\n{cur}\
             ----- tree pipeline -----\n{new}\
             -------------------"
        );
    }
}

// ---------------------------------------------------------------------------
// Cases — start with the simplest atoms.
// ---------------------------------------------------------------------------

#[test]
fn integer_literal()  { assert_parity("42\n",      "integer literal"); }

#[test]
fn float_literal()    { assert_parity("3.14\n",    "float literal"); }

#[test]
fn string_literal()   { assert_parity("\"hi\"\n", "string literal"); }

#[test]
fn true_literal()     { assert_parity("True\n",    "true literal"); }

#[test]
fn false_literal()    { assert_parity("False\n",   "false literal"); }

#[test]
fn none_literal()     { assert_parity("None\n",    "none literal"); }

#[test]
fn name_reference()   { assert_parity("foo\n",     "name reference"); }

// ---------------------------------------------------------------------------
// Compound expressions — added incrementally as tree + lowering grow.
// ---------------------------------------------------------------------------

#[test]
fn member_access_simple() { assert_parity("a.b\n", "simple member access"); }

#[test]
fn member_chain_two()     { assert_parity("a.b.c\n", "two-step member chain"); }

#[test]
fn subscript_simple()     { assert_parity("a[0]\n", "simple subscript"); }

#[test]
fn call_no_args()         { assert_parity("f()\n", "call with no args"); }

#[test]
fn call_with_arg()        { assert_parity("f(x)\n", "call with one arg"); }

#[test]
fn binary_add()           { assert_parity("a + b\n", "binary add"); }

#[test]
fn unary_minus()          { assert_parity("-x\n", "unary minus"); }
