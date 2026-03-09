use std::io::{self, Write};
use std::collections::HashSet;

use rayon::prelude::*;
use tractor_core::{
    XPathEngine, Match,
    OutputFormat, format_matches, SchemaCollector,
    output::{render_document, render_node, RenderOptions},
    parse_to_documents, parse_string_to_documents,
    XeeParseResult,
};

use super::context::RunContext;

// ---------------------------------------------------------------------------
// Batch utility
// ---------------------------------------------------------------------------

/// Split a slice into exponentially growing batches, capped at a maximum.
pub fn exponential_batches<T>(items: &[T], num_threads: usize) -> Vec<&[T]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut batch_size = num_threads.max(1);
    let max_batch_size = num_threads * 8;

    while start < items.len() {
        let end = (start + batch_size).min(items.len());
        batches.push(&items[start..end]);
        start = end;
        batch_size = (batch_size * 2).min(max_batch_size);
    }

    batches
}

// ---------------------------------------------------------------------------
// Query pipeline
// ---------------------------------------------------------------------------

pub fn query_inline_source(
    ctx: &RunContext,
    source: &str,
    lang: &str,
    xpath_expr: &str,
) -> Result<Vec<Match>, Box<dyn std::error::Error>> {
    let mut result = parse_string_to_documents(
        source, lang, "<stdin>".to_string(), ctx.raw, ctx.ignore_whitespace
    )?;

    let engine = XPathEngine::new()
        .with_verbose(ctx.verbose)
        .with_ignore_whitespace(ctx.ignore_whitespace);

    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        xpath_expr,
        result.source_lines.clone(),
        &result.file_path,
    )?;

    let matches = if let Some(limit) = ctx.limit {
        matches.into_iter().take(limit).collect()
    } else {
        matches
    };

    Ok(matches)
}

pub fn query_files_batched(
    ctx: &RunContext,
    files: &[String],
    xpath_expr: &str,
    collect: bool,
) -> Result<(usize, Vec<Match>), Box<dyn std::error::Error>> {
    let batches = exponential_batches(files, ctx.concurrency);
    let mut total_matches = 0usize;
    let mut remaining_limit = ctx.limit;
    let mut all_matches: Vec<Match> = Vec::new();
    let is_count_format = matches!(ctx.format, OutputFormat::Count);

    for batch in batches {
        if remaining_limit == Some(0) {
            break;
        }

        let mut batch_matches: Vec<Match> = batch
            .par_iter()
            .filter_map(|file_path| {
                let mut result = match parse_to_documents(
                    std::path::Path::new(file_path),
                    ctx.lang.as_deref(),
                    ctx.raw,
                    ctx.ignore_whitespace,
                    ctx.parse_depth,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        if ctx.verbose {
                            eprintln!("warning: {}: {}", file_path, e);
                        }
                        return None;
                    }
                };

                let engine = XPathEngine::new()
                    .with_verbose(ctx.verbose)
                    .with_ignore_whitespace(ctx.ignore_whitespace);
                match engine.query_documents(
                    &mut result.documents,
                    result.doc_handle,
                    xpath_expr,
                    result.source_lines.clone(),
                    &result.file_path,
                ) {
                    Ok(matches) => Some(matches),
                    Err(e) => {
                        if ctx.verbose {
                            eprintln!("warning: {}: query error: {}", file_path, e);
                        }
                        None
                    }
                }
            })
            .flatten()
            .collect();

        batch_matches.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

        if let Some(limit) = remaining_limit {
            batch_matches.truncate(limit);
            remaining_limit = Some(limit.saturating_sub(batch_matches.len()));
        }

        total_matches += batch_matches.len();

        if collect {
            all_matches.extend(batch_matches);
        } else if !is_count_format {
            let output = format_matches(&batch_matches, ctx.format.clone(), &ctx.options);
            print!("{}", output);
            io::stdout().flush().ok();
        }
    }

    Ok((total_matches, all_matches))
}

// ---------------------------------------------------------------------------
// Explore (no XPath)
// ---------------------------------------------------------------------------

