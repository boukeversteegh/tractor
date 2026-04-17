use tractor::{
    output::should_use_color,
    output::RenderOptions,
    NormalizedXpath,
    TreeMode,
};
use crate::cli::SharedArgs;
use crate::input::{InputMode, resolve_input};
use crate::format::{
    normalize_output_plan, parse_group_by, parse_view_selection, GroupDimension, OutputFormat,
    Projection, ViewField, ViewSet,
};
use crate::format::options::HookType;

pub struct RunContext {
    pub xpath: Option<NormalizedXpath>,
    /// Output format (-f).
    pub output_format: OutputFormat,
    /// Projection target (-p).
    pub projection: Projection,
    /// Whether to emit the projection as a single bare item.
    pub single: bool,
    /// View field selection (-v).
    pub view: ViewSet,
    pub use_color: bool,
    /// Interpolated message template from `-m`, if provided.
    pub message: Option<String>,
    pub input: InputMode,
    pub limit: Option<usize>,
    pub depth: Option<usize>,
    pub parse_depth: Option<usize>,
    pub meta: bool,
    /// Tree mode from `-t`. None means auto-detect at parse time.
    pub tree_mode: Option<TreeMode>,
    pub no_pretty: bool,
    pub ignore_whitespace: bool,
    pub verbose: bool,
    pub lang: Option<String>,
    pub debug: bool,
    pub group_by: Vec<GroupDimension>,
    /// Claude Code hook type (--hook), used with `-f claude-code`.
    pub hook_type: Option<HookType>,
}

impl RunContext {
    pub fn build(
        shared: &SharedArgs,
        files: Vec<String>,
        xpath: Option<NormalizedXpath>,
        format: &str,
        default_view: &[ViewField],
        user_view: Option<&str>,
        message: Option<String>,
        content: Option<String>,
        debug: bool,
        default_group_by: &[GroupDimension],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let output_format = OutputFormat::from_str(format)?;

        let hook_type = match shared.hook.as_deref() {
            Some(s) => {
                if output_format != OutputFormat::ClaudeCode {
                    return Err("--hook requires -f claude-code".into());
                }
                Some(HookType::from_str(s)?)
            }
            None => {
                if output_format == OutputFormat::ClaudeCode {
                    // Default to post-tool-use when -f claude-code is used without --hook
                    Some(HookType::PostToolUse)
                } else {
                    None
                }
            }
        };

        let parsed_view = parse_view_selection(user_view, default_view)?;
        let plan = normalize_output_plan(
            shared.project.as_deref(),
            shared.single,
            shared.limit,
            parsed_view,
            message,
        )?;
        for warning in &plan.warnings {
            eprintln!("{warning}");
        }
        let use_color     = if shared.no_color { false } else { should_use_color(&shared.color) };
        let input         = resolve_input(shared, files, content)?;

        let group_by = match shared.group_by.as_deref() {
            Some(s) => parse_group_by(s)?,
            None => default_group_by.to_vec(),
        };

        let tree_mode = match shared.tree.as_deref() {
            Some("raw") => Some(TreeMode::Raw),
            Some("structure") => Some(TreeMode::Structure),
            Some("data") => Some(TreeMode::Data),
            Some(other) => return Err(format!(
                "invalid --tree value '{}': use 'raw', 'structure', or 'data'", other
            ).into()),
            None => None, // auto-detect at parse time
        };

        let concurrency = shared.concurrency.unwrap_or_else(|| num_cpus::get());
        rayon::ThreadPoolBuilder::new()
            .num_threads(concurrency)
            .build_global()
            .ok();

        Ok(RunContext {
            xpath,
            output_format,
            projection: plan.projection,
            single: plan.single,
            view: plan.view,
            use_color,
            message: plan.message,
            input,
            limit: plan.limit,
            depth: shared.depth,
            parse_depth: shared.parse_depth,
            meta: shared.meta,
            tree_mode,
            no_pretty: shared.no_pretty,
            ignore_whitespace: shared.ignore_whitespace,
            verbose: shared.verbose,
            lang: shared.lang.clone(),
            debug,
            group_by,
            hook_type,
        })
    }

    pub fn render_options(&self) -> RenderOptions {
        RenderOptions::new()
            .with_color(self.use_color)
            .with_meta(self.meta || self.debug)
            .with_max_depth(self.depth)
            .with_pretty_print(!self.no_pretty)
            .with_language(self.lang.clone())
    }

    pub fn schema_depth(&self) -> Option<usize> {
        self.depth.or(Some(4))
    }
}
