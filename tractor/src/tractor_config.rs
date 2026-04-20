//! Config file parser for tractor batch execution.
//!
//! Parses YAML or TOML config files into a flat `Vec<Operation>`.
//! Supports two forms:
//!
//! 1. Root-level command keys (one of each type):
//!    ```yaml
//!    check:
//!      files: [...]
//!      rules: [...]
//!    set:
//!      files: [...]
//!      mappings: [...]
//!    ```
//!
//! 2. Explicit operations list (ordered, allows duplicates):
//!    ```yaml
//!    operations:
//!      - check:
//!          files: [...]
//!          rules: [...]
//!      - set:
//!          files: [...]
//!          mappings: [...]
//!    ```
//!
//! Both forms produce the same `Vec<Operation>`. When both are present,
//! root-level keys are expanded first, then the operations list is appended.

use std::path::Path;
use serde::Deserialize;
use tractor::declarative_set::parse_set_expr;
use tractor::normalized_xpath::NormalizedXpath;
use tractor::report::Severity;
use tractor::rule::Rule;
use tractor::tree_mode::TreeMode;

use crate::executor::{
    QueryDraft, QueryExpr, SetDraft, SetMapping, SetReportMode, SetWriteMode,
    TestAssertion, TestDraft,
};
use crate::input::Source;

// ---------------------------------------------------------------------------
// Serde schema
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    /// Root-level file scope: glob patterns that constrain all operations.
    /// Intersected with each operation's own `files`.
    /// `None` when the key is missing (unrestricted); `Some(vec![])` when
    /// explicitly empty (`files: []`).
    #[serde(default)]
    files: Option<Vec<String>>,

    /// Root-level exclude patterns applied to all operations.
    #[serde(default)]
    exclude: Vec<String>,

    /// Root-level git diff spec: only consider files changed in this diff.
    /// Intersected with every operation's resolved file set.
    #[serde(default, rename = "diff-files")]
    diff_files: Option<String>,

    /// Root-level git diff spec: only include matches in changed hunks.
    #[serde(default, rename = "diff-lines")]
    diff_lines: Option<String>,

    /// Root-level check shorthand (single check operation).
    #[serde(default)]
    check: Option<CheckConfig>,

    /// Root-level set shorthand (single set operation).
    #[serde(default)]
    set: Option<SetConfig>,

    /// Root-level query shorthand (single query operation).
    #[serde(default)]
    query: Option<QueryConfig>,

    /// Root-level test shorthand (single test operation).
    #[serde(default)]
    test: Option<TestConfig>,

    /// Explicit ordered list of operations.
    #[serde(default)]
    operations: Vec<OperationEntry>,
}

/// A single entry in the operations list.
/// Deserialized from YAML like:
///   - check:
///       files: [...]
///   - set:
///       files: [...]
///   - query:
///       files: [...]
///   - test:
///       files: [...]
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct OperationEntry {
    #[serde(default)]
    check: Option<CheckConfig>,
    #[serde(default)]
    set: Option<SetConfig>,
    #[serde(default)]
    query: Option<QueryConfig>,
    #[serde(default)]
    test: Option<TestConfig>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct CheckConfig {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default, rename = "diff-files")]
    diff_files: Option<String>,
    #[serde(default, rename = "diff-lines")]
    diff_lines: Option<String>,
    #[serde(default)]
    rules: Vec<CheckRuleConfig>,
    #[serde(default, rename = "tree-mode")]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct CheckRuleConfig {
    id: String,
    xpath: NormalizedXpath,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default = "default_severity")]
    severity: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default, rename = "tree-mode")]
    tree_mode: Option<String>,
    #[serde(default)]
    expect: Vec<CheckExpectEntry>,
}

/// A single expectation entry for check rules in tractor config files.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct CheckExpectEntry {
    #[serde(default)]
    valid: Option<String>,
    #[serde(default)]
    invalid: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct SetConfig {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default, rename = "diff-files")]
    diff_files: Option<String>,
    #[serde(default, rename = "diff-lines")]
    diff_lines: Option<String>,
    #[serde(default)]
    mappings: Vec<SetMappingConfig>,
    #[serde(default, alias = "expr", alias = "declarative")]
    expression: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default, rename = "tree-mode")]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default, rename = "ignore-whitespace")]
    ignore_whitespace: bool,
    // Parsed for forward compatibility with shared command config, but
    // currently ignored because set mutates full documents rather than
    // supporting depth-limited parses.
    #[allow(dead_code)]
    #[serde(default, rename = "parse-depth")]
    parse_depth: Option<usize>,
    #[serde(default, rename = "inline-source", alias = "source", alias = "string")]
    inline_source: Option<String>,
    #[serde(default, rename = "write-mode", alias = "write")]
    write_mode: Option<String>,
    #[serde(default)]
    verify: bool,
    #[serde(default, rename = "report-mode", alias = "report")]
    report_mode: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct SetMappingConfig {
    xpath: String,
    value: String,
    #[serde(default, rename = "value-kind", alias = "kind", alias = "type")]
    value_kind: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct QueryConfig {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default, rename = "diff-files")]
    diff_files: Option<String>,
    #[serde(default, rename = "diff-lines")]
    diff_lines: Option<String>,
    #[serde(default)]
    queries: Vec<QueryExprConfig>,
    #[serde(default, rename = "tree-mode")]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct QueryExprConfig {
    xpath: NormalizedXpath,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct TestConfig {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default, rename = "diff-files")]
    diff_files: Option<String>,
    #[serde(default, rename = "diff-lines")]
    diff_lines: Option<String>,
    #[serde(default)]
    assertions: Vec<TestAssertionConfig>,
    #[serde(default, rename = "tree-mode")]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct TestAssertionConfig {
    xpath: NormalizedXpath,
    #[serde(default = "default_expect")]
    expect: String,
}

