//! Run mode: execute a tractor config file containing mixed operations.

use tractor_core::report::Report;
use crate::executor::{self, ExecuteOptions};
use crate::tractor_config::load_tractor_config;
use crate::cli::RunArgs;
use crate::pipeline::{
    RunContext, ViewField,
    project_report, apply_message_template,
};
use crate::pipeline::format::render_run_report;

pub fn run_run(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::path::Path::new(&args.config);

    if !config_path.exists() {
        return Err(format!("config file not found: {}", args.config).into());
    }

    let operations = load_tractor_config(config_path)?;

    if operations.is_empty() {
        if args.shared.verbose {
            eprintln!("no operations found in {}", args.config);
        }
        return Ok(());
    }

    // Build RunContext for output formatting.
    let ctx = RunContext::build(
        &args.shared,
        vec![],           // no files — they come from the config
        None,             // no xpath — they come from the config
        &args.format,
        &[ViewField::Reason, ViewField::Severity, ViewField::Lines, ViewField::Status, ViewField::Value],
        args.view.as_deref(),
        args.message,
        None,             // no content
        false,            // no debug
        true,             // group by file
    )?;

    // Resolve base_dir: use the config file's parent directory so that
    // relative file globs in the config are resolved relative to it.
    let base_dir = config_path.parent()
        .map(|p| if p.as_os_str().is_empty() { std::path::Path::new(".") } else { p })
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));

    let options = ExecuteOptions {
        verbose: args.shared.verbose,
        base_dir,
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
    };

    let reports = executor::execute(&operations, &options)?;

    // Apply view projection and grouping to each sub-report.
    let sub_reports: Vec<Report> = reports.into_iter().map(|mut r| {
        // Apply message template if provided.
        if let Some(ref template) = ctx.message {
            apply_message_template(&mut r, template);
        }
        project_report(&mut r, &ctx.view);
        if ctx.group_by_file { r.with_groups() } else { r }
    }).collect();

    let report = Report::run(sub_reports);

    render_run_report(&report, &ctx)
}
