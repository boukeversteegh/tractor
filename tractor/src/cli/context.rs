use tractor::{
    output::should_use_color,
    output::RenderOptions,
    NormalizedXpath,
    TreeMode,
};
use crate::cli::SharedArgs;
use crate::input::{InputMode, resolve_input};
use crate::format::{OutputFormat, GroupDimension, Projection, ViewField, ViewSet, parse_view_set, parse_group_by};
use crate::format::options::HookType;

pub struct RunContext {
    pub xpath: Option<NormalizedXpath>,
    /// Output format (-f).
    pub output_format: OutputFormat,
    /// View field selection (-v), after `-p` normalization.
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
    /// Report projection (-p). None = default = `Projection::Report`.
    pub projection: Option<Projection>,
    /// `--single` modifier. When set with a sequence projection, emits first element bare.
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

        let user_view_explicit = user_view.is_some();
        let initial_view = if let Some(s) = user_view {
            parse_view_set(s, default_view)?
        } else {
            ViewSet::from_fields(default_view.to_vec())
        };
        let use_color     = if shared.no_color { false } else { should_use_color(&shared.color) };
        let input         = resolve_input(shared, files, content)?;

        // -- Projection normalization (-p/--project, --single) --
        // Order: parse `-p`, then apply `--single` defaulting, then validate against `-n`.
        let mut projection = match shared.project.as_deref() {
            Some(s) => Some(Projection::from_str(s)?),
            None => None,
        };

        // `--single` with `-p` omitted defaults to `-p results` (per design).
        if shared.single && projection.is_none() {
            projection = Some(Projection::Results);
        }

        // `--single -n N` (for any N != 1) is a contradiction.
        if shared.single {
            if let Some(n) = shared.limit {
                if n != 1 {
                    return Err(format!(
                        "--single and -n {} conflict: --single implies -n 1 (first match only). \
                         Drop either --single or -n.", n,
                    ).into());
                }
            }
        }

        // View replacement rule: when `-p` is view-level, replace `-v` with
        // `[that field]`. Warn on stderr for any explicitly-requested fields
        // that get discarded.
        let view = normalize_view_for_projection(
            initial_view,
            projection,
            user_view_explicit,
            message.is_some(),
        );

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

        // `--single` implies `-n 1` when no limit set.
        let limit = if shared.single { Some(1) } else { shared.limit };

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
            single: shared.single,
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

/// Apply the `-p` view replacement rule and emit stderr warnings for
/// explicitly-requested view fields that won't appear in the output.
///
/// - View-level `-p X` (tree/value/source/lines/schema/count): replaces `-v`
///   with `[X]` and warns about any other explicitly-set `-v`/`-m` fields.
/// - Metadata `-p` (summary/totals): leaves `-v` intact (it's irrelevant
///   anyway) but warns if `-v`/`-m` was set, since those fields are unreachable.
/// - Structural or absent `-p`: no change.
///
/// `user_view_explicit` and `has_message` track whether the user typed `-v`
/// or `-m` — we only warn when something the user explicitly asked for gets
/// dropped.
fn normalize_view_for_projection(
    initial_view: ViewSet,
    projection: Option<Projection>,
    user_view_explicit: bool,
    has_message: bool,
) -> ViewSet {
    let Some(proj) = projection else {
        return initial_view;
    };

    if let Some(proj_field) = proj.as_view_field() {
        // View-level projection: replace the view set with exactly `[proj_field]`.
        if user_view_explicit {
            let discarded: Vec<&str> = initial_view.fields.iter()
                .filter(|f| **f != proj_field)
                .map(|f| f.name())
                .collect();
            if !discarded.is_empty() {
                eprintln!(
                    "warning: -v fields {{{}}} were discarded because -p {} replaces the view set.",
                    discarded.join(", "), proj.name(),
                );
                eprintln!(
                    "  To keep -v intact, use `-p results` (respects -v) instead of `-p {}`.",
                    proj.name(),
                );
            }
        }
        if has_message {
            eprintln!(
                "warning: -m message template has no effect with -p {} (view replaced).",
                proj.name(),
            );
        }
        ViewSet::single(proj_field)
    } else if proj.is_metadata() {
        // Metadata projection: `-v`/`-m` are unreachable. Warn if user set them.
        if user_view_explicit && !initial_view.fields.is_empty() {
            let names: Vec<&str> = initial_view.fields.iter().map(|f| f.name()).collect();
            eprintln!(
                "warning: -v fields {{{}}} have no effect with -p {} (no per-match rendering).",
                names.join(", "), proj.name(),
            );
        }
        if has_message {
            eprintln!(
                "warning: -m message template has no effect with -p {} (no per-match rendering).",
                proj.name(),
            );
        }
        initial_view
    } else {
        // Structural (`results`, `report`) — `-v`/`-m` fully respected.
        initial_view
    }
}
