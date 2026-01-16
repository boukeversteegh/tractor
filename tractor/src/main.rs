//! tractor - Multi-language code query tool using XPath 3.1
//!
//! This is the main CLI entry point that orchestrates parsing and querying.

mod cli;

use std::collections::HashSet;
use std::io::{self, BufRead, Read, Write};
use std::process::ExitCode;

use rayon::prelude::*;
use tractor_core::{
    parse_string, parse_file, generate_xml_document,
    ParseResult, XPathEngine, Match,
    OutputFormat, format_matches, OutputOptions,
    expand_globs, filter_supported_files,
    output::should_use_color,
    output::{render_xml_string, RenderOptions},
    load_xml, load_xml_file, detect_language,
};

use cli::Args;
use clap::Parser;

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

fn main() -> ExitCode {
    let args = Args::parse();

    if let Err(e) = run(args) {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Check if running under MSYS/MinGW which mangles // paths
fn is_msys_environment() -> bool {
    std::env::var("MSYSTEM").is_ok()
}

/// Normalize XPath expression - auto-prefix with // if not starting with /
/// Also fixes MSYS/MinGW path mangling where // gets converted to /
fn normalize_xpath(xpath: &str) -> String {
    let xpath = fix_msys_xpath_mangling(xpath);

    // Don't prefix expressions that are already absolute or context-relative
    if xpath.starts_with('/') || xpath.starts_with('(') || xpath == "." {
        xpath
    } else {
        format!("//{}", xpath)
    }
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

/// Check if a file is an XML file (for passthrough mode)
fn is_xml_file(path: &str) -> bool {
    detect_language(path) == "xml"
}

/// Load a file - either as XML passthrough or by parsing source code
fn load_file(
    path: &std::path::Path,
    lang_override: Option<&str>,
    raw_mode: bool,
) -> Result<ParseResult, tractor_core::parser::ParseError> {
    let path_str = path.to_string_lossy();

    // XML passthrough: load directly without parsing
    if lang_override.map_or_else(|| is_xml_file(&path_str), |l| l == "xml") {
        return load_xml_file(path);
    }

    // Normal: parse source code
    parse_file(path, lang_override, raw_mode)
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Normalize XPath if provided (auto-prefix // for convenience)
    let xpath = args.xpath.as_ref().map(|x| normalize_xpath(x));

    // Validate output format
    let format = OutputFormat::from_str(&args.output)
        .ok_or_else(|| {
            format!(
                "invalid format '{}'. Valid formats: {}",
                args.output,
                OutputFormat::valid_formats().join(", ")
            )
        })?;

    // Determine color mode
    let use_color = if args.no_color {
        false
    } else {
        should_use_color(&args.color)
    };

    // Collect files
    let mut files: Vec<String> = expand_globs(&args.files);

    // Handle stdin input modes
    let stdin_source = files.is_empty() && args.lang.is_some() && !atty::is(atty::Stream::Stdin);
    let stdin_files = files.is_empty() && args.lang.is_none() && !atty::is(atty::Stream::Stdin);

    if stdin_files {
        // Read file paths from stdin
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

    // Filter to supported languages
    files = filter_supported_files(files);

    if files.is_empty() && !stdin_source {
        eprintln!("Usage: tractor <files...> [OPTIONS]");
        eprintln!("   or: cat source.rs | tractor --lang rust -x \"//query\"");
        eprintln!("   or: echo 'file.rs' | tractor -x \"//query\"");
        eprintln!("\nUse --help for more information.");
        return Err("no input files".into());
    }

    // Configure thread pool
    let concurrency = args.concurrency.unwrap_or_else(|| num_cpus::get());
    rayon::ThreadPoolBuilder::new()
        .num_threads(concurrency)
        .build_global()
        .ok();

    // Handle stdin source separately
    if stdin_source {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        let lang = args.lang.as_deref().unwrap();

        // XML passthrough for stdin
        let result = if lang == "xml" {
            load_xml(source, "<stdin>".to_string())
        } else {
            parse_string(&source, lang, "<stdin>".to_string(), args.raw)?
        };
        return process_single_result(result, &args, xpath.as_deref(), format, use_color);
    }

    // Execute XPath query if provided - run per-file in parallel with streaming batches
    if let Some(ref xpath_expr) = xpath {
        let verbose = args.verbose;
        let raw = args.raw;
        let lang_override = args.lang.clone();

        // Output options for formatting
        let options = OutputOptions {
            message: args.message.clone(),
            use_color,
            strip_locations: !args.keep_locations,
            max_depth: args.depth,
            pretty_print: !args.no_pretty,
        };

        // Debug mode: show full XML with highlighted matches for each file
        if args.debug {
            let mut total_matches = 0usize;
            let mut remaining_limit = args.limit;

            for file_path in &files {
                if remaining_limit == Some(0) {
                    break;
                }

                let result = match load_file(std::path::Path::new(file_path), lang_override.as_deref(), raw) {
                    Ok(r) => r,
                    Err(e) => {
                        if verbose {
                            eprintln!("warning: {}: {}", file_path, e);
                        }
                        continue;
                    }
                };

                // Use compact XML for XPath query (no formatting whitespace)
                let xml_compact = generate_xml_document(&[result.clone()], false);
                let engine = XPathEngine::new().with_verbose(verbose).with_ignore_whitespace(args.ignore_whitespace);

                match engine.query(&xml_compact, xpath_expr, &result.source_lines, &result.file_path) {
                    Ok(matches) if !matches.is_empty() => {
                        // Apply limit
                        let matches: Vec<_> = if let Some(limit) = remaining_limit {
                            let take = limit.min(matches.len());
                            remaining_limit = Some(limit - take);
                            matches.into_iter().take(take).collect()
                        } else {
                            matches
                        };

                        total_matches += matches.len();

                        // Collect match positions for highlighting
                        let highlights: HashSet<(u32, u32)> = matches
                            .iter()
                            .map(|m| (m.line, m.column))
                            .collect();

                        // Show full XML with highlights
                        let xml_display = generate_xml_document(&[result.clone()], !args.no_pretty);
                        let render_opts = RenderOptions::new()
                            .with_color(use_color)
                            .with_locations(true)
                            .with_max_depth(args.depth)
                            .with_highlights(highlights)
                            .with_pretty_print(!args.no_pretty);
                        let output = render_xml_string(&xml_display, &render_opts);
                        print!("{}", output);
                    }
                    Ok(_) => {} // No matches in this file
                    Err(e) => {
                        if verbose {
                            eprintln!("warning: {}: query error: {}", file_path, e);
                        }
                    }
                }
            }

            return check_expectation_count(total_matches, &args);
        }

        // Normal mode: process files in exponentially growing batches for streaming output
        let batches = exponential_batches(&files, concurrency);
        let mut total_matches = 0usize;
        let mut remaining_limit = args.limit;

        // Count format doesn't benefit from streaming - just count everything
        let is_count_format = matches!(format, OutputFormat::Count);

        // Test mode: collect all matches for error output; suppress streaming
        let is_test_mode = args.expect.is_some();
        let mut all_matches: Vec<Match> = Vec::new();

        for batch in batches {
            // Check if we've hit the limit
            if remaining_limit == Some(0) {
                break;
            }

            // Process this batch in parallel
            let mut batch_matches: Vec<Match> = batch
                .par_iter()
                .filter_map(|file_path| {
                    // Parse the file
                    let result = match load_file(std::path::Path::new(file_path), lang_override.as_deref(), raw) {
                        Ok(r) => r,
                        Err(e) => {
                            if verbose {
                                eprintln!("warning: {}: {}", file_path, e);
                            }
                            return None;
                        }
                    };

                    // Generate compact XML for XPath query (no formatting whitespace)
                    let xml = generate_xml_document(&[result.clone()], false);

                    // Query this file's XML
                    let engine = XPathEngine::new().with_verbose(verbose).with_ignore_whitespace(args.ignore_whitespace);
                    match engine.query(&xml, xpath_expr, &result.source_lines, &result.file_path) {
                        Ok(matches) => Some((file_path.clone(), matches)),
                        Err(e) => {
                            if verbose {
                                eprintln!("warning: {}: query error: {}", file_path, e);
                            }
                            None
                        }
                    }
                })
                .flat_map(|(_, matches)| matches)
                .collect();

            // Sort by file path for consistent ordering within batch
            batch_matches.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

            // Apply remaining limit
            if let Some(limit) = remaining_limit {
                batch_matches.truncate(limit);
                remaining_limit = Some(limit.saturating_sub(batch_matches.len()));
            }

            total_matches += batch_matches.len();

            if is_test_mode {
                // Collect matches for test output later
                all_matches.extend(batch_matches);
            } else {
                // Stream output immediately (except for count format)
                if !is_count_format {
                    let output = format_matches(&batch_matches, format.clone(), &options);
                    print!("{}", output);
                    io::stdout().flush().ok();
                }
            }
        }

        // For count format (non-test mode), print the total at the end
        if is_count_format && !is_test_mode {
            println!("{}", total_matches);
        }

        // Check expectation and output test results
        return check_expectation_with_matches(total_matches, &all_matches, &args, use_color, &format, &options);
    }

    // No XPath - output full files using same formatting pipeline
    let lang_override = args.lang.as_deref();
    let raw = args.raw;
    let verbose = args.verbose;

    let parse_results: Vec<ParseResult> = files
        .par_iter()
        .filter_map(|file_path| {
            match load_file(std::path::Path::new(file_path), lang_override, raw) {
                Ok(r) => Some(r),
                Err(e) => {
                    if verbose {
                        eprintln!("warning: {}", e);
                    }
                    None
                }
            }
        })
        .collect();

    if parse_results.is_empty() {
        return Err("no files could be parsed".into());
    }

    // For XML format, use combined document with Files wrapper
    if matches!(format, OutputFormat::Xml) {
        let xml = generate_xml_document(&parse_results, !args.no_pretty);
        let render_opts = RenderOptions::new()
            .with_color(use_color)
            .with_locations(args.keep_locations || args.debug)
            .with_max_depth(args.depth);
        let output = render_xml_string(&xml, &render_opts);
        print!("{}", output);
        return Ok(());
    }

    // For other formats, create Match objects and use format_matches
    let matches: Vec<Match> = parse_results
        .iter()
        .map(|result| {
            let end_line = result.source_lines.len() as u32;
            let end_column = result.source_lines.last()
                .map(|l| l.len() as u32 + 1)
                .unwrap_or(1);

            // Extract text content from XML for value output
            let value = extract_text_content(&result.xml);

            Match::with_location(
                result.file_path.clone(),
                1,
                1,
                end_line.max(1),
                end_column,
                value,
                result.source_lines.clone(),
            ).with_xml_fragment(result.xml.clone())
        })
        .collect();

    let options = OutputOptions {
        message: args.message.clone(),
        use_color,
        strip_locations: !args.keep_locations,
        max_depth: args.depth,
        pretty_print: !args.no_pretty,
    };

    let output = format_matches(&matches, format, &options);
    print!("{}", output);

    Ok(())
}

fn process_single_result(
    result: ParseResult,
    args: &Args,
    xpath: Option<&str>,
    format: OutputFormat,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use compact XML for XPath queries (no formatting whitespace)
    let xml_compact = generate_xml_document(&[result.clone()], false);

    if let Some(xpath_expr) = xpath {
        let engine = XPathEngine::new().with_verbose(args.verbose).with_ignore_whitespace(args.ignore_whitespace);
        let matches = engine.query(&xml_compact, xpath_expr, &result.source_lines, &result.file_path)?;

        let matches: Vec<Match> = if let Some(limit) = args.limit {
            matches.into_iter().take(limit).collect()
        } else {
            matches
        };

        if args.debug {
            let highlights: HashSet<(u32, u32)> = matches
                .iter()
                .map(|m| (m.line, m.column))
                .collect();
            // Display XML (pretty unless --no-pretty)
            let xml_display = generate_xml_document(&[result.clone()], !args.no_pretty);
            let render_opts = RenderOptions::new()
                .with_color(use_color)
                .with_locations(true)
                .with_max_depth(args.depth)
                .with_highlights(highlights)
                .with_pretty_print(!args.no_pretty);
            let output = render_xml_string(&xml_display, &render_opts);
            print!("{}", output);
            let options = OutputOptions {
                message: args.message.clone(),
                use_color,
                strip_locations: !args.keep_locations,
                max_depth: args.depth,
                pretty_print: !args.no_pretty,
            };
            return check_expectation(&matches, args, use_color, &format, &options);
        }

        let options = OutputOptions {
            message: args.message.clone(),
            use_color,
            strip_locations: !args.keep_locations,
            max_depth: args.depth,
            pretty_print: !args.no_pretty,
        };

        let output = format_matches(&matches, format.clone(), &options);
        print!("{}", output);

        check_expectation(&matches, args, use_color, &format, &options)
    } else {
        // For XML format, use the render_xml_string
        if matches!(format, OutputFormat::Xml) {
            let xml_display = generate_xml_document(&[result.clone()], !args.no_pretty);
            let render_opts = RenderOptions::new()
                .with_color(use_color)
                .with_locations(args.keep_locations || args.debug)
                .with_max_depth(args.depth)
                .with_pretty_print(!args.no_pretty);
            let output = render_xml_string(&xml_display, &render_opts);
            print!("{}", output);
            return Ok(());
        }

        // For other formats, create a Match for the whole file and use format_matches
        let end_line = result.source_lines.len() as u32;
        let end_column = result.source_lines.last()
            .map(|l| l.len() as u32 + 1)
            .unwrap_or(1);
        let value = extract_text_content(&result.xml);

        let file_match = Match::with_location(
            result.file_path.clone(),
            1,
            1,
            end_line.max(1),
            end_column,
            value,
            result.source_lines.clone(),
        ).with_xml_fragment(result.xml.clone());

        let options = OutputOptions {
            message: args.message.clone(),
            use_color,
            strip_locations: !args.keep_locations,
            max_depth: args.depth,
            pretty_print: !args.no_pretty,
        };

        let output = format_matches(&[file_match], format, &options);
        print!("{}", output);
        Ok(())
    }
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

/// ANSI colors for test output
mod test_colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BOLD: &str = "\x1b[1m";
}

/// Result of an expectation check
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

fn check_expectation(matches: &[Match], args: &Args, use_color: bool, format: &OutputFormat, options: &OutputOptions) -> Result<(), Box<dyn std::error::Error>> {
    check_expectation_with_matches(matches.len(), matches, args, use_color, format, options)
}

fn check_expectation_count(count: usize, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    // Legacy function for contexts where we don't have matches available
    check_expectation_with_matches(count, &[], args, false, &OutputFormat::Count, &OutputOptions::default())
}

fn check_expectation_with_matches(
    count: usize,
    matches: &[Match],
    args: &Args,
    use_color: bool,
    format: &OutputFormat,
    options: &OutputOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(ref expect) = args.expect else {
        return Ok(());
    };

    let result = TestResult::check(expect, count)?;

    // Determine symbols and colors
    let (symbol, color) = if result.passed {
        ("✓", test_colors::GREEN)
    } else if args.warning {
        ("⚠", test_colors::YELLOW)
    } else {
        ("✗", test_colors::RED)
    };

    // Build the output message
    let label = args.message.as_deref().unwrap_or("");

    // Print the test result line
    if use_color {
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

    // Generate output text for validation (even if test passes)
    let mut output_text = String::new();
    if !matches.is_empty() {
        if let Some(ref error_template) = args.error {
            let error_options = OutputOptions {
                message: Some(error_template.clone()),
                use_color: false,
                strip_locations: options.strip_locations,
                max_depth: options.max_depth,
                pretty_print: options.pretty_print,
            };
            output_text = format_matches(matches, OutputFormat::Gcc, &error_options);
        } else {
            output_text = format_matches(matches, format.clone(), options);
        }
    }

    // Check expected output string
    if let Some(ref expected) = args.expect_output {
        if !output_text.contains(expected) {
            return Err(format!(
                "output validation failed: expected output to contain '{}'",
                expected
            ).into());
        }
    }

    // On failure, show per-match details
    if !result.passed && !matches.is_empty() {
        for line in output_text.lines() {
            if use_color {
                println!("  {}{}{}", color, line, test_colors::RESET);
            } else {
                println!("  {}", line);
            }
        }
    }

    // Return error only if failed and not warning mode
    if !result.passed && !args.warning {
        return Err(format!(
            "expectation failed: expected {}, got {} matches",
            result.expected, result.actual
        ).into());
    }

    Ok(())
}
