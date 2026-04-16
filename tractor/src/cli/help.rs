//! CLI help enhancements: shared `--view` long_help and parse error hints.

use super::Cli;
use crate::format::{OutputFormat, ViewField};

// ---------------------------------------------------------------------------
// --view long_help injection
// ---------------------------------------------------------------------------

/// Extension trait for `clap::Command` to inject help text that can't be
/// expressed as string literals in derive attributes.
pub(crate) trait CommandExt {
    /// Inject shared `--view` long_help (per-subcommand defaults) and `after_help`.
    fn with_help(self) -> clap::Command;
}

impl CommandExt for clap::Command {
    fn with_help(self) -> clap::Command {
        use ViewField::*;
        let cmd = self
            .after_help(AFTER_HELP)
            .after_long_help(AFTER_HELP);
        let cmd = augment_arg_help(cmd, &[File, Line, Tree], "text"); // root = query defaults
        cmd.mut_subcommand("query", |c| augment_arg_help(c, &[File, Line, Tree], "text"))
           .mut_subcommand("check", |c| augment_arg_help(c, &[Reason, Severity, Lines], "gcc"))
           .mut_subcommand("test",  |c| augment_arg_help(c, &[Totals], "text"))
           .mut_subcommand("set",   |c| augment_arg_help(c, &[File, Line, Status, Reason], "text"))
           .mut_subcommand("run",   |c| augment_arg_help(c, &[ViewField::Command, Reason, Severity, Lines, Status, Value], "gcc"))
    }
}

const AFTER_HELP: &str = r#"WORKFLOW:
    1. Explore structure across files with schema view (depth 4 by default):
       tractor "src/**/*.cs" -v schema
       tractor "src/**/*.cs" -x "//class" -v schema

    2. View the full XML of specific code:
       tractor src/main.rs

    3. Add -x to select specific elements:
       tractor src/main.rs -x "//function"

    4. Refine with predicates:
       tractor src/main.rs -x "//function[name='main']"

    5. Choose view with -v:
       tractor src/main.rs -x "//function/name" -v value

EXAMPLES:
    # See what element types exist across all C# files (default depth 4)
    tractor "src/**/*.cs" -v schema

    # See deeper structure with custom depth
    tractor "src/**/*.cs" -v schema -d 6

    # See structure of all classes
    tractor "src/**/*.cs" -x "//class" -v schema

    # Query all C# files for classes
    tractor "src/**/*.cs" -x "//class"

    # Find methods missing OrderBy in Repository classes
    tractor "src/**/*.cs" -x "//class[contains(name,'Repository')]//method[not(contains(.,'OrderBy'))]" -v value

    # Parse from stdin
    echo "public class Foo { }" | tractor -l csharp -x "//class/name" -v value

    # Parse from argument — escape-proof, works with multiline code
    tractor -s "$(cat <<'CODE'
    public class Foo {
        public void Bar() { }
    }
    CODE
    )" -l csharp -x "//class/name" -v value

    # CI: fail if any TODO comments found
    tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO comment found"

    # CI: test expectations
    tractor test "src/**/*.cs" -x "//class" --expect 5 -m "should have 5 classes"

    # GitHub Actions: annotate errors in PR
    tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO comment found" -f github

    # JSON report output
    tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO" -f json

    # Whitespace-insensitive matching
    tractor file.cs -x "//type[.='Dictionary<string,int>']" -W

    # Replace values in files
    tractor set config.yaml -x "//database/host" "db.example.com"
"#;

/// Inject `long_help` for `--view` and `--format` on a single `clap::Command`.
fn augment_arg_help(cmd: clap::Command, view_defaults: &[ViewField], format_default: &str) -> clap::Command {
    let view_help = ViewField::view_long_help(view_defaults);
    let format_help = OutputFormat::format_long_help(format_default);
    cmd.mut_args(|arg| {
        match arg.get_id().as_str() {
            "view"   => arg.long_help(view_help.clone()),
            "format" => arg.long_help(format_help.clone()),
            _ => arg,
        }
    })
}

// ---------------------------------------------------------------------------
// Parse error handling
// ---------------------------------------------------------------------------

pub fn handle_parse_error(e: clap::Error) -> Cli {
    e.exit();
}
