//! [`SqlIr`] → canonical source string.
//!
//! Reconstructs syntactically valid SQL source from a `SqlIr` tree
//! WITHOUT consulting the original `source: &str` byte ranges. This
//! is a stronger statement than round-trip identity (`to_source(ir,
//! source) == source`) — it asserts that the IR carries every
//! semantic distinction needed to regenerate equivalent source from
//! scratch.
//!
//! ## Why
//!
//! Per the project principle (added 2026-05-07): if the IR loses any
//! syntactic distinction (e.g. `[dbo]` vs `dbo`, single-quoted vs
//! double-quoted strings) by relying on the source range to
//! reconstruct text, the IR is incomplete. `canonical_source`
//! exercises this — its output may differ in whitespace / comment
//! placement / case from the input but must be *semantically
//! equivalent* (parses to the same `SqlIr`).
//!
//! ## Status
//!
//! Initial slice covers the identifier-class atoms whose quoting
//! style is captured by `QuoteStyle`. Other variants are stubbed —
//! they emit a deterministic but possibly non-roundtripping
//! placeholder. Subsequent iters will fill them in.

#![cfg(feature = "native")]

use super::sql::{QuoteStyle, SqlIr};

/// Reconstruct canonical SQL source from a [`SqlIr`] tree.
///
/// **Invariant:** `parse(canonical_source(parse(s))) == parse(s)` —
/// the canonical text re-parses to the same IR. This is weaker than
/// `canonical_source(parse(s)) == s` (byte-identical round-trip) but
/// stronger than just-not-crashing.
pub fn canonical_source(ir: &SqlIr) -> String {
    match ir {
        // ----- Atoms with quoting --------------------------------------
        SqlIr::Identifier { value, quoting, .. } => quoting.wrap(value),
        SqlIr::Schema { value, quoting, .. } => quoting.wrap(value),
        SqlIr::Alias { value, quoting, .. } => quoting.wrap(value),

        // ----- Other atoms (use range slice for now) -------------------
        // These don't yet capture full syntactic info, so canonical
        // form falls back to the source slice. Future iters extend.
        SqlIr::Variable { range, span: _ } => format!("@{}", range_placeholder(range)),
        SqlIr::Literal { range, .. } => range_placeholder(range),
        SqlIr::Comment { range, .. } => range_placeholder(range),

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
fn range_placeholder(_range: &super::types::ByteRange) -> String {
    "/*range*/".to_string()
}

fn variant_name(ir: &SqlIr) -> &'static str {
    match ir {
        SqlIr::File { .. } => "File",
        SqlIr::Statement { .. } => "Statement",
        SqlIr::Select { .. } => "Select",
        SqlIr::Insert { .. } => "Insert",
        SqlIr::Update { .. } => "Update",
        SqlIr::Delete { .. } => "Delete",
        SqlIr::Compare { .. } => "Compare",
        SqlIr::Where { .. } => "Where",
        SqlIr::From { .. } => "From",
        SqlIr::Relation { .. } => "Relation",
        SqlIr::Reference { .. } => "Reference",
        SqlIr::Column { .. } => "Column",
        SqlIr::Star { .. } => "Star",
        _ => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{ByteRange, Span};

    fn ident(value: &str, q: QuoteStyle) -> SqlIr {
        SqlIr::Identifier {
            value: value.to_string(),
            quoting: q,
            range: ByteRange::new(0, value.len() as u32),
            span: Span::point(1, 1),
        }
    }

    #[test]
    fn bare_identifier_canonical() {
        let ir = ident("dbo", QuoteStyle::None);
        assert_eq!(canonical_source(&ir), "dbo");
    }

    #[test]
    fn bracketed_identifier_canonical() {
        let ir = ident("dbo", QuoteStyle::Brackets);
        assert_eq!(canonical_source(&ir), "[dbo]");
    }

    #[test]
    fn double_quoted_identifier_canonical() {
        let ir = ident("dbo", QuoteStyle::DoubleQuote);
        assert_eq!(canonical_source(&ir), "\"dbo\"");
    }

    #[test]
    fn backticked_identifier_canonical() {
        let ir = ident("dbo", QuoteStyle::Backtick);
        assert_eq!(canonical_source(&ir), "`dbo`");
    }

    #[test]
    fn schema_canonical_uses_quoting() {
        let ir = SqlIr::Schema {
            value: "dbo".into(),
            quoting: QuoteStyle::Brackets,
            range: ByteRange::new(0, 5),
            span: Span::point(1, 1),
        };
        assert_eq!(canonical_source(&ir), "[dbo]");
    }

    #[test]
    fn alias_canonical_uses_quoting() {
        let ir = SqlIr::Alias {
            value: "u".into(),
            quoting: QuoteStyle::None,
            range: ByteRange::new(0, 1),
            span: Span::point(1, 1),
        };
        assert_eq!(canonical_source(&ir), "u");
    }
}
