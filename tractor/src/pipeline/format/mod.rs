pub mod options;
pub mod gcc;
pub mod github;
pub mod xml;
pub mod json;
pub mod yaml;
pub mod text;
mod shared;

pub use options::{OutputFormat, ViewField, ViewSet, parse_view_set};
pub use gcc::{render_gcc, render_gcc_report_with_template};
pub use github::render_github;
pub use xml::render_xml_report;
pub use json::render_json_report;
pub use yaml::render_yaml_report;
pub use text::render_text_report;

use tractor_core::{
    render_xml_node,
    render_source_precomputed, render_lines,
    report::Report,
};
use crate::pipeline::context::RunContext;
use crate::modes::test::test_colors;

// ---------------------------------------------------------------------------
// Query report renderer — dispatches to format-specific renderers
// ---------------------------------------------------------------------------

/// Render a query (or explore) report to stdout.
pub fn render_query_report(
    report: &Report,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Gcc    => print!("{}", render_gcc(report, &ctx.render_options())),
        OutputFormat::Github => print!("{}", render_github(report)),
        OutputFormat::Text   => print!("{}", render_text_report(report, &ctx.view, &ctx.render_options())),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Set report renderer — dispatches to format-specific renderers
// ---------------------------------------------------------------------------

/// Render a set-command report to stdout.
///
/// In text format, the summary is printed to stderr (not stdout) so that
/// stdout is clean for piping when stdout mode is active.
pub fn render_set_report(
    report: &Report,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Text | OutputFormat::Gcc | OutputFormat::Github => {
            print!("{}", render_text_report(report, &ctx.view, &ctx.render_options()))
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Check report renderer — dispatches to format-specific renderers
// ---------------------------------------------------------------------------

/// Render a check report. Returns Err if there are error-severity violations.
pub fn render_check_report(
    report: &Report,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let totals = report.totals.as_ref().expect("check report must have totals");

    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Gcc    => { print!("{}", render_gcc(report, &ctx.render_options())); print_check_summary(totals); }
        OutputFormat::Github => print!("{}", render_github(report)),
        OutputFormat::Text   => print!("{}", render_text_report(report, &ctx.view, &ctx.render_options())),
    }

    if totals.errors > 0 {
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
) -> Result<(), Box<dyn std::error::Error>> {
    let success = report.success.unwrap_or(true);
    let totals = report.totals.as_ref().expect("test report must have totals");

    match ctx.output_format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Xml => {
            match ctx.output_format {
                OutputFormat::Json => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
                OutputFormat::Yaml => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
                OutputFormat::Xml  => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
                _ => unreachable!(),
            }
            if !success {
                return Err(Box::new(crate::SilentExit));
            }
            return Ok(());
        }
        _ => {}
    }

    // Text/gcc/github: colored pass/fail line
    let (symbol, color) = if success {
        ("✓", test_colors::GREEN)
    } else {
        ("✗", test_colors::RED)
    };

    let label        = message.as_deref().unwrap_or("");
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

    if !success && !report.matches.is_empty() {
        if let Some(ref error_tmpl) = error_template {
            let out = render_gcc_report_with_template(&report.matches, error_tmpl, false, &ctx.render_options());
            for line in out.lines() {
                if ctx.use_color {
                    println!("  {}{}{}", color, line, test_colors::RESET);
                } else {
                    println!("  {}", line);
                }
            }
        } else {
            let opts = ctx.render_options();
            for rm in &report.matches {
                let rendered = if let Some(ref s) = rm.source {
                    render_source_precomputed(
                        s, rm.tree.as_ref(),
                        rm.line, rm.column, rm.end_line, rm.end_column,
                        &opts,
                    )
                } else if let Some(ref ls) = rm.lines {
                    render_lines(ls, rm.tree.as_ref(), rm.line, rm.column, rm.end_line, rm.end_column, &opts)
                } else if let Some(ref v) = rm.value {
                    format!("{}\n", v)
                } else if let Some(ref node) = rm.tree {
                    let rendered = render_xml_node(node, &opts);
                    if opts.pretty_print && !rendered.ends_with('\n') {
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
// Run report renderer — dispatches to format-specific renderers
// ---------------------------------------------------------------------------

/// Render a unified run report (multiple operations) to stdout.
pub fn render_run_report(
    report: &Report,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    use tractor_core::report::ReportKind;

    let success = report.success.unwrap_or(true);
    let totals = report.totals.as_ref().expect("run report must have totals");

    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Gcc | OutputFormat::Github | OutputFormat::Text => {
            // For text-based formats, render each sub-report inline.
            if let Some(ref ops) = report.operations {
                for sub in ops {
                    match sub.kind {
                        ReportKind::Check => {
                            match ctx.output_format {
                                OutputFormat::Gcc => print!("{}", render_gcc(sub, &ctx.render_options())),
                                OutputFormat::Github => print!("{}", render_github(sub)),
                                OutputFormat::Text => print!("{}", render_text_report(sub, &ctx.view, &ctx.render_options())),
                                _ => unreachable!(),
                            }
                        }
                        ReportKind::Query => {
                            match ctx.output_format {
                                OutputFormat::Text => print!("{}", render_text_report(sub, &ctx.view, &ctx.render_options())),
                                _ => print!("{}", render_gcc(sub, &ctx.render_options())),
                            }
                        }
                        ReportKind::Test => {
                            if let Some(ref t) = sub.totals {
                                let sub_success = sub.success.unwrap_or(true);
                                let expected = sub.expected.as_deref().unwrap_or("?");
                                if sub_success {
                                    eprintln!("test passed: expected {}, got {} match{}", expected, t.results, if t.results == 1 { "" } else { "es" });
                                } else {
                                    eprintln!("test failed: expected {}, got {} match{}", expected, t.results, if t.results == 1 { "" } else { "es" });
                                }
                            }
                        }
                        ReportKind::Set => {
                            match ctx.output_format {
                                OutputFormat::Text => print!("{}", render_text_report(sub, &ctx.view, &ctx.render_options())),
                                _ => {
                                    // Gcc-style set output: render using groups if available
                                    let matches: Vec<&tractor_core::report::ReportMatch> = if let Some(ref groups) = sub.groups {
                                        groups.iter().flat_map(|g| g.matches.iter()).collect()
                                    } else {
                                        sub.matches.iter().collect()
                                    };
                                    for rm in matches {
                                        let file = if rm.file.is_empty() {
                                            // Get file from group
                                            sub.groups.as_ref()
                                                .and_then(|gs| gs.first())
                                                .map(|g| g.file.as_str())
                                                .unwrap_or("")
                                        } else {
                                            &rm.file
                                        };
                                        let status = rm.status.as_deref().unwrap_or("unknown");
                                        eprintln!("{}: {}", file, status);
                                    }
                                }
                            }
                        }
                        ReportKind::Run => {} // nested run not expected
                    }
                }
            }
            // Print run-level summary
            print_run_summary(totals, report.operations.as_deref());
        }
    }

    if !success {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

fn print_run_summary(totals: &tractor_core::report::Totals, operations: Option<&[Report]>) {
    use tractor_core::report::ReportKind;
    let mut parts = Vec::new();

    if totals.errors > 0 {
        parts.push(format!("{} check violation{}", totals.errors,
            if totals.errors == 1 { "" } else { "s" }));
    }

    // Count set updates from sub-reports
    if let Some(ops) = operations {
        let set_updated: usize = ops.iter()
            .filter(|r| matches!(r.kind, ReportKind::Set))
            .filter_map(|r| r.totals.as_ref())
            .map(|t| t.updated)
            .sum();
        let set_drift: usize = ops.iter()
            .filter(|r| matches!(r.kind, ReportKind::Set))
            .filter(|r| !r.success.unwrap_or(true))
            .filter_map(|r| r.totals.as_ref())
            .map(|t| t.files)
            .sum();
        if set_drift > 0 {
            parts.push(format!("{} file{} out of sync", set_drift,
                if set_drift == 1 { "" } else { "s" }));
        } else if set_updated > 0 {
            parts.push(format!("updated {} file{}", set_updated,
                if set_updated == 1 { "" } else { "s" }));
        }
    }

    if !parts.is_empty() {
        eprintln!();
        for part in &parts {
            eprintln!("{}", part);
        }
    }
}

fn print_check_summary(totals: &tractor_core::report::Totals) {
    if totals.results > 0 {
        eprintln!();
        let kind = if totals.errors > 0 {
            format!("{} error{}", totals.errors, if totals.errors == 1 { "" } else { "s" })
        } else {
            format!("{} warning{}", totals.warnings, if totals.warnings == 1 { "" } else { "s" })
        };
        eprintln!("{} in {} file{}", kind, totals.files, if totals.files == 1 { "" } else { "s" });
    }
}
