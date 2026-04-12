//! Check operation: run XPath rules against files, report violations.

use tractor_core::report::{ReportBuilder, ReportMatch, Severity};
use tractor_core::tree_mode::TreeMode;
use tractor_core::rule::{Rule, RuleSet};
use tractor_core::parse_string_to_documents;

use crate::matcher::validate_xpath_diagnostic;
use crate::matcher::run_rules;
use crate::input::file_resolver::{FileResolver, FileRequest};

use super::{ExecuteOptions, filter_refs, match_to_report_match};

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
    /// Inline source string to parse instead of files.
    pub inline_source: Option<String>,
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

    // --- Phase 2: Inline source mode — parse a string and run rules against it ---
    if let Some(ref source) = op.inline_source {
        let lang = op.language.as_deref()
            .ok_or("inline source requires a language (--lang)")?;
        let mut result = parse_string_to_documents(
            source, lang, "<stdin>".to_string(), op.tree_mode, op.ignore_whitespace,
        )?;
        for rule in &op.rules {
            let matches = result.query(rule.xpath.as_str())?;
            let reason = rule
                .reason
                .clone()
                .unwrap_or_else(|| format!("[{}] check failed", rule.id));
            let severity = rule.severity;
            let message_tpl = rule.message.as_deref();
            for m in matches {
                let message = message_tpl.map(|t| tractor_core::format_message(t, &m));
                let mut report_match = match_to_report_match(m, "check");
                report_match.reason = Some(reason.clone());
                report_match.severity = Some(severity);
                report_match.rule_id = Some(rule.id.clone());
                report_match.message = message;
                report.add(report_match);
            }
        }
        return Ok(());
    }

    // --- Phase 3: Run the actual file check ---
    let request = FileRequest {
        files: &op.files,
        exclude: &op.exclude,
        diff_files: op.diff_files.as_deref(),
        diff_lines: op.diff_lines.as_deref(),
        command: "check",
    };
    let (files, filters) = resolver.resolve(&request, report);

    // Build a RuleSet from the operation. Ruleset-level include/exclude
    // come from rules files; per-rule patterns still participate in glob matching.
    let ruleset = RuleSet {
        rules: op.rules.clone(),
        include: op.ruleset_include.clone(),
        exclude: op.ruleset_exclude.clone(),
        default_tree_mode: op.tree_mode,
        default_language: op.language.clone(),
    };

    if !files.is_empty() {
        let rule_matches = run_rules(
            &ruleset,
            &files,
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
                .map(|t| tractor_core::format_message(t, &rm.m));

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
    use tractor_core::report::ReportBuilder;
    use tractor_core::rule::Rule;

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
}
