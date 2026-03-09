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
    report::{Severity, CheckSummary},
};

use cli::{Cli, Command, SharedArgs, QueryArgs, CheckArgs, TestArgs, SetArgs};
use clap::{CommandFactory, Parser};

// ---------------------------------------------------------------------------
// Batch utility
// ---------------------------------------------------------------------------

/// Split a slice into exponentially growing batches, capped at a maximum.
fn exponential_batches<T>(items: &[T], num_threads: usize) -> Vec<&[T]> {
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
// Main
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Handle --version flag (only in query/default mode)
    if cli.command.is_none() && cli.query.version {
        if cli.query.shared.verbose {
            version::print_version_verbose();
        } else {
            version::print_version();
        }
        return ExitCode::SUCCESS;
    }

    let result = match cli.command {
        Some(Command::Check(args)) => run_check(args),
        Some(Command::Test(args)) => run_test(args),
        Some(Command::Set(args)) => run_set(args),
        None => run_query(cli.query),
    };

    if let Err(e) = result {
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

fn is_msys_environment() -> bool {
    std::env::var("MSYSTEM").is_ok()
}

fn normalize_xpath(xpath: &str) -> String {
    let xpath = fix_msys_xpath_mangling(xpath);

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

fn looks_like_xpath_expression(xpath: &str) -> bool {
    let keywords = ["let ", "let$", "for ", "for$", "if ", "if(", "some ", "some$", "every ", "every$"];
    keywords.iter().any(|kw| xpath.starts_with(kw))
        || xpath.starts_with("not(")
        || xpath.starts_with("count(")
        || xpath.starts_with("string(")
        || xpath.starts_with("contains(")
        || xpath.starts_with("starts-with(")
        || xpath.chars().next().map_or(false, |c| c.is_ascii_digit())
}

fn fix_msys_xpath_mangling(xpath: &str) -> String {
    if !is_msys_environment() {
        return xpath.to_string();
    }

    if xpath.starts_with('/') && !xpath.starts_with("//") {
        let rest = &xpath[1..];
        if !rest.is_empty() && (rest.chars().next().unwrap().is_alphabetic() || rest.starts_with('*')) {
            return format!("/{}", xpath);
        }
    }

    xpath.to_string()
}

// ---------------------------------------------------------------------------
// Run context (shared infrastructure)
// ---------------------------------------------------------------------------

enum InputMode {
    Files(Vec<String>),
    InlineSource { source: String, lang: String },
}

struct RunContext {
    xpath: Option<String>,
    format: OutputFormat,
    use_color: bool,
    options: OutputOptions,
    input: InputMode,
    concurrency: usize,
    // Shared args (borrowed fields exposed individually)
    limit: Option<usize>,
    depth: Option<usize>,
    parse_depth: Option<usize>,
    keep_locations: bool,
    raw: bool,
    no_pretty: bool,
    ignore_whitespace: bool,
    verbose: bool,
    lang: Option<String>,
    // Mode-specific
    debug: bool,
}

impl RunContext {
    fn build(
        shared: &SharedArgs,
        files: Vec<String>,
        xpath: Option<String>,
        output_format: &str,
        message: Option<String>,
        content: Option<String>,
        warning: bool,
        debug: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let xpath = xpath.as_ref().map(|x| normalize_xpath(x));

        let format = OutputFormat::from_str(output_format)
            .ok_or_else(|| {
                format!(
                    "invalid format '{}'. Valid formats: {}",
                    output_format,
                    OutputFormat::valid_formats().join(", ")
                )
            })?;

        let use_color = if shared.no_color {
            false
        } else {
            should_use_color(&shared.color)
        };

        let mut files: Vec<String> = expand_globs(&files);

        let input = if let Some(ref content_str) = content {
            if shared.lang.is_none() {
                return Err("--string requires --lang to specify the language".into());
            }
            InputMode::InlineSource {
                source: content_str.clone(),
                lang: shared.lang.clone().unwrap(),
            }
        } else if files.is_empty() && shared.lang.is_some() && !atty::is(atty::Stream::Stdin) {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            InputMode::InlineSource {
                source: s,
                lang: shared.lang.clone().unwrap(),
            }
        } else {
            if files.is_empty() && shared.lang.is_none() && !atty::is(atty::Stream::Stdin) {
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

        let concurrency = shared.concurrency.unwrap_or_else(|| num_cpus::get());
        rayon::ThreadPoolBuilder::new()
            .num_threads(concurrency)
            .build_global()
            .ok();

        let options = OutputOptions {
            message,
            use_color,
            strip_locations: !shared.keep_locations,
            max_depth: shared.depth,
            pretty_print: !shared.no_pretty,
            language: shared.lang.clone(),
            warning,
        };

        Ok(RunContext {
            xpath,
            format,
            use_color,
            options,
            input,
            concurrency,
            limit: shared.limit,
            depth: shared.depth,
            parse_depth: shared.parse_depth,
            keep_locations: shared.keep_locations,
            raw: shared.raw,
            no_pretty: shared.no_pretty,
            ignore_whitespace: shared.ignore_whitespace,
            verbose: shared.verbose,
            lang: shared.lang.clone(),
            debug,
        })
    }

    fn render_options(&self) -> RenderOptions {
        RenderOptions::new()
            .with_color(self.use_color)
            .with_locations(self.keep_locations || self.debug)
            .with_max_depth(self.depth)
            .with_pretty_print(!self.no_pretty)
    }

    fn schema_depth(&self) -> Option<usize> {
        self.depth.or(Some(4))
    }
}

// ---------------------------------------------------------------------------
// Query mode (default)
// ---------------------------------------------------------------------------

fn run_query(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
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
                    print_schema_from_matches(&matches, ctx.schema_depth(), ctx.use_color);
                }
            } else {
                explore_files(&ctx, files)?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Test mode
// ---------------------------------------------------------------------------

fn run_test(args: TestArgs) -> Result<(), Box<dyn std::error::Error>> {
    let warning = args.warning;
    let expect = args.expect.clone();
    let error_template = args.error.clone();
    let message = args.message.clone();

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.output, args.message, args.content, args.warning, false,
    )?;

    let dot = ".".to_string();
    let xpath_expr = ctx.xpath.as_ref().unwrap_or(&dot);

    let (count, matches) = match &ctx.input {
        InputMode::InlineSource { source, lang } => {
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            let count = matches.len();
            (count, matches)
        }
        InputMode::Files(files) => {
            query_files_batched(&ctx, files, xpath_expr, true)?
        }
    };

    check_expectation_with_matches(count, &matches, &ctx, &expect, &message, &error_template, warning)
}

// ---------------------------------------------------------------------------
// Set mode
// ---------------------------------------------------------------------------

fn run_set(args: SetArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        "xml", None, None, false, false,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("set requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::InlineSource { .. } => {
            return Err("set cannot be used with stdin input (no file to modify)".into());
        }
    };

    let (_, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;

    let summary = apply_replacements(&matches, &args.value)?;
    eprintln!(
        "Set {} match{} in {} file{}",
        summary.replacements_made,
        if summary.replacements_made == 1 { "" } else { "es" },
        summary.files_modified,
        if summary.files_modified == 1 { "" } else { "s" },
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Check mode (new)
// ---------------------------------------------------------------------------

fn run_check(args: CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    let severity = match args.severity.as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        s => return Err(format!("invalid severity '{}': use 'error' or 'warning'", s).into()),
    };
    let reason = args.reason.clone().unwrap_or_else(|| "check failed".to_string());

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.output, args.message, None, false, false,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("check requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::InlineSource { .. } => {
            return Err("check cannot be used with stdin input".into());
        }
    };

    if files.is_empty() {
        return Ok(());
    }

    let (_, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;

    let severity_str = match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };

    // If using gcc (default) or github format, emit per-match lines
    if matches!(ctx.format, OutputFormat::Gcc | OutputFormat::Github) {
        let check_options = OutputOptions {
            message: Some(reason.clone()),
            use_color: false,
            strip_locations: ctx.options.strip_locations,
            max_depth: ctx.options.max_depth,
            pretty_print: ctx.options.pretty_print,
            language: ctx.options.language.clone(),
            warning: matches!(severity, Severity::Warning),
        };
        let output = format_matches(&matches, ctx.format.clone(), &check_options);
        print!("{}", output);
    } else {
        // For other formats (json, etc.), just output matches normally
        let output = format_matches(&matches, ctx.format.clone(), &ctx.options);
        print!("{}", output);
    }

    // Summary
    let mut files_affected = HashSet::new();
    for m in &matches {
        files_affected.insert(&m.file);
    }
    let summary = CheckSummary {
        total: matches.len(),
        files_affected: files_affected.len(),
        errors: if matches!(severity, Severity::Error) { matches.len() } else { 0 },
        warnings: if matches!(severity, Severity::Warning) { matches.len() } else { 0 },
    };

    if summary.total > 0 {
        eprintln!();
        let kind = if summary.errors > 0 {
            format!("{} error{}", summary.errors, if summary.errors == 1 { "" } else { "s" })
        } else {
            format!("{} warning{}", summary.warnings, if summary.warnings == 1 { "" } else { "s" })
        };
        eprintln!("{} in {} file{}", kind, summary.files_affected,
            if summary.files_affected == 1 { "" } else { "s" });
    }

    // Exit code: 1 if any errors, 0 for warnings-only or no matches
    if summary.errors > 0 {
        return Err(format!("{} {} found", summary.errors, severity_str).into());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Debug mode
// ---------------------------------------------------------------------------

fn run_debug(ctx: &RunContext, files: &[String], xpath_expr: &str) -> Result<(), Box<dyn std::error::Error>> {
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

fn query_files_batched(
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

fn explore_files(ctx: &RunContext, files: &[String]) -> Result<(), Box<dyn std::error::Error>> {
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

fn explore_inline(ctx: &RunContext, source: &str, lang: &str) -> Result<(), Box<dyn std::error::Error>> {
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
    expect: &str,
    message: &Option<String>,
    error_template: &Option<String>,
    warning: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = TestResult::check(expect, count)?;

    let (symbol, color) = if result.passed {
        ("✓", test_colors::GREEN)
    } else if warning {
        ("⚠", test_colors::YELLOW)
    } else {
        ("✗", test_colors::RED)
    };

    let label = message.as_deref().unwrap_or("");

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

    if !result.passed && !matches.is_empty() {
        if let Some(ref error_tmpl) = error_template {
            let error_options = OutputOptions {
                message: Some(error_tmpl.clone()),
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

    if !result.passed && !warning {
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
