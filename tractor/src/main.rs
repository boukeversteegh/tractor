//! tractor - Multi-language code query tool using XPath 3.1
//!
//! This is the main CLI entry point that orchestrates parsing and querying.

mod cli;
mod version;
mod xpath_utils;
mod pipeline;
mod modes;

use std::process::ExitCode;
use cli::{Cli, Command};
use clap::Parser;
use modes::{check::run_check, test::run_test, set::run_set, query::run_query};

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

    let result = match cli.command {
        Some(Command::Query(args)) => run_query(args),
        Some(Command::Check(args)) => run_check(args),
        Some(Command::Test(args)) => run_test(args),
        Some(Command::Set(args)) => run_set(args),
        None => run_query(cli.query),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }

    // Print timing stats if TRACTOR_PROFILE env var is set
    if std::env::var("TRACTOR_PROFILE").is_ok() {
        tractor_core::print_parse_timing_stats();
        tractor_core::print_timing_stats();
    }

    ExitCode::SUCCESS
}
