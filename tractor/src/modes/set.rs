use tractor_core::apply_replacements;
use tractor_core::xpath_upsert::upsert;
use tractor_core::detect_language;
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

    // Try the upsert path (render-reparse-splice) for languages with renderers.
    // Fall back to the legacy apply_replacements for languages without renderers.
    let lang_override = ctx.lang.as_deref();
    let mut files_modified = 0;
    let mut total_ops = 0;
    let mut fallback_files: Vec<String> = Vec::new();

    let limit = ctx.limit;

    for file_path in files {
        let lang = lang_override
            .unwrap_or_else(|| detect_language(file_path));

        let mut source = std::fs::read_to_string(file_path)?;
        let mut file_ops = 0;
        let mut file_changed = false;
        let mut last_inserted = false;

        // Loop to handle multiple matches. Each upsert call handles one match,
        // re-parsing the modified source each time. Stop when no more matches
        // are found or the limit is reached.
        loop {
            if let Some(lim) = limit {
                if file_ops >= lim {
                    break;
                }
            }

            match upsert(&source, lang, xpath_expr, &args.value) {
                Ok(result) => {
                    if result.source == source {
                        // No change — either value was already correct or no match
                        break;
                    }
                    source = result.source;
                    file_changed = true;
                    file_ops += 1;
                    last_inserted = result.inserted;

                    // Inserts only happen once (XPath now matches after insert)
                    if result.inserted {
                        break;
                    }
                }
                Err(tractor_core::xpath_upsert::UpsertError::UnsupportedLanguage(_)) => {
                    fallback_files.push(file_path.clone());
                    break;
                }
                Err(tractor_core::xpath_upsert::UpsertError::NoInsertionPoint(_)) if file_ops > 0 => {
                    // All matches already updated; XPath no longer matches
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        if file_changed {
            std::fs::write(file_path, &source)?;
            files_modified += 1;
            total_ops += file_ops;
            let action = if last_inserted { "Inserted" } else { "Updated" };
            eprintln!("{} {} match(es) in {}", action, file_ops, file_path);
        }
    }

    // Legacy fallback for languages without renderers
    if !fallback_files.is_empty() {
        let (_, matches) = query_files_batched(&ctx, &fallback_files, xpath_expr, true)?;
        let summary = apply_replacements(&matches, &args.value)?;
        files_modified += summary.files_modified;
        total_ops += summary.replacements_made;
    }

    eprintln!(
        "Set {} match{} in {} file{}",
        total_ops,
        if total_ops == 1 { "" } else { "es" },
        files_modified,
        if files_modified == 1 { "" } else { "s" },
    );
    Ok(())
}
