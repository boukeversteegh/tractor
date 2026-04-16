//! Run mode: execute a tractor config file containing mixed operations.

use clap::Args;
use crate::cli::SharedArgs;

/// Run mode: execute a tractor config file with mixed operations
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the tractor config file (.yaml, .yml, or .toml).
    /// File patterns in the config are resolved relative to the config file's directory.
    #[arg()]
    pub config: String,

    /// Files to process (intersected with config file globs)
    #[arg()]
    pub files: Vec<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Output format [default: gcc]
    #[arg(short = 'f', long = "format", default_value = "gcc", help_heading = "Output")]
    pub format: String,

    /// Report fields to include (e.g. tree, value, source)
    #[arg(short = 'v', long = "view", help_heading = "Output")]
    pub view: Option<String>,

    /// Message template for matches (e.g. "{file}:{line}: {value}")
    #[arg(short = 'm', long = "message", help_heading = "Output")]
    pub message: Option<String>,
}
use crate::format::{ViewField, GroupDimension};
use super::config::{run_from_config, ConfigRunParams};

pub fn run_run(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    run_from_config(ConfigRunParams {
        config_path: &args.config,
        shared: &args.shared,
        cli_files: args.files,
        format: &args.format,
        default_view: &[ViewField::Command, ViewField::Reason, ViewField::Severity, ViewField::Lines, ViewField::Status, ViewField::Value],
        view_override: args.view.as_deref(),
        message: args.message,
        default_group: &[GroupDimension::Command, GroupDimension::File],
        op_filter: |_| true,
        filter_label: "",  // run accepts all operations; empty filter never triggers
    })
}