fn default_expect() -> String {
    "some".to_string()
}

fn default_severity() -> String {
    "error".to_string()
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

fn parse_severity(s: &str) -> Result<Severity, String> {
    match s {
        "error" => Ok(Severity::Error),
        "warning" => Ok(Severity::Warning),
        other => Err(format!("invalid severity '{}': use 'error' or 'warning'", other)),
    }
}

fn parse_tree_mode(s: &str) -> Result<TreeMode, String> {
    match s {
        "raw" => Ok(TreeMode::Raw),
        "structure" => Ok(TreeMode::Structure),
        "data" => Ok(TreeMode::Data),
        other => Err(format!(
            "invalid tree-mode '{}': use 'raw', 'structure', or 'data'",
            other
        )),
    }
}

fn parse_set_write_mode(s: &str) -> Result<SetWriteMode, String> {
    match s {
        "in-place" | "inplace" | "write" => Ok(SetWriteMode::InPlace),
        "verify" => Ok(SetWriteMode::Verify),
        "capture" | "stdout" => Ok(SetWriteMode::Capture),
        other => Err(format!(
            "invalid set write mode '{}': use 'in-place', 'verify', or 'capture'",
            other
        )),
    }
}

fn parse_set_report_mode(s: &str) -> Result<SetReportMode, String> {
    match s {
        "per-match" | "match" | "matches" => Ok(SetReportMode::PerMatch),
        "per-file" | "file" | "files" => Ok(SetReportMode::PerFile),
        other => Err(format!(
            "invalid set report mode '{}': use 'per-match' or 'per-file'",
            other
        )),
    }
}

/// Root-level scope fields that constrain all operations.
/// Note: `files` is handled separately via `LoadedConfig.root_files` and
/// `SharedFileScope` — it's not part of the per-operation merge.
#[derive(Debug, Clone, Default)]
struct RootScope {
    exclude: Vec<String>,
    diff_files: Option<String>,
    diff_lines: Option<String>,
}

// ---------------------------------------------------------------------------
// ConfigOperation — parsed operation with its per-op input resolution data
// ---------------------------------------------------------------------------

/// Per-operation input-resolution data that lives on the config side until
/// the `FileResolver` runs. Mirrors the fields that used to live directly
/// on each `Operation*` struct: files, exclude, diff-files/lines, language
/// override, inline source. The runner in `cli/config.rs` consumes these
/// to drive `FileResolver::resolve`, then hands `sources`/`filters` into
/// the final operation.
#[derive(Debug, Clone, Default)]
pub struct OperationInputs {
    pub files: Vec<String>,
    pub exclude: Vec<String>,
    pub diff_files: Option<String>,
    pub diff_lines: Option<String>,
    /// Language override for disk sources (rule/operation-level).
    pub language: Option<String>,
    /// Inline content declared in config (e.g. set operation with
    /// `inline-source:` key). None means no inline source at config level;
    /// the CLI may still attach one via the shared resolver flow.
    pub inline_source: Option<Source>,
}

/// A check operation as it exists in config-land — pre-compilation.
///
/// Globs are still raw strings here because the base directory they
/// should resolve against isn't known until the CLI layer determines
/// `base_dir` from the config file's location. Compiled into a
/// [`crate::executor::CheckOperation`] by the input planner once the
/// shared `FileResolver` has resolved the op's sources and filters.
#[derive(Debug, Clone)]
pub struct CheckDraft {
    pub rules: Vec<Rule>,
    pub ruleset_include: Vec<String>,
    pub ruleset_exclude: Vec<String>,
    pub ruleset_default_language: Option<String>,
    pub tree_mode: Option<TreeMode>,
    pub ignore_whitespace: bool,
    pub parse_depth: Option<usize>,
}

/// A config-sourced operation, paired with the per-op input-resolution data
/// that the runner needs to call `FileResolver::resolve`. Every variant
/// carries a draft — the op-specific metadata only, with no
/// `sources`/`filters` placeholder state. The planner hands resolved
/// inputs to `OperationDraft::into_operation` to produce the final
/// executor-ready `Operation`. The Check variant additionally defers
/// glob compilation until `base_dir` is known (see [`CheckDraft`]).
#[derive(Debug, Clone)]
pub enum ConfigOperation {
    Check { inputs: OperationInputs, draft: CheckDraft },
    Set { inputs: OperationInputs, draft: SetDraft },
    Query { inputs: OperationInputs, draft: QueryDraft },
    Test { inputs: OperationInputs, draft: TestDraft },
}

impl ConfigOperation {
    /// Split the config-side data into the two parts the input planner needs:
    /// the per-op `OperationInputs` (what files/filters the op asked for) and
    /// the `OperationDraft` (everything else the executor needs). The planner
    /// calls the resolver with `OperationInputs`, then hands the resolved
    /// sources/filters back to `OperationDraft::into_operation`.
    pub fn into_draft(self) -> (OperationInputs, crate::input::plan::OperationDraft) {
        use crate::input::plan::OperationDraft;
        match self {
            ConfigOperation::Check { inputs, draft } => (inputs, OperationDraft::Check(draft)),
            ConfigOperation::Set { inputs, draft } => (inputs, OperationDraft::Set(draft)),
            ConfigOperation::Query { inputs, draft } => (inputs, OperationDraft::Query(draft)),
            ConfigOperation::Test { inputs, draft } => (inputs, OperationDraft::Test(draft)),
        }
    }

    /// Mutable borrow — used by the CLI runner to attach a CLI-provided
    /// inline source to config-loaded operations that accept one.
    pub fn inputs_mut(&mut self) -> &mut OperationInputs {
        match self {
            ConfigOperation::Check { inputs, .. }
            | ConfigOperation::Set { inputs, .. }
            | ConfigOperation::Query { inputs, .. }
            | ConfigOperation::Test { inputs, .. } => inputs,
        }
    }

    /// Predicate-style accessor for the existing `op_filter` API in
    /// `ConfigRunParams`. Returns an `Operation`-shaped token suitable for
    /// matches!() checks without actually materialising the operation.
    pub fn kind(&self) -> ConfigOperationKind {
        match self {
            ConfigOperation::Check { .. } => ConfigOperationKind::Check,
            ConfigOperation::Set { .. } => ConfigOperationKind::Set,
            ConfigOperation::Query { .. } => ConfigOperationKind::Query,
            ConfigOperation::Test { .. } => ConfigOperationKind::Test,
        }
    }
}

/// Lightweight operation-kind marker used by `op_filter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigOperationKind {
    Check,
    Set,
    Query,
    Test,
}

