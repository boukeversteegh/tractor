pub mod options;
pub mod gcc;
pub mod github;
pub mod xml;
pub mod json;
pub mod yaml;
pub mod text;
mod shared;

pub use options::{OutputFormat, GroupDimension, ViewField, ViewSet, parse_view_set, parse_group_by};
pub use gcc::{render_gcc, render_gcc_report_with_template};
pub use github::render_github;
pub use xml::render_xml_report;
pub use json::render_json_report;
pub use yaml::render_yaml_report;
pub use text::render_text_report;

use tractor_core::{
    render_xml_node,
    render_source_precomputed, render_lines,
    report::{Report, ReportMatch},
};
use crate::pipeline::context::RunContext;
use crate::modes::test::test_colors;

/// Options for test-specific rendering (colored pass/fail, error detail).
/// When None, the report is rendered generically.
pub struct TestRenderOptions {
    pub message: Option<String>,
    pub error_template: Option<String>,
}

/// Render any report to stdout. Unified entry point for all command modes.
///
/// - Dispatches to format-specific renderers (json, yaml, xml, gcc, github, text).
/// - Prints gcc-style summary to stderr when format is gcc and report has totals.
/// - Returns Err(SilentExit) when `success == Some(false)`.
/// - For test reports, `test_opts` enables colored pass/fail rendering.
pub fn render_report(
    report: &Report,
    ctx: &RunContext,
    test_opts: Option<&TestRenderOptions>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Test reports with text/gcc/github get special colored pass/fail rendering.
    if let Some(opts) = test_opts {
        if matches!(ctx.output_format, OutputFormat::Text | OutputFormat::Gcc | OutputFormat::Github) {
            return render_test_text(report, ctx, opts);
        }
    }

    // Standard format dispatch — same for all report types.
    let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options(), &dims)),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options(), &dims)),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options(), &dims)),
        OutputFormat::Gcc    => {
            print!("{}", render_gcc(report, &ctx.render_options(), &dims));
            if let Some(ref totals) = report.totals {
                print_gcc_summary(totals);
            }
        }
        OutputFormat::Github => print!("{}", render_github(report, &dims)),
        OutputFormat::Text   => print!("{}", render_text_report(report, &ctx.view, &ctx.render_options(), &dims)),
    }

    // Exit code: fail when success is explicitly false.
    if report.success == Some(false) {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Test-specific text rendering (colored pass/fail with error detail)
// ---------------------------------------------------------------------------

fn render_test_text(
    report: &Report,
    ctx: &RunContext,
    opts: &TestRenderOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let success = report.success.unwrap_or(true);
    let totals = report.totals.as_ref().expect("test report must have totals");

    let (symbol, color) = if success {
        ("✓", test_colors::GREEN)
    } else {
        ("✗", test_colors::RED)
    };

    let label        = opts.message.as_deref().unwrap_or("");
    let expected_str = report.expected.as_deref().unwrap_or("?");

    if ctx.use_color {
        if label.is_empty() {
            println!("{}{}{} {} matches{}", test_colors::BOLD, color, symbol, totals.results, test_colors::RESET);
        } else if success {
            println!("{}{}{} {}{}", test_colors::BOLD, color, symbol, label, test_colors::RESET);
        } else {
            println!("{}{}{} {} {}(expected {}, got {}){}", test_colors::BOLD, color, symbol, label, test_colors::RESET, expected_str, totals.results, test_colors::RESET);
        }
    } else if label.is_empty() {
        println!("{} {} matches", symbol, totals.results);
    } else if success {
        println!("{} {}", symbol, label);
    } else {
        println!("{} {} (expected {}, got {})", symbol, label, expected_str, totals.results);
    }

    let all_matches = report.all_matches();
    if !success && !all_matches.is_empty() {
        if let Some(ref error_tmpl) = opts.error_template {
            let flat_matches: Vec<ReportMatch> = all_matches.into_iter().cloned().collect();
            let out = render_gcc_report_with_template(&flat_matches, error_tmpl, false, &ctx.render_options());
            for line in out.lines() {
                if ctx.use_color {
                    println!("  {}{}{}", color, line, test_colors::RESET);
                } else {
                    println!("  {}", line);
                }
            }
        } else {
            let render_opts = ctx.render_options();
            for rm in &all_matches {
                let rendered = if let Some(ref s) = rm.source {
                    render_source_precomputed(
                        s, rm.tree.as_ref(),
                        rm.line, rm.column, rm.end_line, rm.end_column,
                        &render_opts,
                    )
                } else if let Some(ref ls) = rm.lines {
                    render_lines(ls, rm.tree.as_ref(), rm.line, rm.column, rm.end_line, rm.end_column, &render_opts)
                } else if let Some(ref v) = rm.value {
                    format!("{}\n", v)
                } else if let Some(ref node) = rm.tree {
                    let rendered = render_xml_node(node, &render_opts);
                    if render_opts.pretty_print && !rendered.ends_with('\n') {
                        format!("{}\n", rendered)
                    } else {
                        rendered
                    }
                } else {
                    String::new()
                };
                for line in rendered.lines() {
                    println!("  {}", line);
                }
            }
        }
    }

    if !success {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Gcc summary (stderr) — printed after gcc format output
// ---------------------------------------------------------------------------

fn print_gcc_summary(totals: &tractor_core::report::Totals) {
    let mut parts = Vec::new();

    if totals.fatals > 0 {
        parts.push(format!("{} fatal{}", totals.fatals, if totals.fatals == 1 { "" } else { "s" }));
    }
    if totals.errors > 0 {
        parts.push(format!("{} error{}", totals.errors, if totals.errors == 1 { "" } else { "s" }));
    }
    if totals.warnings > 0 && totals.errors == 0 && totals.fatals == 0 {
        parts.push(format!("{} warning{}", totals.warnings, if totals.warnings == 1 { "" } else { "s" }));
    }
    if totals.updated > 0 {
        parts.push(format!("updated {} file{}", totals.updated, if totals.updated == 1 { "" } else { "s" }));
    }

    if parts.is_empty() { return; }

    let file_part = if totals.files > 0 && (totals.fatals > 0 || totals.errors > 0 || totals.warnings > 0) {
        format!(" in {} file{}", totals.files, if totals.files == 1 { "" } else { "s" })
    } else {
        String::new()
    };

    eprintln!();
    eprintln!("{}{}", parts.join(", "), file_part);
}
