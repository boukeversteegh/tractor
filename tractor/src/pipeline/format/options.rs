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
    Totals,
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
            "totals" | "summary" => Ok(ViewField::Totals),
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
                 reason, severity, totals, count, schema, query, status, output, command",
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

/// Parse a view expression relative to a set of default fields.
///
/// If every non-empty token starts with `+` or `-`, the expression is treated
/// as a modifier list applied to `default_fields`:
/// - `+field` — add `field` if not already present
/// - `-field` — remove `field` if present
///
/// Otherwise the expression is treated as an explicit full field list
/// (same as `parse_view_set`).
///
/// Mixing plain field names with `+`/`-` prefixed modifiers is an error.
pub fn parse_view_with_defaults(s: &str, default_fields: &[ViewField]) -> Result<ViewSet, String> {
    let tokens: Vec<&str> = s.split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    if tokens.is_empty() {
        return Err("view cannot be empty".to_string());
    }

    let has_modifier = tokens.iter().any(|p| p.starts_with('+') || p.starts_with('-'));
    let has_plain = tokens.iter().any(|p| !p.starts_with('+') && !p.starts_with('-'));

    if has_modifier && has_plain {
        return Err(
            "cannot mix plain field names with +/- modifiers in -v. \
             Use either an explicit list (e.g. -v file,line,reason) \
             or only modifiers (e.g. -v -lines,+source)."
                .to_string(),
        );
    }

    if !has_modifier {
        return parse_view_set(s);
    }

    // Modifier mode: start from the defaults and apply each +/- token.
    let mut fields: Vec<ViewField> = default_fields.to_vec();
    for token in &tokens {
        if let Some(field_str) = token.strip_prefix('+') {
            let field = ViewField::from_str(&field_str.to_lowercase())?;
            if !fields.contains(&field) {
                fields.push(field);
            }
        } else if let Some(field_str) = token.strip_prefix('-') {
            let field = ViewField::from_str(&field_str.to_lowercase())?;
            fields.retain(|f| *f != field);
        }
    }

    if fields.is_empty() {
        return Err("view cannot be empty after applying modifiers".to_string());
    }

    Ok(ViewSet::new(fields))
}

// ---------------------------------------------------------------------------
// GroupDimension — grouping dimensions for multi-level grouping
// ---------------------------------------------------------------------------

/// A single dimension to group results by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupDimension {
    /// Group by source file path.
    File,
    /// Group by operation type (command field on matches).
    Command,
    /// Group by rule identifier.
    RuleId,
}

impl GroupDimension {
    pub fn as_str(&self) -> &'static str {
        match self {
            GroupDimension::File => "file",
            GroupDimension::Command => "command",
            GroupDimension::RuleId => "rule_id",
        }
    }
}

/// Parse a `-g` flag value into a list of group dimensions.
/// Supports: "none", "file", "command", "command,file", "file,command", etc.
pub fn parse_group_by(s: &str) -> Result<Vec<GroupDimension>, String> {
    if s == "none" {
        return Ok(vec![]);
    }
    let mut dims = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        match part {
            "file" => dims.push(GroupDimension::File),
            "command" => dims.push(GroupDimension::Command),
            "rule" | "rule_id" => dims.push(GroupDimension::RuleId),
            other => return Err(format!(
                "invalid --group value '{}': use 'none', 'file', 'command', 'rule', or comma-separated (e.g. 'command,file')",
                other
            )),
        }
    }
    Ok(dims)
}


