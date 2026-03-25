use crate::cli::UpdateArgs;
use crate::executor::{self, ExecuteOptions, Operation, UpdateOperation};
use crate::pipeline::{RunContext, ViewField, InputMode};

pub fn run_update(args: UpdateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        "text", &[ViewField::Tree], None, None, None, false, false,
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
        changed: None,
        xpath: xpath_expr.clone(),
        value: args.value.clone(),
        tree_mode: ctx.tree_mode,
        language: ctx.lang.clone(),
        limit: ctx.limit,
        ignore_whitespace: ctx.ignore_whitespace,
        parse_depth: ctx.parse_depth,
    });

    let options = ExecuteOptions {
        verbose: ctx.verbose,
        changed: args.shared.changed.clone(),
        ..Default::default()
    };

    let reports = executor::execute(&[op], &options)?;
    let report = &reports[0];
    let summary = report.summary.as_ref().unwrap();

    if !summary.passed {
        return Err("update matched no nodes".into());
    }

    eprintln!(
        "Updated {} match{} in {} file{}",
        summary.total,
        if summary.total == 1 { "" } else { "es" },
        summary.files_affected,
        if summary.files_affected == 1 { "" } else { "s" },
    );
    Ok(())
}
