use clap::Args;
use crate::cli::SharedArgs;

/// Update mode: modify only existing matched node values (no creation)
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Value to set matched nodes to
    #[arg(long = "value", help_heading = "Update")]
    pub value: String,
}
use crate::executor::{self, ExecuteOptions, Operation, UpdateOperation};
use crate::cli::context::RunContext;
use crate::input::InputMode;
use crate::format::ViewField;

pub fn run_update(args: UpdateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        "text", &[ViewField::Tree], None, None, None, false, &[],
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("update requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::InlineSource { .. } => {
            return Err("update cannot be used with stdin input (no file to modify)".into());
        }
    };

    let op = Operation::Update(UpdateOperation {
        files: files.clone(),
        exclude: vec![],
        diff_files: None,
        diff_lines: None,
        xpath: xpath_expr.to_string(),
        value: args.value.clone(),
        tree_mode: ctx.tree_mode,
        language: ctx.lang.clone(),
        limit: ctx.limit,
        ignore_whitespace: ctx.ignore_whitespace,
        parse_depth: ctx.parse_depth,
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
    let report = builder.build();
    if report.success == Some(false) {
        return Err("update matched no nodes".into());
    }

    let totals = report.totals.as_ref().unwrap();
    eprintln!(
        "Updated {} match{} in {} file{}",
        totals.updated,
        if totals.updated == 1 { "" } else { "es" },
        totals.files,
        if totals.files == 1 { "" } else { "s" },
    );
    Ok(())
}
