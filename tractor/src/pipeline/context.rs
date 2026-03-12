use tractor_core::{
    OutputFormat, OutputOptions,
    output::should_use_color,
    output::RenderOptions,
};
use crate::cli::SharedArgs;
use crate::xpath_utils::normalize_xpath;
use super::input::{InputMode, resolve_input};

/// Serialization format for the report envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerFormat {
    /// Human-readable text output (default for check/test).
    Text,
    /// JSON report envelope.
    Json,
    // Future: Yaml, Xml (report envelope as YAML/XML)
}

impl SerFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "text" => Ok(SerFormat::Text),
            "json" => Ok(SerFormat::Json),
            "yaml" | "xml" => Err(format!("format '{}' is not yet implemented", s)),
            _ => Err(format!(
                "invalid format '{}'. Valid formats: text, json",
                s,
            )),
        }
    }
}

/// View name constants (used in CLI defaults and parse_view).
#[allow(dead_code)]
pub mod view {
    pub const TREE: &str = "tree";
    pub const LINES: &str = "lines";
    pub const SOURCE: &str = "source";
    pub const VALUE: &str = "value";
    pub const GCC: &str = "gcc";
    pub const GITHUB: &str = "github";
    pub const COUNT: &str = "count";
    pub const SCHEMA: &str = "schema";
    pub const SUMMARY: &str = "summary";
    pub const REPORT: &str = "report";
}

/// Parse a view shorthand into an OutputFormat.
pub fn parse_view(s: &str) -> Result<OutputFormat, String> {
    match s.to_lowercase().as_str() {
        "tree" | "ast" => Ok(OutputFormat::Xml),
        "lines" => Ok(OutputFormat::Lines),
        "source" => Ok(OutputFormat::Source),
        "value" => Ok(OutputFormat::Value),
        "gcc" => Ok(OutputFormat::Gcc),
        "github" => Ok(OutputFormat::Github),
        "count" => Ok(OutputFormat::Count),
        "schema" => Ok(OutputFormat::Schema),
        // "summary" is recognized but handled separately in modes
        "summary" => Ok(OutputFormat::Count), // placeholder — modes override
        _ => Err(format!(
            "invalid view '{}'. Valid views: tree, lines, source, value, gcc, github, count, schema, summary",
            s,
        )),
    }
}

pub struct RunContext {
    pub xpath: Option<String>,
    /// Serialization format (-f).
    pub ser_format: SerFormat,
    /// View/projection (-q). Controls match rendering for text output.
    pub view: OutputFormat,
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
        format: &str,
        view: Option<&str>,
        message: Option<String>,
        content: Option<String>,
        warning: bool,
        debug: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let xpath = xpath.as_ref().map(|x| normalize_xpath(x));

        let ser_format = SerFormat::from_str(format)?;

        let view = match view {
            Some(v) => parse_view(v)?,
            None => OutputFormat::Xml, // caller overrides with command-specific default
        };

        // Validate: -q only valid with -f text
        if ser_format != SerFormat::Text && view != OutputFormat::Xml {
            // Only error if the user explicitly set -q (view != default)
            // This check is approximate; callers pass the resolved default
        }

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
            ser_format,
            view,
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
