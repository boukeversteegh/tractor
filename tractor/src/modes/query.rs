use std::collections::HashSet;
use tractor_core::{OutputFormat, report::{Report, ReportMatch, Summary}};
use clap::CommandFactory;
use crate::cli::{Cli, QueryArgs};
use crate::pipeline::{RunContext, SerFormat, InputMode, view, query_inline_source, query_files_batched, output_query_results, explore_inline, explore_files, run_debug};

pub fn run_query(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let default_view = view::TREE;
    let view = args.view.as_deref().unwrap_or(default_view);

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, Some(view), args.message, args.content, false, args.debug,
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
                render_query_output(&ctx, matches)?;
            } else {
                explore_inline(&ctx, source, lang)?;
            }
        }
        InputMode::Files(files) => {
            if let Some(ref xpath_expr) = ctx.xpath {
                // When serializing as JSON, we must collect all matches for the report
                let needs_collect = ctx.ser_format == SerFormat::Json
                    || matches!(ctx.view, OutputFormat::Schema);
                let (count, matches) = query_files_batched(&ctx, files, xpath_expr, needs_collect)?;

                if ctx.ser_format == SerFormat::Json {
                    render_query_output(&ctx, matches)?;
                } else if matches!(ctx.view, OutputFormat::Count) {
                    println!("{}", count);
                } else if matches!(ctx.view, OutputFormat::Schema) {
                    crate::pipeline::print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
                }
                // else: already streamed by query_files_batched
            } else {
                explore_files(&ctx, files)?;
            }
        }
    }
    Ok(())
}

/// Render query results — either as JSON report envelope or via text view.
fn render_query_output(ctx: &RunContext, matches: Vec<tractor_core::Match>) -> Result<(), Box<dyn std::error::Error>> {
    if ctx.ser_format == SerFormat::Json {
        // Build report and serialize as JSON
        let report_matches: Vec<ReportMatch> = matches.iter()
            .map(|m| ReportMatch::from_match(m.clone()))
            .collect();
        let mut files_seen = HashSet::new();
        for m in &matches {
            files_seen.insert(&m.file);
        }
        let summary = Summary {
            passed: true,
            total: matches.len(),
            files_affected: files_seen.len(),
            errors: 0,
            warnings: 0,
            expected: None,
        };
        let report = Report::query(report_matches, summary);
        print!("{}", report.to_json());
    } else {
        output_query_results(&ctx, &matches);
    }
    Ok(())
}
