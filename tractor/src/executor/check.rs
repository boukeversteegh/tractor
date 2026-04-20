//! Check operation: run XPath rules against sources, report violations.

use tractor::report::{ReportBuilder, ReportMatch, Severity};
use tractor::tree_mode::TreeMode;
use tractor::rule::{Rule, RuleSet};
use tractor::parse_string_to_documents;

use crate::matcher::validate_xpath_diagnostic;
use crate::matcher::run_rules;
use crate::input::filter::ResultFilter;
use crate::input::Source;

use crate::cli::context::ExecCtx;

use super::{filter_refs, match_to_report_match};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A check operation: run XPath rules against sources, report violations.
///
/// Construction site has already resolved file globs, CLI intersection,
/// diff-files filter, inline-source wiring, and language detection into
/// `sources`. Downstream is a plain `Vec<Source>` loop.
pub struct CheckOperation {
    /// Pre-resolved unified input list (disk and/or inline).
    pub sources: Vec<Source>,
    /// Pre-built result filters (diff-lines, etc.). Applied inside
    /// `run_rules` to every match.
    pub filters: Vec<Box<dyn ResultFilter>>,
    /// Rules to check.
    pub rules: Vec<Rule>,
    /// Default tree mode for all rules (rules can override).
    pub tree_mode: Option<TreeMode>,
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
    /// Default language for the ruleset (fallback when a rule has none).
    /// Used both during example validation and during source parsing.
    pub ruleset_default_language: Option<String>,
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_check(
    op: &CheckOperation,
    ctx: &ExecCtx<'_>,
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
    validate_rule_examples(&op.rules, op.ruleset_default_language.as_deref(), op.tree_mode, report)?;

    if op.sources.is_empty() {
        return Ok(());
    }

    // Build a RuleSet from the operation. Ruleset-level include/exclude
    // come from rules files; per-rule patterns still participate in glob matching.
    let ruleset = RuleSet {
        rules: op.rules.clone(),
        include: op.ruleset_include.clone(),
        exclude: op.ruleset_exclude.clone(),
        default_tree_mode: op.tree_mode,
        default_language: op.ruleset_default_language.clone(),
    };

    let rule_matches = run_rules(
        &ruleset,
        &op.sources,
        ctx.base_dir,
        op.tree_mode,
        op.ignore_whitespace,
        op.parse_depth,
        ctx.verbose,
        &filter_refs(&op.filters),
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
                example, lang, tractor::PATHLESS_LABEL.to_string(), tree_mode, false,
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
                example, lang, tractor::PATHLESS_LABEL.to_string(), tree_mode, false,
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
}
