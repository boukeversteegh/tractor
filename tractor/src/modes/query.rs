use clap::CommandFactory;
use crate::cli::{Cli, QueryArgs};
use crate::executor::{self, ExecuteOptions, Operation, QueryOperation, QueryExpr};
use crate::pipeline::{
    RunContext, ViewField, InputMode,
    run_debug,
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

    // Build the query operation for either files or inline source.
    let op = match &ctx.input {
        InputMode::Files(files) => Operation::Query(QueryOperation {
            files: files.clone(),
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            queries: vec![QueryExpr { xpath: xpath_expr.to_string() }],
            tree_mode: ctx.tree_mode,
            language: ctx.lang.clone(),
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
            inline_source: None,
            inline_lang: None,
        }),
        InputMode::InlineSource { source, lang } => Operation::Query(QueryOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            queries: vec![QueryExpr { xpath: xpath_expr.to_string() }],
            tree_mode: ctx.tree_mode,
            language: None,
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
            inline_source: Some(source.clone()),
            inline_lang: Some(lang.clone()),
        }),
    };

    let options = ExecuteOptions {
        verbose: ctx.verbose,
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        ..Default::default()
    };

    let reports = executor::execute(&[op], &options)?;
    let mut report = reports.into_iter().next().unwrap();

    if ctx.view.has(ViewField::Count) {
        println!("{}", report.summary.as_ref().unwrap().total);
    } else if ctx.view.has(ViewField::Schema) {
        let mut collector = tractor_core::SchemaCollector::new();
        for m in &report.matches {
            if let Some(ref node) = m.tree {
                collector.collect_from_xml_node(node);
            }
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

    Ok(())
}
