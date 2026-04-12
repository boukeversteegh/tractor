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
    /// Claude Code hook JSON output.
    ClaudeCode,
}

impl OutputFormat {
    /// All variants in display order.
    const ALL: &[OutputFormat] = &[
        OutputFormat::Text, OutputFormat::Json, OutputFormat::Yaml,
        OutputFormat::Xml, OutputFormat::Gcc, OutputFormat::Github,
        OutputFormat::ClaudeCode,
    ];

    /// Canonical CLI name for this format.
    pub fn name(&self) -> &'static str {
        match self {
            OutputFormat::Text      => FORMAT_TEXT,
            OutputFormat::Json      => FORMAT_JSON,
            OutputFormat::Yaml      => FORMAT_YAML,
            OutputFormat::Xml       => FORMAT_XML,
            OutputFormat::Gcc       => FORMAT_GCC,
            OutputFormat::Github    => FORMAT_GITHUB,
            OutputFormat::ClaudeCode => FORMAT_CLAUDE_CODE,
        }
    }

    /// Short description for help and error output.
    pub fn description(&self) -> &'static str {
        match self {
            OutputFormat::Text      => "Human-readable plain text",
            OutputFormat::Json      => "JSON report envelope",
            OutputFormat::Yaml      => "YAML report envelope",
            OutputFormat::Xml       => "XML report envelope",
            OutputFormat::Gcc       => "file:line:col: severity: reason (for CI/editors)",
            OutputFormat::Github    => "GitHub Actions annotation (::error file=...)",
            OutputFormat::ClaudeCode => "Claude Code hook JSON (use with --hook)",
        }
    }

    /// Full `long_help` text for the `-f` / `--format` flag.
    pub fn format_long_help(default: &str) -> String {
        let mut lines = vec![format!("Output format [default: {default}]")];
        let max_name = OutputFormat::ALL.iter().map(|f| f.name().len()).max().unwrap_or(0);
        for f in OutputFormat::ALL {
            lines.push(format!("  {:width$}  {}", f.name(), f.description(), width = max_name));
        }
        lines.join("\n")
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        let all_names: Vec<&str> = OutputFormat::ALL.iter().map(|f| f.name()).collect();
        match s.to_lowercase().as_str() {
            FORMAT_TEXT    => Ok(OutputFormat::Text),
            FORMAT_JSON    => Ok(OutputFormat::Json),
            FORMAT_YAML    => Ok(OutputFormat::Yaml),
            FORMAT_XML     => Ok(OutputFormat::Xml),
            FORMAT_GCC     => Ok(OutputFormat::Gcc),
            FORMAT_GITHUB     => Ok(OutputFormat::Github),
            FORMAT_CLAUDE_CODE => Ok(OutputFormat::ClaudeCode),
            _ => Err(format!(
                "invalid format '{}'. Valid formats: {}", s, all_names.join(", "),
            )),
        }
    }
}

pub const FORMAT_TEXT:   &str = "text";
pub const FORMAT_JSON:   &str = "json";
pub const FORMAT_YAML:   &str = "yaml";
pub const FORMAT_XML:    &str = "xml";
pub const FORMAT_GCC:    &str = "gcc";
pub const FORMAT_GITHUB:     &str = "github";
pub const FORMAT_CLAUDE_CODE: &str = "claude-code";

// ---------------------------------------------------------------------------
// HookType — Claude Code hook event type (--hook flag)
// ---------------------------------------------------------------------------

/// Which Claude Code hook event the output is for. Determines the JSON
/// envelope structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    /// PostToolUse / Stop: `{ "decision": "block", "reason": "..." }`
    PostToolUse,
    /// PreToolUse: `{ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "deny", ... } }`
    PreToolUse,
    /// PostToolUse context (non-blocking): `{ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": "..." } }`
    Context,
}

impl HookType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "post-tool-use" => Ok(HookType::PostToolUse),
            "pre-tool-use"  => Ok(HookType::PreToolUse),
            "stop"          => Ok(HookType::PostToolUse), // same envelope as post-tool-use
            "context"       => Ok(HookType::Context),
            _ => Err(format!(
                "invalid hook type '{}'. Valid types: post-tool-use, pre-tool-use, stop, context", s,
            )),
        }
    }
}

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
    /// Diagnostic origin: "xpath", "cli", "config", "input".
    Origin,
}

impl ViewField {
    /// All variants in canonical display order.
    const ALL: &[ViewField] = &[
        ViewField::Tree, ViewField::Value, ViewField::Source, ViewField::Lines,
        ViewField::File, ViewField::Line, ViewField::Column,
        ViewField::Reason, ViewField::Severity, ViewField::Totals,
        ViewField::Count, ViewField::Schema, ViewField::Query,
        ViewField::Status, ViewField::Output, ViewField::Command, ViewField::Origin,
    ];

