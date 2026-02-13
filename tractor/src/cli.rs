//! CLI argument parsing using clap

use clap::Parser;

/// Multi-language code query tool using XPath 3.1
#[derive(Parser, Debug)]
#[command(name = "tractor")]
#[command(author, about, long_about = None)]
#[command(disable_version_flag = true)]
#[command(before_help = "NOTE: Full help includes WORKFLOW tutorial and EXAMPLES. Do not truncate.")]
#[command(after_help = r#"WORKFLOW:
    1. Explore structure across files with schema view (depth 4 by default):
       tractor "src/**/*.cs" -o schema
       tractor "src/**/*.cs" -x "//class" -o schema

    2. View the full XML of specific code:
       tractor src/main.rs

    3. Add -x to select specific elements:
       tractor src/main.rs -x "//function"

    4. Refine with predicates:
       tractor src/main.rs -x "//function[name='main']"

    5. Choose output format with -o:
       tractor src/main.rs -x "//function/name" -o value

EXAMPLES:
    # See what element types exist across all C# files (default depth 4)
    tractor "src/**/*.cs" -o schema

    # See deeper structure with custom depth
    tractor "src/**/*.cs" -o schema -d 6

    # See structure of all classes
    tractor "src/**/*.cs" -x "//class" -o schema

    # Query all C# files for classes
    tractor "src/**/*.cs" -x "//class"

    # Find methods missing OrderBy in Repository classes
    tractor "src/**/*.cs" -x "//class[contains(name,'Repository')]//method[not(contains(.,'OrderBy'))]" -o gcc

    # Parse from stdin
    echo "public class Foo { }" | tractor -l csharp -x "//class/name" -o value

    # Parse from argument — escape-proof, works with multiline code
    tractor -s "$(cat <<'CODE'
    public class Foo {
        public void Bar() { }
    }
    CODE
    )" -l csharp -x "//class/name" -o value

    # CI: fail if any TODO comments found
    tractor "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --expect none

    # GitHub Actions: annotate errors in PR
    tractor "src/**/*.cs" -x "//comment[contains(.,'TODO')]" -o github -m "TODO comment found"

    # Whitespace-insensitive matching
    tractor file.cs -x "//type[.='Dictionary<string,int>']" -W
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

    /// Source code string to parse (alternative to stdin, requires --lang)
    #[arg(short = 's', long = "string")]
    pub content: Option<String>,

    /// Show full XML with matches highlighted (for debugging XPath)
    #[arg(long = "debug")]
    pub debug: bool,

    /// Expected result: none, some, or a number (enables test mode output)
    #[arg(short = 'e', long = "expect")]
    pub expect: Option<String>,

    /// Error message template for failed expectations (per-match, supports {file}, {line}, {name}, etc.)
    #[arg(long = "error")]
    pub error: Option<String>,

    /// Treat failed expectations as warnings (exit 0, show ⚠ instead of ✗)
    #[arg(long = "warning")]
    pub warning: bool,

    /// Output format: xml (default), lines, source, value, gcc, json, count, schema, github
    #[arg(short = 'o', long = "output", default_value = "xml")]
    pub output: String,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message")]
    pub message: Option<String>,

    /// Limit output to first N matches
    #[arg(short = 'n', long = "limit")]
    pub limit: Option<usize>,

    /// Limit XML output depth (useful for large ASTs)
    #[arg(short = 'd', long = "depth")]
    pub depth: Option<usize>,

    /// [EXPERIMENTAL] Limit tree building depth (skip parsing deeper nodes for speed)
    #[arg(long = "parse-depth")]
    pub parse_depth: Option<usize>,

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

    /// Disable pretty printing (shows XML without formatting, as used by XPath)
    #[arg(long = "no-pretty")]
    pub no_pretty: bool,

    /// Ignore whitespace in XPath string matching (strips whitespace from text nodes)
    #[arg(short = 'W', long = "ignore-whitespace")]
    pub ignore_whitespace: bool,

    /// Show verbose output
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Print version information (use with -v for grammar versions)
    #[arg(short = 'V', long = "version")]
    pub version: bool,
}
