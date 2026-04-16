//! Glob pattern matching, path normalization, and file discovery.

pub mod matching;
pub mod pattern;
pub mod normalized_path;
#[cfg(feature = "native")]
pub mod files;
