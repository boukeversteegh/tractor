//! tractor - Multi-language code query tool using XPath 3.1
//!
//! This is the main CLI entry point that orchestrates parsing and querying.

mod cli;

use std::io::{self, BufRead, Read, Write};
use std::process::ExitCode;

use rayon::prelude::*;
use tractor_core::{
    parse_string, parse_file, generate_xml_document, ParseResult,
    XPathEngine, Match,
    OutputFormat, format_matches, OutputOptions,
    expand_globs, filter_supported_files,
    output::should_use_color,
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

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
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
        let result = parse_string(&source, lang, "<stdin>".to_string(), args.raw)?;
        return process_single_result(result, &args, format, use_color);
    }

    // Execute XPath query if provided - run per-file in parallel with streaming batches
    if let Some(ref xpath) = args.xpath {
        let verbose = args.verbose;
        let raw = args.raw;
        let lang_override = args.lang.clone();

        // Output options for formatting
        let options = OutputOptions {
            message: args.message.clone(),
            use_color,
            strip_locations: !args.keep_locations,
        };

        // Process files in exponentially growing batches for streaming output
        let batches = exponential_batches(&files, concurrency);
        let mut total_matches = 0usize;
        let mut remaining_limit = args.limit;

        // Count format doesn't benefit from streaming - just count everything
        let is_count_format = matches!(format, OutputFormat::Count);

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
                    let result = match parse_file(std::path::Path::new(file_path), lang_override.as_deref(), raw) {
                        Ok(r) => r,
                        Err(e) => {
                            if verbose {
                                eprintln!("warning: {}: {}", file_path, e);
                            }
                            return None;
                        }
                    };

                    // Generate XML for this file
                    let xml = generate_xml_document(&[result.clone()]);

                    // Query this file's XML
                    let engine = XPathEngine::new().with_verbose(verbose);
                    match engine.query(&xml, xpath, &result.source_lines, &result.file_path) {
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

            // Stream output immediately (except for count format)
            if !is_count_format {
                let output = format_matches(&batch_matches, format.clone(), &options);
                print!("{}", output);
                io::stdout().flush().ok();
            }
        }

        // For count format, print the total at the end
        if is_count_format {
            println!("{}", total_matches);
        }

        // Check expectation using total count
        return check_expectation_count(total_matches, &args);
    }

    // No XPath - output full XML (parse all files, combine)
    let lang_override = args.lang.as_deref();
    let raw = args.raw;
    let verbose = args.verbose;

    let parse_results: Vec<ParseResult> = files
        .par_iter()
        .filter_map(|file_path| {
            match parse_file(std::path::Path::new(file_path), lang_override, raw) {
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

    let xml = generate_xml_document(&parse_results);

    if args.debug {
        let output = if use_color {
            tractor_core::output::colorize_xml(&xml)
        } else {
            xml.clone()
        };
        println!("{}", output);
    } else {
        let output = if args.keep_locations {
            xml.clone()
        } else {
            XPathEngine::strip_location_metadata(&xml)
        };
        println!("{}", output);
    }

    Ok(())
}

fn process_single_result(
    result: ParseResult,
    args: &Args,
    format: OutputFormat,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let xml = generate_xml_document(&[result.clone()]);

    if let Some(ref xpath) = args.xpath {
        let engine = XPathEngine::new().with_verbose(args.verbose);
        let matches = engine.query(&xml, xpath, &result.source_lines, &result.file_path)?;

        let matches: Vec<Match> = if let Some(limit) = args.limit {
            matches.into_iter().take(limit).collect()
        } else {
            matches
        };

        if args.debug {
            let match_positions: Vec<(String, u32, u32)> = matches
                .iter()
                .map(|m| (String::new(), m.line, m.column))
                .collect();
            let highlighted = tractor_core::output::colorize_xml_with_highlights(&xml, &match_positions, use_color);
            println!("{}", highlighted);
            return check_expectation(&matches, args);
        }

        let options = OutputOptions {
            message: args.message.clone(),
            use_color,
            strip_locations: !args.keep_locations,
        };

        let output = format_matches(&matches, format, &options);
        print!("{}", output);

        check_expectation(&matches, args)
    } else {
        if args.debug {
            let output = if use_color {
                tractor_core::output::colorize_xml(&xml)
            } else {
                xml.clone()
            };
            println!("{}", output);
        } else {
            let output = if args.keep_locations {
                xml.clone()
            } else {
                XPathEngine::strip_location_metadata(&xml)
            };
            println!("{}", output);
        }
        Ok(())
    }
}

fn check_expectation(matches: &[Match], args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    check_expectation_count(matches.len(), args)
}

fn check_expectation_count(count: usize, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref expect) = args.expect {
        let ok = match expect.as_str() {
            "none" => count == 0,
            "some" => count > 0,
            _ => {
                let expected: usize = expect.parse()
                    .map_err(|_| format!("invalid expectation '{}': use 'none', 'some', or a number", expect))?;
                count == expected
            }
        };

        if !ok {
            return Err(format!(
                "expectation failed: expected {}, got {} matches",
                expect, count
            ).into());
        }
    }
    Ok(())
}
