//! Result filters applied inside the query engine.
//!
//! The [`Filters`] struct is a concrete envelope holding every kind of
//! match filter an operation can accumulate. Today that's just
//! `diff_hunks` (a `Vec<DiffHunkFilter>`), but the envelope shape exists
//! so a future filter (e.g. `ignore_tests`) can land as a new field
//! without reshaping every caller.
//!
//! The `diff_hunks` field is a `Vec` so multiple diff-lines specs
//! (e.g. CLI `--diff-lines` + per-op `diff-lines:`) AND-compose: a
//! match must pass EVERY filter in the vec to be kept. This preserves
//! the project-wide invariant that file/line scoping rules at every
//! level are intersectional, never overrides.
//!
//! Applied inside the query engine (`query_files_multi`, `run_rules`)
//! so downstream code receives pre-filtered results.

use tractor::Match;

use super::git::DiffHunkFilter;

/// The set of result filters an operation carries.
#[derive(Debug, Clone, Default)]
pub struct Filters {
    /// Line-level filters derived from git diff specs. When non-empty,
    /// only matches overlapping a changed hunk in EVERY filter are
    /// kept, and files not touched by ALL diffs are skipped entirely.
    ///
    /// Multiple entries AND-compose — one per applicable diff-lines
    /// spec (e.g. global `--diff-lines` plus per-op `diff-lines:`).
    pub diff_hunks: Vec<DiffHunkFilter>,
}

impl Filters {
    /// Returns true if no filters are configured.
    pub fn is_empty(&self) -> bool {
        self.diff_hunks.is_empty()
    }

    /// Returns true if this match should be included in results.
    ///
    /// AND-composes every configured filter — a match is kept only
    /// when every active filter includes it.
    pub fn include(&self, m: &Match) -> bool {
        self.diff_hunks.iter().all(|f| f.include(m))
    }

    /// Returns true if a file should be processed at all. Used as an
    /// optimization to skip parsing files that no active filter cares
    /// about. A file must be covered by EVERY diff-hunks filter.
    pub fn include_file(&self, file: &str) -> bool {
        self.diff_hunks.iter().all(|f| f.include_file(file))
    }
}

// ---------------------------------------------------------------------------
// Tests — intersection semantics for Filters
// ---------------------------------------------------------------------------
//
// Guards the contract that multiple diff-lines specs AND-compose, not
// override. Regression cover for the `Option<DiffHunkFilter>` slot that
// silently dropped the global spec when the per-op spec was set.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tractor::{Match, NormalizedPath};

    use super::super::git::{DiffHunkFilter, LineRange};
    use super::Filters;

    /// Build a `DiffHunkFilter` covering `file` with a single line range.
    fn filter_for(file: &NormalizedPath, start: u32, end: u32) -> DiffHunkFilter {
        let mut hunks = HashMap::new();
        hunks.insert(file.clone(), vec![LineRange { start, end }]);
        DiffHunkFilter::from_hunks(hunks)
    }

    /// Build a `Match` at a specific line range in `file`.
    fn match_at(file: &NormalizedPath, line: u32, end_line: u32) -> Match {
        let m = Match::new(file.as_str().to_string(), "x".into());
        Match { line, end_line, ..m }
    }

    /// Two diff-lines specs intersect: global covers lines 1..=5, per-op
    /// covers 3..=7. A match at line 4 (in both) passes, at line 2
    /// (global only) is rejected, at line 6 (per-op only) is rejected,
    /// and at line 9 (neither) is rejected.
    #[test]
    fn filters_and_compose_overlapping_diff_lines() {
        let file = NormalizedPath::new("/repo/src/foo.rs");

        let filters = Filters {
            diff_hunks: vec![
                filter_for(&file, 1, 5), // simulates global --diff-lines
                filter_for(&file, 3, 7), // simulates per-op diff-lines
            ],
        };

        // Line 4 is in BOTH ranges — kept.
        assert!(filters.include(&match_at(&file, 4, 4)),
            "match in the intersection must pass");

        // Line 2 is only in the global range — rejected by per-op.
        assert!(!filters.include(&match_at(&file, 2, 2)),
            "match outside per-op range must be rejected (not override)");

        // Line 6 is only in the per-op range — rejected by global.
        assert!(!filters.include(&match_at(&file, 6, 6)),
            "match outside global range must be rejected (not override)");

        // Line 9 is outside both — rejected.
        assert!(!filters.include(&match_at(&file, 9, 9)),
            "match outside both ranges must be rejected");
    }

    /// File-level AND-composition: if either filter doesn't cover the
    /// file, it's dropped. This is the pre-parse optimization path.
    #[test]
    fn filters_include_file_requires_all_diff_hunks_to_cover_it() {
        let a = NormalizedPath::new("/repo/src/a.rs");
        let b = NormalizedPath::new("/repo/src/b.rs");

        // Global touches both; per-op only touches a.rs.
        let mut global_hunks = HashMap::new();
        global_hunks.insert(a.clone(), vec![LineRange { start: 1, end: 10 }]);
        global_hunks.insert(b.clone(), vec![LineRange { start: 1, end: 10 }]);

        let mut op_hunks = HashMap::new();
        op_hunks.insert(a.clone(), vec![LineRange { start: 1, end: 10 }]);

        let filters = Filters {
            diff_hunks: vec![
                DiffHunkFilter::from_hunks(global_hunks),
                DiffHunkFilter::from_hunks(op_hunks),
            ],
        };

        // a.rs is covered by both → kept.
        assert!(filters.include_file(a.as_str()));
        // b.rs is covered only by global → dropped (per-op narrows).
        assert!(!filters.include_file(b.as_str()),
            "file not covered by every filter must be dropped");
    }

    /// Sanity: a single filter still works (no-regression for the common
    /// case where only one diff-lines spec is set).
    #[test]
    fn filters_with_single_diff_hunk_works_like_one_layer() {
        let file = NormalizedPath::new("/repo/src/foo.rs");
        let filters = Filters {
            diff_hunks: vec![filter_for(&file, 3, 5)],
        };

        assert!(filters.include(&match_at(&file, 4, 4)));
        assert!(!filters.include(&match_at(&file, 9, 9)));
    }

    /// An empty `Filters` keeps everything.
    #[test]
    fn filters_default_is_permissive() {
        let filters = Filters::default();
        assert!(filters.is_empty());
        let file = NormalizedPath::new("/repo/src/foo.rs");
        assert!(filters.include(&match_at(&file, 1, 1)));
        assert!(filters.include_file(file.as_str()));
    }
}
