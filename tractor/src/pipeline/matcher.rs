use std::collections::HashSet;

use rayon::prelude::*;
use tractor_core::{
    Match,
    output::{render_document, RenderOptions},
    parse_to_documents, parse_string_to_documents,
    report::Report,
    rule::{RuleSet, GlobMatcher},
};
use crate::filter::ResultFilter;

use super::context::RunContext;
use super::format::{ViewField, ViewSet};

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

    let matches = result.query(xpath_expr)?;

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

                match result.query(xpath_expr) {
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

        match result.query(xpath_expr) {
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

// ---------------------------------------------------------------------------
// Batch rule execution
// ---------------------------------------------------------------------------

/// A match tagged with its originating rule, ready for report building.
pub struct RuleMatch {
    pub rule_index: usize,
    pub m: Match,
}

/// Precomputed per-rule state: the glob matcher and the XPath expression.
struct CompiledRule {
    glob: GlobMatcher,
    xpath: String,
}

/// Execute all rules in a `RuleSet` against a list of files.
///
/// Each file is parsed once. Every applicable rule (determined by glob
/// intersection) is run against the parsed document. Returns matches
/// tagged with their originating rule index.
///
/// `verbose` controls whether parse/query warnings are printed to stderr.
pub fn run_rules(
    ruleset: &RuleSet,
    files: &[String],
    tree_mode: Option<tractor_core::TreeMode>,
    ignore_whitespace: bool,
    parse_depth: Option<usize>,
    verbose: bool,
    filters: &[&dyn ResultFilter],
) -> Result<Vec<RuleMatch>, Box<dyn std::error::Error>> {
    // Compile glob matchers for each rule upfront.
    let compiled: Vec<CompiledRule> = ruleset
        .rules
        .iter()
        .map(|rule| {
            let glob = ruleset.glob_matcher(rule)?;
            Ok(CompiledRule {
                glob,
                xpath: rule.xpath.clone(),
            })
        })
        .collect::<Result<Vec<_>, tractor_core::rule::GlobError>>()?;

    // Process files in parallel. Each file is parsed once, then all
    // applicable rules are queried against it.
    let results: Vec<Vec<RuleMatch>> = files
        .par_iter()
        .filter_map(|file_path| {
            // Determine which rules apply to this file.
            let applicable: Vec<usize> = compiled
                .iter()
                .enumerate()
                .filter(|(_, cr)| cr.glob.matches(file_path))
                .map(|(i, _)| i)
                .collect();

            if applicable.is_empty() {
                return None;
            }

            // Resolve per-file language/tree_mode. Uses the first applicable
            // rule's overrides or the ruleset defaults. If rules specify
            // different tree_mode/language for the same file, only the first
            // rule's settings apply — a future improvement could group rules
            // by (lang, tree_mode) and re-parse when needed.
            let first_rule = &ruleset.rules[applicable[0]];
            let lang_override = ruleset.effective_language(first_rule);
            let effective_tree_mode = ruleset.effective_tree_mode(first_rule).or(tree_mode);

            let mut result = match parse_to_documents(
                std::path::Path::new(file_path),
                lang_override,
                effective_tree_mode,
                ignore_whitespace,
                parse_depth,
            ) {
                Ok(r) => r,
                Err(e) => {
                    if verbose {
                        eprintln!("warning: {}: {}", file_path, e);
                    }
                    return None;
                }
            };

            let mut file_matches = Vec::new();

            for rule_idx in applicable {
                match result.query(&compiled[rule_idx].xpath) {
                    Ok(matches) => {
                        for m in matches {
                            file_matches.push(RuleMatch {
                                rule_index: rule_idx,
                                m,
                            });
                        }
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!(
                                "warning: {}: rule '{}' query error: {}",
                                file_path, ruleset.rules[rule_idx].id, e
                            );
                        }
                    }
                }
            }

            // Apply result filters at the query engine level.
            if !filters.is_empty() {
                file_matches.retain(|rm| filters.iter().all(|f| f.include(&rm.m)));
            }

            if file_matches.is_empty() {
                None
            } else {
                Some(file_matches)
            }
        })
        .collect();

    // Flatten and sort by file, line, column for stable output.
    let mut all_matches: Vec<RuleMatch> = results.into_iter().flatten().collect();
    all_matches.sort_by(|a, b| {
        (&a.m.file, a.m.line, a.m.column).cmp(&(&b.m.file, b.m.line, b.m.column))
    });

    Ok(all_matches)
}

// ---------------------------------------------------------------------------
// Report post-processing helpers
// ---------------------------------------------------------------------------

/// Project a report to only contain the fields requested by the view.
///
/// The executor populates all content fields. This function prunes
/// fields that are not in the view, ensuring renderers see `None`
/// for unselected fields (matching the behaviour of `match_to_report_match`).
pub fn project_report(report: &mut Report, view: &ViewSet) {
    for m in &mut report.matches {
        // Map/Array nodes are always kept — they're the only representation for data formats.
        // For other nodes, keep when tree/lines/source is selected (needed for rendering).
        let keep_tree = match &m.tree {
            Some(node) if matches!(
                node,
                tractor_core::xpath::XmlNode::Map { .. }
                    | tractor_core::xpath::XmlNode::Array { .. }
            ) => true,
            _ => view.has(ViewField::Tree) || view.has(ViewField::Lines) || view.has(ViewField::Source),
        };
        if !keep_tree {
            m.tree = None;
        }
        if !view.has(ViewField::Value) {
            m.value = None;
        }
        if !view.has(ViewField::Source) {
            m.source = None;
        }
        if !view.has(ViewField::Lines) {
            m.lines = None;
        }
        if !view.has(ViewField::Reason) {
            m.reason = None;
        }
        if !view.has(ViewField::Severity) {
            m.severity = None;
        }
        if !view.has(ViewField::Status) {
            m.status = None;
        }
    }
}

/// Apply a CLI-level message template (`-m`) to all matches in a report.
///
/// This overwrites any existing message (e.g. from rule-level templates).
/// Placeholders: `{file}`, `{line}`, `{col}`, `{value}`.
pub fn apply_message_template(report: &mut Report, template: &str) {
    if !template.contains('{') {
        // Static template — same for every match.
        let msg = template.to_string();
        for m in &mut report.matches {
            m.message = Some(msg.clone());
        }
        return;
    }

    for m in &mut report.matches {
        m.message = Some(
            template
                .replace("{file}", &tractor_core::output::normalize_path(&m.file))
                .replace("{line}", &m.line.to_string())
                .replace("{col}", &m.column.to_string())
                .replace("{value}", m.value.as_deref().unwrap_or(""))
        );
    }
}
