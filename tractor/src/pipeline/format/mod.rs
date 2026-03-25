pub mod options;
pub mod gcc;
pub mod github;
pub mod xml;
pub mod json;
pub mod yaml;
pub mod text;
mod shared;

pub use options::{OutputFormat, ViewField, ViewSet, parse_view_with_defaults};
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
    let summary = report.summary.as_ref().expect("check report must have summary");

    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options())),
        OutputFormat::Gcc    => { print!("{}", render_gcc(report, &ctx.render_options())); print_check_summary(summary); }
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
            if !summary.passed {
                return Err(Box::new(crate::SilentExit));
            }
            return Ok(());
        }
        _ => {}
    }

    // Text/gcc/github: colored pass/fail line
    let (symbol, color) = if summary.passed {
        ("✓", test_colors::GREEN)
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

    if !summary.passed {
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

    let summary = report.summary.as_ref().expect("run report must have summary");

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
                            let sub_summary = sub.summary.as_ref();
                            if let Some(s) = sub_summary {
                                let expected = s.expected.as_deref().unwrap_or("?");
                                if s.passed {
                                    eprintln!("test passed: expected {}, got {} match{}", expected, s.total, if s.total == 1 { "" } else { "es" });
                                } else {
                                    eprintln!("test failed: expected {}, got {} match{}", expected, s.total, if s.total == 1 { "" } else { "es" });
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
            print_run_summary(summary, report.operations.as_deref());
        }
    }

    if !summary.passed {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

fn print_run_summary(summary: &tractor_core::report::Summary, operations: Option<&[Report]>) {
    use tractor_core::report::ReportKind;
    let mut parts = Vec::new();

    if summary.errors > 0 {
        parts.push(format!("{} check violation{}", summary.errors,
            if summary.errors == 1 { "" } else { "s" }));
    }

    // Count set updates from sub-reports
    if let Some(ops) = operations {
        let set_updated: usize = ops.iter()
            .filter(|r| matches!(r.kind, ReportKind::Set))
            .filter_map(|r| r.summary.as_ref())
            .map(|s| s.errors) // errors = updated count for set
            .sum();
        let set_drift: usize = ops.iter()
            .filter(|r| matches!(r.kind, ReportKind::Set))
            .filter_map(|r| r.summary.as_ref())
            .filter(|s| !s.passed)
            .map(|s| s.files_affected)
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
