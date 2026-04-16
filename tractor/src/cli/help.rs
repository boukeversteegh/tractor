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
// Parse error hints
// ---------------------------------------------------------------------------

/// Handle a clap parse error, adding a hint about `=` syntax when a flag
/// like `-v` is followed by a hyphen-prefixed value (e.g. `-v -tree`).
pub fn handle_parse_error(e: clap::Error) -> Cli {
    let plain = e.render().to_string();
    if plain.contains("a value is required for") {
        if let Some((flag, next)) = detect_hyphen_value_pattern() {
            let use_color = e.use_stderr();
            let rendered = if use_color {
                e.render().ansi().to_string()
            } else {
                plain.clone()
            };
            let (green, yellow, reset) = if use_color {
                ("\x1b[32m", "\x1b[33m", "\x1b[0m")
            } else {
                ("", "", "")
            };
            let tip = format!(
                "\n  {green}tip:{reset} to pass a value starting with '-', use '{yellow}{flag}={next}{reset}'\n"
            );
            // Insert tip before the "For more information" line.
            // Locate in plain text, then find the matching newline in
            // the rendered (possibly ANSI) string by newline count.
            let marker = "\nFor more information";
            if let Some(plain_pos) = plain.find(marker) {
                let nl_count = plain[..plain_pos].matches('\n').count();
                if let Some(ansi_pos) = rendered.match_indices('\n')
                    .nth(nl_count).map(|(i, _)| i)
                {
                    eprint!("{}{}{}", &rendered[..ansi_pos], tip, &rendered[ansi_pos..]);
                } else {
                    eprint!("{rendered}{tip}");
                }
            } else {
                eprint!("{rendered}{tip}");
            }
            std::process::exit(2);
        }
    }
    e.exit();
}

/// Check if the raw CLI args contain a pattern like `-v -something` where
/// a flag that takes a value is followed by what looks like another flag.
/// Returns `(flag, next_arg)` for the hint, e.g. `("-v", "-tree")`.
fn detect_hyphen_value_pattern() -> Option<(String, String)> {
    let args: Vec<String> = std::env::args().collect();
    // Flags that take a value and support values starting with - or +
    let value_flags: &[(&str, &str)] = &[
        ("-v", "--view"),
        ("-t", "--tree"),
    ];

    for window in args.windows(2) {
        let (flag, next) = (&window[0], &window[1]);
        let is_value_flag = value_flags.iter().any(|(short, long)| flag == short || flag == *long);
        if !is_value_flag {
            continue;
        }
        // Next arg looks like a flag (starts with -), but not bare `--`
        if next.starts_with('-') && next.len() > 1 && next != "--" {
            return Some((flag.clone(), next.clone()));
        }
    }
    None
}
