use tractor_core::apply_replacements;
use crate::cli::SetArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched};

pub fn run_set(args: SetArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        "text", &[ViewField::Tree], None, None, None, false, false,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("set requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::InlineSource { .. } => {
            return Err("set cannot be used with stdin input (no file to modify)".into());
        }
    };

    let (_, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;

    let summary = apply_replacements(&matches, &args.value)?;
    eprintln!(
        "Set {} match{} in {} file{}",
        summary.replacements_made,
        if summary.replacements_made == 1 { "" } else { "es" },
        summary.files_modified,
        if summary.files_modified == 1 { "" } else { "s" },
    );
    Ok(())
}
