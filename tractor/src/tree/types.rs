//! Back-compat re-export of `tree::syntax::types::*`.
//!
//! Historically, every shape type (the `SyntaxTree` enum plus the
//! cross-family types `NodeId`, `Span`, `ByteRange`, `Marker`,
//! `Flag`, `QuoteStyle`, `TreeNode`) lived directly in
//! `tree::types`. They have since moved under
//! [`crate::tree::syntax::types`] so the SyntaxTree-specific layer
//! mirrors the symmetric structure of `tree::data` / `tree::sql`.
//!
//! This module is a thin alias kept so that the many existing
//! `use crate::tree::types::…` imports across `tree::data::*`,
//! `tree::sql::*`, `transform::*`, `languages::*`, and the parser
//! keep compiling unchanged. New code may prefer the explicit
//! `tree::syntax::types::…` path.

pub use super::syntax::types::*;
