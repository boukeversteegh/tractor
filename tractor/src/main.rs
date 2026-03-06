//! tractor - Multi-language code query tool using XPath 3.1
//!
//! This is the main CLI entry point that orchestrates parsing and querying.

mod cli;
mod version;

use std::collections::HashSet;
use std::io::{self, BufRead, Read, Write};
use std::process::ExitCode;

use rayon::prelude::*;
use tractor_core::{
    XPathEngine, Match,
    OutputFormat, format_matches, OutputOptions, SchemaCollector,
    expand_globs, filter_supported_files,
    output::should_use_color,
    output::{render_document, render_node, RenderOptions},
    print_timing_stats,
    apply_replacements,
    // Unified parsing pipeline (always returns Documents)
    parse_to_documents, parse_string_to_documents,
    XeeParseResult,
};

use cli::Args;
use clap::{CommandFactory, Parser};

// ---------------------------------------------------------------------------
// Batch utility
// ---------------------------------------------------------------------------

/// Split a slice into exponentially growing batches, capped at a maximum.
/// Batch sizes: n, 2n, 4n, 8n, 8n, 8n... (where n = num_threads)
/// This provides:
/// - Fast initial output (small first batches)
/// - Consistent update frequency (~0.25s per batch)
/// - Efficient parallelism (batch size >= num_threads)
fn exponential_batches<T>(items: &[T], num_threads: usize) -> Vec<&[T]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut batch_size = num_threads.max(1);
    let max_batch_size = num_threads * 8; // Cap for consistent update frequency

    while start < items.len() {
        let end = (start + batch_size).min(items.len());
        batches.push(&items[start..end]);
        start = end;
        // Double batch size until we hit the cap
        batch_size = (batch_size * 2).min(max_batch_size);
    }

    batches
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let args = Args::parse();

    // Handle --version flag (respects --verbose for detailed output)
    if args.version {
        if args.verbose {
            version::print_version_verbose();
        } else {
            version::print_version();
        }
        return ExitCode::SUCCESS;
    }

    if let Err(e) = run(args) {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }

    // Print timing stats if TRACTOR_PROFILE env var is set
    if std::env::var("TRACTOR_PROFILE").is_ok() {
        tractor_core::print_parse_timing_stats();
        print_timing_stats();
    }

    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// XPath normalization
// ---------------------------------------------------------------------------

/// Check if running under MSYS/MinGW which mangles // paths
fn is_msys_environment() -> bool {
    std::env::var("MSYSTEM").is_ok()
}

/// Normalize XPath expression - auto-prefix with // if not starting with /
/// Also fixes MSYS/MinGW path mangling where // gets converted to /
fn normalize_xpath(xpath: &str) -> String {
    let xpath = fix_msys_xpath_mangling(xpath);

    // Don't prefix expressions that are already absolute, context-relative,
    // or full XPath 3.1 expressions (let/for/if/some/every, variable refs, literals, function calls)
    if xpath.starts_with('/')
        || xpath.starts_with('(')
        || xpath.starts_with('$')
        || xpath.starts_with('"')
        || xpath.starts_with('\'')
        || xpath == "."
        || looks_like_xpath_expression(&xpath)
    {
        xpath
    } else {
        format!("//{}", xpath)
    }
}

/// Check if the expression starts with an XPath 3.1 keyword or looks like
/// a full expression rather than a simple element name to auto-prefix with //
fn looks_like_xpath_expression(xpath: &str) -> bool {
    // XPath 3.1 expression keywords that can start an expression
    let keywords = ["let ", "let$", "for ", "for$", "if ", "if(", "some ", "some$", "every ", "every$"];
    keywords.iter().any(|kw| xpath.starts_with(kw))
        || xpath.starts_with("not(")
        || xpath.starts_with("count(")
        || xpath.starts_with("string(")
        || xpath.starts_with("contains(")
        || xpath.starts_with("starts-with(")
        || xpath.chars().next().map_or(false, |c| c.is_ascii_digit())
}