pub fn explore_files(ctx: &RunContext, files: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let lang_override = ctx.lang.as_deref();
    let raw = ctx.raw;
    let verbose = ctx.verbose;

    if matches!(ctx.format, OutputFormat::Count) {
        let count: usize = files
            .par_iter()
            .filter(|file_path| {
                match parse_to_documents(
                    std::path::Path::new(file_path),
                    lang_override, raw, ctx.ignore_whitespace, ctx.parse_depth,
                ) {
                    Ok(_) => true,
                    Err(e) => {
                        if verbose { eprintln!("warning: {}", e); }
                        false
                    }
                }
            })
            .count();
        println!("{}", count);
        return Ok(());
    }

    if matches!(ctx.format, OutputFormat::Schema) {
        let collectors: Vec<SchemaCollector> = files
            .par_iter()
            .filter_map(|file_path| {
                match parse_to_documents(
                    std::path::Path::new(file_path),
                    lang_override, raw, ctx.ignore_whitespace, ctx.parse_depth,
                ) {
                    Ok(result) => {
                        let mut collector = SchemaCollector::new();
                        let doc_node = result.documents.document_node(result.doc_handle)?;
                        collector.collect_from_xot(result.documents.xot(), doc_node);
                        Some(collector)
                    }
                    Err(e) => {
                        if verbose { eprintln!("warning: {}", e); }
                        None
                    }
                }
            })
            .collect();

        let mut final_collector = SchemaCollector::new();
        for collector in collectors {
            final_collector.merge(collector);
        }
        print!("{}", final_collector.format(ctx.schema_depth(), ctx.use_color));
        return Ok(());
    }

    let parse_results: Vec<XeeParseResult> = files
        .iter()
        .filter_map(|file_path| {
            match parse_to_documents(
                std::path::Path::new(file_path),
                lang_override, raw, ctx.ignore_whitespace, ctx.parse_depth,
            ) {
                Ok(r) => Some(r),
                Err(e) => {
                    if verbose { eprintln!("warning: {}", e); }
                    None
                }
            }
        })
        .collect();

    if parse_results.is_empty() {
        return Err("no files could be parsed".into());
    }

    let render_opts = ctx.render_options();

    if matches!(ctx.format, OutputFormat::Xml) {
        if parse_results.len() == 1 {
            let result = &parse_results[0];
            let doc_node = result.documents.document_node(result.doc_handle).unwrap();
            let xot = result.documents.xot();
            for child in xot.children(doc_node) {
                let output = render_node(xot, child, &render_opts);
                print!("{}", output);
            }
        } else {
            println!(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
            println!("<Files>");
            for result in &parse_results {
                let doc_node = result.documents.document_node(result.doc_handle).unwrap();
                let xot = result.documents.xot();
                for files_child in xot.children(doc_node) {
                    for file_child in xot.children(files_child) {
                        let output = render_node(xot, file_child, &render_opts);
                        if !ctx.no_pretty {
                            for line in output.lines() {
                                println!("  {}", line);
                            }
                        } else {
                            print!("{}", output);
                        }
                    }
                }
            }
            println!("</Files>");
        }
        return Ok(());
    }

    let matches: Vec<Match> = parse_results
        .iter()
        .map(|result| {
            let end_line = result.source_lines.len() as u32;
            let end_column = result.source_lines.last()
                .map(|l| l.len() as u32 + 1)
                .unwrap_or(1);

            let doc_node = result.documents.document_node(result.doc_handle).unwrap();
            let xot = result.documents.xot();
            let xml: String = xot.children(doc_node)
                .map(|child| render_node(xot, child, &render_opts))
                .collect();
            let value = extract_text_content(&xml);

            Match::with_location(
                result.file_path.clone(),
                1, 1,
                end_line.max(1), end_column,
                value,
                result.source_lines.clone(),
            ).with_xml_fragment(xml)
        })
        .collect();

    let output = format_matches(&matches, ctx.format.clone(), &ctx.options);
    print!("{}", output);

    Ok(())
}

pub fn explore_inline(ctx: &RunContext, source: &str, lang: &str) -> Result<(), Box<dyn std::error::Error>> {
    let result = parse_string_to_documents(
        source, lang, "<stdin>".to_string(), ctx.raw, ctx.ignore_whitespace
    )?;

    let render_opts = ctx.render_options();

    if matches!(ctx.format, OutputFormat::Xml) {
        let doc_node = result.documents.document_node(result.doc_handle).unwrap();
        let xot = result.documents.xot();
        for child in xot.children(doc_node) {
            let output = render_node(xot, child, &render_opts);
            print!("{}", output);
        }
        return Ok(());
    }

    if matches!(ctx.format, OutputFormat::Schema) {
        let doc_node = result.documents.document_node(result.doc_handle).unwrap();
        let mut collector = SchemaCollector::new();
        collector.collect_from_xot(result.documents.xot(), doc_node);
        print!("{}", collector.format(ctx.schema_depth(), ctx.use_color));
        return Ok(());
    }

    let doc_node = result.documents.document_node(result.doc_handle).unwrap();
    let xot = result.documents.xot();
    let xml: String = xot.children(doc_node)
        .map(|child| render_node(xot, child, &render_opts))
        .collect();
    let value = extract_text_content(&xml);

    let end_line = result.source_lines.len() as u32;
    let end_column = result.source_lines.last()
        .map(|l| l.len() as u32 + 1)
        .unwrap_or(1);

    let file_match = Match::with_location(
        result.file_path.clone(),
        1, 1,
        end_line.max(1), end_column,
        value,
        result.source_lines.clone(),
    ).with_xml_fragment(xml);

    let output = format_matches(&[file_match], ctx.format.clone(), &ctx.options);
    print!("{}", output);
    Ok(())
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

pub fn output_query_results(ctx: &RunContext, matches: &[Match]) {
    if matches!(ctx.format, OutputFormat::Schema) {
        print_schema_from_matches(matches, ctx.schema_depth(), ctx.use_color);
        return;
    }
    let output = format_matches(matches, ctx.format.clone(), &ctx.options);
    print!("{}", output);
}

pub fn print_schema_from_matches(matches: &[Match], depth: Option<usize>, use_color: bool) {
    let mut collector = SchemaCollector::new();
    for m in matches {
        if let Some(xml) = &m.xml_fragment {
            collector.collect_from_xml_string(xml);
        }
    }
    print!("{}", collector.format(depth, use_color));
}

pub fn extract_text_content(xml: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Debug mode
// ---------------------------------------------------------------------------

pub fn run_debug(ctx: &RunContext, files: &[String], xpath_expr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut remaining_limit = ctx.limit;

    for file_path in files {
        if remaining_limit == Some(0) {
            break;
        }

        let mut result = match parse_to_documents(
            std::path::Path::new(file_path),
            ctx.lang.as_deref(),
            ctx.raw,
            ctx.ignore_whitespace,
            ctx.parse_depth,
        ) {
            Ok(r) => r,
            Err(e) => {
                if ctx.verbose {
                    eprintln!("warning: {}: {}", file_path, e);
                }
                continue;
            }
        };

        let engine = XPathEngine::new()
            .with_verbose(ctx.verbose)
            .with_ignore_whitespace(ctx.ignore_whitespace);

        match engine.query_documents(
            &mut result.documents,
            result.doc_handle,
            xpath_expr,
            result.source_lines.clone(),
            &result.file_path,
        ) {
            Ok(matches) if !matches.is_empty() => {
                let matches: Vec<_> = if let Some(limit) = remaining_limit {
                    let take = limit.min(matches.len());
                    remaining_limit = Some(limit - take);
                    matches.into_iter().take(take).collect()
                } else {
                    matches
                };

                let highlights: HashSet<(u32, u32)> = matches
                    .iter()
                    .map(|m| (m.line, m.column))
                    .collect();

                let doc_node = result.documents.document_node(result.doc_handle).unwrap();
                let render_opts = RenderOptions::new()
                    .with_color(ctx.use_color)
                    .with_locations(true)
                    .with_max_depth(ctx.depth)
                    .with_highlights(highlights)
                    .with_pretty_print(!ctx.no_pretty);
                let output = render_document(result.documents.xot(), doc_node, &render_opts);
                print!("{}", output);
            }
            Ok(_) => {}
            Err(e) => {
                if ctx.verbose {
                    eprintln!("warning: {}: query error: {}", file_path, e);
                }
            }
        }
    }

    Ok(())
}