fn convert_check(config: CheckConfig, scope: &RootScope) -> Result<ConfigOperation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let rules: Vec<Rule> = config.rules.into_iter().map(|r| {
        let severity = parse_severity(&r.severity)?;
        let rule_tree_mode = r.tree_mode.as_deref().map(parse_tree_mode).transpose()?;
        let mut rule = Rule::new(r.id, r.xpath).with_severity(severity);
        if let Some(reason) = r.reason {
            rule = rule.with_reason(reason);
        }
        if let Some(message) = r.message {
            rule = rule.with_message(message);
        }
        if !r.include.is_empty() {
            rule = rule.with_include(r.include);
        }
        if !r.exclude.is_empty() {
            rule = rule.with_exclude(r.exclude);
        }
        if let Some(lang) = r.language {
            rule = rule.with_language(lang);
        }
        if let Some(tm) = rule_tree_mode {
            rule = rule.with_tree_mode(tm);
        }
        let valid_examples: Vec<String> = r.expect.iter().filter_map(|e| e.valid.clone()).collect();
        let invalid_examples: Vec<String> = r.expect.iter().filter_map(|e| e.invalid.clone()).collect();
        if !valid_examples.is_empty() {
            rule = rule.with_valid_examples(valid_examples);
        }
        if !invalid_examples.is_empty() {
            rule = rule.with_invalid_examples(invalid_examples);
        }
        Ok::<Rule, Box<dyn std::error::Error>>(rule)
    }).collect::<Result<_, _>>()?;

    let (files, exclude, diff_files, diff_lines) = merge_scope(scope, config.files, config.exclude, config.diff_files, config.diff_lines);

    let inputs = OperationInputs {
        files,
        exclude,
        diff_files,
        diff_lines,
        language: config.language.clone(),
        inline_source: None,
    };

    // Draft — per-rule globs stay as raw strings until `base_dir` is
    // known in the CLI layer; `into_operation` compiles them into a
    // ready-to-match `Vec<CompiledRule>`.
    let draft = CheckDraft {
        rules,
        ruleset_include: vec![],
        ruleset_exclude: vec![],
        ruleset_default_language: config.language,
        tree_mode,
        ignore_whitespace: false,
        parse_depth: None,
    };

    Ok(ConfigOperation::Check { inputs, draft })
}

