use clap::Args;
use crate::cli::SharedArgs;

/// Update mode: modify only existing matched node values (no creation)
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Files to process (supports glob patterns like "src/**/*.cs")
    #[arg()]
    pub files: Vec<String>,

    /// Value to set matched nodes to
    #[arg(long = "value", help_heading = "Update")]
    pub value: String,

    #[command(flatten)]
    pub shared: SharedArgs,
}
use crate::executor::{self, Operation, UpdateOperation};
use crate::cli::context::RunContext;
use crate::input::{InputMode, FileResolver, ResolverOptions, SourceRequest};
use crate::format::ViewField;

pub fn run_update(args: UpdateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        "text", &[ViewField::Tree], None, None, None, false, &[],
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("update requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::Inline(_) => {
            return Err("update cannot be used with stdin input (no file to modify)".into());
        }
    };

    // Build the file resolver for this single-op run.
    let resolver_opts = ResolverOptions {
        diff_files: args.shared.diff_files.clone(),
        diff_lines: args.shared.diff_lines.clone(),
        max_files: args.shared.max_files,
        cli_files: Vec::new(),
        config_root_files: None,
    };
    let env = ctx.exec_ctx();
    let resolver = FileResolver::new(&resolver_opts, &env)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    let mut builder = tractor::ReportBuilder::new();

    let request = SourceRequest {
        files,
        exclude: &[],
        diff_files: None,
        diff_lines: None,
        command: "update",
        language: ctx.lang.as_deref(),
        inline_source: None,
    };
    let (sources, filters) = resolver.resolve(&request, &mut builder);

    if !builder.has_fatals() {
        let op = Operation::Update(UpdateOperation {
            sources,
            filters,
            xpath: xpath_expr.to_string(),
            value: args.value.clone(),
            tree_mode: ctx.tree_mode,
            language: ctx.lang.clone(),
            limit: ctx.limit,
            ignore_whitespace: ctx.ignore_whitespace,
            parse_depth: ctx.parse_depth,
        });

        executor::execute(&[op], &env, &mut builder)?;
    }
    let report = builder.build();
    if report.success == Some(false) {
        return Err("update matched no nodes".into());
    }

    let totals = report.totals.as_ref().unwrap();
    eprintln!(
        "Updated {} match{} in {} file{}",
        totals.updated,
        if totals.updated == 1 { "" } else { "es" },
        totals.files,
        if totals.files == 1 { "" } else { "s" },
    );
    Ok(())
}
