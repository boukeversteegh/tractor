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
             Use +/- modifiers to adjust defaults: -v -lines or -v +source,-lines"
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
             -v -lines               Remove a field from defaults\n  \
             -v +source,-lines       Add and remove in one expression",
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

// ---------------------------------------------------------------------------
// Projection — what subtree of the report to emit (-p / --project flag)
// ---------------------------------------------------------------------------

/// A selection of a report element to emit. Values are names of real elements
/// in the revised report shape (`<summary>`, `<schema>`, `<results>`, etc.) or
/// singular scalar aliases (`count`).
///
/// Each projection has:
/// - a [`ProjectionKind`] determining how it interacts with the `-v` view set,
/// - a natural [`Cardinality`] (how many elements it selects per run), with
///   `--single` collapsing sequences to a single element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// `<tree>` elements, one per match. Replaces `-v` with `[tree]`.
    Tree,
    /// `<value>` elements, one per match. Replaces `-v` with `[value]`.
    Value,
    /// `<source>` elements, one per match. Replaces `-v` with `[source]`.
    Source,
    /// `<lines>` elements, one per match. Replaces `-v` with `[lines]`.
    Lines,
    /// The `<schema>` element. Replaces `-v` with `[schema]` (triggers schema
    /// computation — the only view-level projection with real cost).
    Schema,
    /// The scalar total result count (`/summary/totals/results`). Replaces
    /// `-v` with `[count]`. No `<count>` element exists in the report shape;
    /// `count` is sugar in the same way `-v count` is.
    Count,
    /// The `<summary>` container. Metadata — `-v` has no effect.
    Summary,
    /// The `<totals>` element inside summary. Metadata — `-v` has no effect.
    Totals,
    /// The `<results>` list wrapper. Structural — `-v` drives per-match
    /// field selection as usual.
    Results,
    /// The whole `<report>` envelope. Default when `-p` is omitted;
    /// structural — `-v` drives per-match field selection as usual.
    Report,
}

/// How a projection interacts with the `-v` view set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionKind {
    /// A view-level field (`tree`, `value`, `source`, `lines`, `schema`,
    /// `count`): the projection *replaces* the view set with `[that field]`.
    /// Any explicit `-v`/`-m` fields other than the target are discarded
    /// (warning emitted).
    ViewLevel,
    /// A report-structural element (`results`, `report`): the projection
    /// *respects* the user's `-v` because these elements contain per-match
    /// fields that `-v` controls.
    Structural,
    /// A metadata container with no per-match content (`summary`, `totals`):
    /// `-v` is irrelevant. Any explicit `-v`/`-m` is unreachable (warning).
    Metadata,
}

/// How many elements the projection selects from the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// Emits exactly one element (or zero, for empty sequences with `--single`).
    Singular,
    /// Emits a sequence of zero-or-more elements, wrapped in a list root.
    Sequence,
}

impl Projection {
    /// All variants in display/help order.
    const ALL: &[Projection] = &[
        Projection::Tree, Projection::Value, Projection::Source, Projection::Lines,
        Projection::Schema, Projection::Count,
        Projection::Summary, Projection::Totals,
        Projection::Results, Projection::Report,
    ];

