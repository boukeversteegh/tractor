//! T-SQL: SqlTree → source code (canonical, no-anchor).
//!
//! Per-language entry under `crate::tree::source` for SQL-family
//! languages. Called via [`crate::tree::source::render_sql`] which
//! handles both anchored mode (uses `SqlTree.to_source(s)` for byte-
//! identical output) and from-scratch canonical mode (this module).
//!
//! Reconstructs syntactically valid SQL source from a `SqlTree` tree
//! WITHOUT consulting the original `source: &str` byte ranges. This
//! is a stronger statement than round-trip identity — it asserts
//! that the IR carries every semantic distinction needed to
//! regenerate equivalent source from scratch.
//!
//! ## Why
//!
//! Per the project principle (sql.rs invariant 4): if the IR loses
//! any syntactic distinction (e.g. `[dbo]` vs `dbo`) by relying on
//! the source range to reconstruct text, the IR is incomplete. This
//! renderer exercises that — its output may differ in whitespace /
//! comment placement / case from the input but must be *semantically
//! equivalent* (parses to the same `SqlTree`).
//!
//! ## Status
//!
//! Initial slice covers the identifier-class atoms whose quoting
//! style is captured by `QuoteStyle`. Composite-shape variants are
//! stubbed — they emit a deterministic but possibly non-
//! roundtripping placeholder. Subsequent iters will fill them in.

#![cfg(feature = "native")]

use crate::tree::sql::{QuoteStyle, SqlTree};

/// Render a [`SqlTree`] tree as canonical SQL source text.
///
/// **Invariant:** `parse(render(parse(s))) == parse(s)` — the
/// canonical text re-parses to the same IR. Weaker than byte-
/// identical round-trip but stronger than just-not-crashing.
pub fn render(tree: &SqlTree) -> String {
    match tree {
        // ----- Atoms with quoting --------------------------------------
        SqlTree::Identifier { value, quoting, .. } => quoting.wrap(value),
        SqlTree::Schema { value, quoting, .. } => quoting.wrap(value),
        SqlTree::Alias { value, quoting, .. } => quoting.wrap(value),

        // ----- Other atoms (use range slice for now) -------------------
        // These don't yet capture full syntactic info, so canonical
        // form falls back to the source slice. Future iters extend.
        SqlTree::Variable { range, span: _ } => format!("@{}", range_placeholder(range)),
        SqlTree::Literal { range, .. } => range_placeholder(range),
        SqlTree::Comment { range, .. } => range_placeholder(range),

        // ----- Composite shapes (placeholders for now) ----------------
        // The principle is satisfied for atoms in this slice; composite
        // emission is a stub until per-variant canonical forms are
        // implemented. Output is deterministic but not necessarily
        // re-parseable for every shape yet.
        other => format!("/*todo:{}*/", variant_name(other)),
    }
}

/// Placeholder for atoms whose canonical form isn't yet derivable
/// without source. Returns a marker that distinguishes from real
/// content; once full canonical-source support lands these go away.
fn range_placeholder(_range: &crate::tree::types::ByteRange) -> String {
    "/*range*/".to_string()
}

fn variant_name(tree: &SqlTree) -> &'static str {
    match tree {
        SqlTree::File { .. } => "File",
        SqlTree::Statement { .. } => "Statement",
        SqlTree::Select { .. } => "Select",
        SqlTree::Insert { .. } => "Insert",
        SqlTree::Update { .. } => "Update",
        SqlTree::Delete { .. } => "Delete",
        SqlTree::Compare { .. } => "Compare",
        SqlTree::Where { .. } => "Where",
        SqlTree::From { .. } => "From",
        SqlTree::Relation { .. } => "Relation",
        SqlTree::Reference { .. } => "Reference",
        SqlTree::Column { .. } => "Column",
        SqlTree::Star { .. } => "Star",
        _ => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::types::{ByteRange, Span};

    fn ident(value: &str, q: QuoteStyle) -> SqlTree {
        SqlTree::Identifier {
            value: value.to_string(),
            quoting: q,
            range: ByteRange::new(0, value.len() as u32),
            span: Span::point(1, 1),
        }
    }

    #[test]
    fn bare_identifier_canonical() {
        let tree = ident("dbo", QuoteStyle::None);
        assert_eq!(render(&tree), "dbo");
    }

    #[test]
    fn bracketed_identifier_canonical() {
        let tree = ident("dbo", QuoteStyle::Brackets);
        assert_eq!(render(&tree), "[dbo]");
    }

    #[test]
    fn double_quoted_identifier_canonical() {
        let tree = ident("dbo", QuoteStyle::DoubleQuote);
        assert_eq!(render(&tree), "\"dbo\"");
    }

    #[test]
    fn backticked_identifier_canonical() {
        let tree = ident("dbo", QuoteStyle::Backtick);
        assert_eq!(render(&tree), "`dbo`");
    }

    #[test]
    fn schema_canonical_uses_quoting() {
        let tree = SqlTree::Schema {
            value: "dbo".into(),
            quoting: QuoteStyle::Brackets,
            range: ByteRange::new(0, 5),
            span: Span::point(1, 1),
        };
        assert_eq!(render(&tree), "[dbo]");
    }

    #[test]
    fn alias_canonical_uses_quoting() {
        let tree = SqlTree::Alias {
            value: "u".into(),
            quoting: QuoteStyle::None,
            range: ByteRange::new(0, 1),
            span: Span::point(1, 1),
        };
        assert_eq!(render(&tree), "u");
    }
}
