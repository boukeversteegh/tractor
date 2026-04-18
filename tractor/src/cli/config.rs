//! Shared config file executor for all commands.
//!
//! Each command (`check`, `query`, `test`, `set`, `run`) can execute operations
//! from a tractor config file. The only differences are:
//!
//! - **Operation filter**: which operation types to run (e.g. check only runs
//!   `Check` operations, run executes everything).
//! - **Defaults**: view fields, grouping, format — tuned per command.

use tractor::report::{ReportMatch, Severity};

use crate::cli::SharedArgs;
use crate::executor::{self, ExecuteOptions, Operation};
use crate::cli::context::RunContext;
use crate::format::{ViewField, GroupDimension, render_report};
use crate::input::{resolve_input, InputMode, FileResolver, ResolverOptions, SourceRequest};
use crate::matcher::{project_report, apply_message_template};
use crate::tractor_config::{ConfigOperation, ConfigOperationKind};

/// Parameters that vary per command when executing a config file.
pub struct ConfigRunParams<'a> {
    pub config_path: &'a str,
    pub shared: &'a SharedArgs,
    pub cli_files: Vec<String>,
    /// CLI-provided inline content (from `-s/--string`). Together with any
    /// piped stdin, this becomes a virtual `Source` attached to every
    /// config-loaded operation that can accept one.
    pub cli_content: Option<String>,
    pub format: &'a str,
    pub default_view: &'a [ViewField],
    pub view_override: Option<&'a str>,
    pub message: Option<String>,
    pub default_group: &'a [GroupDimension],
    pub op_filter: fn(ConfigOperationKind) -> bool,
    pub filter_label: &'a str,
}

/// Load a config file, filter operations, and execute through the standard pipeline.
pub fn run_from_config(params: ConfigRunParams) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::path::Path::new(params.config_path);

    if !config_path.exists() {
        return Err(format!("config file not found: {}", params.config_path).into());
    }

    let loaded = crate::tractor_config::load_tractor_config(config_path)?;

    // Resolve CLI input once. Inline mode consumes the positional `files`
    // arg as the Source's virtual path, so it must NOT leak into
    // `ResolverOptions.cli_files` (which would ask FileResolver to
    // intersect the operation against a file the user never wanted to
    // read from disk). Disk mode keeps the cli_files flowing as today.
    let (cli_inline_source, cli_files_for_resolver) = match resolve_input(
        params.shared,
        params.cli_files.clone(),
        params.cli_content,
    )? {
        InputMode::Inline(source) => (Some(source), Vec::new()),
        InputMode::Files(files) => (None, files),
    };

    let mut config_ops: Vec<ConfigOperation> = loaded.operations.into_iter()
        .filter(|op| (params.op_filter)(op.kind()))
        .collect();

    // Attach the CLI inline source to every config-loaded operation of a
    // kind that understands inline input. This is what unlocks
    //   `cat proposed.cs | tractor check --config tractor.yml src/Foo.cs`
    // by making the piped content participate in the config's rules exactly
    // like a disk file at the virtual path. Per-op already-set sources win.
    if let Some(ref inline) = cli_inline_source {
        for op in &mut config_ops {
            let inputs = op.inputs_mut();
            if inputs.inline_source.is_none() {
                inputs.inline_source = Some(inline.clone());
            }
        }
    }

    let ctx = RunContext::build(
        params.shared, vec![], None, params.format,
        params.default_view,
        params.view_override, params.message, None, false, params.default_group,
    )?;

    let mut builder = tractor::ReportBuilder::new();

    if config_ops.is_empty() {
        builder.add(ReportMatch {
            file: params.config_path.to_string(),
            line: 0, column: 0, end_line: 0, end_column: 0,
            command: String::new(),
            tree: None, value: None, source: None, lines: None,
            reason: Some(format!("no {} operations found", params.filter_label)),
            severity: Some(Severity::Fatal),
            message: None, origin: None, rule_id: None, status: None, output: None,
        });
    } else {
        let base_dir = config_path.parent()
            .map(|p| if p.as_os_str().is_empty() { std::path::Path::new(".") } else { p })
            .map(|p| {
                // Absolutize without following symlinks — matches the glob
                // walker and CLI path resolution, so `base_dir`-derived paths
                // intersect by set equality with those pipelines.
                let normalized = tractor::NormalizedPath::absolute(&p.to_string_lossy());
                std::path::PathBuf::from(normalized.as_str())
            });

        // Build the resolver ONCE for the whole config run — shared state
        // (root files, CLI files, global diff) is expanded here.
        let resolver_opts = ResolverOptions {
            verbose: ctx.verbose,
            base_dir: base_dir.clone(),
            diff_files: params.shared.diff_files.clone(),
            diff_lines: params.shared.diff_lines.clone(),
            max_files: params.shared.max_files,
            cli_files: cli_files_for_resolver,
            config_root_files: loaded.root_files,
        };
        let resolver = match FileResolver::new(&resolver_opts) {
            Ok(r) => r,
            Err(e) => {
                builder.add(crate::input::make_fatal_diagnostic(params.filter_label, e));
                let mut report = builder.build();
                project_report(&mut report, &ctx.view);
                let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
                let report = report.with_grouping(&dims);
                return render_report(&report, &ctx, None);
            }
        };

        // For each config operation, resolve its inputs into the unified
        // `sources + filters` pair, then inject into the operation skeleton.
        let mut operations: Vec<Operation> = Vec::with_capacity(config_ops.len());
        for config_op in config_ops {
            let inputs = config_op.inputs().clone();
            // Build the SourceRequest from per-op inputs.
            let request = SourceRequest {
                files: &inputs.files,
                exclude: &inputs.exclude,
                diff_files: inputs.diff_files.as_deref(),
                diff_lines: inputs.diff_lines.as_deref(),
                command: params.filter_label,
                language: inputs.language.as_deref(),
                inline_source: inputs.inline_source.as_ref(),
            };
            let (sources, filters) = resolver.resolve(&request, &mut builder);
            operations.push(config_op.into_operation(sources, filters));
        }

        let options = ExecuteOptions {
            verbose: ctx.verbose,
            base_dir,
        };

        executor::execute(&operations, &options, &mut builder)?;
    }

    let mut report = builder.build();

    if let Some(ref template) = ctx.message {
        apply_message_template(&mut report, template);
    }

    project_report(&mut report, &ctx.view);
    let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
    let report = report.with_grouping(&dims);
    render_report(&report, &ctx, None)
}