    /// Canonical CLI name for this projection (the name used in `-p <NAME>`).
    pub fn name(&self) -> &'static str {
        match self {
            Projection::Tree    => "tree",
            Projection::Value   => "value",
            Projection::Source  => "source",
            Projection::Lines   => "lines",
            Projection::Schema  => "schema",
            Projection::Count   => "count",
            Projection::Summary => "summary",
            Projection::Totals  => "totals",
            Projection::Results => "results",
            Projection::Report  => "report",
        }
    }

    /// Short description for help and error output.
    pub fn description(&self) -> &'static str {
        match self {
            Projection::Tree    => "<tree> per match (replaces -v)",
            Projection::Value   => "<value> per match (replaces -v)",
            Projection::Source  => "<source> per match (replaces -v)",
            Projection::Lines   => "<lines> per match (replaces -v)",
            Projection::Schema  => "<schema> rendering (replaces -v, triggers schema)",
            Projection::Count   => "scalar total match count (replaces -v)",
            Projection::Summary => "<summary> container (success, totals, expected, query)",
            Projection::Totals  => "<totals> element inside summary",
            Projection::Results => "<results> list wrapper (respects -v)",
            Projection::Report  => "whole <report> envelope (default)",
        }
    }

    /// How this projection interacts with the user's `-v`.
    pub fn kind(&self) -> ProjectionKind {
        match self {
            Projection::Tree | Projection::Value | Projection::Source
            | Projection::Lines | Projection::Schema | Projection::Count
                => ProjectionKind::ViewLevel,
            Projection::Results | Projection::Report
                => ProjectionKind::Structural,
            Projection::Summary | Projection::Totals
                => ProjectionKind::Metadata,
        }
    }

    /// The cardinality this projection emits, accounting for `--single`.
    ///
    /// `--single` always collapses to `Singular`. For projections that are
    /// already singular, this returns `Singular` regardless of `single`.
    pub fn cardinality(&self, single: bool) -> Cardinality {
        if single {
            return Cardinality::Singular;
        }
        match self {
            Projection::Tree | Projection::Value | Projection::Source
            | Projection::Lines | Projection::Results
                => Cardinality::Sequence,
            Projection::Schema | Projection::Count
            | Projection::Summary | Projection::Totals
            | Projection::Report
                => Cardinality::Singular,
        }
    }

    /// True when `--single` on this projection is a no-op (already singular).
    /// Used to emit a warning when the user passes `--single` with one of
    /// these projections.
    pub fn is_already_singular(&self) -> bool {
        matches!(self,
            Projection::Schema | Projection::Count
            | Projection::Summary | Projection::Totals
            | Projection::Report
        )
    }

    /// If this projection is view-level, the single [`ViewField`] that the
    /// view set should be replaced with. `Count`/`Schema` map to the
    /// matching `ViewField` so downstream code that keys off view fields
    /// (e.g. schema computation) still fires.
    pub fn replacement_view_field(&self) -> Option<ViewField> {
        match self {
            Projection::Tree   => Some(ViewField::Tree),
            Projection::Value  => Some(ViewField::Value),
            Projection::Source => Some(ViewField::Source),
            Projection::Lines  => Some(ViewField::Lines),
            Projection::Schema => Some(ViewField::Schema),
            Projection::Count  => Some(ViewField::Count),
            _ => None,
        }
    }

    /// Full `long_help` text for the `-p` / `--project` flag.
    pub fn project_long_help() -> String {
        let mut lines = vec!["Project a single element from the report [default: report]".to_string()];
        let max_name = Projection::ALL.iter().map(|p| p.name().len()).max().unwrap_or(0);
        for p in Projection::ALL {
            lines.push(format!("  {:width$}  {}", p.name(), p.description(), width = max_name));
        }
        lines.push(String::new());
        lines.push("Use --single to emit one element bare (strip the list wrapper).".to_string());
        lines.join("\n")
    }
}

/// Parse a `-p <NAME>` value into a [`Projection`].
pub fn parse_projection(s: &str) -> Result<Projection, String> {
    match s.to_lowercase().as_str() {
        "tree"    => Ok(Projection::Tree),
        "value"   => Ok(Projection::Value),
        "source"  => Ok(Projection::Source),
        "lines"   => Ok(Projection::Lines),
        "schema"  => Ok(Projection::Schema),
        "count"   => Ok(Projection::Count),
        "summary" => Ok(Projection::Summary),
        "totals"  => Ok(Projection::Totals),
        "results" => Ok(Projection::Results),
        "report"  => Ok(Projection::Report),
        _ => {
            let valid: Vec<&str> = Projection::ALL.iter().map(|p| p.name()).collect();
            Err(format!(
                "invalid -p value '{}'. Valid values: {}",
                s,
                valid.join(", "),
            ))
        }
    }
}
