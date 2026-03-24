//! Batch executor for tractor operations.
//!
//! The `Operation` enum is the stable entry point for programmatic use.
//! Operations can be constructed from a config file or built directly in code.
//! The executor takes a list of operations and returns structured results.

use std::path::PathBuf;
use tractor_core::rule::{Rule, RuleSet};
use tractor_core::report::Severity;
use tractor_core::tree_mode::TreeMode;
use tractor_core::{expand_globs, filter_supported_files, detect_language};
use tractor_core::xpath_upsert::upsert;

use crate::pipeline::run_rules;

// ---------------------------------------------------------------------------
// Operation types (stable API)
// ---------------------------------------------------------------------------

/// A single operation to execute. This is the stable intermediate
/// representation — config files parse into this, and the executor
/// consumes it.
#[derive(Debug, Clone)]
pub enum Operation {
    Check(CheckOperation),
    Set(SetOperation),
}

/// A check operation: run XPath rules against files, report violations.
#[derive(Debug, Clone)]
pub struct CheckOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Rules to check.
    pub rules: Vec<Rule>,
    /// Default tree mode for all rules (rules can override).
    pub tree_mode: Option<TreeMode>,
    /// Default language for all rules (rules can override).
    pub language: Option<String>,
}

/// A set operation: ensure values exist at specified XPaths.
#[derive(Debug, Clone)]
pub struct SetOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Mappings to apply.
    pub mappings: Vec<SetMapping>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override for parsing.
    pub language: Option<String>,
}

