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

use tractor_core::{expand_globs_checked, filter_supported_files, normalize_path, NormalizedPath, GlobPattern, CompiledPattern};
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
    root_files: Option<HashSet<NormalizedPath>>,
    /// Expanded CLI file args, or None if not provided.
    cli_files: Option<HashSet<NormalizedPath>>,
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
            .map(|b| normalize_path(&b.display().to_string()))
            .unwrap_or_else(|| ".".to_string());

        let relative_path = |path: &str| -> String {
            if let Some(base) = &options.base_dir {
                let base_str = normalize_path(&base.display().to_string());
                path.strip_prefix(&base_str)
                    .and_then(|p| p.strip_prefix('/'))
                    .unwrap_or(path)
                    .to_string()
            } else {
                path.to_string()
            }
        };
        macro_rules! log_files {
            ($files:expr) => {
                for f in $files.iter().take(5) {
                    eprintln!("    {}", relative_path(AsRef::<str>::as_ref(f)));
                }
                if $files.len() > 5 {
                    eprintln!("    ... and {} more", $files.len() - 5);
                }
            };
        }

        if options.verbose {
            let cwd_display = std::env::current_dir()
                .and_then(|p| std::fs::canonicalize(&p).or(Ok(p)))
                .map(|p| normalize_path(&p.display().to_string()))
                .unwrap_or_else(|_| ".".to_string());
            eprintln!("  files: working directory {}", cwd_display);
            eprintln!("  files: resolving relative to {}", base_dir_display);
            eprintln!("  files: max {} files", options.max_files);
        }

        // --- Root scope (fix #99) ---
        // None = key missing → unrestricted; Some([]) = explicit empty; Some([...]) = expand
        let root_files = match &options.config_root_files {
            Some(patterns) if !patterns.is_empty() => {
                let root_globs = resolve_globs_to_absolute(&options.base_dir, patterns);
                let expansion = expand_globs_checked(&root_globs, expansion_limit)
                    .map_err(|e| {
                        format!(
                            "root pattern \"{}\" expanded to over {} paths — use a more specific pattern or increase --max-files",
                            e.pattern, e.limit
                        )
                    })?;
                if options.verbose {
                    eprintln!("  files: root scope {} expanded to {} file(s)",
                        format_patterns(patterns), expansion.files.len());
                    log_files!(&expansion.files);
                }
                Some(expansion.files.into_iter()
                    .map(|f| NormalizedPath::new(&f)).collect())
            }
            Some(_) => Some(HashSet::new()), // files: [] → explicit empty
            None => None,                     // key missing → unrestricted
        };

        // --- CLI files (fix #98: normalize paths) ---
        let cli_files = if !options.cli_files.is_empty() {
            let expansion = expand_globs_checked(&options.cli_files, expansion_limit)
                .map_err(|e| format!(
                    "CLI pattern \"{}\" expanded to over {} paths — use a more specific pattern or increase --max-files",
                    e.pattern, e.limit
                ))?;
            let cli_set: HashSet<NormalizedPath> = expansion.files.into_iter()
                .map(|f| NormalizedPath::absolute(&f)).collect();
            if options.verbose {
                eprintln!("  files: CLI args {} expanded to {} file(s)",
                    format_patterns(&options.cli_files), cli_set.len());
                for f in cli_set.iter().take(5) {
                    eprintln!("    {}", relative_path(f.as_str()));
                }
                if cli_set.len() > 5 {
                    eprintln!("    ... and {} more", cli_set.len() - 5);
                }
            }
            Some(cli_set)
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
                        eprintln!("  files: git diff \"{}\" found {} changed file(s)", spec, changed.len());
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
    ) -> (Vec<NormalizedPath>, Vec<Box<dyn ResultFilter>>) {
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
    ) -> Vec<NormalizedPath> {
        let expansion_limit = self.max_files * 10;

        // --- Verbose logging helpers ---
        let format_patterns = |patterns: &[String]| -> String {
            patterns.iter().map(|g| format!("\"{}\"", g)).collect::<Vec<_>>().join(", ")
        };
        let relative_path = |path: &str| -> String {
            if let Some(base) = &self.base_dir {
                let base_str = normalize_path(&base.display().to_string());
                path.strip_prefix(&base_str)
                    .and_then(|p| p.strip_prefix('/'))
                    .unwrap_or(path)
                    .to_string()
            } else {
                path.to_string()
            }
        };
        macro_rules! log_files {
            ($files:expr) => {
                for f in $files.iter().take(5) {
                    eprintln!("    {}", relative_path(AsRef::<str>::as_ref(f)));
                }
                if $files.len() > 5 {
                    eprintln!("    ... and {} more", $files.len() - 5);
                }
            };
        }

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
            // (verbose log after expansion below)
            let globs = resolve_globs_to_absolute(&self.base_dir, request.files);

            let (mut files, empty_patterns) = match expand_globs_checked(&globs, expansion_limit) {
                Ok(result) => {
                    // Normalize and deduplicate expanded paths. Multiple patterns
                    // can expand to the same file (e.g. "src/**/*.cs" and
                    // "src/sub/**/*.cs" both yield "src/sub/foo.cs"). Using a
                    // set ensures each file appears once regardless of how many
                    // patterns matched it (#127 follow-up).
                    let mut seen = HashSet::new();
                    let files: Vec<NormalizedPath> = result.files.into_iter()
                        .map(|f| NormalizedPath::new(&f))
                        .filter(|f| seen.insert(f.clone()))
                        .collect();
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
                eprintln!("  files: operation {} expanded to {} file(s)",
                    format_patterns(request.files), files.len());
                log_files!(&files);
            }

            // Intersect with root scope (when both exist)
            if let Some(ref root_set) = self.root_files {
                let before = files.len();
                files.retain(|f| root_set.contains(f));
                if self.verbose {
                    eprintln!("  files: {} file(s) after root \u{2229} operation intersection (was {})", files.len(), before);
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
        } else if self.base_dir.is_some() {
            // Config-based run with no files at any level — fail with a clear
            // message rather than silently doing nothing (fix #127 bug 4).
            report.add(make_fatal_diagnostic(
                request.command,
                "no file patterns specified — add `files:` to your config, pass files as CLI arguments, or add `include:` to your rules".to_string(),
            ));
            return Vec::new();
        } else {
            // Non-config usage with no files (e.g. inline source) — empty set.
            (vec![], vec![])
        };

        // --- Intersect with pre-computed CLI files ---
        // Skip when CLI files were already used as the base set.
        if let Some(ref cli_set) = self.cli_files {
            if has_op_files || self.root_files.is_some() {
                let before = files.len();
                files.retain(|f| cli_set.contains(f));
                if self.verbose {
                    eprintln!("  files: {} file(s) after root/operation \u{2229} CLI intersection (was {})", files.len(), before);
                }
            }
        }

        // --- Filter excludes ---
        // Resolve relative exclude patterns to absolute (same as include patterns)
        // so they match correctly against absolute file paths.
        if !request.exclude.is_empty() {
            let before = files.len();
            let resolved = GlobPattern::resolve_all(request.exclude, &self.base_dir);
            let exclude_patterns: Vec<CompiledPattern> = resolved.iter()
                .filter_map(|p| CompiledPattern::new(p.as_str()).ok())
                .collect();

            files.retain(|f| {
                !exclude_patterns.iter().any(|p| p.matches(f.as_str()))
            });
            if self.verbose {
                eprintln!("  files: {} file(s) after exclude filter (was {})", files.len(), before);
            }
        }

        let before_lang = files.len();
        let files: Vec<NormalizedPath> = filter_supported_files(
            files.into_iter().map(|f| f.into_string()).collect()
        ).into_iter().map(|f| NormalizedPath::new(&f)).collect();
        if self.verbose && files.len() != before_lang {
            eprintln!("  files: {} file(s) after language filter (was {})", files.len(), before_lang);
        }

        // --- Intersect with git diff-files ---
        // Convert to strings for git module, then back to NormalizedPath
        let string_files: Vec<String> = files.into_iter().map(|f| f.into_string()).collect();
        let before_diff = string_files.len();
        let string_files = if let Some(ref global_diff) = self.global_diff_files {
            git::intersect_changed(string_files, global_diff)
        } else {
            string_files
        };
        let cwd = self.base_dir.as_deref()
            .unwrap_or_else(|| Path::new("."));
        let string_files = apply_diff_files_filter(string_files, request.diff_files, cwd);
        let mut files: Vec<NormalizedPath> = string_files.into_iter()
            .map(|f| NormalizedPath::new(&f)).collect();
        if self.verbose && files.len() != before_diff {
            eprintln!("  files: {} file(s) after diff filter (was {})", files.len(), before_diff);
        }

        // --- Apply file-level result filters ---
        if !filters.is_empty() {
            let before = files.len();
            files.retain(|f| filters.iter().all(|filter| filter.include_file(f.as_str())));
            if self.verbose && files.len() != before {
                eprintln!("  files: {} file(s) after diff-lines filter (was {})", files.len(), before);
            }
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
            let patterns_str = empty_patterns.iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(", ");
            report.add(make_fatal_diagnostic(
                request.command,
                format!("file patterns matched 0 files: {}", patterns_str),
            ));
        }

        if self.verbose {
            eprintln!("  files: result {} file(s)", files.len());
            log_files!(&files);
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
    use tractor_core::GlobPattern;

    #[test]
    fn normalized_path_hashset_intersection_works() {
        let mut set: HashSet<NormalizedPath> = HashSet::new();
        set.insert(NormalizedPath::new("src\\foo.rs"));
        assert!(set.contains("src/foo.rs"));
    }

    #[test]
    fn normalized_path_strips_windows_prefix() {
        let p = NormalizedPath::new("//?/C:/project/src/foo.rs");
        assert_eq!(p, "C:/project/src/foo.rs");
    }

    #[test]
    fn absolute_cli_path_intersects_with_relative_root_glob() {
        let root: HashSet<NormalizedPath> =
            [NormalizedPath::new("/home/user/project/src/foo.rs")].into();
        let cli: HashSet<NormalizedPath> =
            [NormalizedPath::new("/home/user/project/src/foo.rs")].into();
        let intersection: Vec<_> = root.intersection(&cli).collect();
        assert_eq!(intersection.len(), 1);
    }

    #[test]
    fn glob_pattern_matches_normalized_path() {
        use tractor_core::rule::GlobMatcher;

        let m = GlobMatcher::new(
            &[], &[], &[GlobPattern::new("src/**/*.rs")], &[],
        ).unwrap();
        assert!(m.matches(&NormalizedPath::new("src/main.rs")));
        assert!(!m.matches(&NormalizedPath::new("test/main.rs")));
    }

    // -----------------------------------------------------------------------
    // Bug 1 regression tests: CLI ∩ root intersection
    // -----------------------------------------------------------------------

    /// Fix #127 bug 1: CLI files (via NormalizedPath::absolute) must intersect
    /// with root files (from glob expansion) even when cwd casing differs from
    /// the canonicalized base_dir.
    #[test]
    fn cli_files_intersect_with_root_files_same_canonical_path() {
        // Simulate: root files from glob expansion use canonical casing
        let root: HashSet<NormalizedPath> = [
            NormalizedPath::new("C:/Work/Repo/src/foo.rs"),
            NormalizedPath::new("C:/Work/Repo/src/bar.rs"),
        ].into();

        // CLI files also use canonical casing (after our fix)
        let cli: HashSet<NormalizedPath> = [
            NormalizedPath::new("C:/Work/Repo/src/foo.rs"),
        ].into();

        // Intersection must find the common file
        let mut files: Vec<NormalizedPath> = root.iter().cloned().collect();
        files.retain(|f| cli.contains(f));
        assert_eq!(files.len(), 1, "intersection should find the matching file");
        assert_eq!(files[0], "C:/Work/Repo/src/foo.rs");
    }

    /// Fix #127 bug 1: demonstrate that the old bug would produce 0 files
    /// when cwd casing differs from canonical casing.
    #[test]
    fn different_casing_does_not_intersect() {
        // Without the canonicalization fix, CLI files would have cwd casing
        let root: HashSet<NormalizedPath> = [
            NormalizedPath::new("C:/Work/Repo/src/foo.rs"),
        ].into();
        let wrong_case = NormalizedPath::new("c:/work/repo/src/foo.rs");

        // This demonstrates the bug: different casing = no match
        assert!(!root.contains(&wrong_case),
            "NormalizedPath comparison is case-sensitive (by design)");
    }

    // -----------------------------------------------------------------------
    // Bug 4 regression test: error on no files in config mode
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_with_no_files_config_mode_emits_diagnostic() {
        use tractor_core::report::Severity;

        let resolver = FileResolver {
            root_files: None,
            cli_files: None,
            global_diff_files: None,
            verbose: false,
            base_dir: Some(std::path::PathBuf::from(".")),  // config mode
            max_files: 1000,
            global_diff_lines: None,
        };

        let mut builder = tractor_core::ReportBuilder::new();
        let request = FileRequest {
            files: &[],
            exclude: &[],
            diff_files: None,
            diff_lines: None,
            command: "check",
        };
        let (files, _) = resolver.resolve(&request, &mut builder);
        assert!(files.is_empty(), "should return no files");

        let report = builder.build();
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1, "should emit exactly one diagnostic");
        assert_eq!(matches[0].severity, Some(Severity::Fatal));
        assert!(matches[0].reason.as_ref().unwrap().contains("no file patterns"));
    }

    #[test]
    fn resolve_with_no_files_non_config_mode_returns_empty() {
        let resolver = FileResolver {
            root_files: None,
            cli_files: None,
            global_diff_files: None,
            verbose: false,
            base_dir: None,  // non-config mode
            max_files: 1000,
            global_diff_lines: None,
        };

        let mut builder = tractor_core::ReportBuilder::new();
        let request = FileRequest {
            files: &[],
            exclude: &[],
            diff_files: None,
            diff_lines: None,
            command: "query",
        };
        let (files, _) = resolver.resolve(&request, &mut builder);
        assert!(files.is_empty());

        let report = builder.build();
        assert!(report.all_matches().is_empty(), "should not emit diagnostic in non-config mode");
    }

    // -----------------------------------------------------------------------
    // Bug 2 regression: case-insensitive exclude on Windows
    // -----------------------------------------------------------------------

    #[cfg(target_os = "windows")]
    #[test]
    fn exclude_is_case_insensitive_on_windows() {
        use tractor_core::rule::GlobMatcher;

        // Exclude pattern uses uppercase
        let m = GlobMatcher::new(
            &[], &[GlobPattern::new("VENDOR/**")], &[], &[],
        ).unwrap();

        // Lowercase path should still be excluded on Windows
        assert!(!m.matches(&NormalizedPath::new("vendor/lib.rs")));
    }
}
