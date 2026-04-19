use clap::Args;
use tractor::NormalizedXpath;
use crate::cli::SharedArgs;

/// Test mode: assert match count expectations
#[derive(Args, Debug)]
pub struct TestArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    /// Expected result: none, some, or a number (required unless --config is used)
    #[arg(short = 'e', long = "expect", help_heading = "Test", required_unless_present = "config")]
    pub expect: Option<String>,

    /// Error message template for failed expectations (per-match, supports {file}, {line}, {name}, etc.)
    #[arg(long = "error", help_heading = "Test")]
    pub error: Option<String>,

    /// Path to a tractor config file (YAML/TOML) — runs only test operations from it
    #[arg(long = "config", help_heading = "Config")]
    pub config: Option<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Source code string to parse (alternative to stdin, requires --lang)
    #[arg(short = 's', long = "string", help_heading = None)]
    pub content: Option<String>,

    /// Report fields to include (e.g. tree, value, source) [default: totals]
    #[arg(short = 'v', long = "view", help_heading = "View", allow_hyphen_values = true)]
    pub view: Option<String>,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message", help_heading = "View")]
    pub message: Option<String>,

    /// Output format [default: text]
    #[arg(short = 'f', long = "format", default_value = "text", help_heading = "Format")]
    pub format: String,
}
use crate::executor::{self, ExecuteOptions, Operation, TestOperation, TestAssertion};
use crate::cli::context::RunContext;
use crate::input::InputMode;
use crate::format::{ViewField, TestRenderOptions, render_report};
use crate::matcher::prune_match_fields_by_view;
use super::config::{run_from_config, ConfigRunParams};

pub mod test_colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const BOLD: &str = "\x1b[1m";
}

pub fn run_test(args: TestArgs) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref config_path) = args.config {
        return run_from_config(ConfigRunParams {
            config_path,
            shared: &args.shared,
            cli_files: args.files.clone(),
            format: &args.format,
            default_view: &[ViewField::Totals],
            view_override: args.view.as_deref(),
            message: args.message.clone(),
            default_group: &[],
            op_filter: |op| matches!(op, Operation::Test(_)),
            filter_label: "test",
        });
    }

    let expect = args.expect.clone()
        .ok_or("test requires --expect when not using --config")?;
    let error_template = args.error.clone();
    let message = args.message.clone();

    let mut ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::Totals], args.view.as_deref(), args.message, args.content, false, &[],
    )?;
    ctx.plan.flush_warnings();

    let dot = NormalizedXpath::new(".");
    let xpath_expr = ctx.xpath.as_ref().unwrap_or(&dot);

    // Build the test operation for either files or inline source.
    let op = match &ctx.input {
        InputMode::Files(files) => Operation::Test(TestOperation {
            files: files.clone(),
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            assertions: vec![TestAssertion {
                xpath: xpath_expr.clone(),
                expect: expect.clone(),
            }],
            tree_mode: ctx.tree_mode,
            language: ctx.lang.clone(),
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
            inline_source: None,
        }),
        InputMode::InlineSource { source, lang } => Operation::Test(TestOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            assertions: vec![TestAssertion {
                xpath: xpath_expr.clone(),
                expect: expect.clone(),
            }],
            tree_mode: ctx.tree_mode,
            language: Some(lang.clone()),
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
            inline_source: Some(source.clone()),
        }),
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
    // Set expected value for test summary rendering (test-mode only, not shared with run mode)
    builder.set_expected(expect.clone());
    let mut report = builder.build();

    prune_match_fields_by_view(&mut report, &ctx.view);
    let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
    let report = report.with_grouping(&dims);
    let test_opts = TestRenderOptions { message, error_template };
    render_report(&report, &ctx, Some(&test_opts))
}
