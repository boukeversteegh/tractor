use tractor_core::report::Severity;
use tractor_core::rule::Rule;
use crate::cli::CheckArgs;
use crate::executor::{self, CheckOperation, ExecuteOptions, Operation};
use crate::pipeline::{
    RunContext, ViewField, InputMode,
    render_report,
    project_report, apply_message_template,
    GroupDimension,
};

pub fn run_check(args: CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.config.is_some() {
        let config_path = args.config.clone().unwrap();
        return run_check_config(args, &config_path);
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
    let mut rule = Rule::new("_check", xpath_expr.clone())
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
        max_files: args.shared.max_files,
        ..Default::default()
    };

    let mut builder = tractor_core::ReportBuilder::new();
    executor::execute(&[op], &options, &mut builder)?;
    let mut report = builder.build();

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
    render_report(&report, &ctx, None)
}

// ---------------------------------------------------------------------------
// Config-based batch check — loads a tractor config and runs check operations
// ---------------------------------------------------------------------------

fn run_check_config(args: CheckArgs, config_path_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::path::Path::new(config_path_str);

    if !config_path.exists() {
        return Err(format!("config file not found: {}", config_path_str).into());
    }

    let loaded = crate::tractor_config::load_tractor_config(config_path)?;

    if loaded.operations.is_empty() {
        return Ok(());
    }

    let ctx = RunContext::build(
        &args.shared, vec![], None, &args.format,
        &[ViewField::Reason, ViewField::Severity, ViewField::Lines],
        args.view.as_deref(), args.message, None, false, &[GroupDimension::File],
    )?;

    // Resolve base_dir from the config file's parent directory so that
    // relative file globs in the config are resolved relative to it.
    let base_dir = config_path.parent()
        .map(|p| if p.as_os_str().is_empty() { std::path::Path::new(".") } else { p })
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));

    let options = executor::ExecuteOptions {
        verbose: ctx.verbose,
        base_dir,
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        max_files: args.shared.max_files,
        cli_files: args.files.clone(),
        config_root_files: loaded.root_files,
    };

    let mut builder = tractor_core::ReportBuilder::new();
    executor::execute(&loaded.operations, &options, &mut builder)?;
    let mut report = builder.build();

    // Apply CLI-level message template (-m) if provided.
    if let Some(ref template) = ctx.message {
        apply_message_template(&mut report, template);
    }

    project_report(&mut report, &ctx.view);
    let report = {
        let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
        report.with_grouping(&dims)
    };
    render_report(&report, &ctx, None)
}