/// Fix MSYS/MinGW path conversion that mangles // to /
/// In MSYS, "//item" becomes "/item" due to UNC path conversion
fn fix_msys_xpath_mangling(xpath: &str) -> String {
    if !is_msys_environment() {
        return xpath.to_string();
    }

    // MSYS converts "//foo" to "/foo" - restore the double slash
    // Only do this for patterns like "/word" (not "//" which is already correct)
    if xpath.starts_with('/') && !xpath.starts_with("//") {
        // Check if it looks like a mangled descendant query (e.g., "/item" was "//item")
        let rest = &xpath[1..];
        if !rest.is_empty() && (rest.chars().next().unwrap().is_alphabetic() || rest.starts_with('*')) {
            return format!("/{}", xpath);
        }
    }

    xpath.to_string()
}

// ---------------------------------------------------------------------------
// Run context
// ---------------------------------------------------------------------------

enum InputMode {
    Files(Vec<String>),
    InlineSource { source: String, lang: String },
}

struct RunContext {
    args: Args,
    xpath: Option<String>,
    format: OutputFormat,
    use_color: bool,
    options: OutputOptions,
    input: InputMode,
    concurrency: usize,
}

impl RunContext {
    fn from_args(args: Args) -> Result<Self, Box<dyn std::error::Error>> {
        let xpath = args.xpath.as_ref().map(|x| normalize_xpath(x));

        let format = OutputFormat::from_str(&args.output)
            .ok_or_else(|| {
                format!(
                    "invalid format '{}'. Valid formats: {}",
                    args.output,
                    OutputFormat::valid_formats().join(", ")
                )
            })?;

        let use_color = if args.no_color {
            false
        } else {
            should_use_color(&args.color)
        };

        let mut files: Vec<String> = expand_globs(&args.files);

        // Determine input mode
        let input = if let Some(ref content) = args.content {
            if args.lang.is_none() {
                return Err("--string requires --lang to specify the language".into());
            }
            InputMode::InlineSource {
                source: content.clone(),
                lang: args.lang.clone().unwrap(),
            }
        } else if files.is_empty() && args.lang.is_some() && !atty::is(atty::Stream::Stdin) {
            // Stdin with --lang: read source from stdin
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            InputMode::InlineSource {
                source: s,
                lang: args.lang.clone().unwrap(),
            }
        } else {
            // Check for file paths on stdin
            if files.is_empty() && args.lang.is_none() && !atty::is(atty::Stream::Stdin) {
                let stdin = io::stdin();
                for line in stdin.lock().lines() {
                    if let Ok(path) = line {
                        let path = path.trim().to_string();
                        if !path.is_empty() {
                            files.push(path);
                        }
                    }
                }
            }
            files = filter_supported_files(files);
            InputMode::Files(files)
        };

        // Configure thread pool
        let concurrency = args.concurrency.unwrap_or_else(|| num_cpus::get());
        rayon::ThreadPoolBuilder::new()
            .num_threads(concurrency)
            .build_global()
            .ok();

        // Validate flag combinations
        if args.set.is_some() && xpath.is_none() {
            return Err("--set requires an XPath query (-x)".into());
        }
        if args.set.is_some() && matches!(input, InputMode::InlineSource { .. }) {
            return Err("--set cannot be used with stdin input (no file to modify)".into());
        }

        let options = OutputOptions {
            message: args.message.clone(),
            use_color,
            strip_locations: !args.keep_locations,
            max_depth: args.depth,
            pretty_print: !args.no_pretty,
            language: args.lang.clone(),
            warning: args.warning,
        };

        Ok(RunContext { args, xpath, format, use_color, options, input, concurrency })
    }

    fn render_options(&self) -> RenderOptions {
        RenderOptions::new()
            .with_color(self.use_color)
            .with_locations(self.args.keep_locations || self.args.debug)
            .with_max_depth(self.args.depth)
            .with_pretty_print(!self.args.no_pretty)
    }

    fn schema_depth(&self) -> Option<usize> {
        self.args.depth.or(Some(4))
    }
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::from_args(args)?;

