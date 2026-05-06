//! IR → source code rendering. Per-language source emitters that walk
//! the typed [`Ir`](crate::ir::Ir) tree and produce source code text.
//!
//! ## Two rendering modes
//!
//! 1. **Anchored** (`render(ir, lang, Some(source))`): Uses the IR's
//!    byte ranges to slice gap text from the original source, the same
//!    mechanism as [`crate::ir::to_source`]. Output is byte-for-byte
//!    identical to the input source.
//!
//! 2. **From-scratch** (`render(ir, lang, None)`): Renders source from
//!    IR alone with canonical formatting rules per language. The
//!    output should `lower_<lang>_root(parse(.))` back to a
//!    structurally-equivalent IR.
//!
//! ## Status
//!
//! **Scaffold only.** This module is not yet wired into the production
//! pipeline. The per-language emitters cover the major IR variants but
//! are not yet exhaustive — atoms (`Name`/`Int`/`String`/...) emit
//! placeholders in canonical mode because their text is only available
//! via the source-anchor (their byte range). The structural
//! scaffolding is the foundation for IR-driven source mutation
//! (`tractor modify --set …`) where structural edits will be combined
//! with anchored byte slicing for unchanged regions.

#![cfg(feature = "native")]
#![allow(dead_code)]

pub mod common;
pub mod csharp;
pub mod java;
pub mod python;
pub mod typescript;
pub mod rust_lang;
pub mod go_lang;
pub mod ruby;
pub mod php;
pub mod tsql;

use super::Ir;

/// Render an IR tree to source code for the named language.
///
/// `source_anchor`: the original source the IR was lowered from. When
/// supplied, gap-text slicing is used to preserve original whitespace
/// / comments / formatting (anchored mode). Pass `None` for canonical
/// from-scratch rendering.
pub fn render(ir: &Ir, lang: &str, source_anchor: Option<&str>) -> String {
    if let Some(source) = source_anchor {
        return super::to_source(ir, source).to_string();
    }
    match lang {
        "csharp" => csharp::render(ir),
        "java" => java::render(ir),
        "python" => python::render(ir),
        "typescript" => typescript::render(ir),
        "rust" => rust_lang::render(ir),
        "go" => go_lang::render(ir),
        "ruby" => ruby::render(ir),
        "php" => php::render(ir),
        "tsql" => tsql::render(ir),
        _ => common::render_generic(ir),
    }
}
