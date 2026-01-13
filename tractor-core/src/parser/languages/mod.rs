//! Language-specific transform configurations
//!
//! Each language has its own file with a `LangTransforms` config.
//!
//! ## Adding a new language
//! 1. Create a new file (e.g., `python.rs`)
//! 2. Define `PYTHON_TRANSFORMS: LangTransforms`
//! 3. Add module declaration here
//! 4. Add to `get_transforms()` match

pub mod typescript;
pub mod csharp;
pub mod python;
pub mod go;
pub mod rust_lang;
pub mod java;

use super::transform::LangTransforms;

// Re-export language configs
pub use typescript::TYPESCRIPT_TRANSFORMS;
pub use csharp::CSHARP_TRANSFORMS;
pub use python::PYTHON_TRANSFORMS;
pub use go::GO_TRANSFORMS;
pub use rust_lang::RUST_TRANSFORMS;
pub use java::JAVA_TRANSFORMS;

/// Get transform configuration for a language
pub fn get_transforms(lang: &str) -> &'static LangTransforms {
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => &TYPESCRIPT_TRANSFORMS,
        "csharp" | "cs" => &CSHARP_TRANSFORMS,
        "python" | "py" => &PYTHON_TRANSFORMS,
        "go" => &GO_TRANSFORMS,
        "rust" | "rs" => &RUST_TRANSFORMS,
        "java" => &JAVA_TRANSFORMS,
        _ => &TYPESCRIPT_TRANSFORMS, // Default
    }
}
