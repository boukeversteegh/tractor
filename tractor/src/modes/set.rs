use std::collections::{HashSet, HashMap};
use tractor_core::{apply_replacements, apply_set_to_string, compute_set_output};
use tractor_core::report::{Report, ReportMatch, Summary};
use crate::cli::SetArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched, query_inline_source, render_set_report};

pub fn run_set(args: SetArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Early normalization: if stdin is provided as source input (--lang set, no files,
    // stdin is not a TTY), implicitly enable stdout mode — there is no file to modify.
    let stdin_source = args.files.is_empty()
        && args.shared.lang.is_some()
        && !atty::is(atty::Stream::Stdin);
    let stdout = args.stdout || stdin_source;

    // Default view depends on mode:
    //   stdout mode: only the raw output content (per file, at group level)
    //   in-place mode: file path + line + set status per match
    let default_view: &[ViewField] = if stdout {
        &[ViewField::Output]
    } else {
        &[ViewField::File, ViewField::Line, ViewField::Status]
    };

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, default_view, args.view.as_deref(), None, None, false, false,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("set requires an XPath query (-x)")?;

    match &ctx.input {
        InputMode::Files(files) => {
            let (_, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;

            if stdout {
                // Stdout mode: compute modified content per file, build per-match
                // report, group by file, and attach file outputs to groups.
                let file_outputs = compute_set_output(files, &matches, &args.value)?;
                let output_map: HashMap<String, String> = file_outputs.into_iter().collect();
                let report = build_set_report_matches(&matches, &args.value, &ctx);
                let report = report.with_groups().with_file_outputs(&output_map);
                render_set_report(&report, &ctx)?;
            } else {
                // In-place mode: write files, then build per-match status report.
                let report = build_set_inplace_report(&matches, &args.value, &ctx)?;
                let report = report.with_groups();
                render_set_report(&report, &ctx)?;
            }
        }
        InputMode::InlineSource { source, lang } => {
            // Inline source is always stdout mode — one group (no file path), output = modified string.
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            let modified = apply_set_to_string(source, &matches, &args.value)?;
            let report = build_set_inline_report(modified, &ctx);
            render_set_report(&report, &ctx)?;
        }
    }
    Ok(())
}

/// Build per-match `ReportMatch` entries annotated with status.
/// Does NOT write any files. Returns a flat (non-grouped) `Report`.
fn build_set_report_matches(
    matches: &[tractor_core::Match],
    new_value: &str,
    ctx: &RunContext,
) -> Report {
    let mut files_affected = HashSet::new();
    let mut updated_count = 0usize;
    let mut unchanged_count = 0usize;

    let report_matches: Vec<ReportMatch> = matches.iter().map(|m| {
        files_affected.insert(m.file.clone());
        let is_unchanged = m.value == new_value;
        let status_str = if is_unchanged { "unchanged" } else { "updated" };
        if is_unchanged { unchanged_count += 1; } else { updated_count += 1; }

        ReportMatch {
            file: m.file.clone(),
            line: m.line,
            column: m.column,
            end_line: m.end_line,
            end_column: m.end_column,
            tree: None,
            value: if ctx.view.has(ViewField::Value) { Some(m.value.clone()) } else { None },
            source: if ctx.view.has(ViewField::Source) { Some(m.extract_source_snippet()) } else { None },
            lines: if ctx.view.has(ViewField::Lines) {
                Some(m.get_source_lines_range().into_iter()
                    .map(|l| l.trim_end_matches('\r').to_owned())
                    .collect())
            } else { None },
            reason: None,
            severity: None,
            message: None,
            rule_id: None,
            status: if ctx.view.has(ViewField::Status) { Some(status_str.to_string()) } else { None },
            output: None, // output is at group level for stdout mode
        }
    }).collect();

    let summary = Summary {
        passed: true,
        total: matches.len(),
        files_affected: files_affected.len(),
        errors: updated_count,
        warnings: unchanged_count,
        expected: None,
        query: None,
    };
    Report::set(report_matches, summary)
}

/// Build a set report for inline (stdin) stdout mode.
/// Creates a single group with no file path and `output` = the modified string.
fn build_set_inline_report(modified: String, ctx: &RunContext) -> Report {
    use tractor_core::report::FileGroup;

    let output = if ctx.view.has(ViewField::Output) { Some(modified) } else { None };
    let group = FileGroup { file: String::new(), matches: vec![], output };

    let summary = Summary {
        passed: true,
        total: 1,
        files_affected: 0,
        errors: 0,
        warnings: 0,
        expected: None,
        query: None,
    };
    let mut report = Report::set(vec![], summary);
    report.groups = Some(vec![group]);
    report
}

/// Build a set report for in-place mode: writes files, annotates matches with status.
fn build_set_inplace_report(
    matches: &[tractor_core::Match],
    new_value: &str,
    ctx: &RunContext,
) -> Result<Report, Box<dyn std::error::Error>> {
    // Apply replacements to disk first, then annotate matches with status.
    let summary_result = apply_replacements(matches, new_value)?;
    let _ = summary_result;
    Ok(build_set_report_matches(matches, new_value, ctx))
}
