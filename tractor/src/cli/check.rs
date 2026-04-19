use clap::Args;
use tractor::report::Severity;
use tractor::rule::Rule;
use crate::cli::SharedArgs;

/// Check mode: lint/report violations
#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    /// Path to a tractor config file (YAML/TOML) for batch checking.
    ///
    /// Uses the standard tractor config format. Example:
    ///
    ///   check:
    ///     rules:
    ///       - id: no-eval
    ///         xpath: "//call[function='eval']"
    ///         severity: error
    ///         expect:
    ///           - valid: "JSON.parse(data)"
    ///           - invalid: "eval(userInput)"
    #[arg(long = "config", help_heading = "Config", verbatim_doc_comment)]
    pub config: Option<String>,

    /// Reason message for each violation
    #[arg(long = "reason", help_heading = "Inline Rule (use with -x)")]
    pub reason: Option<String>,

    /// Severity level: error (default) or warning
    #[arg(long = "severity", default_value = "error", help_heading = "Inline Rule (use with -x)")]
    pub severity: String,

    /// A code example that should pass the check (no matches expected)
    #[arg(long = "expect-valid", help_heading = "Inline Rule (use with -x)")]
    pub expect_valid: Option<String>,

    /// A code example that should fail the check (matches expected)
    #[arg(long = "expect-invalid", help_heading = "Inline Rule (use with -x)")]
    pub expect_invalid: Option<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Source code string to parse (alternative to stdin, requires --lang)
    #[arg(short = 's', long = "string", help_heading = None)]
    pub content: Option<String>,

    /// Report fields to include (e.g. tree, value, source) [default: reason,severity,lines]
    #[arg(short = 'v', long = "view", help_heading = "View", allow_hyphen_values = true)]
    pub view: Option<String>,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message", help_heading = "View")]
    pub message: Option<String>,

    /// Output format [default: gcc]
    #[arg(short = 'f', long = "format", default_value = "gcc", help_heading = "Format")]
    pub format: String,
}
use crate::executor::{self, CheckOperation, ExecuteOptions, Operation};
use crate::cli::context::RunContext;
use crate::input::InputMode;
use crate::format::{ViewField, GroupDimension, render_report};
use crate::matcher::{project_report, apply_message_template, populate_schema};
use super::config::{run_from_config, ConfigRunParams};

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
        &args.format, &[ViewField::Reason, ViewField::Severity, ViewField::Lines], args.view.as_deref(), args.message, args.content, false, &[GroupDimension::File],
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("check requires an XPath query (-x)")?;

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

    let op = match &ctx.input {
        InputMode::Files(files) => {
            if files.is_empty() {
                return Ok(());
            }
            Operation::Check(CheckOperation {
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
                inline_source: None,
            })
        }
        InputMode::InlineSource { source, lang } => {
            Operation::Check(CheckOperation {
                files: vec![],
                exclude: vec![],
                diff_files: None,
                diff_lines: None,
                rules: vec![rule],
                tree_mode: ctx.tree_mode,
                language: Some(lang.clone()),
                ignore_whitespace: ctx.ignore_whitespace,
                parse_depth: ctx.parse_depth,
                ruleset_include: vec![],
                ruleset_exclude: vec![],
                inline_source: Some(source.clone()),
            })
        }
    };

    let options = ExecuteOptions {
        verbose: ctx.verbose,
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        max_files: args.shared.max_files,
        ..Default::default()
    };

    let mut builder = tractor::ReportBuilder::new();
    executor::execute(&[op], &options, &mut builder)?;
    let mut report = builder.build();

    // Single-xpath check: don't expose internal rule ID in output.
    for m in report.all_matches_mut() {
        m.rule_id = None;
    }

    if ctx.view.has(ViewField::Schema) {
        populate_schema(&mut report, ctx.schema_depth(), ctx.use_color);
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
    run_from_config(ConfigRunParams {
        config_path: config_path_str,
        shared: &args.shared,
        cli_files: args.files,
        format: &args.format,
        default_view: &[ViewField::Reason, ViewField::Severity, ViewField::Lines],
        view_override: args.view.as_deref(),
        message: args.message,
        default_group: &[GroupDimension::File],
        op_filter: |op| matches!(op, Operation::Check(_)),
        filter_label: "check",
    })
}
