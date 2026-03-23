use tractor_core::apply_replacements;
use tractor_core::xpath_upsert::update_only;
use tractor_core::detect_language;
use crate::cli::UpdateArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched};

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

    // Try the update-only path (render-reparse-splice) for languages with renderers.
    // Fall back to the legacy apply_replacements for languages without renderers.
    let lang_override = ctx.lang.as_deref();
    let mut files_modified = 0;
    let mut total_ops = 0;
    let mut fallback_files: Vec<String> = Vec::new();

    for file_path in files {
        let lang = lang_override
            .unwrap_or_else(|| detect_language(file_path));

        let source = std::fs::read_to_string(file_path)?;

        match update_only(&source, lang, xpath_expr, &args.value, ctx.limit) {
            Ok(result) => {
                if result.source != source {
                    std::fs::write(file_path, &result.source)?;
                    files_modified += 1;
                    total_ops += result.matches_updated;
                    eprintln!("Updated {} match(es) in {}", result.matches_updated, file_path);
                }
            }
            Err(tractor_core::xpath_upsert::UpsertError::UnsupportedLanguage(_)) => {
                fallback_files.push(file_path.clone());
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Legacy fallback for languages without renderers
    if !fallback_files.is_empty() {
        let (_, matches) = query_files_batched(&ctx, &fallback_files, xpath_expr, true)?;
        let summary = apply_replacements(&matches, &args.value)?;
        files_modified += summary.files_modified;
        total_ops += summary.replacements_made;
    }

    if total_ops == 0 {
        return Err("update matched no nodes".into());
    }

    eprintln!(
        "Updated {} match{} in {} file{}",
        total_ops,
        if total_ops == 1 { "" } else { "es" },
        files_modified,
        if files_modified == 1 { "" } else { "s" },
    );
    Ok(())
}
