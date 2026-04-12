//! Shared config file executor for all commands.
//!
//! Each command (`check`, `query`, `test`, `set`, `run`) can execute operations
//! from a tractor config file. The only differences are:
//!
//! - **Operation filter**: which operation types to run (e.g. check only runs
//!   `Check` operations, run executes everything).
//! - **Defaults**: view fields, grouping, format — tuned per command.

use tractor_core::report::{ReportMatch, Severity};

use crate::cli::SharedArgs;
use crate::executor::{self, ExecuteOptions, Operation};
use crate::cli::context::RunContext;
use crate::format::{ViewField, GroupDimension, render_report};
use crate::matcher::{project_report, apply_message_template};

/// Parameters that vary per command when executing a config file.
pub struct ConfigRunParams<'a> {
    pub config_path: &'a str,
    pub shared: &'a SharedArgs,
    pub cli_files: Vec<String>,
    pub format: &'a str,
    pub default_view: &'a [ViewField],
    pub view_override: Option<&'a str>,
    pub message: Option<String>,
    pub default_group: &'a [GroupDimension],
    pub op_filter: fn(&Operation) -> bool,
    pub filter_label: &'a str,
}

/// Load a config file, filter operations, and execute through the standard pipeline.
pub fn run_from_config(params: ConfigRunParams) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::path::Path::new(params.config_path);

    if !config_path.exists() {
        return Err(format!("config file not found: {}", params.config_path).into());
    }

    let loaded = crate::tractor_config::load_tractor_config(config_path)?;

    let operations: Vec<_> = loaded.operations.into_iter()
        .filter(params.op_filter)
        .collect();

    let ctx = RunContext::build(
        params.shared, vec![], None, params.format,
        params.default_view,
        params.view_override, params.message, None, false, params.default_group,
    )?;

    let mut builder = tractor_core::ReportBuilder::new();

    if operations.is_empty() {
        builder.add(ReportMatch {
            file: params.config_path.to_string(),
            line: 0, column: 0, end_line: 0, end_column: 0,
            command: String::new(),
            tree: None, value: None, source: None, lines: None,
            reason: Some(format!("no {} operations found", params.filter_label)),
            severity: Some(Severity::Fatal),
            message: None, origin: None, rule_id: None, status: None, output: None,
        });
    } else {
        let base_dir = config_path.parent()
            .map(|p| if p.as_os_str().is_empty() { std::path::Path::new(".") } else { p })
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));

        let options = ExecuteOptions {
            verbose: ctx.verbose,
            base_dir,
            diff_files: params.shared.diff_files.clone(),
            diff_lines: params.shared.diff_lines.clone(),
            max_files: params.shared.max_files,
            cli_files: params.cli_files,
            config_root_files: loaded.root_files,
        };

        executor::execute(&operations, &options, &mut builder)?;
    }

    let mut report = builder.build();

    if let Some(ref template) = ctx.message {
        apply_message_template(&mut report, template);
    }

    project_report(&mut report, &ctx.view);
    let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
    let report = report.with_grouping(&dims);
    render_report(&report, &ctx, None)
}
