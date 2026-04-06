//! Run mode: execute a tractor config file containing mixed operations.

use crate::cli::RunArgs;
use crate::pipeline::{ViewField, GroupDimension};
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
