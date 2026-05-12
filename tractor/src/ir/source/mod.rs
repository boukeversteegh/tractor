//! IR → source code rendering. Per-language source emitters that walk
//! the typed [`SyntaxTree`](crate::ir::SyntaxTree) tree and produce source code text.
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
//! ## Status — pending wiring (TODO slice **S4**)
//!
//! This module is the IR-side reverse renderer, but it is **not yet
//! reachable from the CLI**. While it sits unwired, `tractor render`,
//! `tractor set`, and `tractor update` are stuck on the legacy
//! `crate::render` (XmlNode + `TreeMode::Data`) path, which only
//! supports 3 languages (csharp, json, yaml). Wiring this module is
//! what lifts those commands to every IR-supported language (csharp,
//! python, java, ts/js/tsx/jsx, rust, go, ruby, php, plus tsql via
//! [`render_sql`]).
//!
//! Concrete work items (see `TODO.md`):
//! - **S4A** — `cli/render.rs` calls [`render`] with parsed IR.
//! - **S4B** — `mutation/xpath_upsert.rs` switches its
//!   re-render/span-tracking to anchored IR rendering.
//! - **S4C** — retire `crate::render::{parse_xml, parse_json}` once
//!   no caller reads XmlNode-from-text.
//! - **S4D** — delete `crate::render::{csharp,json,yaml}` once their
//!   callers are gone.
//!
//! The per-language emitters cover the major IR variants today but
//! are not yet exhaustive — atoms (`Name`/`Int`/`String`/...) emit
//! placeholders in canonical mode because their text is only
//! available via the source-anchor (their byte range). Anchored mode
//! is already complete (it slices the original source); canonical
//! mode is the work that remains for from-scratch source generation.

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
// SQL-family languages use their own typed IR (`SqlIr`) and a
// separate renderer entrypoint `render_sql`. Iter 53 retired the
// legacy `SyntaxTree`-based TSQL pipeline.
pub mod sql;
// Data-language IR-direct source emitters (S4B-Z2). Read [`DataIr`]
// directly and produce JSON / YAML text with optional span tracking
// — replaces the [`crate::render`] XmlNode roundtrip for IR-pipeline
// data languages.
pub mod data_json;
pub mod data_yaml;

use super::SyntaxTree;

/// Render an IR tree to source code for the named language.
///
/// `source_anchor`: the original source the IR was lowered from. When
/// supplied, gap-text slicing is used to preserve original whitespace
/// / comments / formatting (anchored mode). Pass `None` for canonical
/// from-scratch rendering.
pub fn render(ir: &SyntaxTree, lang: &str, source_anchor: Option<&str>) -> String {
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
        // T-SQL uses `SqlIr`, not `SyntaxTree` — call `render_sql` instead.
        // This arm exists so users passing "tsql" get a clear panic
        // rather than silently falling through to generic.
        "tsql" => panic!("tsql uses SqlIr; call render_sql instead"),
        _ => common::render_generic(ir),
    }
}

/// Render a SQL [`SqlIr`](crate::ir::sql::SqlIr) tree to source text.
///
/// `source_anchor`: the original source the IR was lowered from. When
/// supplied, gap-text slicing returns the original bytes verbatim
/// (anchored / lossless mode). Pass `None` for canonical from-scratch
/// rendering.
pub fn render_sql(ir: &super::sql::SqlIr, source_anchor: Option<&str>) -> String {
    if let Some(source) = source_anchor {
        return ir.to_source(source).to_string();
    }
    sql::render(ir)
}
