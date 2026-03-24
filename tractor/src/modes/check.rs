use std::collections::HashSet;
use tractor_core::report::{Report, ReportMatch, Severity, Summary};
use crate::cli::CheckArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched, render_check_report, match_to_report_match, run_rules};

pub fn run_check(args: CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.rules.is_some() {
        let rules_path = args.rules.clone().unwrap();
        return run_check_rules(args, &rules_path);
    }

    let severity = match args.severity.as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        s => return Err(format!("invalid severity '{}': use 'error' or 'warning'", s).into()),
    };
    let reason = args.reason.clone().unwrap_or_else(|| "check failed".to_string());

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::Reason, ViewField::Severity, ViewField::Lines], args.view.as_deref(), args.message, None, false, true,
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

// ---------------------------------------------------------------------------
// Rules-based batch check
// ---------------------------------------------------------------------------

fn run_check_rules(args: CheckArgs, rules_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ruleset = crate::rules_config::load_rules(std::path::Path::new(rules_path))?;

    if ruleset.rules.is_empty() {
        return Ok(());
    }

    // Build RunContext for output formatting. XPath is not needed here since
    // each rule carries its own. Files come from CLI args (then filtered per rule).
    let ctx = RunContext::build(
        &args.shared,
        args.files,
        None, // no single xpath
        &args.format,
        &[ViewField::Reason, ViewField::Severity, ViewField::Lines],
        args.view.as_deref(),
        args.message,
        None,
        false,
        true,
    )?;

    let files = match &ctx.input {
        InputMode::Files(files) => files.clone(),
        InputMode::InlineSource { .. } => {
            return Err("check --rules cannot be used with stdin input".into());
        }
    };

    if files.is_empty() {
        return Ok(());
    }

    let rule_matches = run_rules(
        &ruleset,
        &files,
        ctx.tree_mode,
        ctx.ignore_whitespace,
        ctx.parse_depth,
        ctx.verbose,
    )?;

    // Convert RuleMatches into ReportMatches, using each rule's metadata.
    let mut files_affected = HashSet::new();
    let mut errors = 0usize;
    let mut warnings = 0usize;

    let report_matches: Vec<ReportMatch> = rule_matches
        .into_iter()
        .map(|rm| {
            let rule = &ruleset.rules[rm.rule_index];
            let reason = rule
                .reason
                .clone()
                .unwrap_or_else(|| format!("[{}] check failed", rule.id));
            let severity = rule.severity;

            match severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
            }
            files_affected.insert(rm.m.file.clone());

            let message = rule
                .message
                .as_deref()
                .map(|t| tractor_core::format_message(t, &rm.m));

            let mut report_match = match_to_report_match(
                rm.m,
                &ctx.view,
                Some(reason),
                Some(severity),
                message,
            );
            report_match.rule_id = Some(rule.id.clone());
            report_match
        })
        .collect();

    let total = report_matches.len();
    let summary = Summary {
        passed: errors == 0,
        total,
        files_affected: files_affected.len(),
        errors,
        warnings,
        expected: None,
        query: None,
    };

    let report = Report::check(report_matches, summary);
    let report = if ctx.group_by_file { report.with_groups() } else { report };
    render_check_report(&report, &ctx)
}
