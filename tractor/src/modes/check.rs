use std::collections::HashSet;
use tractor_core::report::{Report, Severity, Summary};
use crate::cli::CheckArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched, render_check_report, match_to_report_match};

pub fn run_check(args: CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    let severity = match args.severity.as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        s => return Err(format!("invalid severity '{}': use 'error' or 'warning'", s).into()),
    };
    let reason = args.reason.clone().unwrap_or_else(|| "check failed".to_string());

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::Reason, ViewField::Severity], args.view.as_deref(), args.message, None, false, true,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("check requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::InlineSource { .. } => {
            return Err("check cannot be used with stdin input".into());
        }
    };

    if files.is_empty() {
        return Ok(());
    }

    let (_, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;

    // Build ReportMatches with reason and severity, populating only selected fields
    let message_template = ctx.message.clone();
    let mut files_affected = HashSet::new();
    for m in &matches {
        files_affected.insert(m.file.clone());
    }
    let total = matches.len();

    let report_matches = matches.into_iter().map(|m| {
        let message = message_template.as_deref().map(|t| tractor_core::format_message(t, &m));
        match_to_report_match(m, &ctx.view, Some(reason.clone()), Some(severity), message)
    }).collect();

    let summary = Summary {
        passed: total == 0,
        total,
        files_affected: files_affected.len(),
        errors: if matches!(severity, Severity::Error) { total } else { 0 },
        warnings: if matches!(severity, Severity::Warning) { total } else { 0 },
        expected: None,
        query: None,
    };

    let report = Report::check(report_matches, summary);
    let report = if ctx.group_by_file { report.with_groups() } else { report };
    render_check_report(&report, &ctx)
}
