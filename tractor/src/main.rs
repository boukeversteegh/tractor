//! tractor - Multi-language code query tool using XPath 3.1
//!
//! This is the main CLI entry point that orchestrates parsing and querying.

mod cli;
mod version;
mod xpath_utils;
mod pipeline;
mod modes;
mod rules_config;
mod tractor_config;
mod executor;
mod filter;

use std::process::ExitCode;
use cli::{Cli, Command};
use clap::Parser;
use modes::{check::run_check, test::run_test, set::run_set, update::run_update, query::run_query, render::run_render, run::run_run};
use tractor_core::report::{ReportBuilder, ReportMatch, Severity, DiagnosticOrigin};
use pipeline::format::{OutputFormat, ViewField, ViewSet, render_gcc, render_text_report, render_json_report, render_yaml_report, render_xml_report, render_github};
use tractor_core::output::{should_use_color, RenderOptions};

/// An error that has already been reported to the user; main should exit with
/// failure but not print an additional "error: ..." line.
pub struct SilentExit;
impl std::fmt::Display for SilentExit {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }
}
impl std::fmt::Debug for SilentExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "SilentExit") }
}
impl std::error::Error for SilentExit {}

// ---------------------------------------------------------------------------
// Error report rendering
// ---------------------------------------------------------------------------

/// Render a diagnostic report in the user's requested format.
/// Used as fallback when errors reach main() before a RunContext is built.
/// Machine-consumed formats go to stdout for consistency with normal output.
fn render_error_report(
    report: &tractor_core::report::Report,
    format: OutputFormat,
    use_color: bool,
) {
    let view = ViewSet::new(vec![
        ViewField::Origin, ViewField::Reason, ViewField::Severity, ViewField::Lines,
    ]);
    let render_opts = RenderOptions::new().with_color(use_color);
    match format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &view, &render_opts, &[])),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &view, &render_opts, &[])),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &view, &render_opts, &[])),
        OutputFormat::Github => print!("{}", render_github(report, &[])),
        OutputFormat::Gcc    => print!("{}", render_gcc(report, &render_opts, &[])),
        OutputFormat::Text   => print!("{}", render_text_report(report, &view, &render_opts, &[])),
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Handle --version flag (query/default mode)
    let version_args = match &cli.command {
        Some(Command::Query(args)) => Some(args),
        None => Some(&cli.query),
        _ => None,
    };
    if let Some(args) = version_args {
        if args.version {
            if args.shared.verbose {
                version::print_version_verbose();
            } else {
                version::print_version();
            }
            return ExitCode::SUCCESS;
        }
    }

    // Extract format and color settings before dispatch (for error fallback rendering).
    let format_str = match &cli.command {
        Some(Command::Check(a)) => a.format.as_str(),
        Some(Command::Query(a)) => a.format.as_str(),
        Some(Command::Test(a))  => a.format.as_str(),
        Some(Command::Set(a))   => a.format.as_str(),
        Some(Command::Run(a))   => a.format.as_str(),
        Some(Command::Update(_)) | Some(Command::Render(_)) => "text",
        None => cli.query.format.as_str(),
    };
    let fallback_format = OutputFormat::from_str(format_str).unwrap_or(OutputFormat::Text);
    let shared = match &cli.command {
        Some(Command::Check(a)) => &a.shared,
        Some(Command::Query(a)) => &a.shared,
        Some(Command::Test(a))  => &a.shared,
        Some(Command::Set(a))   => &a.shared,
        Some(Command::Update(a)) => &a.shared,
        Some(Command::Run(a))   => &a.shared,
        Some(Command::Render(_)) => &cli.query.shared,
        None => &cli.query.shared,
    };
    let fallback_color = if shared.no_color { false } else { should_use_color(&shared.color) };

    let result = match cli.command {
        Some(Command::Query(args)) => run_query(args),
        Some(Command::Check(args)) => run_check(args),
        Some(Command::Test(args)) => run_test(args),
        Some(Command::Set(args)) => run_set(args),
        Some(Command::Update(args)) => run_update(args),
        Some(Command::Render(args)) => run_render(args),
        Some(Command::Run(args)) => run_run(args),
        None => run_query(cli.query),
    };

    if let Err(e) = result {
        let msg = e.to_string();
        if msg.is_empty() {
            // SilentExit — already reported
            return ExitCode::FAILURE;
        }
        // Wrap the error in a minimal fatal report for format-aware rendering
        let rm = ReportMatch {
            file: String::new(),
            line: 0, column: 0, end_line: 0, end_column: 0,
            command: String::new(),
            tree: None, value: None, source: None, lines: None,
            reason: Some(msg),
            severity: Some(Severity::Fatal),
            message: None, hint: None,
            origin: Some(DiagnosticOrigin::Cli),
            rule_id: None, status: None, output: None,
        };
        let mut builder = ReportBuilder::new();
        builder.add(rm);
        let report = builder.build();
        render_error_report(&report, fallback_format, fallback_color);
        return ExitCode::FAILURE;
    }

    // Print timing stats if TRACTOR_PROFILE env var is set
    if std::env::var("TRACTOR_PROFILE").is_ok() {
        tractor_core::print_parse_timing_stats();
        tractor_core::print_timing_stats();
    }

    ExitCode::SUCCESS
}
