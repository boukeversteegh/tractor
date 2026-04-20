use clap::Args;
use crate::cli::SharedArgs;

/// Update mode: modify only existing matched node values (no creation)
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    /// Value to set matched nodes to
    #[arg(long = "value", help_heading = "Update")]
    pub value: String,

    #[command(flatten)]
    pub shared: SharedArgs,
}
use crate::executor::{self, UpdateDraft};
use crate::cli::context::RunContext;
use crate::input::{plan_single, InputMode, OperationDraft, SingleOpRequest};
use crate::tractor_config::OperationInputs;
use crate::format::ViewField;

pub fn run_update(args: UpdateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        "text", &[ViewField::Tree], None, None, None, false, &[],
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("update requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files.clone(),
        InputMode::Inline(_) => {
            return Err("update cannot be used with stdin input (no file to modify)".into());
        }
    };

    let inputs = OperationInputs {
        files,
        exclude: Vec::new(),
        diff_files: None,
        diff_lines: None,
        language: ctx.lang.clone(),
        inline_source: None,
    };

    let draft = OperationDraft::Update(UpdateDraft {
        xpath: xpath_expr.to_string(),
        value: args.value.clone(),
        tree_mode: ctx.tree_mode,
        language: ctx.lang.clone(),
        limit: ctx.limit,
        ignore_whitespace: ctx.ignore_whitespace,
        parse_depth: ctx.parse_depth,
    });

    let mut builder = tractor::ReportBuilder::new();
    let env = ctx.exec_ctx();
    let op = plan_single(
        SingleOpRequest { draft, inputs, command: "update" },
        args.shared.diff_files.clone(),
        args.shared.diff_lines.clone(),
        args.shared.max_files,
        &env,
        &mut builder,
    )?;

    if let Some(op) = op {
        executor::execute(&[op], &env, &mut builder)?;
    }
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