    // Nothing to do?
    if let InputMode::Files(ref files) = ctx.input {
        if files.is_empty() {
            Args::command().print_help().ok();
            println!();
            return Ok(());
        }
    }

    // Debug mode (file-based only, handles --expect internally)
    if ctx.args.debug {
        if let (Some(ref xpath), InputMode::Files(ref files)) = (&ctx.xpath, &ctx.input) {
            return run_debug(&ctx, files, xpath);
        }
    }

    if ctx.args.set.is_some() {
        return run_set(&ctx);
    }

    if ctx.args.expect.is_some() {
        return run_test(&ctx);
    }

    run_query(&ctx)
}

// ---------------------------------------------------------------------------
// Mode handlers
// ---------------------------------------------------------------------------

fn run_query(ctx: &RunContext) -> Result<(), Box<dyn std::error::Error>> {
    match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            if let Some(ref xpath_expr) = ctx.xpath {
                let matches = query_inline_source(ctx, source, lang, xpath_expr)?;
                output_query_results(ctx, &matches);
            } else {
                explore_inline(ctx, source, lang)?;
            }
        }
        InputMode::Files(files) => {
            if let Some(ref xpath_expr) = ctx.xpath {
                let collect = matches!(ctx.format, OutputFormat::Schema);
                let (count, matches) = query_files_batched(ctx, files, xpath_expr, collect)?;
                if matches!(ctx.format, OutputFormat::Count) {
                    println!("{}", count);
                } else if matches!(ctx.format, OutputFormat::Schema) {
                    print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
                }
            } else {
                explore_files(ctx, files)?;
            }
        }
    }
    Ok(())
}

fn run_test(ctx: &RunContext) -> Result<(), Box<dyn std::error::Error>> {
    let dot = ".".to_string();
    let xpath_expr = ctx.xpath.as_ref().unwrap_or(&dot);

    let (count, matches) = match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            let matches = query_inline_source(ctx, source, lang, xpath_expr)?;
            let count = matches.len();
            (count, matches)
        }
        InputMode::Files(files) => {
            query_files_batched(ctx, files, xpath_expr, true)?
        }
    };

    check_expectation_with_matches(count, &matches, ctx)
}

fn run_set(ctx: &RunContext) -> Result<(), Box<dyn std::error::Error>> {
    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("--set requires an XPath query (-x)")?;
    let set_value = ctx.args.set.as_ref().unwrap();

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::InlineSource { .. } => unreachable!(), // validated in from_args
    };

    let (_, matches) = query_files_batched(ctx, files, xpath_expr, true)?;

    let summary = apply_replacements(&matches, set_value)?;
    eprintln!(
        "Set {} match{} in {} file{}",
        summary.replacements_made,
        if summary.replacements_made == 1 { "" } else { "es" },
        summary.files_modified,
        if summary.files_modified == 1 { "" } else { "s" },
    );
    Ok(())
}

