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
// Projection — `-p` / `--project` selects which element of the report to emit.
// ---------------------------------------------------------------------------

/// Which element of the report the user wants emitted.
///
/// Every variant names a real element in the report shape (`<report>/<summary>`,
/// `<report>/<results>`, `<report>/<schema>`, `<report>/<results>/<match>/<tree>`, …).
/// `-p report` is the default — emit the whole envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// Full report envelope (default when `-p` is omitted).
    Report,
    /// The `<results>` list — sequence of matches.
    Results,
    /// The `<summary>` container.
    Summary,
    /// The `<totals>` element inside summary.
    Totals,
    /// The `<schema>` element.
    Schema,
    /// Scalar match count (sugar alias for `/summary/totals/results`).
    Count,
    /// `<tree>` elements, one per match.
    Tree,
    /// `<value>` elements, one per match.
    Value,
    /// `<source>` elements, one per match.
    Source,
    /// `<lines>` elements, one per match.
    Lines,
}

/// Category of a projection — drives `-v` interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionKind {
    /// One view-level field per match: `-v` is **replaced** with `[that field]`.
    /// Members: tree, value, source, lines, schema, count.
    ViewField,
    /// Full-report shapes: `-v` is **respected** (drives per-match fields).
    /// Members: results, report.
    Structural,
    /// Summary/totals: `-v` is **irrelevant** (no per-match rendering).
    /// Members: summary, totals.
    Metadata,
}

impl Projection {
    /// Canonical CLI name.
    pub fn name(&self) -> &'static str {
        match self {
            Projection::Report  => "report",
            Projection::Results => "results",
            Projection::Summary => "summary",
            Projection::Totals  => "totals",
            Projection::Schema  => "schema",
            Projection::Count   => "count",
            Projection::Tree    => "tree",
            Projection::Value   => "value",
            Projection::Source  => "source",
            Projection::Lines   => "lines",
        }
    }

    /// One-line description for help output.
    pub fn description(&self) -> &'static str {
        match self {
            Projection::Report  => "Full report envelope (default)",
            Projection::Results => "Just the <results> list (respects -v)",
            Projection::Summary => "Just the <summary> container",
            Projection::Totals  => "Just the <totals> element",
            Projection::Schema  => "Just the <schema> element (triggers schema collection)",
            Projection::Count   => "Scalar match count",
            Projection::Tree    => "Per-match <tree> elements",
            Projection::Value   => "Per-match <value> elements",
            Projection::Source  => "Per-match <source> elements",
            Projection::Lines   => "Per-match <lines> elements",
        }
    }

    pub const ALL: &[Projection] = &[
        Projection::Report,  Projection::Results, Projection::Summary, Projection::Totals,
        Projection::Schema,  Projection::Count,   Projection::Tree,    Projection::Value,
        Projection::Source,  Projection::Lines,
    ];

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "report"  => Ok(Projection::Report),
            "results" => Ok(Projection::Results),
            "summary" => Ok(Projection::Summary),
            "totals"  => Ok(Projection::Totals),
            "schema"  => Ok(Projection::Schema),
            "count"   => Ok(Projection::Count),
            "tree" | "ast" => Ok(Projection::Tree),
            "value"   => Ok(Projection::Value),
            "source"  => Ok(Projection::Source),
            "lines"   => Ok(Projection::Lines),
            _ => {
                let names: Vec<&str> = Projection::ALL.iter().map(|p| p.name()).collect();
                Err(format!(
                    "invalid -p / --project value '{}'. Valid values: {}",
                    s, names.join(", "),
                ))
            }
        }
    }

    /// Which category of interaction this projection has with `-v`.
    pub fn kind(&self) -> ProjectionKind {
        match self {
            Projection::Tree | Projection::Value | Projection::Source
            | Projection::Lines | Projection::Schema | Projection::Count => ProjectionKind::ViewField,
            Projection::Results | Projection::Report => ProjectionKind::Structural,
            Projection::Summary | Projection::Totals => ProjectionKind::Metadata,
        }
    }

    /// When this projection corresponds to a single `ViewField`, return it —
    /// used to replace the view set under the `-v` replacement rule. `count`
    /// is sugar for `/summary/totals/results` and maps to `ViewField::Count`.
    pub fn as_view_field(&self) -> Option<ViewField> {
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

    /// True when the projection conceptually returns a sequence of elements
    /// (one per match). `--single` applies only to these.
    pub fn is_sequence(&self) -> bool {
        matches!(
            self,
            Projection::Tree | Projection::Value | Projection::Source
            | Projection::Lines | Projection::Results
        )
    }

    /// Help text listing all projection values with descriptions.
    pub fn help_text() -> String {
        let max = Projection::ALL.iter().map(|p| p.name().len()).max().unwrap_or(0);
        let mut lines = Vec::new();
        for p in Projection::ALL {
            lines.push(format!("  {:width$}  {}", p.name(), p.description(), width = max));
        }
        lines.join("\n")
    }
}

/// A normalized projection plan — the single source of truth for output shape.
///
/// Computed once from `(user projection, --single, -v, -m, -n)` at the CLI
/// boundary, so downstream stages never re-derive "what did the user mean".
#[derive(Debug, Clone)]
pub struct ProjectionPlan {
    pub projection: Projection,
    /// True when `--single` is in effect (strip list wrappers, take first match).
    pub single: bool,
    /// Final view set after the replacement rule. This is the set of per-match
    /// fields downstream renderers should consume for structural projections;
    /// for view-level projections it has been replaced by `[that field]`.
    pub view: ViewSet,
    /// Warnings to emit on stderr for discarded view fields. Accumulated during
    /// normalization so later stages don't have to recompute which fields the
    /// user asked for that won't appear.
    pub warnings: Vec<String>,
    /// True when `-v` was explicitly set by the user (not the mode default).
    /// Used to suppress "redundant overlap" warnings.
    pub view_was_explicit: bool,
    /// True when `-m` (message template) was explicitly set. Used for warnings.
    pub message_was_explicit: bool,
}

