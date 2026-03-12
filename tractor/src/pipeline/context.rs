use std::collections::HashSet;

use tractor_core::{
    OutputFormat, OutputOptions,
    output::should_use_color,
    output::RenderOptions,
};
use crate::cli::SharedArgs;
use crate::xpath_utils::normalize_xpath;
use super::input::{InputMode, resolve_input};

// ---------------------------------------------------------------------------
// SerFormat — serialization/rendering format (-f flag)
// ---------------------------------------------------------------------------

/// Serialization format for the report envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerFormat {
    /// Human-readable text output.
    Text,
    /// JSON report envelope.
    Json,
    /// YAML report envelope.
    Yaml,
    /// XML report envelope.
    Xml,
    /// GCC-style `file:line:col: severity: reason` (one line per match).
    Gcc,
    /// GitHub Actions annotation: `::error file=...,line=...::reason`.
    Github,
}

impl SerFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            format::TEXT => Ok(SerFormat::Text),
            format::JSON => Ok(SerFormat::Json),
            format::YAML => Ok(SerFormat::Yaml),
            format::XML => Ok(SerFormat::Xml),
            format::GCC => Ok(SerFormat::Gcc),
            format::GITHUB => Ok(SerFormat::Github),
            _ => Err(format!(
                "invalid format '{}'. Valid formats: text, json, yaml, xml, gcc, github",
                s,
            )),
        }
    }
}

/// Serialization format constants — correspond to `-f` values.
pub mod format {
    pub const TEXT: &str = "text";
    pub const JSON: &str = "json";
    pub const YAML: &str = "yaml";
    pub const XML: &str = "xml";
    pub const GCC: &str = "gcc";
    pub const GITHUB: &str = "github";
}

// ---------------------------------------------------------------------------
// ViewField + ViewSet — field selection (-v flag)
// ---------------------------------------------------------------------------

/// A single selectable field or section in the output view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewField {
    /// Parsed source tree (XML fragment).
    Tree,
    /// Text content of the matched node.
    Value,
    /// Exact matched source text.
    Source,
    /// Full source lines containing the match.
    Lines,
    /// File path of the match.
    File,
    /// Line number of the match.
    Line,
    /// Column number of the match.
    Column,
    /// Violation reason (check mode).
    Reason,
    /// Violation severity (check mode).
    Severity,
    /// Report summary section.
    Summary,
    /// Total match count.
    Count,
    /// Structural schema overview.
    Schema,
}

impl ViewField {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "tree" | "ast" => Ok(ViewField::Tree),
            "value" => Ok(ViewField::Value),
            "source" => Ok(ViewField::Source),
            "lines" => Ok(ViewField::Lines),
            "file" => Ok(ViewField::File),
            "line" => Ok(ViewField::Line),
            "column" => Ok(ViewField::Column),
            "reason" => Ok(ViewField::Reason),
            "severity" => Ok(ViewField::Severity),
            "summary" => Ok(ViewField::Summary),
            "count" => Ok(ViewField::Count),
            "schema" => Ok(ViewField::Schema),
            "gcc" | "github" => Err(format!(
                "'{}' is a format, not a view. Use -f {} instead of -v {}",
                s, s, s,
            )),
            _ => Err(format!(
                "invalid view '{}'. Valid views: tree, value, source, lines, file, line, column, \
                 reason, severity, summary, count, schema",
                s,
            )),
        }
    }
}

/// A set of selected view fields. Supports comma-separated composition: `-v tree,summary`.
#[derive(Debug, Clone)]
pub struct ViewSet {
    pub fields: HashSet<ViewField>,
}

#[allow(dead_code)]
impl ViewSet {
    pub fn new(fields: HashSet<ViewField>) -> Self {
        ViewSet { fields }
    }

    pub fn single(field: ViewField) -> Self {
        let mut f = HashSet::new();
        f.insert(field);
        ViewSet { fields: f }
    }

    pub fn has(&self, field: ViewField) -> bool {
        self.fields.contains(&field)
    }

    /// Primary `OutputFormat` for `format_matches()` — backward compat bridge.
    /// When multiple fields are selected, picks the most specific one.
    pub fn primary_output_format(&self) -> OutputFormat {
        if self.fields.contains(&ViewField::Schema) { return OutputFormat::Schema; }
        if self.fields.contains(&ViewField::Count) { return OutputFormat::Count; }
        if self.fields.contains(&ViewField::Lines) { return OutputFormat::Lines; }
        if self.fields.contains(&ViewField::Source) { return OutputFormat::Source; }
        if self.fields.contains(&ViewField::Value) { return OutputFormat::Value; }
        OutputFormat::Xml // Tree is the default
    }
}

/// Parse a comma-separated view expression into a `ViewSet`.
/// Example: `"tree,summary"` → `ViewSet` with Tree and Summary.
pub fn parse_view_set(s: &str) -> Result<ViewSet, String> {
    let mut fields = HashSet::new();
    for part in s.split(',') {
        let part = part.trim().to_lowercase();
        if part.is_empty() {
            continue;
        }
        fields.insert(ViewField::from_str(&part)?);
    }
    if fields.is_empty() {
        return Err("view cannot be empty".to_string());
    }
    Ok(ViewSet::new(fields))
}

/// View name constants — correspond to `-v` values.
#[allow(dead_code)]
pub mod view {
    pub const TREE: &str = "tree";
    pub const LINES: &str = "lines";
    pub const SOURCE: &str = "source";
    pub const VALUE: &str = "value";
    pub const COUNT: &str = "count";
    pub const SCHEMA: &str = "schema";
    pub const SUMMARY: &str = "summary";
    pub const REASON: &str = "reason";
    pub const SEVERITY: &str = "severity";
}

// ---------------------------------------------------------------------------
// RunContext
// ---------------------------------------------------------------------------

pub struct RunContext {
    pub xpath: Option<String>,
    /// Serialization format (-f).
    pub ser_format: SerFormat,
    /// View field selection (-v).
    pub view: ViewSet,
    pub use_color: bool,
    pub options: OutputOptions,
    pub input: InputMode,
    pub concurrency: usize,
    // Shared args
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
        default_view: &str,
        user_view: Option<&str>,
        message: Option<String>,
        content: Option<String>,
        warning: bool,
        debug: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let xpath = xpath.as_ref().map(|x| normalize_xpath(x));

        let ser_format = SerFormat::from_str(format)?;

        let view_str = user_view.unwrap_or(default_view);
        let view = parse_view_set(view_str)?;

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