fn selector_xpath(expr: &str) -> String {
    if expr.starts_with('/') {
        expr.to_string()
    } else {
        format!("//{}", expr)
    }
}

fn normalize_set_expression(
    expr: &str,
    explicit_value: Option<&str>,
) -> Result<Vec<SetMapping>, Box<dyn std::error::Error>> {
    if let Some(value) = explicit_value {
        return Ok(vec![SetMapping {
            xpath: selector_xpath(expr),
            value: value.to_string(),
            value_kind: Some("string".to_string()),
        }]);
    }

    Ok(parse_set_expr(expr)?.into_iter().map(|op| SetMapping {
        xpath: op.xpath,
        value: op.value.text().to_string(),
        value_kind: Some(op.value.kind().to_string()),
    }).collect())
}

fn convert_set(config: SetConfig, scope: &RootScope) -> Result<ConfigOperation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let mut mappings = config.mappings.into_iter().map(|m| {
        SetMapping {
            xpath: m.xpath,
            value: m.value,
            value_kind: m.value_kind,
        }
    }).collect::<Vec<_>>();

    if let Some(ref expr) = config.expression {
        mappings.extend(normalize_set_expression(expr, config.value.as_deref())?);
    }
    if mappings.is_empty() {
        return Err("set operation requires either mappings or an expression".into());
    }

    let write_mode = if let Some(mode) = config.write_mode.as_deref() {
        parse_set_write_mode(mode)?
    } else if config.verify {
        SetWriteMode::Verify
    } else if config.inline_source.is_some() {
        SetWriteMode::Capture
    } else {
        SetWriteMode::InPlace
    };
    let report_mode = config.report_mode.as_deref()
        .map(parse_set_report_mode)
        .transpose()?
        .unwrap_or(SetReportMode::PerMatch);

    let (files, exclude, diff_files, diff_lines) = merge_scope(scope, config.files, config.exclude, config.diff_files, config.diff_lines);

    // Config-declared inline source becomes a pathless virtual source. The
    // language override doubles as the Source's language (the input boundary
    // normally resolves this, but config inputs arrive post-boundary).
    let inline_source = if let Some(content) = config.inline_source {
        let lang = config.language.as_deref()
            .ok_or("set operation with inline source requires `language`")?;
        Some(crate::input::Source::inline_pathless(
            lang,
            std::sync::Arc::new(content),
        ))
    } else {
        None
    };

    let inputs = OperationInputs {
        files,
        exclude,
        diff_files,
        diff_lines,
        language: config.language,
        inline_source,
    };

    let draft = SetDraft {
        mappings,
        tree_mode,
        limit: config.limit,
        ignore_whitespace: config.ignore_whitespace,
        write_mode,
        report_mode,
    };

    Ok(ConfigOperation::Set { inputs, draft })
}

fn convert_query(config: QueryConfig, scope: &RootScope) -> Result<ConfigOperation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let queries = config.queries.into_iter().map(|q| {
        QueryExpr { xpath: q.xpath }
    }).collect();

    let (files, exclude, diff_files, diff_lines) = merge_scope(scope, config.files, config.exclude, config.diff_files, config.diff_lines);

    let inputs = OperationInputs {
        files,
        exclude,
        diff_files,
        diff_lines,
        language: config.language.clone(),
        inline_source: None,
    };

    let draft = QueryDraft {
        queries,
        tree_mode,
        language: config.language,
        limit: config.limit,
        ignore_whitespace: false,
        parse_depth: None,
    };

    Ok(ConfigOperation::Query { inputs, draft })
}

fn convert_test(config: TestConfig, scope: &RootScope) -> Result<ConfigOperation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let assertions = config.assertions.into_iter().map(|a| {
        TestAssertion {
            xpath: a.xpath,
            expect: a.expect,
        }
    }).collect();

    let (files, exclude, diff_files, diff_lines) = merge_scope(scope, config.files, config.exclude, config.diff_files, config.diff_lines);

    let inputs = OperationInputs {
        files,
        exclude,
        diff_files,
        diff_lines,
        language: config.language.clone(),
        inline_source: None,
    };

    let draft = TestDraft {
        assertions,
        tree_mode,
        language: config.language,
        limit: config.limit,
        ignore_whitespace: false,
        parse_depth: None,
    };

    Ok(ConfigOperation::Test { inputs, draft })
}

