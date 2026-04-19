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
    /// View field selection (-v), after projection normalization.
    pub view: ViewSet,
    pub use_color: bool,
    /// Interpolated message template from `-m`, if provided (None when suppressed by -p).
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
    /// Projection (-p): which part of the report to emit.
    pub projection: Projection,
    /// Whether to emit first result only, without list wrappers (--single).
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
                    Some(HookType::PostToolUse)
                } else {
                    None
                }
            }
        };

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
            None => None,
        };

        let concurrency = shared.concurrency.unwrap_or_else(|| num_cpus::get());
        rayon::ThreadPoolBuilder::new()
            .num_threads(concurrency)
            .build_global()
            .ok();

        // ---------------------------------------------------------------------------
        // Projection + normalization
        // ---------------------------------------------------------------------------

        let single = shared.single;

        // --single + -n contradiction check
        if single {
            if let Some(limit) = shared.limit {
                if limit != 1 {
                    return Err(format!(
                        "--single and -n {} are contradictory: --single means first result only (implicit -n 1). \
                         Use -n 1 to match, or drop --single.",
                        limit
                    ).into());
                }
            }
        }

        // Parse projection from -p flag, applying --single default
        let raw_projection = match shared.project.as_deref() {
            Some(s) => Some(Projection::from_str(s)?),
            None    => None,
        };

        let projection = match (raw_projection, single) {
            (Some(p), _) => p,
            // --single without -p implies -p results
            (None, true) => Projection::Results,
            (None, false) => Projection::Report,
        };

        // --single warning for non-sequence projections
        if single && !projection.is_sequence() {
            eprintln!(
                "warning: --single has no effect with -p {} (already singular). \
                 Drop --single to silence this warning.",
                projection.name()
            );
        }

        // Track whether the user explicitly set -v or -m
        let user_explicit_view    = user_view.is_some();
        let user_explicit_message = message.is_some();

        // Parse the user view (before replacement)
        let parsed_view = if let Some(s) = user_view {
            parse_view_set(s, default_view)?
        } else {
            ViewSet::from_fields(default_view.to_vec())
        };

        // Apply -p replacement rule and emit warnings
        let (view, message) = normalize_projection(
            projection,
            parsed_view,
            message,
            user_explicit_view,
            user_explicit_message,
        );

        Ok(RunContext {
            xpath,
            output_format,
            view,
            use_color,
            message,
            input,
            limit: shared.limit,
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

/// Apply the -p replacement rule to the view set and emit warnings as needed.
///
/// Returns the (possibly replaced) view and (possibly suppressed) message.
fn normalize_projection(
    projection: Projection,
    parsed_view: ViewSet,
    message: Option<String>,
    user_explicit_view: bool,
    user_explicit_message: bool,
) -> (ViewSet, Option<String>) {
    match projection {
        // View-level projections: replace -v with the single projection field
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            let proj_field = projection.as_per_match_view_field().unwrap();

            // Warn about dropped explicit -v fields
            if user_explicit_view {
                let dropped: Vec<&str> = parsed_view.fields.iter()
                    .filter(|&&f| f != proj_field)
                    .map(|f| f.name())
                    .collect();
                if !dropped.is_empty() {
                    eprintln!(
                        "warning: -v fields {{{}}} were discarded because -p {} replaces the view set.\n  \
                         To keep -v intact, use `-p results` (respects -v) instead of `-p {}`.",
                        dropped.join(", "),
                        projection.name(),
                        projection.name(),
                    );
                }
            }
            // Warn about dropped message
            if user_explicit_message {
                eprintln!(
                    "warning: -m message template has no effect with -p {} (view-level projection replaces per-match fields).\n  \
                     To include the message field, use `-p results` instead.",
                    projection.name(),
                );
            }
            (ViewSet::single(proj_field), None)
        }

        // Schema and Count: replace view entirely (no per-match fields)
        Projection::Schema | Projection::Count => {
            if user_explicit_view {
                let names: Vec<&str> = parsed_view.fields.iter().map(|f| f.name()).collect();
                eprintln!(
                    "warning: -v fields {{{}}} were discarded because -p {} replaces the view set.\n  \
                     To keep -v intact, use `-p results` (respects -v) instead of `-p {}`.",
                    names.join(", "),
                    projection.name(),
                    projection.name(),
                );
            }
            if user_explicit_message {
                eprintln!(
                    "warning: -m message template has no effect with -p {} (no per-match rendering).",
                    projection.name(),
                );
            }
            (ViewSet::from_fields(vec![]), None)
        }

        // Metadata projections (summary, totals): -v is irrelevant
        Projection::Summary | Projection::Totals => {
            if user_explicit_view {
                let names: Vec<&str> = parsed_view.fields.iter().map(|f| f.name()).collect();
                if !names.is_empty() {
                    eprintln!(
                        "warning: -v fields {{{}}} have no effect with -p {} (no per-match rendering).",
                        names.join(", "),
                        projection.name(),
                    );
                }
            }
            if user_explicit_message {
                eprintln!(
                    "warning: -m message template has no effect with -p {} (no per-match rendering).",
                    projection.name(),
                );
            }
            (parsed_view, message)
        }

        // Structural projections (results, report): respect -v and -m in full
        Projection::Results | Projection::Report => (parsed_view, message),
    }
}
