use std::collections::HashSet;
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
    //   stdout mode: only the raw output content
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
                // Stdout mode: compute modified content without writing to disk.
                let file_outputs = compute_set_output(files, &matches, &args.value)?;
                let report = build_set_stdout_report(file_outputs, &ctx);
                render_set_report(&report, &ctx)?;
            } else {
                // In-place mode: apply replacements and render status per match.
                let report = build_set_inplace_report(&matches, &args.value, &ctx)?;
                render_set_report(&report, &ctx)?;
            }
        }
        InputMode::InlineSource { source, lang } => {
            // Inline source is always stdout mode.
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            let modified = apply_set_to_string(source, &matches, &args.value)?;
            let report = build_set_inline_report(modified, &ctx);
            render_set_report(&report, &ctx)?;
        }
    }
    Ok(())
}

/// Build a set report for stdout mode (files → modified content, no disk writes).
/// Creates one ReportMatch per file with the `output` field set to modified content.
fn build_set_stdout_report(
    file_outputs: Vec<(String, String)>,
    ctx: &RunContext,
) -> Report {
    let files_affected = file_outputs.len();
    let report_matches: Vec<ReportMatch> = file_outputs.into_iter()
        .map(|(file, content)| ReportMatch {
            file,
            line: 0, column: 0, end_line: 0, end_column: 0,
            tree: None, value: None, source: None, lines: None,
            reason: None, severity: None, message: None, rule_id: None,
            status: None,
            output: if ctx.view.has(ViewField::Output) { Some(content) } else { None },
        })
        .collect();

    let summary = Summary {
        passed: true,
        total: report_matches.len(),
        files_affected,
        errors: 0,
        warnings: 0,
        expected: None,
        query: None,
    };
    Report::set(report_matches, summary)
}

/// Build a set report for inline (stdin) stdout mode.
/// Creates a single ReportMatch with the `output` field set to the modified string.
fn build_set_inline_report(modified: String, ctx: &RunContext) -> Report {
    let report_matches = vec![ReportMatch {
        file: String::new(),
        line: 0, column: 0, end_line: 0, end_column: 0,
        tree: None, value: None, source: None, lines: None,
        reason: None, severity: None, message: None, rule_id: None,
        status: None,
        output: if ctx.view.has(ViewField::Output) { Some(modified) } else { None },
    }];

    let summary = Summary {
        passed: true,
        total: 1,
        files_affected: 0,
        errors: 0,
        warnings: 0,
        expected: None,
        query: None,
    };
    Report::set(report_matches, summary)
}

/// Build a set report for in-place mode: applies replacements to files,
/// annotates each match with "updated" or "unchanged" status.
fn build_set_inplace_report(
    matches: &[tractor_core::Match],
    new_value: &str,
    ctx: &RunContext,
) -> Result<Report, Box<dyn std::error::Error>> {
    // Apply replacements to files
    let summary_result = apply_replacements(matches, new_value)?;

    // Build report matches annotated with status
    let mut files_affected = HashSet::new();
    let mut updated_count = 0usize;
    let mut unchanged_count = 0usize;

    let report_matches: Vec<ReportMatch> = matches.iter().map(|m| {
        files_affected.insert(m.file.clone());
        // Determine status: "unchanged" if the new value equals the current match value
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
            output: None,
        }
    }).collect();

    // apply_replacements writes files and returns disk-level counts.
    // We use the per-match analysis above for status/summary counts to stay
    // consistent: every matched node is either "updated" or "unchanged"
    // regardless of whether the byte range was skipped due to out-of-bounds.
    let _ = summary_result;

    let summary = Summary {
        passed: true,
        total: matches.len(),
        files_affected: files_affected.len(),
        errors: updated_count,      // reuse "errors" field for updated count
        warnings: unchanged_count,  // reuse "warnings" field for unchanged count
        expected: None,
        query: None,
    };
    Ok(Report::set(report_matches, summary))
}
