use std::collections::HashSet;
use tractor_core::report::{Report, Summary};
use clap::CommandFactory;
use crate::cli::{Cli, QueryArgs};
use crate::executor::{self, ExecuteOptions, Operation, QueryOperation};
use crate::pipeline::{
    RunContext, ViewField, InputMode,
    query_inline_source, run_debug, match_to_report_match,
    project_report, apply_message_template,
};
use crate::pipeline::format::render_query_report;

pub fn run_query(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::File, ViewField::Line, ViewField::Tree], args.view.as_deref(), args.message, args.content, args.debug, false,
    )?;

    if let InputMode::Files(ref files) = ctx.input {
        if files.is_empty() {
            Cli::command().print_help().ok();
            println!();
            return Ok(());
        }
    }

    // Debug mode — needs the full parsed document, stays on existing pipeline.
    if ctx.debug {
        if let (Some(ref xpath), InputMode::Files(ref files)) = (&ctx.xpath, &ctx.input) {
            return run_debug(&ctx, files, xpath);
        }
    }

    // Explore (no XPath) = query with implicit "/*" — selects the document root.
    let xpath_expr = ctx.xpath.as_deref().unwrap_or("/*");

    match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            // Inline source stays on existing pipeline (no files to resolve).
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            if ctx.view.has(ViewField::Count) {
                println!("{}", matches.len());
            } else if ctx.view.has(ViewField::Schema) {
                crate::pipeline::print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
            } else {
                let report = build_query_report(matches, &ctx);
                let report = if ctx.group_by_file { report.with_groups() } else { report };
                render_query_report(&report, &ctx)?;
            }
        }
        InputMode::Files(files) => {
            // Delegate file-based queries to the executor.
            let op = Operation::Query(QueryOperation {
                files: files.clone(),
                exclude: vec![],
                xpath: xpath_expr.to_string(),
                tree_mode: ctx.tree_mode,
                language: ctx.lang.clone(),
                limit: ctx.limit,
                ignore_whitespace: ctx.ignore_whitespace,
                parse_depth: ctx.parse_depth,
            });

            let options = ExecuteOptions {
                verbose: ctx.verbose,
                ..Default::default()
            };

            let reports = executor::execute(&[op], &options)?;
            let mut report = reports.into_iter().next().unwrap();

            if ctx.view.has(ViewField::Count) {
                println!("{}", report.summary.as_ref().unwrap().total);
            } else if ctx.view.has(ViewField::Schema) {
                // Extract xml_nodes from report matches for schema collection.
                let nodes: Vec<_> = report.matches.iter()
                    .filter_map(|m| m.tree.as_ref())
                    .collect();
                let mut collector = tractor_core::SchemaCollector::new();
                for node in nodes {
                    collector.collect_from_xml_node(node);
                }
                print!("{}", collector.format(ctx.schema_depth(), ctx.use_color));
            } else {
                // Set the query field in summary if requested.
                if ctx.view.has(ViewField::Query) {
                    if let Some(ref mut summary) = report.summary {
                        summary.query = ctx.xpath.clone();
                    }
                }

                // Apply CLI message template if provided.
                if let Some(ref template) = ctx.message {
                    apply_message_template(&mut report, template);
                }

                // Project for the requested view and render.
                project_report(&mut report, &ctx.view);
                let report = if ctx.group_by_file { report.with_groups() } else { report };
                render_query_report(&report, &ctx)?;
            }
        }
    }
    Ok(())
}

/// Build a Report from query matches for inline source mode (existing pipeline).
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
        query: ctx.view.has(ViewField::Query).then(|| ctx.xpath.clone()).flatten(),
    };
    Report::query(report_matches, summary)
}
