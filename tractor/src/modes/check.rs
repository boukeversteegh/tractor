use std::collections::HashSet;
use tractor_core::report::{Report, ReportMatch, Severity, Summary};
use tractor_core::rule::Rule;
use crate::cli::CheckArgs;
use crate::executor::{self, CheckOperation, ExecuteOptions, Operation};
use crate::pipeline::{
    RunContext, ViewField, InputMode,
    render_check_report, match_to_report_match, run_rules,
    project_report, apply_message_template,
};

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

    // Build RunContext for input resolution + rendering config.
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

    // Build a single-rule check operation and delegate to the executor.
    let rule = Rule::new("_check", xpath_expr)
        .with_reason(reason)
        .with_severity(severity);

    let op = Operation::Check(CheckOperation {
        files: files.clone(),
        exclude: vec![],
        rules: vec![rule],
        tree_mode: ctx.tree_mode,
        language: ctx.lang.clone(),
        ignore_whitespace: ctx.ignore_whitespace,
        parse_depth: ctx.parse_depth,
    });

    let options = ExecuteOptions {
        verbose: ctx.verbose,
        ..Default::default()
    };

    let reports = executor::execute(&[op], &options)?;
    let mut report = reports.into_iter().next().unwrap();

    // Single-xpath check: don't expose internal rule ID in output.
    for m in &mut report.matches {
        m.rule_id = None;
    }

    // Apply CLI-level message template (-m) if provided.
    if let Some(ref template) = ctx.message {
        apply_message_template(&mut report, template);
    }

    // Project for the requested view and render.
    project_report(&mut report, &ctx.view);
    let report = if ctx.group_by_file { report.with_groups() } else { report };
    render_check_report(&report, &ctx)
}

// ---------------------------------------------------------------------------
// Rules-based batch check (stays using existing pipeline for now)
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
