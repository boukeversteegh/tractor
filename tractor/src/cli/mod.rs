//! CLI argument parsing and command execution.

pub mod help;
pub mod context;
pub mod query;
pub mod check;
pub mod test;
pub mod set;
pub mod update;
pub mod render;
pub mod run;
pub mod init;
pub mod config;
pub mod languages;

use clap::{Parser, Subcommand, Args};
use tractor::NormalizedXpath;

pub use query::QueryArgs;
pub use check::CheckArgs;
pub use test::TestArgs;
pub use set::SetArgs;
pub use update::UpdateArgs;
pub use render::RenderArgs;
pub use run::RunArgs;
pub use init::InitArgs;

/// Multi-language code query tool using XPath 3.1
#[derive(Parser, Debug)]
#[command(name = "tractor")]
#[command(author, about, long_about = None)]
#[command(disable_version_flag = true)]
#[command(args_conflicts_with_subcommands = true)]
#[command(before_help = "NOTE: Full help includes WORKFLOW tutorial and EXAMPLES. Do not truncate.")]
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
    /// Execute a tractor config file (batch check/set operations)
    Run(RunArgs),
    /// Create a starter tractor.yml in the current directory
    Init(InitArgs),
    /// Show documentation and reference information
    #[command(subcommand)]
    Docs(DocsCommand),
}

/// Docs subcommands for reference information
#[derive(Subcommand, Debug)]
pub enum DocsCommand {
    /// List all supported languages with their canonical names, extensions, and aliases
    Languages,
}

/// Shared arguments available in all modes
#[derive(Args, Debug, Clone)]
pub struct SharedArgs {
    // -- Input --
    /// Language for stdin or -s/--string input (e.g., csharp, rust, python)
    #[arg(short = 'l', long = "lang", help_heading = None)]
    pub lang: Option<String>,

    // -- Extract --
    /// XPath expression to extract matching AST nodes
    #[arg(short = 'x', long = "extract", value_name = "QUERY", help_heading = "Extract")]
    pub xpath: Option<NormalizedXpath>,

    /// Tree mode: raw, structure, data [default: auto]
    #[arg(short = 't', long = "tree", help_heading = "Extract", allow_hyphen_values = true,
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

    /// Project a specific element from the report (tree, value, source, lines, schema, count, summary, totals, results, report)
    #[arg(short = 'p', long = "project", value_name = "ELEMENT", help_heading = "View")]
    pub project: Option<String>,

    /// Emit the first matching element bare (no list wrapper). Implies -n 1.
    #[arg(long = "single", help_heading = "View")]
    pub single: bool,

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

    /// Claude Code hook type (used with -f claude-code): post-tool-use, pre-tool-use, stop, context
    #[arg(long = "hook", help_heading = "Format")]
    pub hook: Option<String>,

    // -- Group --
    /// Group output by dimension: none, file, command, rule (comma-separated)
    #[arg(short = 'g', long = "group", help_heading = "View")]
    pub group_by: Option<String>,

    // -- Filter --
    /// Only consider files changed in a git diff (e.g. "HEAD~3", "main..HEAD")
    #[arg(long = "diff-files", value_name = "RANGE", help_heading = "Filter", allow_hyphen_values = true)]
    pub diff_files: Option<String>,

    /// Only consider matches in changed hunks of a git diff (e.g. "HEAD~3", "main..HEAD")
    #[arg(long = "diff-lines", value_name = "RANGE", help_heading = "Filter", allow_hyphen_values = true)]
    pub diff_lines: Option<String>,

    // -- Advanced --
    /// [EXPERIMENTAL] Limit tree building depth (skip parsing deeper nodes for speed)
    #[arg(long = "parse-depth", help_heading = "Advanced")]
    pub parse_depth: Option<usize>,

    /// Number of parallel workers
    #[arg(short = 'c', long = "concurrency", help_heading = "Advanced")]
    pub concurrency: Option<usize>,

    /// Maximum number of files to process (default: 10000)
    #[arg(long = "max-files", default_value = "10000", help_heading = "Advanced")]
    pub max_files: usize,

    /// Show verbose output
    #[arg(long = "verbose", help_heading = "Advanced")]
    pub verbose: bool,
}