/// Merge root-level scope with per-operation scope.
///
/// - `files`: operation keeps its own files (empty if not specified).
///   Root-level files are handled separately via `SharedFileScope` at
///   resolve time — intersection when both exist, root as fallback when
///   the operation has none.
/// - `exclude`: union of root and operation excludes (both narrow the scope).
/// - `diff-files`/`diff-lines`: operation takes precedence; root is the
///   fallback. CLI flags are applied separately via the file resolver.
fn merge_scope(
    scope: &RootScope,
    op_files: Vec<String>,
    op_exclude: Vec<String>,
    op_diff_files: Option<String>,
    op_diff_lines: Option<String>,
) -> (Vec<String>, Vec<String>, Option<String>, Option<String>) {
    let mut exclude = scope.exclude.clone();
    exclude.extend(op_exclude);

    let diff_files = op_diff_files.or_else(|| scope.diff_files.clone());
    let diff_lines = op_diff_lines.or_else(|| scope.diff_lines.clone());

    (op_files, exclude, diff_files, diff_lines)
}

fn config_to_operations(config: ConfigFile) -> Result<LoadedConfig, Box<dyn std::error::Error>> {
    let root_files = config.files;

    let scope = RootScope {
        exclude: config.exclude,
        diff_files: config.diff_files,
        diff_lines: config.diff_lines,
    };

    let mut ops = Vec::new();

    // Root-level shorthand keys first
    if let Some(check) = config.check {
        ops.push(convert_check(check, &scope)?);
    }
    if let Some(set) = config.set {
        ops.push(convert_set(set, &scope)?);
    }
    if let Some(query) = config.query {
        ops.push(convert_query(query, &scope)?);
    }
    if let Some(test) = config.test {
        ops.push(convert_test(test, &scope)?);
    }

    // Then explicit operations list
    for entry in config.operations {
        if let Some(c) = entry.check {
            ops.push(convert_check(c, &scope)?);
        }
        if let Some(s) = entry.set {
            ops.push(convert_set(s, &scope)?);
        }
        if let Some(q) = entry.query {
            ops.push(convert_query(q, &scope)?);
        }
        if let Some(t) = entry.test {
            ops.push(convert_test(t, &scope)?);
        }
    }

    Ok(LoadedConfig {
        root_files,
        operations: ops,
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parsed config with root-level file scope kept separate from operations.
///
/// Root-level `files` are intersected with each operation's files at resolve
/// time, so they must be preserved independently rather than merged away.
pub struct LoadedConfig {
    /// Root-level file glob patterns that constrain all operations.
    /// `None` when the key is missing (unrestricted); `Some(vec![])` when
    /// explicitly empty.
    pub root_files: Option<Vec<String>>,
    /// Parsed operations paired with their per-op input-resolution data.
    /// Sources/filters are filled in by the runner once the shared
    /// `FileResolver` has resolved each operation's file set.
    pub operations: Vec<ConfigOperation>,
}

impl std::fmt::Debug for LoadedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedConfig")
            .field("root_files", &self.root_files)
            .field("operations", &self.operations)
            .finish()
    }
}

/// Parse a tractor config file into a `LoadedConfig`.
/// Format is detected from the file extension.
pub fn load_tractor_config(path: &Path) -> Result<LoadedConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;

    match path.extension().and_then(|e| e.to_str()) {
        Some("yml") | Some("yaml") => parse_config_yaml(&content),
        Some("toml") => parse_config_toml(&content),
        Some(ext) => Err(format!(
            "unsupported config file extension '.{}': use .yaml, .yml, or .toml",
            ext
        ).into()),
        None => Err("config file has no extension: use .yaml, .yml, or .toml".into()),
    }
}

/// Parse a tractor config from a YAML string.
pub fn parse_config_yaml(content: &str) -> Result<LoadedConfig, Box<dyn std::error::Error>> {
    let config: ConfigFile = serde_yaml::from_str(content)
        .map_err(|e| format!("invalid tractor config YAML: {}", e))?;
    config_to_operations(config)
}

