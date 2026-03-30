//! Run mode: execute a tractor config file containing mixed operations.

use tractor_core::report::Report;
use crate::executor::{self, ExecuteOptions};
use crate::tractor_config::load_tractor_config;
use crate::cli::RunArgs;
use crate::pipeline::{
    RunContext, ViewField,
    project_report, apply_message_template,
    GroupDimension,
};
use crate::pipeline::render_report;

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
        &[ViewField::Command, ViewField::Reason, ViewField::Severity, ViewField::Lines, ViewField::Status, ViewField::Value],
        args.view.as_deref(),
        args.message,
        None,             // no content
        false,            // no debug
        &[GroupDimension::Command, GroupDimension::File],  // group by command then file
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

    // Merge all sub-reports into a single flat report.
    let mut report = Report::run(reports);

    // Apply message template and view projection on the merged report.
    if let Some(ref template) = ctx.message {
        apply_message_template(&mut report, template);
    }
    project_report(&mut report, &ctx.view);

    // Apply grouping
    let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
    let report = report.with_grouping(&dims);

    render_report(&report, &ctx, None)
}
