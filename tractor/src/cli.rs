//! CLI argument parsing using clap

use clap::{Parser, Subcommand, Args};

/// Multi-language code query tool using XPath 3.1
#[derive(Parser, Debug)]
#[command(name = "tractor")]
#[command(author, about, long_about = None)]
#[command(disable_version_flag = true)]
#[command(args_conflicts_with_subcommands = true)]
#[command(before_help = "NOTE: Full help includes WORKFLOW tutorial and EXAMPLES. Do not truncate.")]
#[command(after_help = r#"WORKFLOW:
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
"#)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub query: QueryArgs,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Query source code ASTs with XPath (default when no subcommand given)
    Query(QueryArgs),
    /// Run checks and report violations (lint mode)
    Check(CheckArgs),
    /// Test expectations against match counts
    Test(TestArgs),
    /// Set matched node values (modify files in-place, create missing nodes)
    Set(SetArgs),
    /// Update matched node values (modify files in-place, skip if not found)
    Update(UpdateArgs),
    /// [EXPERIMENTAL] Render XML AST back to source code
    Render(RenderArgs),
}

/// Shared arguments available in all modes
#[derive(Args, Debug, Clone)]
pub struct SharedArgs {
    // -- Input --
    /// Language for stdin input (e.g., csharp, rust, python)
    #[arg(short = 'l', long = "lang", help_heading = None)]
    pub lang: Option<String>,

    // -- Extract --
    /// XPath expression to extract matching AST nodes
    #[arg(short = 'x', long = "extract", help_heading = "Extract")]
    pub xpath: Option<String>,

    /// Tree mode: raw, structure, data [default: auto]
    #[arg(short = 't', long = "tree", help_heading = "Extract",
        long_help = "\
Tree mode [default: auto]
  raw        Raw tree-sitter AST (no semantic transforms)
  structure  Semantic syntax tree (default for code languages)
  data       Data projection (default for JSON/YAML)

When omitted, auto-selects: data for JSON/YAML, structure for everything else.")]
    pub tree: Option<String>,

    /// Ignore whitespace in XPath string matching (strips whitespace from text nodes)
    #[arg(short = 'W', long = "ignore-whitespace", help_heading = "Extract")]
    pub ignore_whitespace: bool,

    // -- View --
    /// Limit output to first N matches
    #[arg(short = 'n', long = "limit", help_heading = "View")]
    pub limit: Option<usize>,

    /// Limit XML output depth (useful for large ASTs)
    #[arg(short = 'd', long = "depth", help_heading = "View")]
    pub depth: Option<usize>,

    /// Include metadata attributes (start/end, kind, field) in XML output
    #[arg(long = "meta", help_heading = "View")]
    pub meta: bool,

    // -- Format --
    /// Disable pretty printing (shows XML without formatting, as used by XPath)
    #[arg(long = "no-pretty", help_heading = "Format")]
    pub no_pretty: bool,

    /// Color output: auto (default), always, never
    #[arg(long = "color", default_value = "auto", help_heading = "Format")]
    pub color: String,

    /// Disable color output
    #[arg(long = "no-color", help_heading = "Format")]
    pub no_color: bool,

    // -- Group --
    /// Group output by file
    #[arg(short = 'g', long = "group", help_heading = "View")]
    pub group_by: Option<String>,

    // -- Advanced --
    /// [EXPERIMENTAL] Limit tree building depth (skip parsing deeper nodes for speed)
    #[arg(long = "parse-depth", help_heading = "Advanced")]
    pub parse_depth: Option<usize>,

    /// Number of parallel workers
    #[arg(short = 'c', long = "concurrency", help_heading = "Advanced")]
    pub concurrency: Option<usize>,

    /// Show verbose output
    #[arg(long = "verbose", help_heading = "Advanced")]
    pub verbose: bool,
}

