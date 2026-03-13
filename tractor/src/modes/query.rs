use std::collections::HashSet;
use tractor_core::report::{Report, ReportMatch, Summary};
use clap::CommandFactory;
use crate::cli::{Cli, QueryArgs};
use crate::pipeline::{RunContext, OutputFormat, ViewField, InputMode, view, query_inline_source, query_files_batched, explore_inline, explore_files, run_debug};
use crate::pipeline::format::{render_gcc, render_github, render_xml_report, render_json_report, render_yaml_report};

pub fn run_query(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, view::TREE, args.view.as_deref(), args.message, args.content, false, args.debug,
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
                if ctx.view.has(ViewField::Count) {
                    println!("{}", matches.len());
                } else if ctx.view.has(ViewField::Schema) {
                    crate::pipeline::print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
                } else {
                    render_query_output(&ctx, matches)?;
                }
            } else {
                explore_inline(&ctx, source, lang)?;
            }
        }
        InputMode::Files(files) => {
            if let Some(ref xpath_expr) = ctx.xpath {
                let (count, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;

                if ctx.view.has(ViewField::Count) {
                    println!("{}", count);
                } else if ctx.view.has(ViewField::Schema) {
                    crate::pipeline::print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
                } else {
                    render_query_output(&ctx, matches)?;
                }
            } else {
                explore_files(&ctx, files)?;
            }
        }
    }
    Ok(())
}

/// Build a Report from query matches (no reason/severity — query mode).
fn build_query_report(matches: Vec<tractor_core::Match>, message_template: Option<&str>) -> Report {
    let report_matches: Vec<ReportMatch> = matches.iter()
        .map(|m| {
            let message = message_template.map(|t| tractor_core::format_message(t, m));
            let mut rm = ReportMatch::from_match(m.clone());
            rm.message = message;
            rm
        })
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
    Report::query(report_matches, summary)
}

/// Render query results to stdout based on the current OutputFormat.
fn render_query_output(ctx: &RunContext, matches: Vec<tractor_core::Match>) -> Result<(), Box<dyn std::error::Error>> {
    let template = ctx.options.message.as_deref();
    match ctx.output_format {
        OutputFormat::Json => {
            let report = build_query_report(matches, template);
            print!("{}", render_json_report(&report, &ctx.view, &ctx.render_options()));
        }
        OutputFormat::Yaml => {
            let report = build_query_report(matches, template);
            print!("{}", render_yaml_report(&report, &ctx.view, &ctx.render_options()));
        }
        OutputFormat::Xml => {
            let report = build_query_report(matches, template);
            print!("{}", render_xml_report(&report, &ctx.view, &ctx.render_options()));
        }
        OutputFormat::Gcc => {
            let report = build_query_report(matches, template);
            print!("{}", render_gcc(&report));
        }
        OutputFormat::Github => {
            let report = build_query_report(matches, template);
            print!("{}", render_github(&report));
        }
        OutputFormat::Text => {
            let report = build_query_report(matches, template);
            let inner: Vec<_> = report.matches.iter().map(|rm| rm.inner.clone()).collect();
            if template.is_some() {
                for rm in &report.matches {
                    if let Some(ref msg) = rm.message {
                        println!("{}", msg);
                    }
                }
            } else {
                let output = tractor_core::format_matches(&inner, ctx.view.primary_output_format(), &ctx.options);
                print!("{}", output);
            }
        }
    }
    Ok(())
}
