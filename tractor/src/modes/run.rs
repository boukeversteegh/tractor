//! Run mode: execute a tractor config file containing mixed operations.

use tractor_core::report::ReportKind;
use crate::executor::{self, ExecuteOptions, Operation};
use crate::tractor_config::load_tractor_config;
use crate::cli::RunArgs;

pub fn run_run(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::path::Path::new(&args.config);

    if !config_path.exists() {
        return Err(format!("config file not found: {}", args.config).into());
    }

    let mut operations = load_tractor_config(config_path)?;

    if operations.is_empty() {
        if args.shared.verbose {
            eprintln!("no operations found in {}", args.config);
        }
        return Ok(());
    }

    // Apply CLI --verify flag to all set operations.
    if args.verify {
        for op in &mut operations {
            if let Operation::Set(ref mut set_op) = op {
                set_op.verify = true;
            }
        }
    }

    // Resolve base_dir: use the config file's parent directory so that
    // relative file globs in the config are resolved relative to it.
    let base_dir = config_path.parent()
        .map(|p| if p.as_os_str().is_empty() { std::path::Path::new(".") } else { p })
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));

    let options = ExecuteOptions {
        verbose: args.shared.verbose,
        base_dir,
    };

    let reports = executor::execute(&operations, &options)?;

    // Report results from each operation
    let mut check_violations = 0usize;
    let mut set_files_modified = 0usize;
    let mut set_drift_files = 0usize;
    let mut all_passed = true;

    for report in &reports {
        let summary = match report.summary.as_ref() {
            Some(s) => s,
            None => continue,
        };

        if !summary.passed {
            all_passed = false;
        }

        match report.kind {
            ReportKind::Check => {
                for m in &report.matches {
                    eprintln!(
                        "{}:{}:{}: {}: {} {}",
                        m.file,
                        m.line,
                        m.column,
                        m.severity.map_or("error", |s| s.as_str()),
                        m.reason.as_deref().unwrap_or("check failed"),
                        m.rule_id.as_deref().map_or(String::new(), |id| format!("[{}]", id)),
                    );
                }
                check_violations += report.matches.len();
            }
            ReportKind::Set => {
                for m in &report.matches {
                    let status = m.status.as_deref().unwrap_or("unknown");
                    if status == "updated" {
                        if args.verify {
                            eprintln!(
                                "drift: {} ({})",
                                m.file,
                                m.output.as_deref().unwrap_or("would change"),
                            );
                            set_drift_files += 1;
                        } else {
                            if args.shared.verbose {
                                eprintln!("updated: {}", m.file);
                            }
                            set_files_modified += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Summary
    if args.shared.verbose || !all_passed {
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

    if all_passed {
        Ok(())
    } else {
        Err(crate::SilentExit.into())
    }
}
