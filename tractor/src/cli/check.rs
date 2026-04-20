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
use crate::executor::{self, CheckOperation, Operation};
use crate::cli::context::RunContext;
use crate::input::{InputMode, FileResolver, ResolverOptions, SourceRequest};
use crate::format::{ViewField, GroupDimension, render_report};
use crate::matcher::prepare_report_for_output;
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

    // Build the file resolver for this single-operation run. No config file
    // is involved, so root files are None and there is no base_dir.
    let resolver_opts = ResolverOptions {
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        max_files: args.shared.max_files,
        cli_files: Vec::new(),
        config_root_files: None,
    };
    let env = ctx.exec_ctx();
    let resolver = FileResolver::new(&resolver_opts, &env)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    let mut builder = tractor::ReportBuilder::new();

    let (op_files, inline_source, op_language): (Vec<String>, Option<&crate::input::Source>, Option<String>) = match &ctx.input {
        InputMode::Files(files) => {
            if files.is_empty() {
                return Ok(());
            }
            (files.clone(), None, ctx.lang.clone())
        }
        InputMode::Inline(source) => (
            Vec::new(),
            Some(source),
            Some(source.language.clone()),
        ),
    };

    let request = SourceRequest {
        files: &op_files,
        exclude: &[],
        diff_files: None,
        diff_lines: None,
        command: "check",
        language: op_language.as_deref(),
        inline_source,
    };
    let (sources, filters) = resolver.resolve(&request, &mut builder);

    // If the file resolver emitted a fatal diagnostic, skip execution.
    if !builder.has_fatals() {
        let op = Operation::Check(CheckOperation {
            sources,
            filters,
            rules: vec![rule],
            tree_mode: ctx.tree_mode,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            ruleset_default_language: op_language,
        });

        executor::execute(&[op], &env, &mut builder)?;
    }
    let mut report = builder.build();

    // Single-xpath check: don't expose internal rule ID in output.
    for m in report.all_matches_mut() {
        m.rule_id = None;
    }

    prepare_report_for_output(&mut report, &ctx);
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
        cli_content: args.content,
        format: &args.format,
        default_view: &[ViewField::Reason, ViewField::Severity, ViewField::Lines],
        view_override: args.view.as_deref(),
        message: args.message,
        default_group: &[GroupDimension::File],
        op_filter: |kind| matches!(kind, crate::tractor_config::ConfigOperationKind::Check),
        filter_label: "check",
    })
}