/// Query/explore mode (default, no subcommand)
#[derive(Args, Debug)]
pub struct QueryArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Source code string to parse (alternative to stdin, requires --lang)
    #[arg(short = 's', long = "string", help_heading = None)]
    pub content: Option<String>,

    /// Report view [default: tree]
    #[arg(short = 'v', long = "view", help_heading = "View",
        long_help = "\
Report view [default: tree]
  tree      Parsed source tree (XML or JSON, depending on -f)
  value     Text content of matched nodes
  source    Exact matched source text
  lines     Full source lines containing each match
  count     Total match count
  query     Echo the XPath query as tractor received it (useful to detect shell/wrapper mangling)
  schema    Structural overview of element types")]
    pub view: Option<String>,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message", help_heading = "View")]
    pub message: Option<String>,

    /// Output format: text (default), json, yaml, xml, gcc, github
    #[arg(short = 'f', long = "format", default_value = "text", help_heading = "Format",
        long_help = "\
Output format [default: text]
  text      Human-readable plain text
  json      JSON report envelope
  yaml      YAML report envelope
  xml       XML report envelope
  gcc       file:line:col: severity: reason (for CI/editors)
  github    GitHub Actions annotation (::error file=...)")]
    pub format: String,

    /// Show full XML with matches highlighted (for debugging XPath)
    #[arg(long = "debug", help_heading = "Advanced")]
    pub debug: bool,

    /// Print version information (use with --verbose for detailed output)
    #[arg(short = 'V', long = "version", help_heading = "Advanced")]
    pub version: bool,
}

/// Check mode: lint/report violations
#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Report view [default: tree]
    #[arg(short = 'v', long = "view", help_heading = "View",
        long_help = "\
Report view [default: tree]
  tree      Parsed source tree
  value     Text content of matched nodes
  source    Exact matched source text
  lines     Full source lines containing each match
  count     Total match count
  query     Echo the XPath query as tractor received it (useful to detect shell/wrapper mangling)
  schema    Structural overview of element types")]
    pub view: Option<String>,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message", help_heading = "View")]
    pub message: Option<String>,

    /// Output format [default: gcc]
    #[arg(short = 'f', long = "format", default_value = "gcc", help_heading = "Format",
        long_help = "\
Output format [default: gcc]
  gcc       file:line:col: severity: reason (default for check)
  github    GitHub Actions annotation (::error file=...)
  text      Human-readable plain text
  json      JSON report envelope
  yaml      YAML report envelope
  xml       XML report envelope")]
    pub format: String,

    /// Reason message for each violation
    #[arg(long = "reason", help_heading = "Check")]
    pub reason: Option<String>,

    /// Severity level: error (default) or warning
    #[arg(long = "severity", default_value = "error", help_heading = "Check")]
    pub severity: String,
}

/// Test mode: assert match count expectations
#[derive(Args, Debug)]
pub struct TestArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Source code string to parse (alternative to stdin, requires --lang)
    #[arg(short = 's', long = "string", help_heading = None)]
    pub content: Option<String>,

    /// Report view [default: tree]
    #[arg(short = 'v', long = "view", help_heading = "View",
        long_help = "\
Report view [default: tree]
  tree      Parsed source tree (XML or JSON, depending on -f)
  value     Text content of matched nodes
  source    Exact matched source text
  lines     Full source lines containing each match
  count     Total match count
  query     Echo the XPath query as tractor received it (useful to detect shell/wrapper mangling)
  schema    Structural overview of element types")]
    pub view: Option<String>,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message", help_heading = "View")]
    pub message: Option<String>,

    /// Output format: text (default), json, yaml, gcc, github
    #[arg(short = 'f', long = "format", default_value = "text", help_heading = "Format")]
    pub format: String,

    /// Expected result: none, some, or a number
    #[arg(short = 'e', long = "expect", help_heading = "Test")]
    pub expect: String,

    /// Error message template for failed expectations (per-match, supports {file}, {line}, {name}, etc.)
    #[arg(long = "error", help_heading = "Test")]
    pub error: Option<String>,

}

/// Set mode: modify matched node values in-place
#[derive(Args, Debug)]
pub struct SetArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Value to set matched nodes to
    #[arg(long = "value", help_heading = "Set")]
    pub value: String,
}

/// Update mode: modify only existing matched node values (no creation)
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Value to set matched nodes to
    #[arg(long = "value", help_heading = "Update")]
    pub value: String,
}

/// Render mode: convert XML AST back to source code
#[derive(Args, Debug)]
pub struct RenderArgs {
    /// Target file (determines language from extension). If omitted, output goes to stdout.
    #[arg()]
    pub file: Option<String>,

    /// Language (required when no file is given, e.g., csharp, rust)
    #[arg(short = 'l', long = "lang")]
    pub lang: Option<String>,

    /// XML input string (alternative to stdin)
    #[arg(short = 's', long = "string")]
    pub input: Option<String>,
}
