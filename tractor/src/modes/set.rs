use tractor_core::apply_replacements;
use tractor_core::xpath_upsert::upsert;
use tractor_core::declarative_set::declarative_set;
use tractor_core::detect_language;
use crate::cli::SetArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched};

/// Separate positional args into files and an optional path expression.
///
/// When -x is given, all positional args are files.
/// Otherwise, the last arg that looks like a path expression (contains `[`
/// or doesn't resolve to any existing file/glob) is the expression.
fn split_files_and_expr(args: &[String], has_xpath: bool) -> (Vec<String>, Option<String>) {
    if has_xpath || args.is_empty() {
        return (args.to_vec(), None);
    }

    // Check if the last arg looks like a declarative expression
    if let Some(last) = args.last() {
        let is_expr = last.contains('[')
            || last.contains('=')
            || (!std::path::Path::new(last).exists() && !last.contains('*') && !last.contains('?'));

        if is_expr {
            let files = args[..args.len() - 1].to_vec();
            return (files, Some(last.clone()));
        }
    }

    (args.to_vec(), None)
}

pub fn run_set(args: SetArgs) -> Result<(), Box<dyn std::error::Error>> {
    let has_xpath = args.shared.xpath.is_some();
    let (files, expr) = split_files_and_expr(&args.args, has_xpath);

    // Declarative mode: path expression without -x
    if let Some(expr) = &expr {
        let ctx = RunContext::build(
            &args.shared, files, None,
            "text", &[ViewField::Tree], None, None, None, false, false,
        )?;

        let file_list = match &ctx.input {
            InputMode::Files(files) => files,
            InputMode::InlineSource { .. } => {
                return Err("set cannot be used with stdin input (no file to modify)".into());
            }
        };

        let lang_override = ctx.lang.as_deref();
        let mut files_modified = 0;
        let mut total_ops = 0;

        for file_path in file_list {
            let lang = lang_override
                .unwrap_or_else(|| detect_language(file_path));

            let source = std::fs::read_to_string(file_path)?;
            let result = declarative_set(
                &source, lang, expr, args.value.as_deref(),
            )?;

            if result.source != source {
                std::fs::write(file_path, &result.source)?;
                files_modified += 1;
                total_ops += result.ops_applied;
                for desc in &result.descriptions {
                    eprintln!("  {} in {}", desc, file_path);
                }
            }
        }

        eprintln!(
            "Set {} value{} in {} file{}",
            total_ops,
            if total_ops == 1 { "" } else { "s" },
            files_modified,
            if files_modified == 1 { "" } else { "s" },
        );
        return Ok(());
    }

    // XPath mode: -x with optional --value
    let ctx = RunContext::build(
        &args.shared, files, args.shared.xpath.clone(),
        "text", &[ViewField::Tree], None, None, None, false, false,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("set requires either an XPath query (-x) or a path expression")?;

    let value = args.value.as_ref()
        .ok_or("set with -x requires --value")?;

    let file_list = match &ctx.input {
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

    for file_path in file_list {
        let lang = lang_override
            .unwrap_or_else(|| detect_language(file_path));

        let source = std::fs::read_to_string(file_path)?;

        match upsert(&source, lang, xpath_expr, value, ctx.limit) {
            Ok(result) => {
                if result.source != source {
                    std::fs::write(file_path, &result.source)?;
                    files_modified += 1;
                    let ops = if result.inserted { 1 } else { result.matches_updated };
                    total_ops += ops;
                    let action = if result.inserted { "Inserted" } else { "Updated" };
                    eprintln!("{} {} match(es) in {}", action, ops, file_path);
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
        let summary = apply_replacements(&matches, value)?;
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
