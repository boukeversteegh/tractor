//! CLI argument parsing using clap

use clap::Parser;

/// Multi-language code query tool using XPath 3.1
#[derive(Parser, Debug)]
#[command(name = "tractor")]
#[command(author, version, about, long_about = None)]
#[command(after_help = r#"EXAMPLES:
    # Query all C# files for classes
    tractor "src/**/*.cs" -x "//class"

    # Find methods without OrderBy in Repository classes
    tractor "src/**/*.cs" -x "//class[name[contains(.,'Repository')]]/method[not(contains(.,'OrderBy'))]" -o gcc

    # Parse from stdin
    echo "public class Foo { }" | tractor --lang csharp -x "//class/name" -o value

    # CI: fail if any TODO comments found
    tractor "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --expect none

    # Show full XML AST for debugging
    tractor src/main.rs --debug

LOW-LEVEL TOOLS:
    tractor-parse     TreeSitter parser (files → XML AST) - kept as standalone utility
"#)]
pub struct Args {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    /// XPath 3.1 query expression
    #[arg(short = 'x', long = "xpath")]
    pub xpath: Option<String>,

    /// Language for stdin input (e.g., csharp, rust, python)
    #[arg(short = 'l', long = "lang")]
    pub lang: Option<String>,

    /// Show full XML with matches highlighted (for debugging XPath)
    #[arg(long = "debug")]
    pub debug: bool,

    /// Expected result: none, some, or a number (exit 1 if not met)
    #[arg(short = 'e', long = "expect")]
    pub expect: Option<String>,

    /// Output format: xml (default), lines, source, value, gcc, json, count
    #[arg(short = 'o', long = "output", default_value = "xml")]
    pub output: String,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message")]
    pub message: Option<String>,

    /// Limit output to first N matches
    #[arg(short = 'n', long = "limit")]
    pub limit: Option<usize>,

    /// Include start/end location attributes in XML output
    #[arg(long = "keep-locations")]
    pub keep_locations: bool,

    /// Color output: auto (default), always, never
    #[arg(long = "color", default_value = "auto")]
    pub color: String,

    /// Disable color output
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Number of parallel workers
    #[arg(short = 'c', long = "concurrency")]
    pub concurrency: Option<usize>,

    /// Output raw TreeSitter XML (not semantic)
    #[arg(long = "raw")]
    pub raw: bool,

    /// Show verbose output
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
}
