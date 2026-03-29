// ---------------------------------------------------------------------------
// OutputFormat — serialization/rendering format (-f flag)
// ---------------------------------------------------------------------------

/// Serialization format for the report envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
    Xml,
    /// GCC-style `file:line:col: severity: reason` (one line per match).
    Gcc,
    /// GitHub Actions annotation: `::error file=...,line=...::reason`.
    Github,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            FORMAT_TEXT    => Ok(OutputFormat::Text),
            FORMAT_JSON    => Ok(OutputFormat::Json),
            FORMAT_YAML    => Ok(OutputFormat::Yaml),
            FORMAT_XML     => Ok(OutputFormat::Xml),
            FORMAT_GCC     => Ok(OutputFormat::Gcc),
            FORMAT_GITHUB  => Ok(OutputFormat::Github),
            _ => Err(format!(
                "invalid format '{}'. Valid formats: text, json, yaml, xml, gcc, github", s,
            )),
        }
    }

}

pub const FORMAT_TEXT:   &str = "text";
pub const FORMAT_JSON:   &str = "json";
pub const FORMAT_YAML:   &str = "yaml";
pub const FORMAT_XML:    &str = "xml";
pub const FORMAT_GCC:    &str = "gcc";
pub const FORMAT_GITHUB: &str = "github";

// ---------------------------------------------------------------------------
// ViewField + ViewSet — field selection (-v flag)
// ---------------------------------------------------------------------------

/// A single selectable field or section in the output view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewField {
    Tree,
    Value,
    Source,
    Lines,
    File,
    Line,
    Column,
    Reason,
    Severity,
    Summary,
    Count,
    Schema,
    Query,
    /// Set-command status: "updated" or "unchanged" (default for set mode).
    Status,
    /// Full modified file/string content (set command stdout mode).
    Output,
    /// Operation type: "check", "query", "test", "set", "update".
    Command,
}

impl ViewField {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "tree" | "ast" => Ok(ViewField::Tree),
            "value"        => Ok(ViewField::Value),
            "source"       => Ok(ViewField::Source),
            "lines"        => Ok(ViewField::Lines),
            "file"         => Ok(ViewField::File),
            "line"         => Ok(ViewField::Line),
            "column"       => Ok(ViewField::Column),
            "reason"       => Ok(ViewField::Reason),
            "severity"     => Ok(ViewField::Severity),
            "summary"      => Ok(ViewField::Summary),
            "count"        => Ok(ViewField::Count),
            "schema"       => Ok(ViewField::Schema),
            "query"        => Ok(ViewField::Query),
            "status"       => Ok(ViewField::Status),
            "output"       => Ok(ViewField::Output),
            "command"      => Ok(ViewField::Command),
            "gcc" | "github" => Err(format!(
                "'{}' is a format, not a view. Use -f {} instead of -v {}", s, s, s,
            )),
            _ => Err(format!(
                "invalid view '{}'. Valid views: tree, value, source, lines, file, line, column, \
                 reason, severity, summary, count, schema, query, status, output, command",
                s,
            )),
        }
    }
}

/// A set of selected view fields, preserving declaration order.
///
/// Order determines output field order in renderers.
/// Supports comma-separated composition: `-v tree,summary`.
#[derive(Debug, Clone)]
pub struct ViewSet {
    pub fields: Vec<ViewField>,
}

#[allow(dead_code)]
impl ViewSet {
    pub fn new(fields: Vec<ViewField>) -> Self {
        ViewSet { fields }
    }

    pub fn single(field: ViewField) -> Self {
        ViewSet { fields: vec![field] }
    }

    pub fn from_fields(fields: Vec<ViewField>) -> Self {
        ViewSet { fields }
    }

    pub fn has(&self, field: ViewField) -> bool {
        self.fields.contains(&field)
    }

    /// Add a field at the end if not already present.
    pub fn push_if_absent(&mut self, field: ViewField) {
        if !self.has(field) {
            self.fields.push(field);
        }
    }

    /// Returns true if the view contains any per-match content fields
    /// (fields that produce content on individual match entries, not groups).
    pub fn has_per_match_fields(&self) -> bool {
        self.has(ViewField::Status)
            || self.has(ViewField::Value)
            || self.has(ViewField::Source)
            || self.has(ViewField::Lines)
            || self.has(ViewField::Reason)
            || self.has(ViewField::Severity)
            || self.has(ViewField::Tree)
    }
}

/// Parse a comma-separated view expression into a `ViewSet`, preserving order.
/// Duplicate fields are silently ignored.
pub fn parse_view_set(s: &str) -> Result<ViewSet, String> {
    let mut fields = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for part in s.split(',') {
        let part = part.trim().to_lowercase();
        if part.is_empty() { continue; }
        let field = ViewField::from_str(&part)?;
        if seen.insert(field) {
            fields.push(field);
        }
    }
    if fields.is_empty() {
        return Err("view cannot be empty".to_string());
    }
    Ok(ViewSet::new(fields))
}

// ---------------------------------------------------------------------------
// GroupBy — controls whether the file field is emitted on individual matches
// ---------------------------------------------------------------------------

/// Describes how matches are grouped in the output.
/// When grouped by file, the file is on the parent — individual matches omit it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// Matches are not grouped; include the `file` field on each match.
    None,
    /// Matches are grouped by file; omit `file` from individual matches.
    File,
}

