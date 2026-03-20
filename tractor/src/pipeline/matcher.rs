use std::collections::HashSet;

use rayon::prelude::*;
use tractor_core::{
    XPathEngine, Match,
    SchemaCollector,
    output::{render_document, RenderOptions},
    parse_to_documents, parse_string_to_documents,
    report::{ReportMatch, Severity},
};

use super::context::RunContext;
use super::format::{ViewField, ViewSet};

// ---------------------------------------------------------------------------
// Report-match builder
// ---------------------------------------------------------------------------

/// Convert a raw `Match` into a `ReportMatch`, populating only the content
/// fields that are present in `view`. The `Match` (including source_lines and
/// xml_fragment) is consumed and dropped after extraction.
pub fn match_to_report_match(
    m: Match,
    view: &ViewSet,
    reason: Option<String>,
    severity: Option<Severity>,
    message: Option<String>,
) -> ReportMatch {
    // Always keep structured data (Map/Array) — it's the only representation.
    // For XML elements, only keep when the view requests tree.
    let tree = match &m.xml_node {
        Some(node) if matches!(node,
            tractor_core::xpath::XmlNode::Map { .. } |
            tractor_core::xpath::XmlNode::Array { .. }
        ) => m.xml_node.clone(),
        // Keep tree when explicitly requested OR when lines are selected (needed for syntax highlighting).
        _ => if view.has(ViewField::Tree) || view.has(ViewField::Lines) || view.has(ViewField::Source) {
            m.xml_node.clone()
        } else {
            None
        },
    };
    let value  = view.has(ViewField::Value)
                     .then(|| m.value.clone());
    let source = view.has(ViewField::Source)
                     .then(|| m.extract_source_snippet());
    let lines  = view.has(ViewField::Lines)
                     .then(|| m.get_source_lines_range()
                               .into_iter()
                               .map(|l| l.trim_end_matches('\r').to_owned())
                               .collect());

    ReportMatch {
        file:       m.file.clone(),
        line:       m.line,
        column:     m.column,
        end_line:   m.end_line,
        end_column: m.end_column,
        tree,
        value,
        source,
        lines,
        reason,
        severity,
        message,
        rule_id: None,
    }
}

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
        source, lang, "<stdin>".to_string(), ctx.tree_mode, ctx.ignore_whitespace
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
                    ctx.tree_mode,
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
        }
        // collect=false: streaming placeholder for future large-repo optimization
    }

    Ok((total_matches, all_matches))
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

pub fn print_schema_from_matches(matches: &[Match], depth: Option<usize>, use_color: bool) {
    let mut collector = SchemaCollector::new();
    for m in matches {
        if let Some(ref node) = m.xml_node {
            collector.collect_from_xml_node(node);
        }
    }
    print!("{}", collector.format(depth, use_color));
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
            ctx.tree_mode,
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
                    .with_meta(true)
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
