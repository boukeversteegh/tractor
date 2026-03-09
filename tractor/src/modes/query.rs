use tractor_core::OutputFormat;
use clap::CommandFactory;
use crate::cli::{Cli, QueryArgs};
use crate::pipeline::{RunContext, InputMode, query_inline_source, query_files_batched, output_query_results, explore_inline, explore_files, run_debug};

pub fn run_query(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.output, args.message, args.content, false, args.debug,
    )?;

    if let InputMode::Files(ref files) = ctx.input {
        if files.is_empty() {
            Cli::command().print_help().ok();
            println!();
            return Ok(());
        }
    }

    // Debug mode
    if ctx.debug {
        if let (Some(ref xpath), InputMode::Files(ref files)) = (&ctx.xpath, &ctx.input) {
            return run_debug(&ctx, files, xpath);
        }
    }

    match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            if let Some(ref xpath_expr) = ctx.xpath {
                let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
                output_query_results(&ctx, &matches);
            } else {
                explore_inline(&ctx, source, lang)?;
            }
        }
        InputMode::Files(files) => {
            if let Some(ref xpath_expr) = ctx.xpath {
                let collect = matches!(ctx.format, OutputFormat::Schema);
                let (count, matches) = query_files_batched(&ctx, files, xpath_expr, collect)?;
                if matches!(ctx.format, OutputFormat::Count) {
                    println!("{}", count);
                } else if matches!(ctx.format, OutputFormat::Schema) {
                    crate::pipeline::print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
                }
            } else {
                explore_files(&ctx, files)?;
            }
        }
    }
    Ok(())
}
