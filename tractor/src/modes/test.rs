use std::collections::HashSet;
use tractor_core::report::{Report, Summary};
use crate::cli::TestArgs;
use crate::executor::{self, ExecuteOptions, Operation, TestOperation};
use crate::pipeline::{
    RunContext, ViewField, InputMode,
    query_inline_source, render_test_report, match_to_report_match,
    project_report,
};

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

    match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            // Inline source stays on existing pipeline (no files to resolve).
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            let count = matches.len();
            let passed = check_expectation(&expect, count)?;

            let mut files_affected = HashSet::new();
            for m in &matches { files_affected.insert(m.file.clone()); }

            let report_matches = matches.into_iter().map(|m| {
                let msg = ctx.message.as_deref().map(|t| tractor_core::format_message(t, &m));
                match_to_report_match(m, &ctx.view, None, None, msg)
            }).collect();

            let summary = Summary {
                passed, total: count, files_affected: files_affected.len(),
                errors: 0, warnings: 0, expected: Some(expect.clone()), query: None,
            };

            let report = Report::test(report_matches, summary);
            let report = if ctx.group_by_file { report.with_groups() } else { report };
            render_test_report(&report, &ctx, &message, &error_template)
        }
        InputMode::Files(files) => {
            // Delegate file-based tests to the executor.
            let op = Operation::Test(TestOperation {
                files: files.clone(),
                exclude: vec![],
                xpath: xpath_expr.to_string(),
                expect: expect.clone(),
                tree_mode: ctx.tree_mode,
                language: ctx.lang.clone(),
                limit: ctx.limit,
                ignore_whitespace: ctx.ignore_whitespace,
                parse_depth: ctx.parse_depth,
            });

            let options = ExecuteOptions {
                verbose: ctx.verbose,
                ..Default::default()
            };

            let reports = executor::execute(&[op], &options)?;
            let mut report = reports.into_iter().next().unwrap();

            project_report(&mut report, &ctx.view);
            let report = if ctx.group_by_file { report.with_groups() } else { report };
            render_test_report(&report, &ctx, &message, &error_template)
        }
    }
}
