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
use tractor_core::report::Severity;
use tractor_core::rule::Rule;
use tractor_core::tree_mode::TreeMode;

use crate::executor::{
    CheckOperation, Operation, QueryExpr, QueryOperation,
    SetMapping, SetOperation, TestAssertion, TestOperation,
};

// ---------------------------------------------------------------------------
// Serde schema
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    /// Root-level file scope: glob patterns that constrain all operations.
    /// Intersected with each operation's own `files`.
    #[serde(default)]
    files: Vec<String>,

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
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct CheckRuleConfig {
    id: String,
    xpath: String,
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
    mappings: Vec<SetMappingConfig>,
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct SetMappingConfig {
    xpath: String,
    value: String,
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
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct QueryExprConfig {
    xpath: String,
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
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct TestAssertionConfig {
    xpath: String,
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
            "invalid tree_mode '{}': use 'raw', 'structure', or 'data'",
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

fn convert_check(config: CheckConfig, scope: &RootScope) -> Result<Operation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let rules: Vec<Rule> = config.rules.into_iter().map(|r| {
        let severity = parse_severity(&r.severity)?;
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

    Ok(Operation::Check(CheckOperation {
        files,
        exclude,
        diff_files,
        diff_lines,
        rules,
        tree_mode,
        language: config.language,
        ignore_whitespace: false,
        parse_depth: None,
        ruleset_include: vec![],
        ruleset_exclude: vec![],
    }))
}

fn convert_set(config: SetConfig, scope: &RootScope) -> Result<Operation, Box<dyn std::error::Error>> {
    // Validate tree_mode if provided (even though set doesn't use it yet)
    if let Some(ref tm) = config.tree_mode {
        parse_tree_mode(tm)?;
    }

    let mappings = config.mappings.into_iter().map(|m| {
        SetMapping {
            xpath: m.xpath,
            value: m.value,
        }
    }).collect();

    let (files, exclude, diff_files, diff_lines) = merge_scope(scope, config.files, config.exclude, config.diff_files, config.diff_lines);

    Ok(Operation::Set(SetOperation {
        files,
        exclude,
        diff_files,
        diff_lines,
        mappings,
        language: config.language,
        verify: false,
    }))
}

fn convert_query(config: QueryConfig, scope: &RootScope) -> Result<Operation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let queries = config.queries.into_iter().map(|q| {
        QueryExpr { xpath: q.xpath }
    }).collect();

    let (files, exclude, diff_files, diff_lines) = merge_scope(scope, config.files, config.exclude, config.diff_files, config.diff_lines);

    Ok(Operation::Query(QueryOperation {
        files,
        exclude,
        diff_files,
        diff_lines,
        queries,
        tree_mode,
        language: config.language,
        limit: config.limit,
        ignore_whitespace: false,
        parse_depth: None,
        inline_source: None,
        inline_lang: None,
    }))
}

fn convert_test(config: TestConfig, scope: &RootScope) -> Result<Operation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let assertions = config.assertions.into_iter().map(|a| {
        TestAssertion {
            xpath: a.xpath,
            expect: a.expect,
        }
    }).collect();

    let (files, exclude, diff_files, diff_lines) = merge_scope(scope, config.files, config.exclude, config.diff_files, config.diff_lines);

    Ok(Operation::Test(TestOperation {
        files,
        exclude,
        diff_files,
        diff_lines,
        assertions,
        tree_mode,
        language: config.language,
        limit: config.limit,
        ignore_whitespace: false,
        parse_depth: None,
        inline_source: None,
        inline_lang: None,
    }))
}

/// Merge root-level scope with per-operation scope.
///
/// - `files`: operation keeps its own files (empty if not specified).
///   Root-level files are handled separately via `SharedFileScope` at
///   resolve time — intersection when both exist, root as fallback when
///   the operation has none.
/// - `exclude`: union of root and operation excludes (both narrow the scope).
/// - `diff-files`/`diff-lines`: operation takes precedence; root is the
///   fallback. CLI flags are applied separately via `ExecuteOptions`.
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
    let root_files = config.files.clone();

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
#[derive(Debug)]
pub struct LoadedConfig {
    /// Root-level file glob patterns that constrain all operations.
    pub root_files: Vec<String>,
    /// Parsed operations (with their own files, excludes, etc.).
    pub operations: Vec<Operation>,
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
        match &ops[0] {
            Operation::Check(c) => {
                assert_eq!(c.files, vec!["src/**/*.rs"]);
                assert_eq!(c.rules.len(), 1);
                assert_eq!(c.rules[0].id, "no-todo");
                assert_eq!(c.rules[0].reason.as_deref(), Some("TODO found"));
            }
            _ => panic!("expected Check operation"),
        }
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
        match &ops[0] {
            Operation::Set(s) => {
                assert_eq!(s.files, vec!["config.json"]);
                assert_eq!(s.mappings.len(), 2);
                assert_eq!(s.mappings[0].xpath, "//database/host");
                assert_eq!(s.mappings[0].value, "localhost");
                assert_eq!(s.mappings[1].xpath, "//database/port");
                assert_eq!(s.mappings[1].value, "5432");
            }
            _ => panic!("expected Set operation"),
        }
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
        assert!(matches!(&ops[0], Operation::Check(_)));
        assert!(matches!(&ops[1], Operation::Set(_)));
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
        assert!(matches!(&ops[0], Operation::Check(_)));
        assert!(matches!(&ops[1], Operation::Set(_)));
        assert!(matches!(&ops[2], Operation::Check(_)));

        // Verify ordering is preserved
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.rules[0].id, "rule-a");
        }
        if let Operation::Check(c) = &ops[2] {
            assert_eq!(c.rules[0].id, "rule-b");
        }
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
        assert!(matches!(&ops[0], Operation::Check(_)));
        // Then operations list
        assert!(matches!(&ops[1], Operation::Set(_)));
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
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.rules[0].severity, Severity::Warning);
            assert_eq!(c.rules[1].severity, Severity::Error);
        }
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
        if let Operation::Set(s) = &ops[0] {
            assert_eq!(s.exclude, vec!["node_modules/**"]);
        }
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
        match &ops[0] {
            Operation::Query(q) => {
                assert_eq!(q.files, vec!["src/**/*.rs"]);
                assert_eq!(q.queries.len(), 2);
                assert_eq!(q.queries[0].xpath, "//function");
                assert_eq!(q.queries[1].xpath, "//class");
            }
            _ => panic!("expected Query operation"),
        }
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
        match &ops[0] {
            Operation::Test(t) => {
                assert_eq!(t.files, vec!["src/**/*.rs"]);
                assert_eq!(t.assertions.len(), 2);
                assert_eq!(t.assertions[0].xpath, "//function");
                assert_eq!(t.assertions[0].expect, "some");
                assert_eq!(t.assertions[1].xpath, "//class");
                assert_eq!(t.assertions[1].expect, "none");
            }
            _ => panic!("expected Test operation"),
        }
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
        if let Operation::Test(t) = &ops[0] {
            assert_eq!(t.assertions[0].expect, "some");
        }
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
        assert!(matches!(&ops[0], Operation::Query(_)));
        assert!(matches!(&ops[1], Operation::Test(_)));
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
        match &ops[0] {
            Operation::Set(s) => {
                assert_eq!(s.mappings.len(), 1);
                assert_eq!(s.mappings[0].value, "localhost");
            }
            _ => panic!("expected Set operation"),
        }
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
        assert_eq!(loaded.root_files, vec!["src/**/*.rs"]);
        if let Operation::Check(c) = &loaded.operations[0] {
            assert!(c.files.is_empty(), "operation should have no files when not specified");
        } else {
            panic!("expected Check");
        }
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
        assert_eq!(loaded.root_files, vec!["src/**/*.rs"]);
        if let Operation::Check(c) = &loaded.operations[0] {
            assert_eq!(c.files, vec!["test/**/*.rs"]);
        } else {
            panic!("expected Check");
        }
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
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.exclude, vec!["target/**", "src/generated/**"]);
        } else {
            panic!("expected Check");
        }
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
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.diff_files.as_deref(), Some("main..HEAD"));
        } else {
            panic!("expected Check");
        }
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
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.diff_files.as_deref(), Some("HEAD~3"));
        } else {
            panic!("expected Check");
        }
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
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.diff_lines.as_deref(), Some("main..HEAD"));
        } else {
            panic!("expected Check");
        }
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
        assert_eq!(loaded.root_files, vec!["src/**/*.rs"]);

        // Operations have no files of their own — root files are applied
        // at resolve time via SharedFileScope.
        if let Operation::Check(c) = &ops[0] {
            assert!(c.files.is_empty());
            assert_eq!(c.exclude, vec!["vendor/**"]);
            assert_eq!(c.diff_files.as_deref(), Some("main..HEAD"));
        } else {
            panic!("expected Check");
        }

        if let Operation::Query(q) = &ops[1] {
            assert!(q.files.is_empty());
            assert_eq!(q.exclude, vec!["vendor/**"]);
            assert_eq!(q.diff_files.as_deref(), Some("main..HEAD"));
        } else {
            panic!("expected Query");
        }
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
        assert_eq!(loaded.root_files, vec!["src/**/*.rs", "lib/**/*.rs"]);
    }

    #[test]
    fn loaded_config_root_files_empty_when_not_specified() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
"#;
        let loaded = parse_config_yaml(yaml).unwrap();
        assert!(loaded.root_files.is_empty());
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
        assert_eq!(loaded.root_files, vec!["src/**/*.rs"]);
        // Operation files are kept as-is (merge_scope overrides at parse time;
        // actual intersection happens in resolve_files)
        if let Operation::Check(c) = &loaded.operations[0] {
            assert_eq!(c.files, vec!["test/**/*.rs"]);
        } else {
            panic!("expected Check");
        }
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
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.rules[0].valid_examples, vec!["fn main() {}"]);
            assert_eq!(c.rules[0].invalid_examples, vec!["// TODO: fix"]);
        } else {
            panic!("expected Check");
        }
    }
}
