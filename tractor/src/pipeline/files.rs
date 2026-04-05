//! File resolution pipeline: glob expansion, intersection, exclusion, and limits.
//!
//! This module owns the file resolution logic for `tractor run`. It expands
//! glob patterns, intersects scope levels (root ∩ operation ∩ CLI), applies
//! excludes and diff filters, and enforces file count limits.
//!
//! The `SharedFileScope` pre-computes shared state (root files, CLI files,
//! global diff) once before iterating operations. Each operation then calls
//! `resolve_op_files` with its own patterns to get its resolved file set.

use std::path::PathBuf;
use tractor_core::{expand_globs_checked, filter_supported_files};
use tractor_core::report::{ReportBuilder, ReportMatch, Severity, DiagnosticOrigin};

use crate::executor::ExecuteOptions;
use crate::filter::ResultFilter;
use super::git;

/// Pre-computed shared file scope, expanded once before iterating operations.
///
/// Root-level `files`, CLI file args, and global `diff-files` are the same for
/// every operation in a config. Expanding them once avoids redundant filesystem
/// walks and git commands when a config has many operations.
pub(crate) struct SharedFileScope {
    /// Expanded root-level files (from config `files:`), or None if not set.
    pub root_files: Option<std::collections::HashSet<String>>,
    /// Expanded CLI file args, or None if not provided.
    cli_files: Option<std::collections::HashSet<String>>,
    /// Expanded global diff-files set, or None if not provided.
    global_diff_files: Option<std::collections::HashSet<PathBuf>>,
}

impl SharedFileScope {
    /// Build from ExecuteOptions, expanding shared globs once.
    /// Returns Err with a fatal diagnostic message on expansion failure.
    pub fn build(options: &ExecuteOptions) -> Result<Self, String> {
        let expansion_limit = options.max_files * 10;

        let resolve_globs = |patterns: &[String]| -> Vec<String> {
            if let Some(base) = &options.base_dir {
                patterns.iter().map(|g| {
                    if std::path::Path::new(g).is_absolute() {
                        g.clone()
                    } else {
                        base.join(g).to_string_lossy().to_string()
                    }
                }).collect()
            } else {
                patterns.to_vec()
            }
        };

        let format_patterns = |patterns: &[String]| -> String {
            patterns.iter().map(|g| format!("\"{}\"", g)).collect::<Vec<_>>().join(", ")
        };

        let base_dir_display = options.base_dir.as_ref()
            .map(|b| b.display().to_string())
            .unwrap_or_else(|| ".".to_string());

        // Strip base_dir prefix from absolute paths to show relative paths in logs.
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

        // --- Root scope ---
        let root_files = if !options.config_root_files.is_empty() {
            if options.verbose {
                eprintln!("  files: expanding root scope {} ...",
                    format_patterns(&options.config_root_files));
            }
            let root_globs = resolve_globs(&options.config_root_files);
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
            Some(expansion.files.into_iter().collect())
        } else {
            None
        };

        // --- CLI files ---
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
            Some(expansion.files.into_iter().collect())
        } else {
            None
        };

        // --- Global diff-files ---
        let global_diff_files = if let Some(ref spec) = options.diff_files {
            let cwd = options.base_dir.as_deref()
                .unwrap_or_else(|| std::path::Path::new("."));
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

        Ok(SharedFileScope { root_files, cli_files, global_diff_files })
    }
}