/// Parse a tractor config from a TOML string.
pub fn parse_config_toml(content: &str) -> Result<LoadedConfig, Box<dyn std::error::Error>> {
    let config: ConfigFile = toml::from_str(content)
        .map_err(|e| format!("invalid tractor config TOML: {}", e))?;
    config_to_operations(config)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Unwrap a `ConfigOperation::Check` into (`&OperationInputs`, `&CheckDraft`).
    fn as_check(op: &ConfigOperation) -> (&OperationInputs, &CheckDraft) {
        match op {
            ConfigOperation::Check { inputs, draft } => (inputs, draft),
            _ => panic!("expected Check operation"),
        }
    }
    fn as_set(op: &ConfigOperation) -> (&OperationInputs, &SetDraft) {
        match op {
            ConfigOperation::Set { inputs, draft } => (inputs, draft),
            _ => panic!("expected Set operation"),
        }
    }
    fn as_query(op: &ConfigOperation) -> (&OperationInputs, &QueryDraft) {
        match op {
            ConfigOperation::Query { inputs, draft } => (inputs, draft),
            _ => panic!("expected Query operation"),
        }
    }
    fn as_test(op: &ConfigOperation) -> (&OperationInputs, &TestDraft) {
        match op {
            ConfigOperation::Test { inputs, draft } => (inputs, draft),
            _ => panic!("expected Test operation"),
        }
    }

    #[test]
    fn parse_yaml_root_level_check() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO found"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 1);
        let (inputs, c) = as_check(&ops[0]);
        assert_eq!(inputs.files, vec!["src/**/*.rs"]);
        assert_eq!(c.rules.len(), 1);
        assert_eq!(c.rules[0].id, "no-todo");
        assert_eq!(c.rules[0].reason.as_deref(), Some("TODO found"));
    }

    #[test]
    fn parse_yaml_root_level_set() {
        let yaml = r#"
set:
  files: ["config.json"]
  mappings:
    - xpath: "//database/host"
      value: "localhost"
    - xpath: "//database/port"
      value: "5432"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 1);
        let (inputs, s) = as_set(&ops[0]);
        assert_eq!(inputs.files, vec!["config.json"]);
        assert_eq!(s.mappings.len(), 2);
        assert_eq!(s.mappings[0].xpath, "//database/host");
        assert_eq!(s.mappings[0].value, "localhost");
        assert_eq!(s.mappings[1].xpath, "//database/port");
        assert_eq!(s.mappings[1].value, "5432");
    }

    #[test]
    fn parse_yaml_both_root_level_keys() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
set:
  files: ["config.json"]
  mappings:
    - xpath: "//host"
      value: "localhost"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], ConfigOperation::Check { .. }));
        assert!(matches!(&ops[1], ConfigOperation::Set { .. }));
    }

    #[test]
    fn parse_yaml_operations_list() {
        let yaml = r#"
operations:
  - check:
      files: ["src/**/*.rs"]
      rules:
        - id: rule-a
          xpath: "//a"
  - set:
      files: ["config.json"]
      mappings:
        - xpath: "//host"
          value: "localhost"
  - check:
      files: ["test/**/*.rs"]
      rules:
        - id: rule-b
          xpath: "//b"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 3);
        assert!(matches!(&ops[0], ConfigOperation::Check { .. }));
        assert!(matches!(&ops[1], ConfigOperation::Set { .. }));
        assert!(matches!(&ops[2], ConfigOperation::Check { .. }));

        // Verify ordering is preserved
        let (_, c0) = as_check(&ops[0]);
        assert_eq!(c0.rules[0].id, "rule-a");
        let (_, c2) = as_check(&ops[2]);
        assert_eq!(c2.rules[0].id, "rule-b");
    }

    #[test]
    fn parse_yaml_root_plus_operations() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: root-check
      xpath: "//a"
