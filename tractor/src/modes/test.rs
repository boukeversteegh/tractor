use crate::cli::TestArgs;
use crate::executor::{self, ExecuteOptions, Operation, TestOperation, TestAssertion};
use crate::pipeline::{
    RunContext, ViewField, InputMode,
    render_test_report,
    project_report,
};

pub mod test_colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const BOLD: &str = "\x1b[1m";
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

    // Build the test operation for either files or inline source.
    let op = match &ctx.input {
        InputMode::Files(files) => Operation::Test(TestOperation {
            files: files.clone(),
            exclude: vec![],
            changed: None,
            diff: None,
            assertions: vec![TestAssertion {
                xpath: xpath_expr.to_string(),
                expect: expect.clone(),
            }],
            tree_mode: ctx.tree_mode,
            language: ctx.lang.clone(),
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
            inline_source: None,
            inline_lang: None,
        }),
        InputMode::InlineSource { source, lang } => Operation::Test(TestOperation {
            files: vec![],
            exclude: vec![],
            changed: None,
            diff: None,
            assertions: vec![TestAssertion {
                xpath: xpath_expr.to_string(),
                expect: expect.clone(),
            }],
            tree_mode: ctx.tree_mode,
            language: None,
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
            inline_source: Some(source.clone()),
            inline_lang: Some(lang.clone()),
        }),
    };

    let options = ExecuteOptions {
        verbose: ctx.verbose,
        changed: args.shared.changed.clone(),
        diff: args.shared.diff.clone(),
        ..Default::default()
    };

    let reports = executor::execute(&[op], &options)?;
    let mut report = reports.into_iter().next().unwrap();

    project_report(&mut report, &ctx.view);
    let report = if ctx.group_by_file { report.with_groups() } else { report };
    render_test_report(&report, &ctx, &message, &error_template)
}