/// Build result filters from global and per-operation diff specs.
///
/// Both global (ExecuteOptions) and per-operation diff specs are applied.
/// Each produces a separate filter; all must pass for a match to be included.
pub(crate) fn build_filters(
    global_diff: Option<&str>,
    op_diff: Option<&str>,
    cwd: &std::path::Path,
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

/// Resolve files and build result filters for an operation.
///
/// Combines diff-files (file-level) and diff-lines (hunk-level) filtering
/// with glob expansion and exclude patterns. Pushes fatal diagnostics into
/// the report builder when file resolution fails.
pub(crate) fn resolve_op_files(
    files: &[String],
    exclude: &[String],
    diff_files: Option<&str>,
    diff_lines: Option<&str>,
    command: &str,
    options: &ExecuteOptions,
    shared: &SharedFileScope,
    report: &mut ReportBuilder,
) -> (Vec<String>, Vec<Box<dyn ResultFilter>>) {
    let cwd = options.base_dir.as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    let filters = build_filters(options.diff_lines.as_deref(), diff_lines, cwd);
    let files = resolve_files(files, exclude, diff_files, &filters, command, options, shared, report);
    (files, filters)
}

fn resolve_files(
    file_globs: &[String],
    exclude_globs: &[String],
    op_diff_files: Option<&str>,
    filters: &[Box<dyn ResultFilter>],
    command: &str,
    options: &ExecuteOptions,
    shared: &SharedFileScope,
    report: &mut ReportBuilder,
) -> Vec<String> {
    let expansion_limit = options.max_files * 10;

    // --- Verbose logging helpers ---
    let format_patterns = |patterns: &[String]| -> String {
        patterns.iter().map(|g| format!("\"{}\"", g)).collect::<Vec<_>>().join(", ")
    };
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

    // --- Resolve operation files ---
    // Three cases:
    //   1. Operation has files + root exists → expand op, intersect with root
    //   2. Operation has files, no root      → expand op (no intersection)
    //   3. Operation has no files, root exists → use root as base
    //   4. Neither has files                  → empty set
    let has_op_files = !file_globs.is_empty();

    let (mut files, empty_patterns) = if has_op_files {
        // Expand operation globs
        if options.verbose {
            eprintln!("  files: expanding operation {} ...", format_patterns(file_globs));
        }
        let globs: Vec<String> = if let Some(base) = &options.base_dir {
            file_globs.iter().map(|g| {
                if std::path::Path::new(g).is_absolute() {
                    g.clone()
                } else {
                    base.join(g).to_string_lossy().to_string()
                }
            }).collect()
        } else {
            file_globs.to_vec()
        };

        let (mut files, empty_patterns) = match expand_globs_checked(&globs, expansion_limit) {
            Ok(result) => (result.files, result.empty_patterns),
            Err(e) => {
                let base_hint = options.base_dir.as_ref()
                    .map(|b| format!(" (resolved relative to {})", b.display()))
                    .unwrap_or_default();
                report.add(make_fatal_diagnostic(
                    command,
                    format!(
                        "pattern \"{}\" expanded to over {} paths{} — use a more specific pattern or increase --max-files",
                        e.pattern, e.limit, base_hint
                    ),
                ));
                return Vec::new();
            }
        };

        if options.verbose {
            eprintln!("  files: operation has {} file(s)", files.len());
            log_files(&files);
        }

        // Intersect with root scope (when both exist)
        if let Some(ref root_set) = shared.root_files {
            let before = files.len();
            files.retain(|f| root_set.contains(f));
            if options.verbose {
                eprintln!("  files: {} file(s) after root intersection (was {})", files.len(), before);
            }
        }

        (files, empty_patterns)
    } else if let Some(ref root_set) = shared.root_files {
        // No operation files — use root scope as base
        if options.verbose {
            eprintln!("  files: using root scope ({} file(s))", root_set.len());
        }
        (root_set.iter().cloned().collect(), vec![])
    } else {
        // Neither operation nor root has files
        (vec![], vec![])
    };

    // --- Intersect with pre-computed CLI files ---
    if let Some(ref cli_set) = shared.cli_files {
        let before = files.len();
        files.retain(|f| cli_set.contains(f));
        if options.verbose {
            eprintln!("  files: {} file(s) after CLI intersection (was {})", files.len(), before);
        }
    }

    // --- Filter excludes ---
    if !exclude_globs.is_empty() {
        let exclude_patterns: Vec<glob::Pattern> = exclude_globs.iter()
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
    // Global diff-files is pre-computed in SharedFileScope; only filter here.
    let files = if let Some(ref global_diff) = shared.global_diff_files {
        git::intersect_changed(files, global_diff)
    } else {
        files
    };
    // Per-operation diff-files still needs to run each time.
    let cwd = options.base_dir.as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut files = apply_diff_files_filter(files, op_diff_files, cwd);

    // --- Apply file-level result filters ---
    if !filters.is_empty() {
        files.retain(|f| filters.iter().all(|filter| filter.include_file(f)));
    }

    // --- Check final file count ---
    if files.len() > options.max_files {
        report.add(make_fatal_diagnostic(
            command,
            format!(
                "resolved {} files, exceeding the limit of {} — use a more specific pattern or increase --max-files",
                files.len(), options.max_files
            ),
        ));
        return Vec::new();
    }

    // --- Check for empty result from glob expansion ---
    if files.is_empty() && !file_globs.is_empty() && !empty_patterns.is_empty() {
        let base_hint = options.base_dir.as_ref()
            .map(|b| format!(" (resolved relative to {})", b.display()))
            .unwrap_or_default();
        let patterns_str = empty_patterns.iter()
            .map(|p| format!("\"{}\"", p))
            .collect::<Vec<_>>()
            .join(", ");
        report.add(make_fatal_diagnostic(
            command,
            format!("file patterns matched 0 files: {}{}", patterns_str, base_hint),
        ));
    }

    files
}

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

fn apply_diff_files_filter(files: Vec<String>, spec: Option<&str>, cwd: &std::path::Path) -> Vec<String> {
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