fn run_debug(ctx: &RunContext, files: &[String], xpath_expr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut total_matches = 0usize;
    let mut remaining_limit = ctx.args.limit;

    for file_path in files {
        if remaining_limit == Some(0) {
            break;
        }

        let mut result = match parse_to_documents(
            std::path::Path::new(file_path),
            ctx.args.lang.as_deref(),
            ctx.args.raw,
            ctx.args.ignore_whitespace,
            ctx.args.parse_depth,
        ) {
            Ok(r) => r,
            Err(e) => {
                if ctx.args.verbose {
                    eprintln!("warning: {}: {}", file_path, e);
                }
                continue;
            }
        };

        let engine = XPathEngine::new()
            .with_verbose(ctx.args.verbose)
            .with_ignore_whitespace(ctx.args.ignore_whitespace);

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

                total_matches += matches.len();

                let highlights: HashSet<(u32, u32)> = matches
                    .iter()
                    .map(|m| (m.line, m.column))
                    .collect();

                let doc_node = result.documents.document_node(result.doc_handle).unwrap();
                let render_opts = RenderOptions::new()
                    .with_color(ctx.use_color)
                    .with_locations(true)
                    .with_max_depth(ctx.args.depth)
                    .with_highlights(highlights)
                    .with_pretty_print(!ctx.args.no_pretty);
                let output = render_document(result.documents.xot(), doc_node, &render_opts);
                print!("{}", output);
            }
            Ok(_) => {}
            Err(e) => {
                if ctx.args.verbose {
                    eprintln!("warning: {}: query error: {}", file_path, e);
                }
            }
        }
    }

    // Debug mode supports --expect (count-only, no per-match details)
    if let Some(ref expect) = ctx.args.expect {
        let test_result = TestResult::check(expect, total_matches)?;
        if !test_result.passed && !ctx.args.warning {
            return Err(format!(
                "expectation failed: expected {}, got {} matches",
                test_result.expected, test_result.actual
            ).into());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Query pipeline
// ---------------------------------------------------------------------------

fn query_inline_source(
    ctx: &RunContext,
    source: &str,
    lang: &str,
    xpath_expr: &str,
) -> Result<Vec<Match>, Box<dyn std::error::Error>> {
    let mut result = parse_string_to_documents(
        source, lang, "<stdin>".to_string(), ctx.args.raw, ctx.args.ignore_whitespace
    )?;

    let engine = XPathEngine::new()
        .with_verbose(ctx.args.verbose)
        .with_ignore_whitespace(ctx.args.ignore_whitespace);

    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        xpath_expr,
        result.source_lines.clone(),
        &result.file_path,
    )?;

    let matches = if let Some(limit) = ctx.args.limit {
        matches.into_iter().take(limit).collect()
    } else {
        matches
    };

    Ok(matches)
}

/// Query files in parallel batches. When `collect` is true, all matches are
/// returned. When false, matches are streamed to stdout per batch (query mode).
fn query_files_batched(
    ctx: &RunContext,
    files: &[String],
    xpath_expr: &str,
    collect: bool,
) -> Result<(usize, Vec<Match>), Box<dyn std::error::Error>> {
    let batches = exponential_batches(files, ctx.concurrency);
    let mut total_matches = 0usize;
    let mut remaining_limit = ctx.args.limit;
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
                    ctx.args.lang.as_deref(),
                    ctx.args.raw,
                    ctx.args.ignore_whitespace,
                    ctx.args.parse_depth,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        if ctx.args.verbose {
                            eprintln!("warning: {}: {}", file_path, e);
                        }
                        return None;
                    }
                };

                let engine = XPathEngine::new()
                    .with_verbose(ctx.args.verbose)
                    .with_ignore_whitespace(ctx.args.ignore_whitespace);
                match engine.query_documents(
                    &mut result.documents,
                    result.doc_handle,
                    xpath_expr,
                    result.source_lines.clone(),
                    &result.file_path,
                ) {
                    Ok(matches) => Some(matches),
                    Err(e) => {
                        if ctx.args.verbose {
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
// Explore (no XPath — fast paths skip the XPath engine entirely)
// ---------------------------------------------------------------------------

fn explore_files(ctx: &RunContext, files: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let lang_override = ctx.args.lang.as_deref();
    let raw = ctx.args.raw;
    let verbose = ctx.args.verbose;

    // Fast path: count format (parallel, no XPath)
    if matches!(ctx.format, OutputFormat::Count) {
        let count: usize = files
            .par_iter()
            .filter(|file_path| {
                match parse_to_documents(
                    std::path::Path::new(file_path),
                    lang_override, raw, ctx.args.ignore_whitespace, ctx.args.parse_depth,
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

    // Fast path: schema format (parallel collection then merge)
    if matches!(ctx.format, OutputFormat::Schema) {
        let collectors: Vec<SchemaCollector> = files
            .par_iter()
            .filter_map(|file_path| {
                match parse_to_documents(
                    std::path::Path::new(file_path),
                    lang_override, raw, ctx.args.ignore_whitespace, ctx.args.parse_depth,
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

    // Sequential processing for remaining formats (Documents isn't Send)
    let parse_results: Vec<XeeParseResult> = files
        .iter()
        .filter_map(|file_path| {
            match parse_to_documents(
                std::path::Path::new(file_path),
                lang_override, raw, ctx.args.ignore_whitespace, ctx.args.parse_depth,
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

    // XML format: render document tree(s)
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
                        if !ctx.args.no_pretty {
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

    // Other formats: create Match objects and use format_matches
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

fn explore_inline(ctx: &RunContext, source: &str, lang: &str) -> Result<(), Box<dyn std::error::Error>> {
    let result = parse_string_to_documents(
        source, lang, "<stdin>".to_string(), ctx.args.raw, ctx.args.ignore_whitespace
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

    // Other formats: create a Match for the whole file
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

fn output_query_results(ctx: &RunContext, matches: &[Match]) {
    if matches!(ctx.format, OutputFormat::Schema) {
        print_schema_from_matches(matches, ctx.schema_depth(), ctx.use_color);
        return;
    }
    let output = format_matches(matches, ctx.format.clone(), &ctx.options);
    print!("{}", output);
}

fn print_schema_from_matches(matches: &[Match], depth: Option<usize>, use_color: bool) {
    let mut collector = SchemaCollector::new();
    for m in matches {
        if let Some(xml) = &m.xml_fragment {
            collector.collect_from_xml_string(xml);
        }
    }
    print!("{}", collector.format(depth, use_color));
}

/// Extract text content from XML, removing all tags
fn extract_text_content(xml: &str) -> String {
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

    // Normalize whitespace
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Test / expectation
// ---------------------------------------------------------------------------

mod test_colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BOLD: &str = "\x1b[1m";
}

struct TestResult {
    passed: bool,
    expected: String,
    actual: usize,
}

impl TestResult {
    fn check(expect: &str, count: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let passed = match expect {
            "none" => count == 0,
            "some" => count > 0,
            _ => {
                let expected: usize = expect.parse()
                    .map_err(|_| format!("invalid expectation '{}': use 'none', 'some', or a number", expect))?;
                count == expected
            }
        };
        Ok(TestResult {
            passed,
            expected: expect.to_string(),
            actual: count,
        })
    }
}

fn check_expectation_with_matches(
    count: usize,
    matches: &[Match],
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(ref expect) = ctx.args.expect else {
        return Ok(());
    };

    let result = TestResult::check(expect, count)?;

    let (symbol, color) = if result.passed {
        ("✓", test_colors::GREEN)
    } else if ctx.args.warning {
        ("⚠", test_colors::YELLOW)
    } else {
        ("✗", test_colors::RED)
    };

    let label = ctx.args.message.as_deref().unwrap_or("");

    if ctx.use_color {
        if label.is_empty() {
            println!("{}{}{} {} matches{}",
                test_colors::BOLD, color, symbol, result.actual, test_colors::RESET);
        } else if result.passed {
            println!("{}{}{} {}{}",
                test_colors::BOLD, color, symbol, label, test_colors::RESET);
        } else {
            println!("{}{}{} {} {}(expected {}, got {}){}",
                test_colors::BOLD, color, symbol, label, test_colors::RESET,
                result.expected, result.actual, test_colors::RESET);
        }
    } else {
        if label.is_empty() {
            println!("{} {} matches", symbol, result.actual);
        } else if result.passed {
            println!("{} {}", symbol, label);
        } else {
            println!("{} {} (expected {}, got {})", symbol, label, result.expected, result.actual);
        }
    }

    // On failure, show per-match details
    if !result.passed && !matches.is_empty() {
        if let Some(ref error_template) = ctx.args.error {
            let error_options = OutputOptions {
                message: Some(error_template.clone()),
                use_color: false,
                strip_locations: ctx.options.strip_locations,
                max_depth: ctx.options.max_depth,
                pretty_print: ctx.options.pretty_print,
                language: ctx.options.language.clone(),
                warning: ctx.options.warning,
            };
            let output = format_matches(matches, OutputFormat::Gcc, &error_options);
            for line in output.lines() {
                if ctx.use_color {
                    println!("  {}{}{}", color, line, test_colors::RESET);
                } else {
                    println!("  {}", line);
                }
            }
        } else {
            let output = format_matches(matches, ctx.format.clone(), &ctx.options);
            for line in output.lines() {
                println!("  {}", line);
            }
        }
    }

    if !result.passed && !ctx.args.warning {
        return Err(format!(
            "expectation failed: expected {}, got {} matches",
            result.expected, result.actual
        ).into());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_auto_prefixes_bare_element_names() {
        assert_eq!(normalize_xpath("function"), "//function");
        assert_eq!(normalize_xpath("variable"), "//variable");
        assert_eq!(normalize_xpath("class"), "//class");
        assert_eq!(normalize_xpath("name"), "//name");
    }

    #[test]
    fn test_normalize_preserves_absolute_paths() {
        assert_eq!(normalize_xpath("//function"), "//function");
        assert_eq!(normalize_xpath("//class[name='Foo']"), "//class[name='Foo']");
        // "/root" gets converted to "//root" in MSYS environments (by design,
        // to fix MinGW UNC path mangling), so we only test it outside MSYS
        if !is_msys_environment() {
            assert_eq!(normalize_xpath("/root"), "/root");
        }
    }

    #[test]
    fn test_normalize_preserves_parenthesized_expressions() {
        assert_eq!(normalize_xpath("(//a | //b)"), "(//a | //b)");
    }

    #[test]
    fn test_normalize_preserves_dot() {
        assert_eq!(normalize_xpath("."), ".");
    }

    #[test]
    fn test_normalize_preserves_let_expressions() {
        assert_eq!(
            normalize_xpath("let $v := //function return $v/name"),
            "let $v := //function return $v/name"
        );
        assert_eq!(
            normalize_xpath("let$v := //x return $v"),
            "let$v := //x return $v"
        );
    }

    #[test]
    fn test_normalize_preserves_for_expressions() {
        assert_eq!(
            normalize_xpath("for $v in //name return string($v)"),
            "for $v in //name return string($v)"
        );
        assert_eq!(
            normalize_xpath("for$v in //name return $v"),
            "for$v in //name return $v"
        );
    }

    #[test]
    fn test_normalize_preserves_if_expressions() {
        assert_eq!(
            normalize_xpath("if (//x) then 1 else 0"),
            "if (//x) then 1 else 0"
        );
        assert_eq!(
            normalize_xpath("if(//x) then 1 else 0"),
            "if(//x) then 1 else 0"
        );
    }

    #[test]
    fn test_normalize_preserves_quantified_expressions() {
        assert_eq!(
            normalize_xpath("some $v in //x satisfies $v/name"),
            "some $v in //x satisfies $v/name"
        );
        assert_eq!(
            normalize_xpath("every $v in //x satisfies $v/name"),
            "every $v in //x satisfies $v/name"
        );
    }

    #[test]
    fn test_normalize_preserves_variable_references() {
        assert_eq!(normalize_xpath("$var"), "$var");
    }

    #[test]
    fn test_normalize_preserves_string_literals() {
        assert_eq!(normalize_xpath("\"hello\""), "\"hello\"");
        assert_eq!(normalize_xpath("'hello'"), "'hello'");
    }

    #[test]
    fn test_normalize_preserves_numeric_literals() {
        assert_eq!(normalize_xpath("42"), "42");
        assert_eq!(normalize_xpath("3.14"), "3.14");
    }

    #[test]
    fn test_normalize_preserves_function_calls() {
        assert_eq!(normalize_xpath("count(//item)"), "count(//item)");
        assert_eq!(normalize_xpath("not(//x)"), "not(//x)");
        assert_eq!(normalize_xpath("string(//x)"), "string(//x)");
        assert_eq!(normalize_xpath("contains(//x, 'y')"), "contains(//x, 'y')");
        assert_eq!(normalize_xpath("starts-with(//x, 'y')"), "starts-with(//x, 'y')");
    }
}
