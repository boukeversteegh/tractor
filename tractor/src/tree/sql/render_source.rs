//! T-SQL: SqlTree → source code (canonical, no-anchor).
//!
//! Per-language entry under `crate::tree::render` for SQL-family
//! languages. Called via [`crate::tree::render::render_sql`] which
//! handles both anchored mode (uses `SqlTree.to_source(s)` for byte-
//! identical output) and from-scratch canonical mode (this module).
//!
//! Reconstructs syntactically valid SQL source from a `SqlTree` tree
//! WITHOUT consulting the original `source: &str` byte ranges. This
//! is a stronger statement than round-trip identity — it asserts
//! that the tree carries every semantic distinction needed to
//! regenerate equivalent source from scratch.
//!
//! ## Why
//!
//! Per the project principle (sql.rs invariant 4): if the tree loses
//! any syntactic distinction (e.g. `[dbo]` vs `dbo`) by relying on
//! the source range to reconstruct text, the tree is incomplete. This
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

use crate::tree::render::common::{identity_escape, write_quoted_scalar};
use crate::tree::sql::SqlTree;
#[cfg(test)]
use crate::tree::sql::QuoteStyle;

/// Render a [`SqlTree`] tree as canonical SQL source text.
///
/// **Invariant:** `parse(render(parse(s))) == parse(s)` — the
/// canonical text re-parses to the same tree. Weaker than byte-
/// identical round-trip but stronger than just-not-crashing.
pub fn render(tree: &SqlTree) -> String {
    let mut out = String::new();
    write(tree, &mut out);
    out
}

fn write(tree: &SqlTree, out: &mut String) {
    match tree {
        // ----- Atoms with quoting --------------------------------------
        // SqlTree Tier 1: identifier-class scalars route through the
        // shared `write_quoted_scalar` primitive (Layer A) so the
        // quoting rule lives in one place across all tree types.
        SqlTree::Identifier { value, quoting, .. }
        | SqlTree::Schema { value, quoting, .. }
        | SqlTree::Alias { value, quoting, .. } => {
            write_quoted_scalar(value, quoting, identity_escape, out);
        }

        // ----- Scalars with stored text (SqlTree Tier 1) --------------
        // Variable: T-SQL @foo. The stored range covers the `@`-prefix
        // identifier; emit `@` plus the bare name component. Today the
        // tree doesn't separate `@` from name, so we slice from the
        // stored range as the name. When the tree gains a typed
        // `name` field, this becomes `format!("@{name}")`.
        SqlTree::Variable { text, .. } => out.push_str(text),
        SqlTree::Literal { text, .. } => out.push_str(text),
        SqlTree::Comment { text, .. } => out.push_str(text),

        // ----- Composite shapes (placeholders for now) ----------------
        // The principle is satisfied for atoms in this slice; composite
        // emission is a stub until per-variant canonical forms are
        // implemented (Tier 3 of the SqlTree work). Output is
        // deterministic but not necessarily re-parseable for every
        // shape yet.
        other => {
            out.push_str(&format!("/*todo:{}*/", variant_name(other)));
        }
    }
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
        let tree = ident("dbo", QuoteStyle::Plain);
        assert_eq!(render(&tree), "dbo");
    }

    #[test]
    fn bracketed_identifier_canonical() {
        let tree = ident("dbo", QuoteStyle::Brackets);
        assert_eq!(render(&tree), "[dbo]");
    }

    #[test]
    fn double_quoted_identifier_canonical() {
        let tree = ident("dbo", QuoteStyle::Double);
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
            quoting: QuoteStyle::Plain,
            range: ByteRange::new(0, 1),
            span: Span::point(1, 1),
        };
        assert_eq!(render(&tree), "u");
    }
}
