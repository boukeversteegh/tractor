//! Shared helpers for semantic-tree XPath shape tests.
//!
//! These are the building blocks used by `tests/semantic_tree.rs` to
//! parse a source snippet, run an XPath query against the resulting
//! semantic tree, and pin down structural claims with helpful failure
//! messages.

use std::sync::Arc;
use tractor::{parse, Match, ParseInput, ParseOptions, XPathEngine, XeeParseResult};

pub fn parse_src(lang: &str, source: &str) -> XeeParseResult {
    parse(
        ParseInput::Inline { content: source, file_label: "<semantic_tree_test>" },
        ParseOptions {
            language: Some(lang),
            tree_mode: None,
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

/// Quote-aware whitespace stripper: collapses ASCII whitespace
/// OUTSIDE of `'…'` and `"…"` string literals so queries can be
/// written with indentation in source. Whitespace inside literals
/// (e.g. `[.='// instance counter']`) is preserved verbatim.
pub fn multi_xpath(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_quote: Option<char> = None;
    for c in s.chars() {
        match in_quote {
            Some(q) => {
                out.push(c);
                if c == q { in_quote = None; }
            }
            None if c == '\'' || c == '"' => {
                out.push(c);
                in_quote = Some(c);
            }
            None if c.is_whitespace() => {}
            None => out.push(c),
        }
    }
    out
}
