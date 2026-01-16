//! Language-specific transform modules
//!
//! Each language owns its complete transform logic.
//! The shared infrastructure (xot_transform) provides only the walker and helpers.

pub mod typescript;
pub mod csharp;
pub mod python;
pub mod go;
pub mod rust_lang;
pub mod java;
pub mod ruby;

use xot::{Xot, Node as XotNode};
use crate::xot_transform::TransformAction;

/// Type alias for language transform functions
pub type TransformFn = fn(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>;

/// Get the transform function for a language
pub fn get_transform(lang: &str) -> TransformFn {
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => typescript::transform,
        "csharp" | "cs" => csharp::transform,
        "python" | "py" => python::transform,
        "go" => go::transform,
        "rust" | "rs" => rust_lang::transform,
        "java" => java::transform,
        "ruby" | "rb" => ruby::transform,
        // Default: passthrough (no transforms)
        _ => passthrough_transform,
    }
}

/// Default passthrough transform - just continues without changes
fn passthrough_transform(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}
