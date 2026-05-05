//! Typed semantic-tree IR (experimental).
//!
//! ## Status
//! Parallel implementation under exploration. See
//! `docs/design-transform-redesign-exploration.md` § 11 for the design
//! rationale. The existing `crate::transform` pipeline (in-place Xot
//! mutation) is the production path; this module is a *parity-target*
//! sketch on a small slice (initially: a fragment of Python). When/if it
//! reaches full parity for a language, that language switches to it and
//! the imperative path retires.
//!
//! ## Architecture
//! ```text
//!   tree-sitter CST  ──── lower_<lang>(·)  ───►  Ir  ───── render(·) ─────►  Xot/XML
//! ```
//!
//! - `lower_<lang>` is a *pure function* per language: tree-sitter node
//!   in, [`Ir`] out. Cross-language unification happens at the IR layer
//!   (every language lowers to the *same* IR variants).
//! - `render` is mechanical: walks the IR and emits the corresponding
//!   XML. No decisions live here.
//! - Cross-cutting normalisations (chain inversion, expression-host
//!   wrapping, marker placement) are pure `Ir → Ir` rewrites that fit
//!   between lowering and rendering. None are imperative tree mutations.
//!
//! ## Why not in-place mutation
//! See § 4 of the exploration doc — most accidental costs (custom-handler
//! proliferation, undo-passes, field-wrap-as-side-system,
//! pass-as-imperative) trace back to using the *output* tree as the
//! *workspace* tree. Separating the two reduces those costs to
//! per-language data + a single typed shape.
//!
//! ## What this module deliberately does not have
//! No `Bag(Vec<Ir>)` variant. A bag punctures the contract. Two narrower
//! variants serve the same purpose:
//! - [`Ir::Inline`] — explicit "this CST kind contributes no shape;
//!   inline its children at the parent." Deliberate, named.
//! - [`Ir::Unknown`] — last-resort hatch for an un-handled kind. Renders
//!   as a visible `<unknown kind="…"/>`. Ratchet-able to zero per
//!   language.

pub mod types;
pub mod render;
pub mod python;
pub mod csharp;
#[cfg(feature = "native")]
pub mod coverage;

pub use types::{AccessSegment, ByteRange, Ir, ParamKind, Span, to_source};
pub use render::render_to_xot;
#[cfg(feature = "native")]
pub use python::lower_python_root;
#[cfg(feature = "native")]
pub use csharp::{lower_csharp_root, lower_csharp_node};
#[cfg(feature = "native")]
pub use coverage::{audit_coverage, Coverage, CoverageReport, KindStats};
