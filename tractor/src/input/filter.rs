//! Generic result filtering for tractor operations.
//!
//! The `ResultFilter` trait provides a uniform way to filter matches
//! at the query engine level. Implementations decide which files to
//! process and which matches to include in results.

use tractor_core::Match;

/// A filter that decides which matches to include in results.
///
/// Applied inside the query engine (`query_files_multi`, `run_rules`)
/// so downstream code receives pre-filtered results.
pub trait ResultFilter: Send + Sync {
    /// Returns true if this match should be included in results.
    fn include(&self, m: &Match) -> bool;

    /// Returns true if a file should be processed at all.
    /// Used as an optimization to skip parsing unchanged files.
    fn include_file(&self, _file: &str) -> bool {
        true
    }
}