operations:
  - set:
      files: ["config.json"]
      mappings:
        - xpath: "//host"
          value: "localhost"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 2);
        // Root-level check comes first
        assert!(matches!(&ops[0], ConfigOperation::Check { .. }));
        // Then operations list
        assert!(matches!(&ops[1], ConfigOperation::Set { .. }));
    }

    #[test]
    fn parse_yaml_check_with_severity() {
        let yaml = r#"
check:
  files: ["**/*.rs"]
  rules:
    - id: warn-rule
      xpath: "//x"
      severity: warning
    - id: error-rule
      xpath: "//y"
      severity: error
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, c) = as_check(&ops[0]);
        assert_eq!(c.rules[0].severity, Severity::Warning);
        assert_eq!(c.rules[1].severity, Severity::Error);
    }

    #[test]
    fn parse_yaml_set_with_exclude() {
        let yaml = r#"
set:
  files: ["**/*.json"]
  exclude: ["node_modules/**"]
  mappings:
    - xpath: "//version"
      value: "2.0"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (inputs, _) = as_set(&ops[0]);
        assert_eq!(inputs.exclude, vec!["node_modules/**"]);
    }

    #[test]
    fn parse_yaml_set_expression_into_typed_mappings() {
        let yaml = r#"
set:
  files: ["config.yaml"]
  expression: "database[host='db.example.com'][port=5432]"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, s) = as_set(&ops[0]);
        assert_eq!(s.mappings.len(), 2);
        assert_eq!(s.mappings[0].xpath, "//database/host");
        assert_eq!(s.mappings[0].value, "db.example.com");
        assert_eq!(s.mappings[0].value_kind.as_deref(), Some("string"));
        assert_eq!(s.mappings[1].xpath, "//database/port");
        assert_eq!(s.mappings[1].value, "5432");
        assert_eq!(s.mappings[1].value_kind.as_deref(), Some("number"));
        assert_eq!(s.write_mode, SetWriteMode::InPlace);
        assert_eq!(s.report_mode, SetReportMode::PerMatch);
    }

    #[test]
    fn parse_yaml_set_capture_inline_source() {
        let yaml = r#"
set:
  language: yaml
  inline-source: "database:\n  host: localhost\n"
  expression: "database[host='db.example.com']"
  write: capture
  report: per-file
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (inputs, s) = as_set(&ops[0]);
        assert_eq!(inputs.language.as_deref(), Some("yaml"));
        assert!(inputs.inline_source.is_some());
        assert_eq!(s.write_mode, SetWriteMode::Capture);
        assert_eq!(s.report_mode, SetReportMode::PerFile);
    }

    #[test]
    fn parse_yaml_set_expression_with_value_preserves_predicates() {
        let yaml = r#"
set:
  files: ["config.yaml"]
  expression: "servers[host='localhost']/port"
  value: "5433"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, s) = as_set(&ops[0]);
        assert_eq!(s.mappings.len(), 1);
        assert_eq!(s.mappings[0].xpath, "//servers[host='localhost']/port");
        assert_eq!(s.mappings[0].value, "5433");
        assert_eq!(s.mappings[0].value_kind.as_deref(), Some("string"));
    }

    #[test]
    fn parse_yaml_empty() {
        let yaml = "{}";
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert!(ops.is_empty());
    }

    #[test]
    fn parse_yaml_invalid_severity() {
        let yaml = r#"
check:
  rules:
    - id: bad
      xpath: "//x"
      severity: critical
"#;
        let err = parse_config_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("invalid severity"));
    }

    #[test]
    fn parse_yaml_root_level_query() {
        let yaml = r#"
query:
  files: ["src/**/*.rs"]
  queries:
    - xpath: "//function"
    - xpath: "//class"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 1);
        let (inputs, q) = as_query(&ops[0]);
        assert_eq!(inputs.files, vec!["src/**/*.rs"]);
        assert_eq!(q.queries.len(), 2);
        assert_eq!(q.queries[0].xpath, "//function");
        assert_eq!(q.queries[1].xpath, "//class");
    }

    #[test]
    fn parse_yaml_root_level_test() {
        let yaml = r#"
test:
  files: ["src/**/*.rs"]
  assertions:
    - xpath: "//function"
      expect: some
    - xpath: "//class"
      expect: none
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 1);
        let (inputs, t) = as_test(&ops[0]);
        assert_eq!(inputs.files, vec!["src/**/*.rs"]);
        assert_eq!(t.assertions.len(), 2);
        assert_eq!(t.assertions[0].xpath, "//function");
        assert_eq!(t.assertions[0].expect, "some");
        assert_eq!(t.assertions[1].xpath, "//class");
        assert_eq!(t.assertions[1].expect, "none");
    }

    #[test]
    fn parse_yaml_test_default_expect() {
        let yaml = r#"
test:
  files: ["*.json"]
  assertions:
    - xpath: "//name"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, t) = as_test(&ops[0]);
        assert_eq!(t.assertions[0].expect, "some");
    }

    #[test]
    fn parse_yaml_operations_with_query_and_test() {
        let yaml = r#"
operations:
  - query:
      files: ["*.json"]
      queries:
        - xpath: "//name"
  - test:
      files: ["*.json"]
      assertions:
        - xpath: "//name"
          expect: some
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], ConfigOperation::Query { .. }));
        assert!(matches!(&ops[1], ConfigOperation::Test { .. }));
    }

    #[test]
    fn parse_toml_root_level() {
        let toml = r#"
[set]
files = ["config.json"]

[[set.mappings]]
xpath = "//host"
value = "localhost"
"#;
        let ops = parse_config_toml(toml).unwrap().operations;
        assert_eq!(ops.len(), 1);
        let (_, s) = as_set(&ops[0]);
        assert_eq!(s.mappings.len(), 1);
        assert_eq!(s.mappings[0].value, "localhost");
    }

    // -----------------------------------------------------------------------
    // Root-level scope tests
    // -----------------------------------------------------------------------

    #[test]
    fn root_files_not_merged_into_operation() {
        // Root files are kept in LoadedConfig.root_files and applied at
        // resolve time via SharedFileScope — not copied into operations.
        let yaml = r#"
files: ["src/**/*.rs"]
check:
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        assert_eq!(loaded.root_files, Some(vec!["src/**/*.rs".to_string()]));
        let (inputs, _) = as_check(&loaded.operations[0]);
        assert!(inputs.files.is_empty(), "operation should have no files when not specified");
    }

    #[test]
    fn operation_files_kept_independently_from_root() {
        let yaml = r#"
files: ["src/**/*.rs"]
check:
  files: ["test/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        assert_eq!(loaded.root_files, Some(vec!["src/**/*.rs".to_string()]));
        let (inputs, _) = as_check(&loaded.operations[0]);
        assert_eq!(inputs.files, vec!["test/**/*.rs"]);
    }

    #[test]
    fn root_exclude_merged_with_operation_exclude() {
        let yaml = r#"
exclude: ["target/**"]
check:
  files: ["src/**/*.rs"]
  exclude: ["src/generated/**"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (inputs, _) = as_check(&ops[0]);
        assert_eq!(inputs.exclude, vec!["target/**", "src/generated/**"]);
    }

    #[test]
    fn root_diff_files_inherited_by_operations() {
        let yaml = r#"
diff-files: "main..HEAD"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (inputs, _) = as_check(&ops[0]);
        assert_eq!(inputs.diff_files.as_deref(), Some("main..HEAD"));
    }

    #[test]
    fn operation_diff_files_overrides_root() {
        let yaml = r#"
diff-files: "main..HEAD"
check:
  files: ["src/**/*.rs"]
  diff-files: "HEAD~3"
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (inputs, _) = as_check(&ops[0]);
        assert_eq!(inputs.diff_files.as_deref(), Some("HEAD~3"));
    }

    #[test]
    fn root_diff_lines_inherited_by_operations() {
        let yaml = r#"
diff-lines: "main..HEAD"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (inputs, _) = as_check(&ops[0]);
        assert_eq!(inputs.diff_lines.as_deref(), Some("main..HEAD"));
    }

    #[test]
    fn root_scope_applies_to_operations_list() {
        let yaml = r#"
files: ["src/**/*.rs"]
exclude: ["vendor/**"]
diff-files: "main..HEAD"
operations:
  - check:
      rules:
        - id: rule-a
          xpath: "//a"
  - query:
      queries:
        - xpath: "//b"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        let ops = &loaded.operations;
        assert_eq!(ops.len(), 2);
        assert_eq!(loaded.root_files, Some(vec!["src/**/*.rs".to_string()]));

        // Operations have no files of their own — root files are applied
        // at resolve time via FileResolver.
        let (check_inputs, _) = as_check(&ops[0]);
        assert!(check_inputs.files.is_empty());
        assert_eq!(check_inputs.exclude, vec!["vendor/**"]);
        assert_eq!(check_inputs.diff_files.as_deref(), Some("main..HEAD"));

        let (query_inputs, _) = as_query(&ops[1]);
        assert!(query_inputs.files.is_empty());
        assert_eq!(query_inputs.exclude, vec!["vendor/**"]);
        assert_eq!(query_inputs.diff_files.as_deref(), Some("main..HEAD"));
    }

    #[test]
    fn loaded_config_root_files_populated() {
        let yaml = r#"
files: ["src/**/*.rs", "lib/**/*.rs"]
check:
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        assert_eq!(loaded.root_files, Some(vec!["src/**/*.rs".to_string(), "lib/**/*.rs".to_string()]));
    }

    #[test]
    fn loaded_config_root_files_none_when_key_missing() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        assert!(loaded.root_files.is_none());
    }

    #[test]
    fn loaded_config_root_files_some_empty_when_explicit_empty() {
        let yaml = r#"
files: []
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        assert_eq!(loaded.root_files, Some(vec![]));
    }

    #[test]
    fn loaded_config_root_files_preserved_alongside_op_files() {
        let yaml = r#"
files: ["src/**/*.rs"]
check:
  files: ["test/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        // Root files are preserved for intersection at resolve time
        assert_eq!(loaded.root_files, Some(vec!["src/**/*.rs".to_string()]));
        // Operation files are kept as-is (merge_scope overrides at parse time;
        // actual intersection happens in FileResolver::resolve_files)
        let (inputs, _) = as_check(&loaded.operations[0]);
        assert_eq!(inputs.files, vec!["test/**/*.rs"]);
    }

    #[test]
    fn parse_yaml_check_with_expect_examples() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      expect:
        - valid: "fn main() {}"
        - invalid: "// TODO: fix"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, c) = as_check(&ops[0]);
        assert_eq!(c.rules[0].valid_examples, vec!["fn main() {}"]);
        assert_eq!(c.rules[0].invalid_examples, vec!["// TODO: fix"]);
    }

    // -- XPath normalization (implicit // prefix) --

    #[test]
    fn bare_xpath_normalized_in_check_rules() {
        let yaml = r#"
check:
  files: ["*.json"]
  rules:
    - id: has-debug
      xpath: "debug"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, c) = as_check(&ops[0]);
        assert_eq!(c.rules[0].xpath, "//debug");
    }

    #[test]
    fn bare_xpath_normalized_in_query() {
        let yaml = r#"
query:
  files: ["*.json"]
  queries:
    - xpath: "debug"
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, q) = as_query(&ops[0]);
        assert_eq!(q.queries[0].xpath, "//debug");
    }

    #[test]
    fn bare_xpath_normalized_in_test() {
        let yaml = r#"
test:
  files: ["*.json"]
  assertions:
    - xpath: "debug"
      expect: 1
"#;
        let ops = parse_config_yaml(yaml).unwrap().operations;
        let (_, t) = as_test(&ops[0]);
        assert_eq!(t.assertions[0].xpath, "//debug");
    }
}
