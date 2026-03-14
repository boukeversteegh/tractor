pub mod options;
pub mod gcc;
pub mod github;
pub mod xml;
pub mod json;
pub mod yaml;
pub mod text;
mod shared;

pub use options::{OutputFormat, ViewField, ViewSet, parse_view_set, view};
pub use gcc::{render_gcc, render_gcc_with_template};
pub use github::render_github;
pub use xml::render_xml_report;
pub use json::render_json_report;
pub use yaml::render_yaml_report;
pub use text::render_text_report;

use tractor_core::{format_matches, report::Report};
use crate::pipeline::context::RunContext;
use crate::modes::test::test_colors;

// ---------------------------------------------------------------------------
// Check report renderer — dispatches to format-specific renderers
// ---------------------------------------------------------------------------

/// Render a check report. Returns Err if there are error-severity violations.
pub fn render_check_report(
    report: &Report,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = report.summary.as_ref().expect("check report must have summary");

    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Gcc    => { print!("{}", render_gcc(report)); print_check_summary(summary); }
        OutputFormat::Github => print!("{}", render_github(report)),
        OutputFormat::Text   => print!("{}", render_text_report(report, &ctx.view, &ctx.render_options())),
    }

    if summary.errors > 0 {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Test report renderer — dispatches to format-specific renderers
// ---------------------------------------------------------------------------

/// Render a test report. Returns Err if the test failed and `warning` is false.
pub fn render_test_report(
    report: &Report,
    ctx: &RunContext,
    message: &Option<String>,
    error_template: &Option<String>,
    warning: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = report.summary.as_ref().expect("test report must have summary");

    match ctx.output_format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Xml => {
            match ctx.output_format {
                OutputFormat::Json => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
                OutputFormat::Yaml => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
                OutputFormat::Xml  => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
                _ => unreachable!(),
            }
            if !summary.passed && !warning {
                return Err(Box::new(crate::SilentExit));
            }
            return Ok(());
        }
        _ => {}
    }

    // Text/gcc/github: colored pass/fail line
    let (symbol, color) = if summary.passed {
        ("✓", test_colors::GREEN)
    } else if warning {
        ("⚠", test_colors::YELLOW)
    } else {
        ("✗", test_colors::RED)
    };

    let label        = message.as_deref().unwrap_or("");
    let expected_str = summary.expected.as_deref().unwrap_or("?");

    if ctx.use_color {
        if label.is_empty() {
            println!("{}{}{} {} matches{}", test_colors::BOLD, color, symbol, summary.total, test_colors::RESET);
        } else if summary.passed {
            println!("{}{}{} {}{}", test_colors::BOLD, color, symbol, label, test_colors::RESET);
        } else {
            println!("{}{}{} {} {}(expected {}, got {}){}", test_colors::BOLD, color, symbol, label, test_colors::RESET, expected_str, summary.total, test_colors::RESET);
        }
    } else if label.is_empty() {
        println!("{} {} matches", symbol, summary.total);
    } else if summary.passed {
        println!("{} {}", symbol, label);
    } else {
        println!("{} {} (expected {}, got {})", symbol, label, expected_str, summary.total);
    }

    if !summary.passed && !report.matches.is_empty() {
        let inner: Vec<_> = report.matches.iter().map(|rm| rm.inner.clone()).collect();
        if let Some(ref error_tmpl) = error_template {
            let out = render_gcc_with_template(&inner, error_tmpl, ctx.options.warning);
            for line in out.lines() {
                if ctx.use_color {
                    println!("  {}{}{}", color, line, test_colors::RESET);
                } else {
                    println!("  {}", line);
                }
            }
        } else {
            let out = format_matches(&inner, ctx.view.primary_output_format(), &ctx.options);
            for line in out.lines() {
                println!("  {}", line);
            }
        }
    }

    if !summary.passed && !warning {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

fn print_check_summary(summary: &tractor_core::report::Summary) {
    if summary.total > 0 {
        eprintln!();
        let kind = if summary.errors > 0 {
            format!("{} error{}", summary.errors, if summary.errors == 1 { "" } else { "s" })
        } else {
            format!("{} warning{}", summary.warnings, if summary.warnings == 1 { "" } else { "s" })
        };
        eprintln!("{} in {} file{}", kind, summary.files_affected, if summary.files_affected == 1 { "" } else { "s" });
    }
}
