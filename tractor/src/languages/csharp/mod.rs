//! csharp language transform — split into semantic vocabulary
//! and transform logic.

pub mod semantic;
pub mod transform;

pub use transform::{transform, syntax_category, ACCESS_MODIFIERS, OTHER_MODIFIERS};
