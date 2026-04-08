//! Centralized file resolver: glob expansion, intersection, exclusion, and limits.
//!
//! This module owns all file resolution logic for `tractor run`. It replaces
//! the former `pipeline::files` module, consolidating `SharedFileScope`,
//! `resolve_files()`, `resolve_op_files()`, `build_filters()`, and
//! `apply_diff_files_filter()` into a single `FileResolver` struct.
//!
//! The resolver is constructed once from `ExecuteOptions`, pre-computing shared
//! state (root files, CLI files, global diff). Each operation then calls
//! `resolve()` with a `FileRequest` to get its resolved file set.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tractor_core::{expand_globs_checked, filter_supported_files, normalize_path};
use tractor_core::report::{ReportBuilder, ReportMatch, Severity, DiagnosticOrigin};

use crate::executor::ExecuteOptions;
use crate::filter::ResultFilter;
use crate::pipeline::git;

// ---------------------------------------------------------------------------
// FileRequest
// ---------------------------------------------------------------------------

/// Describes what files an operation needs. The FileResolver decides how
/// to resolve them (glob expansion, intersection, filtering).
pub(crate) struct FileRequest<'a> {
    /// Operation-level glob patterns (from operation `files:` field).
    pub files: &'a [String],
    /// Exclude patterns (union of root + operation excludes).
    pub exclude: &'a [String],
    /// Per-operation diff-files spec (e.g. "HEAD~3").
    pub diff_files: Option<&'a str>,
    /// Per-operation diff-lines spec.
    pub diff_lines: Option<&'a str>,
    /// Command name for diagnostics (e.g. "check", "query").
    pub command: &'a str,
}

// ---------------------------------------------------------------------------
// FileResolver
// ---------------------------------------------------------------------------

/// Centralized file resolver that owns all glob expansion, intersection,
/// exclusion, and filtering. Constructed once from ExecuteOptions, then
/// called per-operation with a FileRequest.
pub(crate) struct FileResolver {
    /// Expanded root-level files (from config `files:`).
    /// `None` when the config key was missing (unrestricted).
    /// `Some(empty)` when explicitly `files: []`.
    root_files: Option<HashSet<String>>,
    /// Expanded CLI file args, or None if not provided.
    cli_files: Option<HashSet<String>>,
    /// Expanded global diff-files set, or None if not provided.
    global_diff_files: Option<HashSet<PathBuf>>,
    // Resolution parameters (copied from ExecuteOptions for self-containment)
    verbose: bool,
    base_dir: Option<PathBuf>,
    max_files: usize,
    global_diff_lines: Option<String>,
}

