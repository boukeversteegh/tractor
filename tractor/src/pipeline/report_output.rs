//! Report output stage — renders a Report to stdout/stderr.

use tractor_core::{
    OutputFormat, format_matches, OutputOptions,
    report::{Report, Severity},
};
use super::context::{RunContext, SerFormat};
use crate::modes::test::test_colors;

/// Render a check report. Returns Err if there are error-severity violations.
pub fn render_check_report(
    report: &Report,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = report.summary.as_ref().expect("check report must have summary");

    if ctx.ser_format == SerFormat::Json {
        // JSON envelope output
        print!("{}", report.to_json());

        if summary.errors > 0 {
            return Err(format!("{} error{} found", summary.errors,
                if summary.errors == 1 { "" } else { "s" }).into());
        }
        return Ok(());
    }

    // Text: extract inner matches and use format_matches with the view
    let inner_matches: Vec<_> = report.matches.iter().map(|rm| rm.inner.clone()).collect();

    if matches!(ctx.view, OutputFormat::Gcc | OutputFormat::Github) {
        // Determine reason and severity from first match (all same in single-rule check)
        let reason = report.matches.first()
            .and_then(|rm| rm.reason.clone())
            .unwrap_or_else(|| "check failed".to_string());
        let is_warning = report.matches.first()
            .and_then(|rm| rm.severity)
            .map_or(false, |s| matches!(s, Severity::Warning));

        let check_options = OutputOptions {
            message: Some(reason),
            use_color: false,
            strip_locations: ctx.options.strip_locations,
            max_depth: ctx.options.max_depth,
            pretty_print: ctx.options.pretty_print,
            language: ctx.options.language.clone(),
            warning: is_warning,
        };
        let output = format_matches(&inner_matches, ctx.view.clone(), &check_options);
        print!("{}", output);
    } else {
        let output = format_matches(&inner_matches, ctx.view.clone(), &ctx.options);
        print!("{}", output);
    }

    // Summary to stderr
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

    if summary.errors > 0 {
        return Err(format!("{} error{} found", summary.errors,
            if summary.errors == 1 { "" } else { "s" }).into());
    }

    Ok(())
}

/// Render a test report. Returns Err if the test failed and `warning` is false.
pub fn render_test_report(
    report: &Report,
    ctx: &RunContext,
    message: &Option<String>,
    error_template: &Option<String>,
    warning: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = report.summary.as_ref().expect("test report must have summary");

    if ctx.ser_format == SerFormat::Json {
        print!("{}", report.to_json());

        if !summary.passed && !warning {
            return Err(format!(
                "expectation failed: expected {}, got {} matches",
                summary.expected.as_deref().unwrap_or("?"), summary.total
            ).into());
        }
        return Ok(());
    }

    // Text output: colored pass/fail
    let (symbol, color) = if summary.passed {
        ("✓", test_colors::GREEN)
    } else if warning {
        ("⚠", test_colors::YELLOW)
    } else {
        ("✗", test_colors::RED)
    };

    let label = message.as_deref().unwrap_or("");
    let expected_str = summary.expected.as_deref().unwrap_or("?");

    if ctx.use_color {
        if label.is_empty() {
            println!("{}{}{} {} matches{}",
                test_colors::BOLD, color, symbol, summary.total, test_colors::RESET);
        } else if summary.passed {
            println!("{}{}{} {}{}",
                test_colors::BOLD, color, symbol, label, test_colors::RESET);
        } else {
            println!("{}{}{} {} {}(expected {}, got {}){}",
                test_colors::BOLD, color, symbol, label, test_colors::RESET,
                expected_str, summary.total, test_colors::RESET);
        }
    } else if label.is_empty() {
        println!("{} {} matches", symbol, summary.total);
    } else if summary.passed {
        println!("{} {}", symbol, label);
    } else {
        println!("{} {} (expected {}, got {})", symbol, label, expected_str, summary.total);
    }

    // Error details when test failed
    if !summary.passed && !report.matches.is_empty() {
        let inner_matches: Vec<_> = report.matches.iter().map(|rm| rm.inner.clone()).collect();

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
            let output = format_matches(&inner_matches, OutputFormat::Gcc, &error_options);
            for line in output.lines() {
                if ctx.use_color {
                    println!("  {}{}{}", color, line, test_colors::RESET);
                } else {
                    println!("  {}", line);
                }
            }
        } else {
            let output = format_matches(&inner_matches, ctx.view.clone(), &ctx.options);
            for line in output.lines() {
                println!("  {}", line);
            }
        }
    }

    if !summary.passed && !warning {
        return Err(format!(
            "expectation failed: expected {}, got {} matches",
            expected_str, summary.total
        ).into());
    }

    Ok(())
}