    /// Canonical CLI name for this field (the primary name accepted by `-v`).
    pub fn name(&self) -> &'static str {
        match self {
            ViewField::Tree     => "tree",
            ViewField::Value    => "value",
            ViewField::Source   => "source",
            ViewField::Lines    => "lines",
            ViewField::File     => "file",
            ViewField::Line     => "line",
            ViewField::Column   => "column",
            ViewField::Reason   => "reason",
            ViewField::Severity => "severity",
            ViewField::Totals   => "totals",
            ViewField::Count    => "count",
            ViewField::Schema   => "schema",
            ViewField::Query    => "query",
            ViewField::Status   => "status",
            ViewField::Output   => "output",
            ViewField::Command  => "command",
            ViewField::Origin   => "origin",
        }
    }

    /// Short description for help and error output.
    pub fn description(&self) -> &'static str {
        match self {
            ViewField::Tree     => "Parsed source tree",
            ViewField::Value    => "Text content of matched nodes",
            ViewField::Source   => "Exact matched source text",
            ViewField::Lines    => "Full source lines containing each match",
            ViewField::File     => "File path of the match",
            ViewField::Line     => "Line number of the match",
            ViewField::Column   => "Column number of the match",
            ViewField::Reason   => "Reason message for violations",
            ViewField::Severity => "Severity level (error/warning)",
            ViewField::Totals   => "Summary totals across all matches",
            ViewField::Count    => "Total match count",
            ViewField::Schema   => "Structural overview of element types",
            ViewField::Query    => "Echo the XPath query as received",
            ViewField::Status   => "Whether each match was updated or unchanged",
            ViewField::Output   => "Full modified content (for --stdout)",
            ViewField::Command  => "Operation type (check, query, etc.)",
            ViewField::Origin   => "Diagnostic origin (xpath, cli, etc.)",
        }
    }

    /// Category label for grouping in help output.
    fn category(&self) -> &'static str {
        match self {
            ViewField::Tree | ViewField::Value | ViewField::Source
            | ViewField::Lines | ViewField::Schema => "content",
            ViewField::File | ViewField::Line | ViewField::Column => "location",
            ViewField::Reason | ViewField::Severity | ViewField::Origin
            | ViewField::Command => "diagnostic",
            ViewField::Count | ViewField::Totals | ViewField::Query => "summary",
            ViewField::Status | ViewField::Output => "set mode",
        }
    }

    /// Categorized view names with descriptions and modifier syntax.
    /// Used by both `--help` (via `view_long_help`) and error messages.
    pub fn help_text() -> String {
        let categories: &[&str] = &["content", "location", "diagnostic", "summary", "set mode"];
        let max_name_len = ViewField::ALL.iter().map(|f| f.name().len()).max().unwrap_or(0);
        let mut sections = Vec::new();
        for &cat in categories {
            let fields: Vec<&ViewField> = ViewField::ALL.iter()
                .filter(|f| f.category() == cat)
                .collect();
            if fields.is_empty() { continue; }
            let mut lines = vec![format!("  {}:", cat)];
            for f in fields {
                lines.push(format!("    {:width$}  {}", f.name(), f.description(), width = max_name_len));
            }
            sections.push(lines.join("\n"));
        }
        let views = sections.join("\n");
        format!(
            "{views}\n\n\
             Combine with commas: -v tree,value\n\
             Use +/- modifiers to adjust defaults: -v=-lines or -v=+source,-lines"
        )
    }

    /// Full `long_help` text for the `-v` / `--view` flag, including the
    /// command-specific default. Call from `Command` augmentation in main.
    pub fn view_long_help(default_fields: &[ViewField]) -> String {
        let defaults: Vec<&str> = default_fields.iter().map(|f| f.name()).collect();
        format!(
            "Choose which fields are included in the output [default: {}]\n\n\
             {}\n\n\
             Examples:\n  \
             -v value                Show only text content\n  \
             -v tree,value           Combine multiple fields\n  \
             -v=-lines               Remove a field from defaults\n  \
             -v=+source,-lines       Add and remove in one expression",
            defaults.join(","),
            ViewField::help_text(),
        )
    }

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
            "origin"       => Ok(ViewField::Origin),
            "gcc" | "github" => Err(format!(
                "'{}' is a format, not a view. Use -f {} instead of -v {}", s, s, s,
            )),
            _ => Err(format!(
                "invalid view '{}'.\n\nValid views:\n{}",
                s, ViewField::help_text(),
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
///
/// If every non-empty token starts with `+` or `-`, the expression is treated
/// as a modifier list applied to `default_fields`:
/// - `+field` — add `field` if not already present
/// - `-field` — remove `field` if present
///
/// Otherwise the expression is treated as an explicit full field list and
/// `default_fields` is ignored. Duplicate fields are silently ignored.
///
/// Mixing plain field names with `+`/`-` prefixed modifiers is an error.
pub fn parse_view_set(s: &str, default_fields: &[ViewField]) -> Result<ViewSet, String> {
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

    if has_modifier {
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
    } else {
        // Explicit list mode: ignore defaults, parse the fields directly.
        let mut fields = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for token in &tokens {
            let field = ViewField::from_str(&token.to_lowercase())?;
            if seen.insert(field) {
                fields.push(field);
            }
        }
        Ok(ViewSet::new(fields))
    }
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


