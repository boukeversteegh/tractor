use std::collections::HashSet;
use tractor_core::expand_globs;
use tractor_core::report::{Report, ReportMatch, Severity, Summary};
use tractor_core::unicode_safety::{self, ThreatCategory, is_likely_binary};
use crate::cli::ScanArgs;
use crate::pipeline::format::{
    OutputFormat, ViewField, ViewSet,
    render_check_report,
};
use crate::pipeline::context::RunContext;
use crate::pipeline::InputMode;

pub fn run_scan(args: ScanArgs) -> Result<(), Box<dyn std::error::Error>> {
    let files = expand_globs(&args.files);

    if files.is_empty() {
        eprintln!("no files matched");
        return Ok(());
    }

    // Parse category filter if provided
    let category_filter: Option<HashSet<ThreatCategory>> = args.category.as_ref().map(|cats| {
        cats.split(',')
            .filter_map(|s| match s.trim() {
                "bidi-control" => Some(ThreatCategory::BidiControl),
                "zero-width" => Some(ThreatCategory::ZeroWidth),
                "variation-selector" => Some(ThreatCategory::VariationSelector),
                "tag-character" => Some(ThreatCategory::TagCharacter),
                "supplementary-private-use" => Some(ThreatCategory::SupplementaryPrivateUse),
                "invisible-formatting" => Some(ThreatCategory::InvisibleFormatting),
                "homoglyph" => Some(ThreatCategory::Homoglyph),
                other => {
                    eprintln!("warning: unknown category '{}', ignoring", other);
                    None
                }
            })
            .collect()
    });

    let output_format = OutputFormat::from_str(&args.format)?;
    let use_color = if args.no_color {
        false
    } else {
        tractor_core::output::should_use_color(&args.color)
    };

    // Scan files
    let mut all_matches: Vec<ReportMatch> = Vec::new();
    let mut files_affected = HashSet::new();
    let mut files_scanned: usize = 0;
    let mut files_skipped_binary: usize = 0;

    for file_path in &files {
        let raw_bytes = match std::fs::read(file_path) {
            Ok(b) => b,
            Err(e) => {
                if args.verbose {
                    eprintln!("warning: cannot read '{}': {}", file_path, e);
                }
                continue;
            }
        };

        // Skip binary files
        if is_likely_binary(&raw_bytes) {
            files_skipped_binary += 1;
            continue;
        }

        let content = match String::from_utf8(raw_bytes) {
            Ok(s) => s,
            Err(_) => {
                // Not valid UTF-8 — skip (could be binary or other encoding)
                files_skipped_binary += 1;
                continue;
            }
        };

        files_scanned += 1;
        let findings = unicode_safety::scan_content(&content);

        for finding in findings {
            // Apply category filter if set
            if let Some(ref filter) = category_filter {
                if !filter.contains(&finding.category) {
                    continue;
                }
            }

            let severity = match finding.category.severity() {
                "error" => Severity::Error,
                _ => Severity::Warning,
            };

            files_affected.insert(file_path.clone());

            all_matches.push(ReportMatch {
                file: file_path.clone(),
                line: finding.line,
                column: finding.column,
                end_line: finding.line,
                end_column: finding.column + 1,
                tree: None,
                value: Some(format!("U+{:04X}", finding.codepoint as u32)),
                source: None,
                lines: None,
                reason: Some(finding.reason()),
                severity: Some(severity),
                message: None,
                rule_id: Some(finding.category.as_str().to_string()),
            });
        }
    }

    let errors = all_matches.iter().filter(|m| matches!(m.severity, Some(Severity::Error))).count();
    let warnings = all_matches.iter().filter(|m| matches!(m.severity, Some(Severity::Warning))).count();
    let total = all_matches.len();

    let summary = Summary {
        passed: errors == 0,
        total,
        files_affected: files_affected.len(),
        errors,
        warnings,
        expected: None,
        query: None,
    };

    if args.verbose {
        eprintln!("scanned {} files ({} skipped as binary)", files_scanned, files_skipped_binary);
    }

    let report = Report::check(all_matches, summary);

    // Build a minimal RunContext for rendering
    let ctx = RunContext {
        xpath: None,
        output_format,
        view: ViewSet::from_fields(vec![ViewField::File, ViewField::Line, ViewField::Column, ViewField::Value, ViewField::Reason, ViewField::Severity]),
        use_color,
        message: None,
        input: InputMode::Files(vec![]),
        concurrency: 1,
        limit: None,
        depth: None,
        parse_depth: None,
        meta: false,
        raw: false,
        no_pretty: false,
        ignore_whitespace: false,
        verbose: args.verbose,
        lang: None,
        debug: false,
        group_by_file: true,
    };

    render_check_report(&report, &ctx)
}
