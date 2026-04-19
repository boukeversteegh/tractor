use tractor::{
    output::should_use_color,
    output::RenderOptions,
    NormalizedXpath,
    TreeMode,
};
use crate::cli::SharedArgs;
use crate::input::{InputMode, resolve_input};
use crate::format::{OutputFormat, GroupDimension, ViewField, ViewSet, Projection, parse_view_set, parse_group_by};
use crate::format::options::HookType;

pub struct RunContext {
    pub xpath: Option<NormalizedXpath>,
    /// Output format (-f).
    pub output_format: OutputFormat,
    /// View field selection (-v), after early normalization from -p.
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
    /// Report projection (-p flag). Defaults to Projection::Report.
    pub projection: Projection,
    /// Emit first projected element bare, no list wrapper (--single flag).
    pub single: bool,
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

        // Parse -p / --project flag.
        let projection = match shared.project.as_deref() {
            Some(s) => Projection::from_str(s).map_err(|e| e)?,
            None => Projection::Report,
        };

        // --single: validate that it's not combined with a contradicting -n.
        let single = shared.single;
        if single {
            if let Some(n) = shared.limit {
                if n != 1 {
                    return Err(format!(
                        "--single contradicts -n {n}: --single means first match only (-n 1). \
                         Use -n 1 --single, or omit --single to keep -n {n}."
                    ).into());
                }
            }
        }

        // When --single is set and -p is omitted, default to -p results instead of -p report.
        let projection = if single && shared.project.is_none() {
            Projection::Results
        } else {
            projection
        };

        // Parse user view, then apply early normalization from -p.
        let view_explicit = user_view.is_some();
        let mut view = if let Some(s) = user_view {
            parse_view_set(s, default_view)?
        } else {
            ViewSet::from_fields(default_view.to_vec())
        };

        // Early normalization: -p view-level field replaces -v.
        // Also warn when explicit -v or -m fields are discarded.
        if let Some(replacement) = projection.view_field_replacement() {
            if view_explicit {
                // Check which explicitly-requested fields are being dropped.
                let dropped: Vec<ViewField> = view.fields.iter()
                    .filter(|&&f| f != replacement)
                    .copied()
                    .collect();
                if !dropped.is_empty() {
                    let names: Vec<&str> = dropped.iter().map(|f| f.name()).collect();
                    eprintln!(
                        "warning: -v fields {{{}}} were discarded because -p {} replaces the view set.\n  \
                         To keep -v intact, use `-p results` (respects -v) instead of `-p {}`.",
                        names.join(", "), projection.name(), projection.name()
                    );
                }
            }
            if view_explicit && message.is_some() {
                let has_message_in_dropped = !view.fields.contains(&replacement);
                if has_message_in_dropped || view.fields.iter().all(|&f| f == replacement) {
                    eprintln!(
                        "warning: -m message template was discarded because -p {} replaces the view set.\n  \
                         To keep -m intact, use `-p results` (respects -v) instead of `-p {}`.",
                        projection.name(), projection.name()
                    );
                }
            }
            view = ViewSet::single(replacement);
        } else if projection.is_metadata_only() && view_explicit {
            // Metadata-only projections (summary, totals) have no per-match rendering.
            let names: Vec<&str> = view.fields.iter().map(|f| f.name()).collect();
            if !names.is_empty() {
                eprintln!(
                    "warning: -v fields {{{}}} have no effect with -p {} (no per-match rendering).",
                    names.join(", "), projection.name()
                );
            }
            if message.is_some() {
                eprintln!(
                    "warning: -m message template has no effect with -p {} (no per-match rendering).",
                    projection.name()
                );
            }
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

        // --single with a sequence projection implies -n 1.
        let limit = if single && projection.is_per_match() || (single && projection == Projection::Results) {
            Some(1)
        } else {
            shared.limit
        };

        let concurrency = shared.concurrency.unwrap_or_else(|| num_cpus::get());
        rayon::ThreadPoolBuilder::new()
            .num_threads(concurrency)
            .build_global()
            .ok();

        Ok(RunContext {
            xpath,
            output_format,
            view,
            use_color,
            message,
            input,
            limit,
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
            projection,
            single,
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