/// A single xpath → value mapping for set operations.
#[derive(Debug, Clone)]
pub struct SetMapping {
    pub xpath: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Execution options
// ---------------------------------------------------------------------------

/// Options controlling how operations are executed.
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    /// If true, set operations check for drift without writing files.
    /// Check operations run normally. The overall result fails if any
    /// set would produce changes.
    pub verify: bool,
    /// Print verbose diagnostics to stderr.
    pub verbose: bool,
    /// Base directory for resolving relative file paths.
    /// If None, uses the current working directory.
    pub base_dir: Option<PathBuf>,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        ExecuteOptions {
            verify: false,
            verbose: false,
            base_dir: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Execution results
// ---------------------------------------------------------------------------

/// Result of executing a batch of operations.
#[derive(Debug)]
pub struct ExecutionResult {
    pub results: Vec<OperationResult>,
}

impl ExecutionResult {
    /// Returns true if all operations passed (no violations, no drift).
    pub fn success(&self) -> bool {
        self.results.iter().all(|r| r.success())
    }
}

/// Result of a single operation.
#[derive(Debug)]
pub enum OperationResult {
    Check(CheckResult),
    Set(SetResult),
}

impl OperationResult {
    pub fn success(&self) -> bool {
        match self {
            OperationResult::Check(r) => r.passed,
            OperationResult::Set(r) => !r.has_drift(),
        }
    }
}

/// Result of a check operation.
#[derive(Debug)]
pub struct CheckResult {
    /// Whether all checks passed (no error-severity violations).
    pub passed: bool,
    /// Violations found.
    pub violations: Vec<Violation>,
}

/// A single check violation.
#[derive(Debug)]
pub struct Violation {
    pub rule_id: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub reason: String,
    pub severity: Severity,
}

/// Result of a set operation.
#[derive(Debug)]
pub struct SetResult {
    /// Per-file change details.
    pub changes: Vec<FileChange>,
    /// Whether we were in verify mode (dry-run).
    pub verify_mode: bool,
}

impl SetResult {
    /// Returns true if any file would be (or was) modified.
    pub fn has_drift(&self) -> bool {
        self.verify_mode && self.changes.iter().any(|c| c.was_modified)
    }

    /// Number of files modified (or that would be modified).
    pub fn files_modified(&self) -> usize {
        self.changes.iter().filter(|c| c.was_modified).count()
    }
}

/// Details about changes to a single file.
#[derive(Debug)]
pub struct FileChange {
    pub file: String,
    pub mappings_applied: usize,
    pub was_modified: bool,
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Execute a list of operations and return structured results.
pub fn execute(
    operations: &[Operation],
    options: &ExecuteOptions,
) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
    let mut results = Vec::with_capacity(operations.len());

    for op in operations {
        let result = match op {
            Operation::Check(check_op) => {
                OperationResult::Check(execute_check(check_op, options)?)
            }
            Operation::Set(set_op) => {
                OperationResult::Set(execute_set(set_op, options)?)
            }
        };
        results.push(result);
    }

    Ok(ExecutionResult { results })
}

// ---------------------------------------------------------------------------
// Check execution
// ---------------------------------------------------------------------------

fn execute_check(
    op: &CheckOperation,
    options: &ExecuteOptions,
) -> Result<CheckResult, Box<dyn std::error::Error>> {
    if op.rules.is_empty() {
        return Ok(CheckResult { passed: true, violations: vec![] });
    }

    // Expand file globs and filter to supported files
    let files = resolve_files(&op.files, &op.exclude, options);

    if files.is_empty() {
        return Ok(CheckResult { passed: true, violations: vec![] });
    }

    // Build a RuleSet from the operation
    let ruleset = RuleSet {
        rules: op.rules.clone(),
        include: vec![], // already resolved
        exclude: vec![], // already resolved
        default_tree_mode: op.tree_mode,
        default_language: op.language.clone(),
    };

    let rule_matches = run_rules(
        &ruleset,
        &files,
        op.tree_mode,
        false, // ignore_whitespace
        None,  // parse_depth
        options.verbose,
    )?;

    let mut violations = Vec::new();
    let mut has_error = false;

    for rm in rule_matches {
        let rule = &ruleset.rules[rm.rule_index];
        let severity = rule.severity;
        if severity == Severity::Error {
            has_error = true;
        }
        violations.push(Violation {
            rule_id: rule.id.clone(),
            file: rm.m.file.clone(),
            line: rm.m.line,
            column: rm.m.column,
            reason: rule.reason.clone().unwrap_or_else(|| format!("[{}] check failed", rule.id)),
            severity,
        });
    }

    Ok(CheckResult {
        passed: !has_error,
        violations,
    })
}

// ---------------------------------------------------------------------------
// Set execution
// ---------------------------------------------------------------------------

fn execute_set(
    op: &SetOperation,
    options: &ExecuteOptions,
) -> Result<SetResult, Box<dyn std::error::Error>> {
    let files = resolve_files(&op.files, &op.exclude, options);
    let mut changes = Vec::new();

    for file_path in &files {
        let lang_override = op.language.as_deref();
        let lang = lang_override
            .unwrap_or_else(|| detect_language(file_path));

        let source = std::fs::read_to_string(file_path)?;
        let mut current = source.clone();
        let mut mappings_applied = 0;

        for mapping in &op.mappings {
            let result = upsert(&current, lang, &mapping.xpath, &mapping.value, None)?;
            if result.source != current {
                mappings_applied += 1;
                current = result.source;
            }
        }

        let was_modified = current != source;

        // Write the file unless we're in verify mode
        if was_modified && !options.verify {
            std::fs::write(file_path, &current)?;
        }

        changes.push(FileChange {
            file: file_path.clone(),
            mappings_applied,
            was_modified,
        });
    }

    Ok(SetResult {
        changes,
        verify_mode: options.verify,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_files(
    file_globs: &[String],
    exclude_globs: &[String],
    options: &ExecuteOptions,
) -> Vec<String> {
    let mut files = if let Some(base) = &options.base_dir {
        // Resolve globs relative to base_dir
        let globs: Vec<String> = file_globs.iter().map(|g| {
            if std::path::Path::new(g).is_absolute() {
                g.clone()
            } else {
                base.join(g).to_string_lossy().to_string()
            }
        }).collect();
        expand_globs(&globs)
    } else {
        expand_globs(file_globs)
    };

    // Filter excludes
    if !exclude_globs.is_empty() {
        let exclude_patterns: Vec<glob::Pattern> = exclude_globs.iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        let opts = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        files.retain(|f| {
            !exclude_patterns.iter().any(|p| p.matches_with(f, opts))
        });
    }

    filter_supported_files(files)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    fn temp_json_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
    }

    fn temp_yaml_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
    }

    // -----------------------------------------------------------------------
    // Set operation tests
    // -----------------------------------------------------------------------

    #[test]
    fn set_updates_json_value() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "old"}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "new-host".into(),
            }],
            tree_mode: None,
            language: None,
        })];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(result.success());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "file should contain new value: {}", content);
        assert!(!content.contains("old"), "file should not contain old value: {}", content);
    }

    #[test]
    fn set_creates_missing_node() {
        let (_dir, path) = temp_json_file(r#"{"database": {}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "localhost".into(),
            }],
            tree_mode: None,
            language: None,
        })];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(result.success());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("localhost"), "missing node should be created: {}", content);
    }

    #[test]
    fn set_multiple_mappings() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "old", "port": 1234}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![
                SetMapping { xpath: "//database/host".into(), value: "new-host".into() },
                SetMapping { xpath: "//database/port".into(), value: "5432".into() },
            ],
            tree_mode: None,
            language: None,
        })];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(result.success());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "host should be updated: {}", content);
        assert!(content.contains("5432"), "port should be updated: {}", content);
    }

    #[test]
    fn set_no_change_when_value_matches() {
        let original = r#"{
  "database": {
    "host": "localhost"
  }
}"#;
        let (_dir, path) = temp_json_file(original);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "localhost".into(),
            }],
            tree_mode: None,
            language: None,
        })];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(result.success());

        // File should be unchanged
        if let OperationResult::Set(ref set_result) = result.results[0] {
            assert_eq!(set_result.changes[0].was_modified, false);
            assert_eq!(set_result.changes[0].mappings_applied, 0);
        } else {
            panic!("expected SetResult");
        }
    }

    // -----------------------------------------------------------------------
    // Verify mode tests
    // -----------------------------------------------------------------------

    #[test]
    fn verify_detects_drift() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "wrong"}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "correct".into(),
            }],
            tree_mode: None,
            language: None,
        })];

        let options = ExecuteOptions { verify: true, ..Default::default() };
        let result = execute(&ops, &options).unwrap();

        // Should fail: drift detected
        assert!(!result.success(), "verify should detect drift");

        // File should NOT be modified
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("wrong"), "file should not be modified in verify mode");
    }

    #[test]
    fn verify_passes_when_in_sync() {
        let (_dir, path) = temp_json_file(r#"{
  "database": {
    "host": "correct"
  }
}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "correct".into(),
            }],
            tree_mode: None,
            language: None,
        })];

        let options = ExecuteOptions { verify: true, ..Default::default() };
        let result = execute(&ops, &options).unwrap();

        assert!(result.success(), "verify should pass when values are in sync");
    }

    // -----------------------------------------------------------------------
    // Check operation tests
    // -----------------------------------------------------------------------

    #[test]
    fn check_finds_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": true, "verbose": true}"#);

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![path.clone()],
            exclude: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled")
                    .with_severity(Severity::Error),
            ],
            tree_mode: None,
            language: None,
        })];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(!result.success(), "check should fail when violations found");

        if let OperationResult::Check(ref check_result) = result.results[0] {
            assert!(!check_result.passed);
            assert_eq!(check_result.violations.len(), 1);
            assert_eq!(check_result.violations[0].rule_id, "no-debug");
        } else {
            panic!("expected CheckResult");
        }
    }

    #[test]
    fn check_passes_when_no_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": false}"#);

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![path.clone()],
            exclude: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled"),
            ],
            tree_mode: None,
            language: None,
        })];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(result.success());
    }

    // -----------------------------------------------------------------------
    // Mixed operations tests
    // -----------------------------------------------------------------------

    #[test]
    fn mixed_check_and_set() {
        let dir = tempfile::tempdir().unwrap();

        // A config file to set values on
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, r#"{"host": "old"}"#).unwrap();

        // A data file to check
        let data_path = dir.path().join("data.json");
        std::fs::write(&data_path, r#"{"name": "test"}"#).unwrap();

        let ops = vec![
            Operation::Check(CheckOperation {
                files: vec![data_path.to_str().unwrap().into()],
                exclude: vec![],
                rules: vec![
                    Rule::new("has-name", "//name[.='missing']")
                        .with_reason("name should not be 'missing'"),
                ],
                tree_mode: None,
                language: None,
            }),
            Operation::Set(SetOperation {
                files: vec![config_path.to_str().unwrap().into()],
                exclude: vec![],
                mappings: vec![SetMapping {
                    xpath: "//host".into(),
                    value: "new-host".into(),
                }],
                tree_mode: None,
                language: None,
            }),
        ];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(result.success());
        assert_eq!(result.results.len(), 2);

        // Config should be updated
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("new-host"));
    }

    #[test]
    fn set_yaml_updates_value() {
        let (_dir, path) = temp_yaml_file("database:\n  host: old\n  port: 5432\n");

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "new-host".into(),
            }],
            tree_mode: None,
            language: None,
        })];

        let result = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(result.success());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "yaml host should be updated: {}", content);
        assert!(content.contains("5432"), "yaml port should be preserved: {}", content);
    }
}
