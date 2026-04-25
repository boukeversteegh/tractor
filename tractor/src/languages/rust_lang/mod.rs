//! rust_lang language transform — split into semantic vocabulary
//! and transform logic.

pub mod semantic;
pub mod transform;

pub use transform::{transform, syntax_category};
