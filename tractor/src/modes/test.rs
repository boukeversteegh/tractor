use tractor_core::{
    Match,
    OutputFormat, format_matches, OutputOptions,
};
use crate::cli::TestArgs;
use crate::pipeline::{RunContext, InputMode, query_inline_source, query_files_batched};

pub mod test_colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BOLD: &str = "\x1b[1m";
}

pub struct TestResult {
    pub passed: bool,
    pub expected: String,
    pub actual: usize,
}

impl TestResult {
    pub fn check(expect: &str, count: usize) -> Result<Self, Box<dyn std::error::Error>> {
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

pub fn run_test(args: TestArgs) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn check_expectation_with_matches(
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
