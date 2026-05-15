//! Typed semantic-tree tree (experimental).
//!
//! ## Status
//! Production path for the languages listed below. The legacy
//! `crate::transform::walk_transform` pipeline (in-place Xot
//! mutation) is unreachable for these languages — their
//! `LanguageOps::transform` is `passthrough_transform`; dispatch
//! happens in `crate::parser::parse_with_ir_pipeline_to_xee` (and
//! its `_to_xot` sibling).
//! See `docs/design-transform-redesign-exploration.md` § 11 for the
//! original design rationale.
//!
//! Languages on this tree (`SyntaxTree`):
//! - C# (`csharp` / `cs`)
//! - Python (`python` / `py`)
//! - Java (`java`)
//! - TypeScript / JavaScript (`typescript` / `ts` / `tsx` /
//!   `javascript` / `js` / `jsx`)
//! - Rust (`rust` / `rs`)
//! - Go (`go`)
//! - Ruby (`ruby` / `rb`)
//! - PHP (`php`)
//!
//! Languages on the parallel `DataTree` (see [`data`]):
//! JSON, YAML, TOML, INI, `.env`, Markdown — in their default
//! `Structure` mode.
//!
//! Languages on the parallel `SqlTree` (see [`sql`]):
//! T-SQL (`tsql` / `mssql` / `sql`).
//!
//! Raw tree-sitter mode (`TreeMode::Raw`) bypasses every tree and
//! returns the bare CST through the legacy builder. Languages without
//! an tree family registered (c, cpp, html, css, bash, scala, lua,
//! haskell, ocaml, r, julia) also flow through the legacy builder.
//!
//! ## Architecture
//! ```text
//!   tree-sitter CST  ──── lower_<lang>(·)  ───►  SyntaxTree  ───── render(·) ─────►  Xot/XML
//! ```
//!
//! - `lower_<lang>` is a *pure function* per language: tree-sitter node
//!   in, [`SyntaxTree`] out. Cross-language unification happens at the tree layer
//!   (every language lowers to the *same* tree variants).
//! - `render` is mechanical: walks the tree and emits the corresponding
//!   XML. No decisions live here.
//! - Chain inversion is part of the lowering: every
//!   `lower_<lang>_root` constructs left-deep [`SyntaxTree::Access`] directly
//!   when it encounters chained member / index / call expressions —
//!   no separate post-walk step exists.
//! - Other cross-cutting normalisations (expression-host wrapping,
//!   marker placement) are *target* `SyntaxTree → SyntaxTree` rewrites that fit
//!   between lowering and rendering. Per-language `post_transform`
//!   passes (`languages/{lang}/post_transform.rs`) still mutate the
//!   rendered xot tree for these — acceptable *only* as a
//!   transitional state. Tracked in `TODO.md` slice **S3B**.
//!
//! ## Why not in-place mutation
//! See § 4 of the exploration doc — most accidental costs (custom-handler
//! proliferation, undo-passes, field-wrap-as-side-system,
//! pass-as-imperative) trace back to using the *output* tree as the
//! *workspace* tree. Separating the two reduces those costs to
//! per-language data + a single typed shape.
//!
//! ## What this module deliberately does not have
//! No `Bag(Vec<SyntaxTree>)` variant. A bag punctures the contract. Two narrower
//! variants serve the same purpose:
//! - [`SyntaxTree::Inline`] — explicit "this CST kind contributes no shape;
//!   inline its children at the parent." Deliberate, named.
//! - [`SyntaxTree::Unknown`] — last-resort hatch for an un-handled kind. Renders
//!   as a visible `<unknown kind="…"/>`. Ratchet-able to zero per
//!   language.

pub mod types;
// tree rendering targets — each module renders the tree tree to one
// concrete representation:
// - `to_xot`  : Xot tree (XML).
// - `to_json` : `serde_json::Value` (skips Xot for tree-direct JSON).
// - `source/` : original byte-anchored or canonical source text.
pub mod to_xot;
pub mod to_json;
// Data-language tree — a separate, simpler typed shape for JSON /
// YAML / TOML / INI / .env / Markdown. Format-agnostic: a single
// `DataTree` tree can be rendered to any of XML / JSON / YAML / TOML.
// Per-format lowering + the to_xot / to_json renderers live under
// `tree::data::*` (regrouped in S10C).
pub mod data;
// `SyntaxTree` → `DataTree` projection. Replacing the
// ad-hoc projection in `to_json.rs` per
// `docs/design-projection-pipeline.md`.
#[cfg(feature = "native")]
pub mod to_data;
// SQL-language tree — typed variants per construct. Parallel to
// `SyntaxTree` (programming languages) and `DataTree` (data languages).
// Per-construct lowering, to_xot, to_json, and render_source live
// under `tree::sql::*` (regrouped in S10D). T-SQL CST lowering moved
// to `crate::languages::tsql::lower`.
pub mod sql;
// Shared helpers for `lower_<lang>` modules — text/range/span
// extraction. Centralized to avoid 200+ LOC of duplication
// across 13 per-language lower files.
pub mod lower_helpers;
// Typed passthrough lowering for `TreeKind::None` languages
// (HTML/CSS/C/C++/bash/scala/lua/haskell/ocaml/r/julia) — see the
// module docs for the rationale (S6R).
pub mod lower_raw_passthrough;
// Stamps a stable [`NodeId`] on every node's Span after the tree is
// built — see `docs/design-editable-trees.md`. Run automatically at
// the parser exit point so callers can rely on IDs being present.
pub mod assign_ids;
// find_by_id walkers — bridge an XPath match's `@id` attribute back
// to the typed-tree node. Companion to `assign_ids`.
#[cfg(feature = "native")]
pub mod locator;
#[cfg(feature = "native")]
pub mod render;
#[cfg(feature = "native")]
pub mod coverage;

pub use types::{Access, AccessSegment, ByteRange, Expression, NodeId, SyntaxTree, Modifiers, ParamKind, Span, TreeNode, to_source};
pub use assign_ids::{assign_ids_data, assign_ids_sql, assign_ids_syntax};
#[cfg(feature = "native")]
pub use locator::{find_by_id, parse_id_attr};
pub use to_xot::render_to_xot;
pub use to_json::tree_to_json;
pub use lower_raw_passthrough::{lower_raw_passthrough, lower_raw_passthrough_all};
pub use data::DataTree;
pub use data::lower_json::lower_json_data_root;
pub use data::lower_yaml::lower_yaml_data_root;
pub use data::lower_toml::lower_toml_data_root;
pub use data::lower_ini::lower_ini_data_root;
pub use data::lower_markdown::lower_markdown_data_root;
pub use data::to_xot::{render_data_to_xot_json, render_data_to_xot_keyed};
pub use data::to_json::data_to_json;
#[cfg(feature = "native")]
pub use to_data::{lower_to_data_ir, has_unhandled};
#[cfg(feature = "native")]
pub use coverage::{audit_coverage, Coverage, CoverageReport, KindStats};
