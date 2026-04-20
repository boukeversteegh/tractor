//! Result filters applied inside the query engine.
//!
//! The [`Filters`] struct is a concrete envelope holding every kind of
//! match filter an operation can accumulate. Today that's just
//! `diff_hunks` (a [`DiffHunkFilter`]), but the envelope shape exists so
//! a future filter (e.g. `ignore_tests`) can land as a new `Option<...>`
//! field without reshaping every caller.
//!
//! Applied inside the query engine (`query_files_multi`, `run_rules`)
//! so downstream code receives pre-filtered results.

use tractor::Match;

use super::git::DiffHunkFilter;

/// The set of result filters an operation carries. Each field is an
/// `Option` so "no filter of that kind" is the default.
#[derive(Debug, Clone, Default)]
pub struct Filters {
    /// Line-level filter derived from a git diff spec. When present,
    /// only matches overlapping a changed hunk are kept, and files not
    /// touched by the diff are skipped entirely.
    pub diff_hunks: Option<DiffHunkFilter>,
}

impl Filters {
    /// Returns true if no filters are configured. Equivalent to the
    /// previous `Vec::is_empty` check.
    pub fn is_empty(&self) -> bool {
        self.diff_hunks.is_none()
    }

    /// Returns true if this match should be included in results.
    ///
    /// AND-composes every configured filter — a match is kept only
    /// when every active filter includes it.
    pub fn include(&self, m: &Match) -> bool {
        if let Some(ref f) = self.diff_hunks {
            if !f.include(m) {
                return false;
            }
        }
        true
    }

    /// Returns true if a file should be processed at all. Used as an
    /// optimization to skip parsing files that no active filter cares
    /// about.
    pub fn include_file(&self, file: &str) -> bool {
        if let Some(ref f) = self.diff_hunks {
            if !f.include_file(file) {
                return false;
            }
        }
        true
    }
}
