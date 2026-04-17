//! `tractor init` — scaffold a starter `tractor.yaml` in the current directory.

use clap::Args;
use std::fs;
use std::path::Path;

/// Starter content written by `tractor init`.
///
/// The template lives in `tests/integration/init/tractor.yml` so it can be
/// reviewed and diffed as a fixture file. `include_str!` pulls it in at build
/// time — same text ships in the binary and the snapshot.
pub const STARTER_CONFIG: &str =
    include_str!("../../../tests/integration/init/tractor.yml");

/// Default file name for the scaffolded config.
const DEFAULT_FILE: &str = "tractor.yml";

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Overwrite the file if it already exists
    #[arg(long = "force")]
    pub force: bool,
}

pub fn run_init(args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(DEFAULT_FILE);

    if path.exists() && !args.force {
        return Err(format!(
            "{DEFAULT_FILE} already exists — pass --force to overwrite"
        )
        .into());
    }

    fs::write(path, STARTER_CONFIG)?;
    println!("created {DEFAULT_FILE}");
    println!("run `tractor run` to execute it");
    Ok(())
}