impl FileResolver {
    /// Build from ExecuteOptions, expanding shared globs once.
    /// Normalizes all paths to forward slashes (fix #98).
    /// Returns Err with a fatal diagnostic message on expansion failure.
    pub fn new(options: &ExecuteOptions) -> Result<Self, String> {
        let expansion_limit = options.max_files * 10;

        let format_patterns = |patterns: &[String]| -> String {
            patterns.iter().map(|g| format!("\"{}\"", g)).collect::<Vec<_>>().join(", ")
        };

        let base_dir_display = options.base_dir.as_ref()
            .map(|b| b.display().to_string())
            .unwrap_or_else(|| ".".to_string());

        let relative_path = |path: &str| -> String {
            if let Some(base) = &options.base_dir {
                let base_str = base.display().to_string();
                path.strip_prefix(&base_str)
                    .and_then(|p| p.strip_prefix('\\').or(p.strip_prefix('/')))
                    .unwrap_or(path)
                    .to_string()
            } else {
                path.to_string()
            }
        };
        let log_files = |files: &[String]| {
            for f in files.iter().take(5) {
                eprintln!("    {}", relative_path(f));
            }
            if files.len() > 5 {
                eprintln!("    ... and {} more", files.len() - 5);
            }
        };

        if options.verbose {
            eprintln!("  files: resolving relative to {}", base_dir_display);
            eprintln!("  files: max {} files, expansion limit {}", options.max_files, expansion_limit);
        }

        // --- Root scope (fix #99) ---
        // None = key missing → unrestricted; Some([]) = explicit empty; Some([...]) = expand
        let root_files = match &options.config_root_files {
            Some(patterns) if !patterns.is_empty() => {
                if options.verbose {
                    eprintln!("  files: expanding root scope {} ...",
                        format_patterns(patterns));
                }
                let root_globs = resolve_globs_to_absolute(&options.base_dir, patterns);
                let expansion = expand_globs_checked(&root_globs, expansion_limit)
                    .map_err(|e| {
                        let base_hint = options.base_dir.as_ref()
                            .map(|b| format!(" (resolved relative to {})", b.display()))
                            .unwrap_or_default();
                        format!(
                            "root pattern \"{}\" expanded to over {} paths{} — use a more specific pattern or increase --max-files",
                            e.pattern, e.limit, base_hint
                        )
                    })?;
                if options.verbose {
                    eprintln!("  files: root scope has {} file(s)", expansion.files.len());
                    log_files(&expansion.files);
                }
                Some(expansion.files.into_iter()
                    .map(|f| normalize_path(&f)).collect())
            }
            Some(_) => Some(HashSet::new()), // files: [] → explicit empty
            None => None,                     // key missing → unrestricted
        };

        // --- CLI files (fix #98: normalize paths) ---
        let cli_files = if !options.cli_files.is_empty() {
            if options.verbose {
                eprintln!("  files: expanding CLI args {} (relative to cwd) ...",
                    format_patterns(&options.cli_files));
            }
            let expansion = expand_globs_checked(&options.cli_files, expansion_limit)
                .map_err(|e| format!(
                    "CLI pattern \"{}\" expanded to over {} paths — use a more specific pattern or increase --max-files",
                    e.pattern, e.limit
                ))?;
            if options.verbose {
                eprintln!("  files: CLI args have {} file(s)", expansion.files.len());
            }
            Some(expansion.files.into_iter()
                .map(|f| {
                    let p = Path::new(&f);
                    if p.is_absolute() {
                        normalize_path(&f)
                    } else if let Ok(cwd) = std::env::current_dir() {
                        normalize_path(&cwd.join(&f).to_string_lossy())
                    } else {
                        normalize_path(&f)
                    }
                }).collect())
        } else {
            None
        };

        // --- Global diff-files ---
        let global_diff_files = if let Some(ref spec) = options.diff_files {
            let cwd = options.base_dir.as_deref()
                .unwrap_or_else(|| Path::new("."));
            match git::git_changed_files(spec, cwd) {
                Ok(changed) => {
                    if options.verbose {
                        eprintln!("  files: git diff \"{}\" has {} changed file(s)", spec, changed.len());
                    }
                    Some(changed.into_iter().collect())
                }
                Err(e) => {
                    eprintln!("warning: --diff-files filter failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(FileResolver {
            root_files,
            cli_files,
            global_diff_files,
            verbose: options.verbose,
            base_dir: options.base_dir.clone(),
            max_files: options.max_files,
            global_diff_lines: options.diff_lines.clone(),
        })
    }

    /// The base directory used for resolving relative paths.
    pub fn base_dir(&self) -> Option<&Path> {
        self.base_dir.as_deref()
    }

    /// Resolve files for an operation and build result filters.
    /// Public entry point replacing resolve_op_files().
    pub fn resolve(
        &self,
        request: &FileRequest,
        report: &mut ReportBuilder,
    ) -> (Vec<String>, Vec<Box<dyn ResultFilter>>) {
        let cwd = self.base_dir.as_deref()
            .unwrap_or_else(|| Path::new("."));
        let filters = build_filters(self.global_diff_lines.as_deref(), request.diff_lines, cwd);
        let files = self.resolve_files(request, &filters, report);
        (files, filters)
    }

    /// Internal: the core resolution pipeline.
    /// expansion → intersection → excludes → language → diff → limits
    fn resolve_files(
        &self,
        request: &FileRequest,
        filters: &[Box<dyn ResultFilter>],
        report: &mut ReportBuilder,
    ) -> Vec<String> {
        let expansion_limit = self.max_files * 10;

        // --- Verbose logging helpers ---
        let format_patterns = |patterns: &[String]| -> String {
            patterns.iter().map(|g| format!("\"{}\"", g)).collect::<Vec<_>>().join(", ")
        };
        let relative_path = |path: &str| -> String {
            if let Some(base) = &self.base_dir {
                let base_str = base.display().to_string();
                path.strip_prefix(&base_str)
                    .and_then(|p| p.strip_prefix('\\').or(p.strip_prefix('/')))
                    .unwrap_or(path)
                    .to_string()
            } else {
                path.to_string()
            }
        };
        let log_files = |files: &[String]| {
            for f in files.iter().take(5) {
                eprintln!("    {}", relative_path(f));
            }
            if files.len() > 5 {
                eprintln!("    ... and {} more", files.len() - 5);
            }
        };

        // --- Resolve operation files ---
        // Five cases:
        //   1. Operation has files + root exists → expand op, intersect with root
        //   2. Operation has files, no root      → expand op (no intersection)
        //   3. Operation has no files, root exists → use root as base
        //   4. Neither has files, CLI exists      → use CLI files as base (fix #99)
        //   5. None of the above                 → empty set
        let has_op_files = !request.files.is_empty();

        let (mut files, empty_patterns) = if has_op_files {
            // Expand operation globs
            if self.verbose {
                eprintln!("  files: expanding operation {} ...", format_patterns(request.files));
            }
            let globs = resolve_globs_to_absolute(&self.base_dir, request.files);

            let (mut files, empty_patterns) = match expand_globs_checked(&globs, expansion_limit) {
                Ok(result) => {
                    // Normalize all expanded paths (fix #98)
                    let files: Vec<String> = result.files.into_iter()
                        .map(|f| normalize_path(&f)).collect();
                    (files, result.empty_patterns)
                }
                Err(e) => {
                    let base_hint = self.base_dir.as_ref()
                        .map(|b| format!(" (resolved relative to {})", b.display()))
                        .unwrap_or_default();
                    report.add(make_fatal_diagnostic(
                        request.command,
                        format!(
                            "pattern \"{}\" expanded to over {} paths{} — use a more specific pattern or increase --max-files",
                            e.pattern, e.limit, base_hint
                        ),
                    ));
                    return Vec::new();
                }
            };

            if self.verbose {
                eprintln!("  files: operation has {} file(s)", files.len());
                log_files(&files);
            }

            // Intersect with root scope (when both exist)
            if let Some(ref root_set) = self.root_files {
                let before = files.len();
                files.retain(|f| root_set.contains(f));
                if self.verbose {
                    eprintln!("  files: {} file(s) after root intersection (was {})", files.len(), before);
                }
            }

            (files, empty_patterns)
        } else if let Some(ref root_set) = self.root_files {
            // No operation files — use root scope as base
            if self.verbose {
                eprintln!("  files: using root scope ({} file(s))", root_set.len());
            }
            (root_set.iter().cloned().collect(), vec![])
        } else if let Some(ref cli_set) = self.cli_files {
            // No root/op files — CLI files become the base set (fix #99)
            if self.verbose {
                eprintln!("  files: using CLI files as base ({} file(s))", cli_set.len());
            }
            (cli_set.iter().cloned().collect(), vec![])
        } else {
            // Nothing specified
            (vec![], vec![])
        };

        // --- Intersect with pre-computed CLI files ---
        // Skip when CLI files were already used as the base set.
        if let Some(ref cli_set) = self.cli_files {
            if has_op_files || self.root_files.is_some() {
                let before = files.len();
                files.retain(|f| cli_set.contains(f));
                if self.verbose {
                    eprintln!("  files: {} file(s) after CLI intersection (was {})", files.len(), before);
                }
            }
        }

        // --- Filter excludes ---
        // Resolve relative exclude patterns to absolute (same as include patterns)
        // so they match correctly against absolute file paths.
        if !request.exclude.is_empty() {
            let resolved: Vec<String> = resolve_globs_to_absolute(&self.base_dir, request.exclude);
            let exclude_patterns: Vec<glob::Pattern> = resolved.iter()
                .filter_map(|p| glob::Pattern::new(p).ok())
                .collect();

            let opts = glob::MatchOptions {
                case_sensitive: true,
                require_literal_separator: false,
                require_literal_leading_dot: false,
            };

            files.retain(|f| {
                !exclude_patterns.iter().any(|p| p.matches_with(f, opts))
            });
        }

        let files = filter_supported_files(files);

        // --- Intersect with git diff-files ---
        let files = if let Some(ref global_diff) = self.global_diff_files {
            git::intersect_changed(files, global_diff)
        } else {
            files
        };
        let cwd = self.base_dir.as_deref()
            .unwrap_or_else(|| Path::new("."));
        let mut files = apply_diff_files_filter(files, request.diff_files, cwd);

        // --- Apply file-level result filters ---
        if !filters.is_empty() {
            files.retain(|f| filters.iter().all(|filter| filter.include_file(f)));
        }

        // --- Check final file count ---
        if files.len() > self.max_files {
            report.add(make_fatal_diagnostic(
                request.command,
                format!(
                    "resolved {} files, exceeding the limit of {} — use a more specific pattern or increase --max-files",
                    files.len(), self.max_files
                ),
            ));
            return Vec::new();
        }

        // --- Check for empty result from glob expansion ---
        if files.is_empty() && !request.files.is_empty() && !empty_patterns.is_empty() {
            let base_hint = self.base_dir.as_ref()
                .map(|b| format!(" (resolved relative to {})", b.display()))
                .unwrap_or_default();
            let patterns_str = empty_patterns.iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(", ");
            report.add(make_fatal_diagnostic(
                request.command,
                format!("file patterns matched 0 files: {}{}", patterns_str, base_hint),
            ));
        }

        files
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve relative glob patterns to absolute by prepending `base_dir`.
/// Absolute patterns are passed through unchanged.
fn resolve_globs_to_absolute(base_dir: &Option<PathBuf>, patterns: &[String]) -> Vec<String> {
    if let Some(base) = base_dir {
        patterns.iter().map(|g| {
            if Path::new(g).is_absolute() {
                normalize_path(g)
            } else {
                normalize_path(&base.join(g).to_string_lossy())
            }
        }).collect()
    } else {
        patterns.iter().map(|g| normalize_path(g)).collect()
    }
}

/// Build result filters from global and per-operation diff specs.
fn build_filters(
    global_diff: Option<&str>,
    op_diff: Option<&str>,
    cwd: &Path,
) -> Vec<Box<dyn ResultFilter>> {
    let mut filters: Vec<Box<dyn ResultFilter>> = Vec::new();

    for spec in [global_diff, op_diff].into_iter().flatten() {
        match git::DiffHunkFilter::from_spec(spec, cwd) {
            Ok(f) => filters.push(Box::new(f)),
            Err(e) => eprintln!("warning: --diff-lines filter failed: {}", e),
        }
    }

    filters
}

fn apply_diff_files_filter(files: Vec<String>, spec: Option<&str>, cwd: &Path) -> Vec<String> {
    match spec {
        Some(spec) => {
            match git::git_changed_files(spec, cwd) {
                Ok(changed) => git::intersect_changed(files, &changed),
                Err(e) => {
                    eprintln!("warning: --diff-files filter failed: {}", e);
                    files
                }
            }
        }
        None => files,
    }
}

/// Build a fatal diagnostic for file resolution failures.
pub(crate) fn make_fatal_diagnostic(command: &str, reason: String) -> ReportMatch {
    ReportMatch {
        file: String::new(),
        line: 0, column: 0, end_line: 0, end_column: 0,
        command: command.to_string(),
        tree: None,
        value: None,
        source: None,
        lines: None,
        reason: Some(reason),
        severity: Some(Severity::Fatal),
        message: None,
        origin: Some(DiagnosticOrigin::Config),
        rule_id: None,
        status: None,
        output: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_makes_hashset_intersection_work() {
        let mut set: HashSet<String> = HashSet::new();
        set.insert(normalize_path("src\\foo.rs"));
        assert!(set.contains(&normalize_path("src/foo.rs")));
    }

    #[test]
    fn normalize_strips_windows_prefix() {
        let mut set: HashSet<String> = HashSet::new();
        set.insert(normalize_path("//?/C:/project/src/foo.rs"));
        assert!(set.contains("C:/project/src/foo.rs"));
    }

    #[test]
    fn absolute_cli_path_intersects_with_relative_root_glob() {
        // Scenario: CLI passes "/home/user/project/src/foo.rs" (absolute)
        // Config has files: ["src/**/*.rs"] resolved relative to base_dir="/home/user/project/"
        // After base_dir resolution, root glob expands to "/home/user/project/src/foo.rs"
        // Both sets contain the same normalized absolute path → intersection succeeds.
        let root: HashSet<String> = [normalize_path("/home/user/project/src/foo.rs")].into();
        let cli: HashSet<String> = [normalize_path("/home/user/project/src/foo.rs")].into();
        let intersection: Vec<_> = root.intersection(&cli).collect();
        assert_eq!(intersection.len(), 1);
    }

    #[test]
    fn relative_path_strips_base_dir_for_per_rule_matching() {
        use tractor_core::rule::GlobMatcher;

        // Absolute path stripped of base_dir yields relative path
        let abs = "/home/user/project/src/main.rs";
        let rel = abs.strip_prefix("/home/user/project/").unwrap();
        assert_eq!(rel, "src/main.rs");

        // The relative path matches the per-rule include pattern
        let m = GlobMatcher::new(&[], &[], &["src/**/*.rs".into()], &[]).unwrap();
        assert!(m.matches(rel));
    }
}
