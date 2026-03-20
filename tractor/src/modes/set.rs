use tractor_core::{apply_replacements, apply_replacements_to_content, compute_replacements_stdout};
use crate::cli::SetArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched, query_inline_source};

pub fn run_set(args: SetArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Early normalization: if stdin is provided as source input (--lang set, no files,
    // stdin is not a TTY), implicitly enable stdout mode — there is no file to modify.
    let stdin_source = args.files.is_empty()
        && args.shared.lang.is_some()
        && !atty::is(atty::Stream::Stdin);
    let stdout = args.stdout || stdin_source;

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        "text", &[ViewField::Tree], None, None, None, false, false,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("set requires an XPath query (-x)")?;

    if stdout {
        // Stdout mode: apply replacements and write result to stdout.
        // Both explicit --stdout and implicit (stdin input) share this single code path.
        match &ctx.input {
            InputMode::Files(files) => {
                let (_, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;
                for (_, content) in compute_replacements_stdout(files, &matches, &args.value)? {
                    print!("{}", content);
                }
            }
            InputMode::InlineSource { source, lang } => {
                let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
                let modified = apply_replacements_to_content(source, &matches, &args.value)?;
                print!("{}", modified);
            }
        }
    } else {
        let files = match &ctx.input {
            InputMode::Files(files) => files,
            InputMode::InlineSource { .. } => {
                return Err("set cannot be used with stdin input (no file to modify). Use --stdout to write to stdout, or pipe source with --lang for implicit stdout.".into());
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
    }
    Ok(())
}
