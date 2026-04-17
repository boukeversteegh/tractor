//! Check operation: run XPath rules against files, report violations.

use tractor::report::{ReportBuilder, ReportMatch, Severity};
use tractor::tree_mode::TreeMode;
use tractor::rule::{Rule, RuleSet};
use tractor::parse_string_to_documents;

use crate::matcher::validate_xpath_diagnostic;
use crate::matcher::run_rules;
use crate::input::file_resolver::{FileResolver, FileRequest};
use crate::input::Source;

use super::{build_sources, ExecuteOptions, filter_refs, match_to_report_match};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A check operation: run XPath rules against files, report violations.
#[derive(Debug, Clone)]
pub struct CheckOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Git diff spec: only consider files changed in this diff.
    pub diff_files: Option<String>,
    /// Git diff spec: only include matches in changed hunks.
    pub diff_lines: Option<String>,
    /// Rules to check.
    pub rules: Vec<Rule>,
    /// Default tree mode for all rules (rules can override).
    pub tree_mode: Option<TreeMode>,
    /// Default language for all rules (rules can override).
    pub language: Option<String>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
    /// Ruleset-level include patterns for per-rule glob matching.
    /// Used by rules-file configs; empty for single-xpath checks.
    #[doc(hidden)]
    pub ruleset_include: Vec<String>,
    /// Ruleset-level exclude patterns for per-rule glob matching.
    #[doc(hidden)]
    pub ruleset_exclude: Vec<String>,
    /// Optional inline source (from stdin or `-s/--string`). When present,
    /// its virtual path participates in rule glob matching and diagnostics
    /// exactly like a disk file — the executor threads it into the same
    /// `run_rules` pipeline.
    pub inline_source: Option<Source>,
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_check(
    op: &CheckOperation,
    options: &ExecuteOptions,
    resolver: &FileResolver,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    if op.rules.is_empty() {
        return Ok(());
    }

    // --- Phase 0: Validate XPath expressions upfront ---
    let diagnostics: Vec<_> = op.rules.iter()
        .filter_map(|rule| validate_xpath_diagnostic(&rule.xpath, "check"))
        .collect();
    if !diagnostics.is_empty() {
        report.add_all(diagnostics);
        return Ok(());
    }

    // --- Phase 1: Validate rule examples inline ---
    validate_rule_examples(&op.rules, op.language.as_deref(), op.tree_mode, report)?;

    // --- Phase 2: Resolve disk files and unify with any inline source ---
    // When the operation has no files but rules define include patterns, hoist
    // their union into the file request so patterns drive file discovery (fix #127 bug 3).
    let hoisted_files: Vec<String> = if op.files.is_empty() && op.inline_source.is_none() {
        let mut seen = std::collections::HashSet::new();
        op.rules.iter()
            .flat_map(|r| r.include.iter())
            .filter(|p| seen.insert((*p).clone()))
            .cloned()
            .collect()
    } else {
        vec![]
    };
    let effective_files = if !hoisted_files.is_empty() {
        &hoisted_files
    } else {
        &op.files
    };

    let inline_content_holder = op.inline_source.as_ref().and_then(|s| {
        if s.is_pathless() { None } else { s.inline_content().map(|c| (&s.path, c)) }
    });
    let request = FileRequest {
        files: effective_files,
        exclude: &op.exclude,
        diff_files: op.diff_files.as_deref(),
        diff_lines: op.diff_lines.as_deref(),
        command: "check",
        inline: inline_content_holder,
        has_inline: op.inline_source.is_some(),
    };
    let (files, filters) = resolver.resolve(&request, report);

    let sources = build_sources(files, op.inline_source.as_ref(), op.language.as_deref());

    // Build a RuleSet from the operation. Ruleset-level include/exclude
    // come from rules files; per-rule patterns still participate in glob matching.
    let ruleset = RuleSet {
        rules: op.rules.clone(),
        include: op.ruleset_include.clone(),
        exclude: op.ruleset_exclude.clone(),
        default_tree_mode: op.tree_mode,
        default_language: op.language.clone(),
    };

    if !sources.is_empty() {
        let rule_matches = run_rules(
            &ruleset,
            &sources,
            resolver.base_dir(),
            op.tree_mode,
            op.ignore_whitespace,
            op.parse_depth,
            options.verbose,
            &filter_refs(&filters),
        )?;

        for rm in rule_matches {
            let rule = &ruleset.rules[rm.rule_index];
            let reason = rule
                .reason
                .clone()
                .unwrap_or_else(|| format!("[{}] check failed", rule.id));
            let severity = rule.severity;

            // Apply rule-level message template (if the rule defines one)
            let message = rule
                .message
                .as_deref()
                .map(|t| tractor::format_message(t, &rm.m));

            let mut report_match = match_to_report_match(rm.m, "check");
            report_match.reason = Some(reason);
            report_match.severity = Some(severity);
            report_match.rule_id = Some(rule.id.clone());
            report_match.message = message;
            report.add(report_match);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Example validation
// ---------------------------------------------------------------------------

/// Validate rule examples by parsing and querying inline.
///
/// For each rule with examples, parses the example source and runs the rule's
/// XPath query. Adds failure matches to the builder for any unmet expectations.
fn validate_rule_examples(
    rules: &[Rule],
    default_language: Option<&str>,
    default_tree_mode: Option<TreeMode>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for rule in rules {
        if !rule.has_examples() {
            continue;
        }

        let lang = rule.language.as_deref()
            .or(default_language)
            .ok_or_else(|| format!(
                "rule '{}' has examples but no language specified (set language on the rule or check operation)",
                rule.id
            ))?;

        let tree_mode = rule.tree_mode.or(default_tree_mode);

        // Validate valid examples: expect "none" (query should NOT match valid code)
        for (i, example) in rule.valid_examples.iter().enumerate() {
            let mut result = parse_string_to_documents(
                example, lang, "<stdin>".to_string(), tree_mode, false,
            )?;
            let matches = result.query(rule.xpath.as_str())?;
            if !super::check_expectation("none", matches.len())? {
                report.add(example_failure_match(
                    &rule.id,
                    &format!(
                        "[{}] valid example {} unexpectedly matched query",
                        rule.id, i + 1
                    ),
                ));
            }
        }

        // Validate invalid examples: expect "some" (query SHOULD match invalid code)
        for (i, example) in rule.invalid_examples.iter().enumerate() {
            let mut result = parse_string_to_documents(
                example, lang, "<stdin>".to_string(), tree_mode, false,
            )?;
            let matches = result.query(rule.xpath.as_str())?;
            if !super::check_expectation("some", matches.len())? {
                report.add(example_failure_match(
                    &rule.id,
                    &format!(
                        "[{}] invalid example {} did not match query",
                        rule.id, i + 1
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// Build a synthetic ReportMatch for a failed example validation.
fn example_failure_match(rule_id: &str, reason: &str) -> ReportMatch {
    ReportMatch {
        file: String::new(),
        line: 0,
        column: 0,
        end_line: 0,
        end_column: 0,
        command: "test".to_string(),
        tree: None,
        value: None,
        source: None,
        lines: None,
        reason: Some(reason.to_string()),
        severity: Some(Severity::Error),
        message: None,
        origin: None,
        rule_id: Some(rule_id.to_string()),
        status: None,
        output: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tractor::report::ReportBuilder;
    use tractor::rule::Rule;

    #[test]
    fn test_validate_examples_pass_and_fail_correct() {
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_valid_examples(vec!["fn main() {}".to_string()])
                .with_invalid_examples(vec!["// hello\nfn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty(), "expected no failures: {:?}", report.all_matches());
    }

    #[test]
    fn test_validate_examples_valid_unexpectedly_matches() {
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_valid_examples(vec!["// oops this is a comment".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.as_ref().unwrap().contains("valid example 1 unexpectedly matched"));
    }

    #[test]
    fn test_validate_examples_invalid_does_not_match() {
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_invalid_examples(vec!["fn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.as_ref().unwrap().contains("invalid example 1 did not match"));
    }

    #[test]
    fn test_validate_examples_language_from_operation() {
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_valid_examples(vec!["fn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, Some("rust"), None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty());
    }

    #[test]
    fn test_validate_examples_no_language_errors() {
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_valid_examples(vec!["fn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        let err = validate_rule_examples(&rules, None, None, &mut builder).unwrap_err();
        assert!(err.to_string().contains("no language specified"));
    }

    #[test]
    fn test_validate_examples_no_examples_is_noop() {
        let rules = vec![Rule::new("simple", "//function")];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty());
    }

    // -----------------------------------------------------------------------
    // Bug 3 regression: rule-level include hoists into file discovery
    // -----------------------------------------------------------------------

    use tractor::normalize_path;
    use crate::executor::{Operation, ExecuteOptions, execute};

    fn run(ops: &[Operation]) -> tractor::report::Report {
        let mut builder = ReportBuilder::new();
        execute(ops, &ExecuteOptions::default(), &mut builder).unwrap();
        builder.build()
    }

    /// Fix #127 bug 3: when a check operation has no files but rules have
    /// include patterns, those patterns must drive file discovery.
    #[test]
    fn check_rule_include_discovers_files() {
        let dir = tempfile::tempdir().unwrap();
        #[allow(clippy::disallowed_methods)] // test-only filesystem setup
        let canon_dir = std::fs::canonicalize(dir.path()).unwrap();
        let src_dir = canon_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let json_path = src_dir.join("data.json");
        std::fs::write(&json_path, r#"{"name": "test"}"#).unwrap();

        let include_pattern = format!("{}/**/*.json", normalize_path(&src_dir.to_string_lossy()));
        let rule = Rule::new("no-name", "//name")
            .with_severity(tractor::report::Severity::Error)
            .with_reason("found name".to_string())
            .with_include(vec![include_pattern]);

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            rules: vec![rule],
            tree_mode: None,
            language: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            inline_source: None,
        })];

        let report = run(&ops);
        assert!(
            !report.all_matches().is_empty(),
            "rule include pattern should discover files and find matches"
        );
        assert_eq!(report.all_matches()[0].reason.as_deref(), Some("found name"));
    }

    /// Fix #127 bug 3: multiple rules with different include patterns — each
    /// rule's include is expanded for discovery, but only matches its own files.
    #[test]
    fn check_multiple_rule_includes_discover_union() {
        let dir = tempfile::tempdir().unwrap();
        #[allow(clippy::disallowed_methods)] // test-only filesystem setup
        let canon_dir = std::fs::canonicalize(dir.path()).unwrap();
        let src_dir = canon_dir.join("src");
        let test_dir = canon_dir.join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();
        std::fs::write(src_dir.join("data.json"), r#"{"name": "src"}"#).unwrap();
        std::fs::write(test_dir.join("data.json"), r#"{"name": "test"}"#).unwrap();

        let src_pattern = format!("{}/**/*.json", normalize_path(&src_dir.to_string_lossy()));
        let test_pattern = format!("{}/**/*.json", normalize_path(&test_dir.to_string_lossy()));

        let rule_src = Rule::new("src-rule", "//name")
            .with_severity(tractor::report::Severity::Error)
            .with_reason("src match".to_string())
            .with_include(vec![src_pattern]);
        let rule_test = Rule::new("test-rule", "//name")
            .with_severity(tractor::report::Severity::Error)
            .with_reason("test match".to_string())
            .with_include(vec![test_pattern]);

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            rules: vec![rule_src, rule_test],
            tree_mode: None,
            language: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            inline_source: None,
        })];

        let report = run(&ops);
        let reasons: Vec<&str> = report.all_matches().iter()
            .filter_map(|m| m.reason.as_deref())
            .collect();
        assert!(reasons.contains(&"src match"), "should find src match");
        assert!(reasons.contains(&"test match"), "should find test match");
    }

    /// Fix #127 bug 1: verify that CLI files intersect correctly with root
    /// files when both refer to the same canonical file.
    #[test]
    fn check_cli_root_intersection_on_real_files() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("test.json");
        std::fs::write(&json_path, r#"{"bad": true}"#).unwrap();

        #[allow(clippy::disallowed_methods)] // test-only filesystem setup
        let canonical = std::fs::canonicalize(&json_path).unwrap();
        let canonical_str = normalize_path(&canonical.to_string_lossy());

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            rules: vec![
                Rule::new("check-bad", "//bad")
                    .with_severity(tractor::report::Severity::Error)
                    .with_reason("found bad".to_string()),
            ],
            tree_mode: None,
            language: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            inline_source: None,
        })];

        let options = ExecuteOptions {
            config_root_files: Some(vec![canonical_str.clone()]),
            cli_files: vec![canonical_str.clone()],
            ..Default::default()
        };
        let mut builder = ReportBuilder::new();
        execute(&ops, &options, &mut builder).unwrap();
        let report = builder.build();
        assert!(
            !report.all_matches().is_empty(),
            "intersection of identical canonical paths should find the file"
        );
    }

    /// When multiple rules have overlapping include patterns, each file should
    /// be processed once — not once per pattern that matched it.
    #[test]
    fn check_overlapping_rule_includes_no_duplicate_matches() {
        let dir = tempfile::tempdir().unwrap();
        #[allow(clippy::disallowed_methods)] // test-only filesystem setup
        let canon_dir = std::fs::canonicalize(dir.path()).unwrap();
        let sub_dir = canon_dir.join("src").join("sub");
        std::fs::create_dir_all(&sub_dir).unwrap();
        let json_path = sub_dir.join("data.json");
        std::fs::write(&json_path, r#"{"name": "test"}"#).unwrap();

        let broad = format!("{}/**/*.json", normalize_path(&canon_dir.join("src").to_string_lossy()));
        let narrow = format!("{}/**/*.json", normalize_path(&sub_dir.to_string_lossy()));

        let rule_a = Rule::new("rule-a", "//name")
            .with_severity(tractor::report::Severity::Error)
            .with_reason("found name".to_string())
            .with_include(vec![broad]);
        let rule_b = Rule::new("rule-b", "//name")
            .with_severity(tractor::report::Severity::Error)
            .with_reason("found name".to_string())
            .with_include(vec![narrow]);

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            rules: vec![rule_a, rule_b],
            tree_mode: None,
            language: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            inline_source: None,
        })];

        let report = run(&ops);
        assert_eq!(
            report.all_matches().len(), 2,
            "each rule should match the file exactly once, not once per overlapping glob"
        );
    }
}
