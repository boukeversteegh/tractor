//! tractor - Multi-language code query tool using XPath 3.1
//!
//! This is the main CLI entry point that orchestrates parsing and querying.

mod cli;
mod version;
mod xpath_utils;
mod pipeline;
mod modes;
mod rules_config;

use std::process::ExitCode;
use cli::{Cli, Command};
use clap::Parser;
use modes::{check::run_check, test::run_test, set::run_set, update::run_update, query::run_query, render::run_render};

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
        Some(Command::Update(args)) => run_update(args),
        Some(Command::Render(args)) => run_render(args),
        None => run_query(cli.query),
    };

    if let Err(e) = result {
        let msg = e.to_string();
        if !msg.is_empty() {
            eprintln!("error: {}", msg);
        }
        return ExitCode::FAILURE;
    }

    // Print timing stats if TRACTOR_PROFILE env var is set
    if std::env::var("TRACTOR_PROFILE").is_ok() {
        tractor_core::print_parse_timing_stats();
        tractor_core::print_timing_stats();
    }

    ExitCode::SUCCESS
}
