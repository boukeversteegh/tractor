use tractor_core::{
    OutputFormat, OutputOptions,
    output::should_use_color,
    output::RenderOptions,
};
use crate::cli::SharedArgs;
use crate::xpath_utils::normalize_xpath;
use super::input::{InputMode, resolve_input};

pub struct RunContext {
    pub xpath: Option<String>,
    pub format: OutputFormat,
    pub use_color: bool,
    pub options: OutputOptions,
    pub input: InputMode,
    pub concurrency: usize,
    // Shared args (borrowed fields exposed individually)
    pub limit: Option<usize>,
    pub depth: Option<usize>,
    pub parse_depth: Option<usize>,
    pub keep_locations: bool,
    pub raw: bool,
    pub no_pretty: bool,
    pub ignore_whitespace: bool,
    pub verbose: bool,
    pub lang: Option<String>,
    // Mode-specific
    pub debug: bool,
}

impl RunContext {
    pub fn build(
        shared: &SharedArgs,
        files: Vec<String>,
        xpath: Option<String>,
        output_format: &str,
        message: Option<String>,
        content: Option<String>,
        warning: bool,
        debug: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let xpath = xpath.as_ref().map(|x| normalize_xpath(x));

        let format = OutputFormat::from_str(output_format)
            .ok_or_else(|| {
                format!(
                    "invalid format '{}'. Valid formats: {}",
                    output_format,
                    OutputFormat::valid_formats().join(", ")
                )
            })?;

        let use_color = if shared.no_color {
            false
        } else {
            should_use_color(&shared.color)
        };

        let input = resolve_input(shared, files, content)?;

        let concurrency = shared.concurrency.unwrap_or_else(|| num_cpus::get());
        rayon::ThreadPoolBuilder::new()
            .num_threads(concurrency)
            .build_global()
            .ok();

        let options = OutputOptions {
            message,
            use_color,
            strip_locations: !shared.keep_locations,
            max_depth: shared.depth,
            pretty_print: !shared.no_pretty,
            language: shared.lang.clone(),
            warning,
        };

        Ok(RunContext {
            xpath,
            format,
            use_color,
            options,
            input,
            concurrency,
            limit: shared.limit,
            depth: shared.depth,
            parse_depth: shared.parse_depth,
            keep_locations: shared.keep_locations,
            raw: shared.raw,
            no_pretty: shared.no_pretty,
            ignore_whitespace: shared.ignore_whitespace,
            verbose: shared.verbose,
            lang: shared.lang.clone(),
            debug,
        })
    }

    pub fn render_options(&self) -> RenderOptions {
        RenderOptions::new()
            .with_color(self.use_color)
            .with_locations(self.keep_locations || self.debug)
            .with_max_depth(self.depth)
            .with_pretty_print(!self.no_pretty)
    }

    pub fn schema_depth(&self) -> Option<usize> {
        self.depth.or(Some(4))
    }
}