impl ProjectionPlan {
    /// Default plan — no projection override, no `--single`, keep the default view.
    pub fn default_with_view(view: ViewSet) -> Self {
        ProjectionPlan {
            projection: Projection::Report,
            single: false,
            view,
            warnings: Vec::new(),
            view_was_explicit: false,
            message_was_explicit: false,
        }
    }

    /// Normalize `-p`, `--single`, and `-v`/`-m` into a single plan.
    ///
    /// Rules (from design):
    /// - `-p X` (view-level) replaces the view set with exactly `[X]`.
    /// - `-p X` (structural) keeps the user's view set intact.
    /// - `-p X` (metadata) leaves the view set alone but warns if the user
    ///   explicitly requested any per-match view field.
    /// - `--single` alone (no `-p`) implies `-p results`.
    /// - `--single -n N` (for N != 1) is a CLI error — contradictory bounds.
    /// - `--single` on a singular projection is a no-op with a warning.
    pub fn resolve(
        projection_str: Option<&str>,
        single: bool,
        user_view: Option<&str>,
        default_view: &[ViewField],
        user_message: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Self, String> {
        let view_was_explicit = user_view.is_some();
        let message_was_explicit = user_message.is_some();

        // `--single` is incompatible with `-n` when they set different bounds.
        if single {
            match limit {
                Some(1) | None => {}
                Some(n) => {
                    return Err(format!(
                        "--single is incompatible with -n {n}: --single takes the first match, \
                         but -n {n} asks for {n}. Use `-n 1` or drop `--single`.",
                    ));
                }
            }
        }

        // Resolve the user's base view set (before projection replacement).
        let base_view = if let Some(s) = user_view {
            parse_view_set(s, default_view)?
        } else {
            ViewSet::from_fields(default_view.to_vec())
        };

        // --single alone implies -p results (emitting the whole report with a
        // single match bare would be a no-op — never what the user meant).
        let projection = match projection_str {
            Some(s) => Projection::from_str(s)?,
            None if single => Projection::Results,
            None => Projection::Report,
        };

        let mut warnings: Vec<String> = Vec::new();
        let view = compute_view(
            projection,
            base_view.clone(),
            default_view,
            view_was_explicit,
            message_was_explicit,
            &mut warnings,
        );

        // `--single` on a singular projection is a no-op — warn so the user
        // knows the flag is doing nothing, but keep producing output.
        if single && !projection.is_sequence() {
            warnings.push(format!(
                "warning: --single has no effect with -p {name} ({name} is already singular). \
                 Drop --single.",
                name = projection.name(),
            ));
        }

        Ok(ProjectionPlan {
            projection,
            single,
            view,
            warnings,
            view_was_explicit,
            message_was_explicit,
        })
    }
}

/// Apply the `-v` replacement rule and build up the final view set.
///
/// Warnings are appended to `out_warnings` so the caller can emit them on
/// stderr after normalization completes. Centralizing the logic here keeps
/// the policy in one place: renderers never have to recompute "which fields
/// are dropped under this projection".
fn compute_view(
    projection: Projection,
    base_view: ViewSet,
    _default_view: &[ViewField],
    view_was_explicit: bool,
    message_was_explicit: bool,
    out_warnings: &mut Vec<String>,
) -> ViewSet {
    match projection.kind() {
        ProjectionKind::Structural => {
            // Respect -v — every explicitly-requested field appears in each match.
            base_view
        }
        ProjectionKind::ViewField => {
            let replacement_field = projection.as_view_field()
                .expect("view-level projection must map to a ViewField");
            let replaced = ViewSet::single(replacement_field);

            // Warn when explicit -v contained anything other than the replacement.
            if view_was_explicit {
                let dropped: Vec<&'static str> = base_view.fields.iter()
                    .filter(|f| **f != replacement_field)
                    .map(|f| f.name())
                    .collect();
                if !dropped.is_empty() {
                    out_warnings.push(format!(
                        "warning: -v fields {{{fields}}} were discarded because \
                         -p {proj} replaces the view set.\n  \
                         To keep -v intact, use `-p results` (respects -v) instead of `-p {proj}`.",
                        fields = dropped.join(", "),
                        proj = projection.name(),
                    ));
                }
            }
            if message_was_explicit {
                out_warnings.push(format!(
                    "warning: -m message template has no effect with -p {proj} \
                     ({proj} replaces the view set).",
                    proj = projection.name(),
                ));
            }
            replaced
        }
        ProjectionKind::Metadata => {
            // Leave the view alone, but warn: explicit per-match fields have
            // no way to surface in summary/totals output.
            if view_was_explicit {
                let dropped: Vec<&'static str> = base_view.fields.iter()
                    .map(|f| f.name())
                    .collect();
                if !dropped.is_empty() {
                    out_warnings.push(format!(
                        "warning: -v fields {{{fields}}} were discarded because \
                         -p {proj} has no per-match rendering.",
                        fields = dropped.join(", "),
                        proj = projection.name(),
                    ));
                }
            }
            if message_was_explicit {
                out_warnings.push(format!(
                    "warning: -m message template has no effect with -p {proj} \
                     (no per-match rendering).",
                    proj = projection.name(),
                ));
            }
            base_view
        }
    }
}
