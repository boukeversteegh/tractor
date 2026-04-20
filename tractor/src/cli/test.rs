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
use crate::executor::{self, TestAssertion, TestOperation};
use crate::cli::context::RunContext;
use crate::input::{plan_single, InputMode, Operation, SingleOpRequest};
use crate::tractor_config::OperationInputs;
use crate::format::{ViewField, TestRenderOptions, render_report};
use crate::matcher::prepare_report_for_output;
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
            cli_content: args.content.clone(),
            format: &args.format,
            default_view: &[ViewField::Totals],
            view_override: args.view.as_deref(),
            message: args.message.clone(),
            default_group: &[],
            op_filter: |kind| matches!(kind, crate::tractor_config::ConfigOperationKind::Test),
            filter_label: "test",
        });
    }

    let expect = args.expect.clone()
        .ok_or("test requires --expect when not using --config")?;
    let error_template = args.error.clone();
    let message = args.message.clone();

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::Totals], args.view.as_deref(), args.message, args.content, false, &[],
    )?;

    let dot = NormalizedXpath::new(".");
    let xpath_expr = ctx.xpath.as_ref().unwrap_or(&dot);

    let (op_files, inline_source, op_language): (Vec<String>, Option<crate::input::Source>, Option<String>) = match &ctx.input {
        InputMode::Files(files) => (files.clone(), None, ctx.lang.clone()),
        InputMode::Inline(source) => (Vec::new(), Some(source.clone()), Some(source.language.clone())),
    };

    let inputs = OperationInputs {
        files: op_files,
        exclude: Vec::new(),
        diff_files: Vec::new(),
        diff_lines: Vec::new(),
        language: op_language.clone(),
        inline_source,
    };

    let op = Operation::Test(TestOperation {
        assertions: vec![TestAssertion {
            xpath: xpath_expr.clone(),
            expect: expect.clone(),
        }],
        tree_mode: ctx.tree_mode,
        language: op_language,
        limit: ctx.limit,
        ignore_whitespace: ctx.ignore_whitespace,
        parse_depth: ctx.parse_depth,
    });

    let mut builder = tractor::ReportBuilder::new();
    let env = ctx.exec_ctx();
    let plan = plan_single(
        SingleOpRequest { op, inputs, command: "test" },
        args.shared.diff_files.clone(),
        args.shared.diff_lines.clone(),
        args.shared.max_files,
        &env,
        &mut builder,
    )?;

    if let Some(plan) = plan {
        executor::execute(&[plan], &env, &mut builder)?;
    }
    // Set expected value for test summary rendering (test-mode only, not shared with run mode)
    builder.set_expected(expect.clone());
    let mut report = builder.build();

    prepare_report_for_output(&mut report, &ctx);
    let test_opts = TestRenderOptions { message, error_template };
    render_report(&report, &ctx, Some(&test_opts))
}
