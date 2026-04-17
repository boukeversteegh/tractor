//! `tractor init` — scaffold a starter `tractor.yaml` in the current directory.

use clap::Args;
use std::fs;
use std::path::Path;

/// Starter content written by `tractor init`. Kept intentionally minimal: a
/// single check rule that flags TODO comments, which works on nearly any
/// source file and is easy for a new user to recognize and adapt.
pub const STARTER_CONFIG: &str = "\
# tractor config — see https://tractor-cli.com/docs/commands/run
#
# Run with `tractor run` (this file is picked up automatically when it
# sits next to the command's working directory).

check:
  files:
    - \"**/*\"
  rules:
    - id: no-todo
      xpath: \"//comment[contains(., 'TODO')]\"
      reason: \"TODO comment found\"
      severity: warning
";

/// Default file name for the scaffolded config.
const DEFAULT_FILE: &str = "tractor.yaml";

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
