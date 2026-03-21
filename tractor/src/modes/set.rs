use tractor_core::xpath_upsert::upsert;
use tractor_core::detect_language;
use crate::cli::SetArgs;
use crate::pipeline::{InputMode, ViewField};
use crate::pipeline::context::RunContext;

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

    let lang_override = ctx.lang.as_deref();
    let mut files_modified = 0;
    let mut total_ops = 0;

    for file_path in files {
        let lang = lang_override
            .unwrap_or_else(|| detect_language(file_path));

        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("{}: {}", file_path, e))?;

        let result = upsert(&source, lang, xpath_expr, &args.value)
            .map_err(|e| format!("{}: {}", file_path, e))?;

        if result.source != source {
            std::fs::write(file_path, &result.source)
                .map_err(|e| format!("{}: {}", file_path, e))?;
            files_modified += 1;
            total_ops += 1;
            let action = if result.inserted { "Inserted" } else { "Updated" };
            eprintln!("{} in {}: {}", action, file_path, result.description);
        }
    }

    if total_ops == 0 {
        eprintln!("No changes made");
    } else {
        eprintln!(
            "Modified {} file{}",
            files_modified,
            if files_modified == 1 { "" } else { "s" },
        );
    }
    Ok(())
}
