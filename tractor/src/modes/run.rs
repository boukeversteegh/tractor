//! Run mode: execute a tractor config file containing mixed operations.

use crate::executor::{self, ExecuteOptions, OperationResult};
use crate::tractor_config::load_tractor_config;
use crate::cli::RunArgs;

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

    // Resolve base_dir: use the config file's parent directory so that
    // relative file globs in the config are resolved relative to it.
    let base_dir = config_path.parent()
        .map(|p| if p.as_os_str().is_empty() { std::path::Path::new(".") } else { p })
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));

    let options = ExecuteOptions {
        verify: args.verify,
        verbose: args.shared.verbose,
        base_dir,
    };

    let result = executor::execute(&operations, &options)?;

    // Report results
    let mut check_violations = 0usize;
    let mut set_files_modified = 0usize;
    let mut set_drift_files = 0usize;

    for op_result in result.results.iter() {
        match op_result {
            OperationResult::Check(check) => {
                for violation in &check.violations {
                    eprintln!(
                        "{}:{}:{}: {}: {} [{}]",
                        violation.file,
                        violation.line,
                        violation.column,
                        match violation.severity {
                            tractor_core::report::Severity::Error => "error",
                            tractor_core::report::Severity::Warning => "warning",
                        },
                        violation.reason,
                        violation.rule_id,
                    );
                }
                check_violations += check.violations.len();
            }
            OperationResult::Set(set) => {
                for change in &set.changes {
                    if change.was_modified {
                        if args.verify {
                            eprintln!(
                                "drift: {} ({} mapping{} would change)",
                                change.file,
                                change.mappings_applied,
                                if change.mappings_applied == 1 { "" } else { "s" },
                            );
                            set_drift_files += 1;
                        } else {
                            if args.shared.verbose {
                                eprintln!(
                                    "updated: {} ({} mapping{})",
                                    change.file,
                                    change.mappings_applied,
                                    if change.mappings_applied == 1 { "" } else { "s" },
                                );
                            }
                            set_files_modified += 1;
                        }
                    }
                }
            }
        }
    }

    // Summary
    if args.shared.verbose || !result.success() {
        if check_violations > 0 {
            eprintln!(
                "{} check violation{}",
                check_violations,
                if check_violations == 1 { "" } else { "s" },
            );
        }
        if args.verify && set_drift_files > 0 {
            eprintln!(
                "{} file{} out of sync",
                set_drift_files,
                if set_drift_files == 1 { "" } else { "s" },
            );
        }
        if !args.verify && set_files_modified > 0 {
            eprintln!(
                "updated {} file{}",
                set_files_modified,
                if set_files_modified == 1 { "" } else { "s" },
            );
        }
    }

    if result.success() {
        Ok(())
    } else {
        Err(crate::SilentExit.into())
    }
}
