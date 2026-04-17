//! Shared config file executor for all commands.
//!
//! Each command (`check`, `query`, `test`, `set`, `run`) can execute operations
//! from a tractor config file. The only differences are:
//!
//! - **Operation filter**: which operation types to run (e.g. check only runs
//!   `Check` operations, run executes everything).
//! - **Defaults**: view fields, grouping, format — tuned per command.

use tractor::report::{ReportMatch, Severity};

use crate::cli::SharedArgs;
use crate::executor::{self, ExecuteOptions, Operation};
use crate::cli::context::RunContext;
use crate::format::{ViewField, GroupDimension, render_report};
use crate::matcher::prepare_report_for_output;

/// Canonical file name tractor probes when `--config` is not passed.
///
/// Kept to a single name on purpose — one consistent filename across projects
/// makes it easier for anyone to jump in and recognize the config. Users who
/// prefer `.yaml` (or any other name) can still point at it explicitly via
/// `tractor run --config path/to/config.yaml`.
pub const DEFAULT_CONFIG_NAME: &str = "tractor.yml";

/// Resolve a config path from `--config`, falling back to `tractor.yml` in the
/// current directory. Returns a clear error when the flag is absent and the
/// default does not exist.
pub fn resolve_config_path(
    explicit: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(path) = explicit {
        return Ok(path.to_string());
    }
    if std::path::Path::new(DEFAULT_CONFIG_NAME).exists() {
        return Ok(DEFAULT_CONFIG_NAME.to_string());
    }
    Err(format!(
        "no {DEFAULT_CONFIG_NAME} in the current directory\n\
         \n\
         hint: run `tractor init` to scaffold one,\n\
         hint: or pass a config path, e.g. `tractor run --config path/to/config.yml`"
    )
    .into())
}

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

    let mut builder = tractor::ReportBuilder::new();

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
            .map(|p| {
                // Absolutize without following symlinks — matches the glob
                // walker and CLI path resolution, so `base_dir`-derived paths
                // intersect by set equality with those pipelines.
                let normalized = tractor::NormalizedPath::absolute(&p.to_string_lossy());
                std::path::PathBuf::from(normalized.as_str())
            });

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

    prepare_report_for_output(&mut report, &ctx);
    let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
    let report = report.with_grouping(&dims);
    render_report(&report, &ctx, None)
}
