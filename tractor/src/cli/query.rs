use clap::{Args, CommandFactory};
use tractor::NormalizedXpath;
use crate::cli::{Cli, SharedArgs};

/// Query/explore mode (default, no subcommand)
#[derive(Args, Debug)]
pub struct QueryArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    /// Path to a tractor config file (YAML/TOML) — runs only query operations from it
    #[arg(long = "config", help_heading = "Config")]
    pub config: Option<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Source code string to parse (alternative to stdin, requires --lang)
    #[arg(short = 's', long = "string", help_heading = None)]
    pub content: Option<String>,

    /// Report fields to include (e.g. tree, value, source) [default: file,line,tree]
    #[arg(short = 'v', long = "view", help_heading = "View", allow_hyphen_values = true)]
    pub view: Option<String>,

    /// Custom message template (supports {value}, {line}, {col}, {file})
    #[arg(short = 'm', long = "message", help_heading = "View")]
    pub message: Option<String>,

    /// Output format [default: text]
    #[arg(short = 'f', long = "format", default_value = "text", help_heading = "Format")]
    pub format: String,

    /// Show full XML with matches highlighted (for debugging XPath)
    #[arg(long = "debug", help_heading = "Advanced")]
    pub debug: bool,

    /// Print version information (use with --verbose for detailed output)
    #[arg(short = 'V', long = "version", help_heading = "Advanced")]
    pub version: bool,
}
use crate::executor::{self, ExecuteOptions, Operation, QueryOperation, QueryExpr};
use crate::cli::context::RunContext;
use crate::input::{InputMode, FileResolver, ResolverOptions, SourceRequest};
use crate::format::{ViewField, GroupDimension, render_report};
use crate::matcher::{prepare_report_for_output, run_debug};
use super::config::{run_from_config, ConfigRunParams};

pub fn run_query(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref config_path) = args.config {
        return run_from_config(ConfigRunParams {
            config_path,
            shared: &args.shared,
            cli_files: args.files.clone(),
            cli_content: args.content.clone(),
            format: &args.format,
            default_view: &[ViewField::File, ViewField::Line, ViewField::Tree],
            view_override: args.view.as_deref(),
            message: args.message.clone(),
            default_group: &[GroupDimension::File],
            op_filter: |kind| matches!(kind, crate::tractor_config::ConfigOperationKind::Query),
            filter_label: "query",
        });
    }

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.format, &[ViewField::File, ViewField::Line, ViewField::Tree], args.view.as_deref(), args.message, args.content, args.debug, &[],
    )?;

    if let InputMode::Files(ref files) = ctx.input {
        if files.is_empty() {
            Cli::command().print_help().ok();
            println!();
            return Ok(());
        }
    }

    // Debug mode — needs the full parsed document, stays on existing pipeline.
    if ctx.debug {
        if let (Some(ref xpath), InputMode::Files(ref files)) = (&ctx.xpath, &ctx.input) {
            return run_debug(&ctx, files, xpath);
        }
    }

    // Explore (no XPath) = query with implicit "/*" — selects the document root.
    let default_xpath = NormalizedXpath::new("/");
    let xpath_expr = ctx.xpath.as_ref().unwrap_or(&default_xpath);

    // Build the file resolver for this single-operation run.
    let resolver_opts = ResolverOptions {
        verbose: ctx.verbose,
        base_dir: None,
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        max_files: args.shared.max_files,
        cli_files: Vec::new(),
        config_root_files: None,
    };
    let resolver = FileResolver::new(&resolver_opts)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    let mut builder = tractor::ReportBuilder::new();
    builder.set_no_verdict();

    let (op_files, inline_source, op_language): (Vec<String>, Option<&crate::input::Source>, Option<String>) = match &ctx.input {
        InputMode::Files(files) => (files.clone(), None, ctx.lang.clone()),
        InputMode::Inline(source) => (Vec::new(), Some(source), Some(source.language.clone())),
    };

    let request = SourceRequest {
        files: &op_files,
        exclude: &[],
        diff_files: None,
        diff_lines: None,
        command: "query",
        language: op_language.as_deref(),
        inline_source,
    };
    let (sources, filters) = resolver.resolve(&request, &mut builder);

    if !builder.has_fatals() {
        let op = Operation::Query(QueryOperation {
            sources,
            filters,
            queries: vec![QueryExpr { xpath: xpath_expr.clone() }],
            tree_mode: ctx.tree_mode,
            language: op_language,
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
        });

        let options = ExecuteOptions {
            verbose: ctx.verbose,
            base_dir: None,
        };

        executor::execute(&[op], &options, &mut builder)?;
    }
    let mut report = builder.build();
    prepare_report_for_output(&mut report, &ctx);
    render_report(&report, &ctx, None)?;

    Ok(())
}
