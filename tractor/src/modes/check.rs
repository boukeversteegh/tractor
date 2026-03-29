use tractor_core::report::Severity;
use tractor_core::rule::Rule;
use crate::cli::CheckArgs;
use crate::executor::{self, CheckOperation, ExecuteOptions, Operation};
use crate::pipeline::{
    RunContext, ViewField, InputMode,
    render_check_report,
    project_report, apply_message_template,
    GroupDimension,
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
        &args.format, &[ViewField::Reason, ViewField::Severity, ViewField::Lines], args.view.as_deref(), args.message, None, false, &[GroupDimension::File],
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
    let mut rule = Rule::new("_check", xpath_expr)
        .with_reason(reason)
        .with_severity(severity);

    if let Some(ref ex) = args.expect_valid {
        rule = rule.with_valid_examples(vec![ex.clone()]);
    }
    if let Some(ref ex) = args.expect_invalid {
        rule = rule.with_invalid_examples(vec![ex.clone()]);
    }

    let op = Operation::Check(CheckOperation {
        files: files.clone(),
        exclude: vec![],
        diff_files: None,
        diff_lines: None,
        rules: vec![rule],
        tree_mode: ctx.tree_mode,
        language: ctx.lang.clone(),
        ignore_whitespace: ctx.ignore_whitespace,
        parse_depth: ctx.parse_depth,
        ruleset_include: vec![],
        ruleset_exclude: vec![],
    });

    let options = ExecuteOptions {
        verbose: ctx.verbose,
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        ..Default::default()
    };

    let reports = executor::execute(&[op], &options)?;
    let mut report = reports.into_iter().next().unwrap();

    // Single-xpath check: don't expose internal rule ID in output.
    for m in report.all_matches_mut() {
        m.rule_id = None;
    }

    // Apply CLI-level message template (-m) if provided.
    if let Some(ref template) = ctx.message {
        apply_message_template(&mut report, template);
    }

    // Project for the requested view and render.
    project_report(&mut report, &ctx.view);
    let report = {
        let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
        report.with_grouping(&dims)
    };
    render_check_report(&report, &ctx)
}

// ---------------------------------------------------------------------------
// Rules-based batch check — delegates to executor
// ---------------------------------------------------------------------------

fn run_check_rules(args: CheckArgs, rules_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ruleset = crate::rules_config::load_rules(std::path::Path::new(rules_path))?;

    if ruleset.rules.is_empty() {
        return Ok(());
    }

    let ctx = RunContext::build(
        &args.shared, args.files, None, &args.format,
        &[ViewField::Reason, ViewField::Severity, ViewField::Lines],
        args.view.as_deref(), args.message, None, false, &[GroupDimension::File],
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

    let op = Operation::Check(CheckOperation {
        files,
        exclude: vec![],
        diff_files: None,
        diff_lines: None,
        rules: ruleset.rules.clone(),
        tree_mode: ctx.tree_mode,
        language: ctx.lang.clone(),
        ignore_whitespace: ctx.ignore_whitespace,
        parse_depth: ctx.parse_depth,
        ruleset_include: ruleset.include.clone(),
        ruleset_exclude: ruleset.exclude.clone(),
    });

    let options = ExecuteOptions {
        verbose: ctx.verbose,
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        ..Default::default()
    };

    let reports = executor::execute(&[op], &options)?;
    let mut report = reports.into_iter().next().unwrap();

    // Apply CLI-level message template (-m) if provided.
    if let Some(ref template) = ctx.message {
        apply_message_template(&mut report, template);
    }

    project_report(&mut report, &ctx.view);
    let report = {
        let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
        report.with_grouping(&dims)
    };
    render_check_report(&report, &ctx)
}
