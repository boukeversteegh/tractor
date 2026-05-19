//! tree → source code rendering. One [`render`] function for all
//! tree-supported languages, walking the typed
//! [`SyntaxTree`](crate::tree::SyntaxTree) tree and assembling source
//! text from three sources:
//!
//! 1. **Language structure** — keywords and structural punctuation
//!    driven by the tree's node type and the per-language
//!    [`common::Syntax`] config.
//! 2. **Stored scalar text** — every leaf variant carries the literal
//!    it was lowered from (S13-Z1), so synthetic / programmatically-
//!    built trees render too.
//! 3. **Source gaps** — when an `Option<&str>` source anchor is
//!    supplied and both adjacent nodes carry anchored byte ranges
//!    (S13-Z2), the gap between them is sliced from source to
//!    preserve whitespace / comments / formatting exactly. Otherwise
//!    the renderer falls back to per-language canonical defaults
//!    (S13-Z6 gap-fallback chain).
//!
//! ## Unified dispatch (S13-Z7)
//!
//! `render(tree, lang, Some(source))` and `render(tree, lang, None)`
//! both flow through the same engine. When the entire tree is
//! anchored, the byte-slice fast path
//! ([`crate::tree::to_source`]) is taken as an internal optimisation
//! — but it is never the canonical entry point. Synthetic or partly-
//! synthetic trees flow through the walker, which renders anchored
//! regions from source and synthetic regions from the Syntax config
//! / stored scalar text.

#![cfg(feature = "native")]

pub mod common;
// Per-language canonical-source emitters live under
// `languages/<lang>/render_source.rs`; each exposes
// `pub fn syntax() -> common::Syntax` consumed by the unified walker.
// Data-language renderers (DataTree → JSON / YAML source text) also
// live per-language at `languages/{json,yaml}/render_source.rs`,
// matching the code-language pattern. Their shared helpers are at
// `tree::data::render_common`.
// SQL-family canonical-source emitter is at `tree::sql::render_source`
// — SqlTree has its own engine in [`render_sql`] below.

use super::SyntaxTree;
use common::{Indent, Syntax};

/// Render a [`SyntaxTree`] to source code for the named language.
///
/// `source_anchor`: the original source the tree was lowered from. When
/// supplied, gap-text slicing preserves original whitespace / comments
/// / formatting at anchored positions (S13-Z6). Pass `None` for fully
/// canonical from-scratch rendering — scalar leaves still render their
/// stored text (S13-Z1) so synthetic trees produce real source.
///
/// All three modes (anchored, canonical, mixed) flow through the same
/// engine (S13-Z7). The `Option<&str>` only affects per-gap behaviour,
/// not whole-function dispatch.
pub fn render(tree: &SyntaxTree, lang: &str, source_anchor: Option<&str>) -> String {
    // T-SQL uses `SqlTree`, not `SyntaxTree` — call `render_sql` instead.
    // This arm exists so callers passing "tsql" get a clear panic
    // rather than silently falling through to generic.
    if lang == "tsql" {
        panic!("tsql uses SqlTree; call render_sql instead");
    }

    // Internal fast-path optimisation: when source is supplied AND the
    // entire tree is anchored AND no transforms have been applied, the
    // byte-slice path produces identical output without walking. This
    // is *not* the canonical entry point — it's a private optimisation
    // (S13-Z7); the walker handles the same case correctly, just more
    // slowly.
    if let Some(source) = source_anchor {
        if tree_fully_anchored(tree) {
            return super::to_source(tree, source).to_string();
        }
    }

    let sx = syntax_for(lang);
    let mut out = String::new();
    common::write_ir_with_source(tree, source_anchor, &mut out, Indent::SPACES_4, &sx);
    out
}

/// Per-language [`Syntax`] config lookup. Falls back to
/// [`Syntax::default`] for unknown languages so the walker still
/// produces *something* readable.
fn syntax_for(lang: &str) -> Syntax {
    match lang {
        "csharp" => crate::languages::csharp::render_source::syntax(),
        "java" => crate::languages::java::render_source::syntax(),
        "python" => crate::languages::python::render_source::syntax(),
        "typescript" => crate::languages::typescript::render_source::syntax(),
        "rust" => crate::languages::rust_lang::render_source::syntax(),
        "go" => crate::languages::go::render_source::syntax(),
        "ruby" => crate::languages::ruby::render_source::syntax(),
        "php" => crate::languages::php::render_source::syntax(),
        _ => Syntax::default(),
    }
}

/// True iff every node in the tree carries an anchored byte range.
/// Used by the fast-path optimisation in [`render`] (S13-Z7).
fn tree_fully_anchored(tree: &SyntaxTree) -> bool {
    if !tree.is_anchored() {
        return false;
    }
    tree.children().into_iter().all(tree_fully_anchored)
}

/// Render a SQL [`SqlTree`](crate::tree::sql::SqlTree) tree to source text.
///
/// `source_anchor`: the original source the tree was lowered from. When
/// supplied AND the whole tree is anchored, the byte-slice fast path
/// returns the original bytes verbatim. Otherwise the canonical
/// renderer is used. (SqlTree's source-anchor reach into the walker
/// — equivalent to S13-Z6 for SqlTree — is future work.)
pub fn render_sql(tree: &super::sql::SqlTree, source_anchor: Option<&str>) -> String {
    if let Some(source) = source_anchor {
        return tree.to_source(source).to_string();
    }
    crate::tree::sql::render_source::render(tree)
}
