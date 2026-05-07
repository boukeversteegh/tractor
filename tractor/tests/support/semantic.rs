//! Shared helpers for semantic-tree XPath shape tests.
//!
//! These are the building blocks used by `tests/semantic_tree.rs` to
//! parse a source snippet, run an XPath query against the resulting
//! semantic tree, and pin down structural claims with helpful failure
//! messages.

use std::sync::Arc;
use tractor::{parse, Match, ParseInput, ParseOptions, TreeMode, XPathEngine, XeeParseResult};

pub fn parse_src(lang: &str, source: &str) -> XeeParseResult {
    parse_src_with_mode(lang, source, None)
}

/// Parse with an explicit tree-mode override — for the rare
/// structure-mode YAML/JSON tests where the default `Data` mode's
/// projection collapses the structural vocabulary.
pub fn parse_src_with_mode(lang: &str, source: &str, tree_mode: Option<TreeMode>) -> XeeParseResult {
    parse(
        ParseInput::Inline { content: source, file_label: "<semantic_tree_test>" },
        ParseOptions {
            language: Some(lang),
            tree_mode,
            ignore_whitespace: false,
            parse_depth: None,
        },
    )
    .expect("parse should succeed")
}

pub fn query(tree: &mut XeeParseResult, xpath: &str) -> Vec<Match> {
    let engine = XPathEngine::new();
    engine
        .query_documents(
            &mut tree.documents,
            tree.doc_handle,
            xpath,
            tree.source_lines.clone(),
            &tree.file_path,
        )
        .unwrap_or_else(|e| panic!("query `{}` failed: {:?}", xpath, e))
}

/// Assert the query matches exactly `expected` nodes. `invariant`
/// names the design rule being enforced — surfaces in the failure
/// message so reviewers know why the assertion exists.
#[track_caller]
pub fn assert_count(tree: &mut XeeParseResult, xpath: &str, expected: usize, invariant: &str) {
    let got = query(tree, xpath).len();
    assert_eq!(
        got, expected,
        "Invariant violated — {}\n  query: `{}`\n  matched {} nodes, expected {}",
        invariant, xpath, got, expected
    );
}

/// Reason-first shape claim — same effect as `assert_count` but the
/// reason reads before the technical XPath, which is much easier to
/// scan in lists of consecutive claims about a single tree.
///
/// Convention: `claim("reason it should hold", tree, xpath, expected)`.
#[track_caller]
pub fn claim(reason: &str, tree: &mut XeeParseResult, xpath: &str, expected: usize) {
    let got = query(tree, xpath).len();
    assert_eq!(
        got, expected,
        "Shape claim violated — {}\n  query: `{}`\n  matched {} nodes, expected {}",
        reason, xpath, got, expected
    );
}

/// Assert the query returns at least one match whose text value
/// equals `expected`.
#[track_caller]
pub fn assert_value(tree: &mut XeeParseResult, xpath: &str, expected: &str, invariant: &str) {
    let matches = query(tree, xpath);
    if matches.is_empty() {
        panic!(
            "Invariant violated — {}\n  query: `{}`\n  returned no matches (expected value {:?})",
            invariant, xpath, expected
        );
    }
    let got = &matches[0].value;
    assert_eq!(
        got, expected,
        "Invariant violated — {}\n  query: `{}`\n  first match value = {:?}, expected {:?}",
        invariant, xpath, got, expected
    );
}

/// Silence unused-Arc warning on platforms that don't see all helpers used.
#[allow(dead_code)]
pub fn _arc_sentinel(_: Arc<Vec<String>>) {}

/// Pass-through. XPath natively allows whitespace anywhere outside
/// string literals, so multi-line indented queries work without any
/// preprocessing. Kept as a thin helper so existing callers can stay
/// unchanged; new code can pass raw strings to `claim`.
pub fn multi_xpath(s: &str) -> String {
    s.to_string()
}
