use std::collections::HashSet;
use tractor_core::report::{Report, Summary};
use crate::cli::TestArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_inline_source, query_files_batched, render_test_report, match_to_report_match};

pub mod test_colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const BOLD: &str = "\x1b[1m";
}

/// Check whether an expectation is met.
pub fn check_expectation(expect: &str, count: usize) -> Result<bool, Box<dyn std::error::Error>> {
    let passed = match expect {
        "none" => count == 0,
        "some" => count > 0,
        _ => {
            let expected: usize = expect.parse()
                .map_err(|_| format!("invalid expectation '{}': use 'none', 'some', or a number", expect))?;
            count == expected
        }
    };
    Ok(passed)
}

pub fn run_test(args: TestArgs) -> Result<(), Box<dyn std::error::Error>> {
    let expect = args.expect.clone();
    let error_template = args.error.clone();
    let message = args.message.clone();

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::Summary], args.view.as_deref(), args.message, args.content, false, false,
    )?;

    let dot = ".".to_string();
    let xpath_expr = ctx.xpath.as_ref().unwrap_or(&dot);

    let (count, matches) = match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            let count = matches.len();
            (count, matches)
        }
        InputMode::Files(files) => {
            query_files_batched(&ctx, files, xpath_expr, true)?
        }
    };

    // Check expectation
    let passed = check_expectation(&expect, count)?;

    // Build ReportMatches (no reason/severity for test matches)
    let message_template = ctx.message.clone();
    let mut files_affected = HashSet::new();
    for m in &matches {
        files_affected.insert(m.file.clone());
    }
    let files_count = files_affected.len();

    let report_matches = matches.into_iter().map(|m| {
        let msg = message_template.as_deref().map(|t| tractor_core::format_message(t, &m));
        match_to_report_match(m, &ctx.view, None, None, msg)
    }).collect();

    let summary = Summary {
        passed,
        total: count,
        files_affected: files_count,
        errors: 0,
        warnings: 0,
        expected: Some(expect.clone()),
    };

    let report = Report::test(report_matches, summary);
    let report = if ctx.group_by_file { report.with_groups() } else { report };
    render_test_report(&report, &ctx, &message, &error_template)
}
