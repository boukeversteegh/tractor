use std::collections::HashSet;
use tractor_core::report::{Report, Summary};
use clap::CommandFactory;
use crate::cli::{Cli, QueryArgs};
use crate::pipeline::{RunContext, ViewField, InputMode, query_inline_source, query_files_batched, run_debug, match_to_report_match};
use crate::pipeline::format::render_query_report;

pub fn run_query(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::File, ViewField::Line, ViewField::Tree], args.view.as_deref(), args.message, args.content, args.debug,
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

    // Explore (no XPath) = query with implicit "/*" — selects the document root of each file.
    // Same pipeline, same output, same -f/-v flags.
    let xpath_expr = ctx.xpath.as_deref().unwrap_or("/*");

    match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            if ctx.view.has(ViewField::Count) {
                println!("{}", matches.len());
            } else if ctx.view.has(ViewField::Schema) {
                crate::pipeline::print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
            } else {
                let report = build_query_report(matches, &ctx);
                render_query_report(&report, &ctx)?;
            }
        }
        InputMode::Files(files) => {
            let (count, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;
            if ctx.view.has(ViewField::Count) {
                println!("{}", count);
            } else if ctx.view.has(ViewField::Schema) {
                crate::pipeline::print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
            } else {
                let report = build_query_report(matches, &ctx);
                render_query_report(&report, &ctx)?;
            }
        }
    }
    Ok(())
}

/// Build a Report from query matches (no reason/severity — query mode).
pub(crate) fn build_query_report(matches: Vec<tractor_core::Match>, ctx: &RunContext) -> Report {
    let message_template = ctx.message.as_deref();
    let mut files_seen = HashSet::new();
    for m in &matches {
        files_seen.insert(m.file.clone());
    }
    let files_affected = files_seen.len();
    let total = matches.len();

    let report_matches = matches.into_iter()
        .map(|m| {
            let message = message_template.map(|t| tractor_core::format_message(t, &m));
            match_to_report_match(m, &ctx.view, None, None, message)
        })
        .collect();

    let summary = Summary {
        passed: true,
        total,
        files_affected,
        errors: 0,
        warnings: 0,
        expected: None,
    };
    Report::query(report_matches, summary)
}
