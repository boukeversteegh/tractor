use tractor_core::{
    output::should_use_color,
    output::RenderOptions,
};
use crate::cli::SharedArgs;
use crate::xpath_utils::normalize_xpath;
use super::input::{InputMode, resolve_input};
use super::format::{OutputFormat, ViewField, ViewSet, parse_view_set};

pub struct RunContext {
    pub xpath: Option<String>,
    /// Output format (-f).
    pub output_format: OutputFormat,
    /// View field selection (-v).
    pub view: ViewSet,
    pub use_color: bool,
    /// Interpolated message template from `-m`, if provided.
    pub message: Option<String>,
    pub input: InputMode,
    pub concurrency: usize,
    pub limit: Option<usize>,
    pub depth: Option<usize>,
    pub parse_depth: Option<usize>,
    pub meta: bool,
    pub raw: bool,
    pub no_pretty: bool,
    pub ignore_whitespace: bool,
    pub verbose: bool,
    pub lang: Option<String>,
    pub debug: bool,
    pub group_by_file: bool,
}

impl RunContext {
    pub fn build(
        shared: &SharedArgs,
        files: Vec<String>,
        xpath: Option<String>,
        format: &str,
        default_view: &[ViewField],
        user_view: Option<&str>,
        message: Option<String>,
        content: Option<String>,
        debug: bool,
        default_group_by_file: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let xpath         = xpath.as_ref().map(|x| normalize_xpath(x));
        let output_format = OutputFormat::from_str(format)?;

        // gcc and github have a fixed output schema — -v is not compatible.
        if matches!(output_format, OutputFormat::Gcc | OutputFormat::Github) {
            if user_view.is_some() {
                return Err(format!(
                    "-v is not compatible with -f {}; {} has a fixed output schema",
                    format, format
                ).into());
            }
        }

        let view = if let Some(s) = user_view {
            parse_view_set(s)?
        } else {
            ViewSet::from_fields(default_view.to_vec())
        };
        let use_color     = if shared.no_color { false } else { should_use_color(&shared.color) };
        let input         = resolve_input(shared, files, content)?;

        let group_by_file = match shared.group_by.as_deref() {
            Some("file") => true,
            Some(other) => return Err(format!("invalid --group-by value '{}': only 'file' is supported", other).into()),
            None => default_group_by_file,
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
            concurrency,
            limit: shared.limit,
            depth: shared.depth,
            parse_depth: shared.parse_depth,
            meta: shared.meta,
            raw: shared.raw,
            no_pretty: shared.no_pretty,
            ignore_whitespace: shared.ignore_whitespace,
            verbose: shared.verbose,
            lang: shared.lang.clone(),
            debug,
            group_by_file,
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
