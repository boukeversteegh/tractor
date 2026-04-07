use std::collections::{HashMap, HashSet};

use rayon::prelude::*;
use tractor_core::{
    Match, NormalizedXpath,
    detect_language,
    output::{render_document, RenderOptions},
    parse_to_documents, parse_string_to_documents,
    report::{Report, ReportMatch, Severity, DiagnosticOrigin},
    rule::{RuleSet, GlobMatcher},
    xpath::validate_xpath,
};
use crate::filter::ResultFilter;

use super::context::RunContext;
use super::format::{ViewField, ViewSet};

// ---------------------------------------------------------------------------
// Batch utility
// ---------------------------------------------------------------------------

/// Split a slice into exponentially growing batches, capped at a maximum.
pub fn exponential_batches<T>(items: &[T], num_threads: usize) -> Vec<&[T]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut batch_size = num_threads.max(1);
    let max_batch_size = num_threads * 8;

    while start < items.len() {
        let end = (start + batch_size).min(items.len());
        batches.push(&items[start..end]);
        start = end;
        batch_size = (batch_size * 2).min(max_batch_size);
    }

    batches
}

// ---------------------------------------------------------------------------
// XPath validation
// ---------------------------------------------------------------------------

/// Validate an XPath expression upfront and return a fatal diagnostic if invalid.
///
/// Builds a `ReportMatch` with `Severity::Fatal`, the XPath string as source,
/// and the error position highlighted. Returns `None` if the XPath is valid.
pub fn validate_xpath_diagnostic(xpath_expr: &NormalizedXpath, command: &str) -> Option<ReportMatch> {
    let result = validate_xpath(xpath_expr.as_str());
    if result.valid {
        return None;
    }

    let reason = result.error.as_deref().unwrap_or("invalid XPath expression").to_string();

    // Default: highlight the entire expression
    let line = 1u32;
    let mut col = 1u32;
    let end_line = 1u32;
    let mut end_col = xpath_expr.as_str().len() as u32 + 1;

    // If we have error position info, narrow the highlight to the error span
    if let (Some(start), Some(end)) = (result.error_start, result.error_end) {
        col = start as u32 + 1; // 1-based
        end_col = if end > start { end as u32 + 1 } else { col + 1 };
    }

    Some(ReportMatch {
        file: String::new(),
        line, column: col, end_line, end_column: end_col,
        command: command.to_string(),
        tree: None,
        value: None,
        source: Some(xpath_expr.to_string()),
        lines: Some(vec![xpath_expr.to_string()]),
        reason: Some(reason),
        severity: Some(Severity::Fatal),
        message: None,

        origin: Some(DiagnosticOrigin::Xpath),
        rule_id: None,
        status: None,
        output: None,
    })
}

// ---------------------------------------------------------------------------
// Query pipeline
// ---------------------------------------------------------------------------

pub fn query_inline_source(
    ctx: &RunContext,
    source: &str,
    lang: &str,
    xpath_expr: &NormalizedXpath,
) -> Result<Vec<Match>, Box<dyn std::error::Error>> {
    let mut result = parse_string_to_documents(
        source, lang, "<stdin>".to_string(), ctx.tree_mode, ctx.ignore_whitespace
    )?;

    let matches = result.query(xpath_expr.as_str())?;

    let matches = if let Some(limit) = ctx.limit {
        matches.into_iter().take(limit).collect()
    } else {
        matches
    };

    Ok(matches)
}

pub fn query_files_batched(
    ctx: &RunContext,
    files: &[String],
    xpath_expr: &NormalizedXpath,
    collect: bool,
) -> Result<(usize, Vec<Match>), Box<dyn std::error::Error>> {
    let batches = exponential_batches(files, ctx.concurrency);
    let mut total_matches = 0usize;
    let mut remaining_limit = ctx.limit;
    let mut all_matches: Vec<Match> = Vec::new();

    for batch in batches {
        if remaining_limit == Some(0) {
            break;
        }

        let mut batch_matches: Vec<Match> = batch
            .par_iter()
            .filter_map(|file_path| {
                let mut result = match parse_to_documents(
                    std::path::Path::new(file_path),
                    ctx.lang.as_deref(),
                    ctx.tree_mode,
                    ctx.ignore_whitespace,
                    ctx.parse_depth,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        if ctx.verbose {
                            eprintln!("warning: {}: {}", file_path, e);
                        }
                        return None;
                    }
                };

                match result.query(xpath_expr.as_str()) {
                    Ok(matches) => Some(matches),
                    Err(e) => {
                        if ctx.verbose {
                            eprintln!("warning: {}: query error: {}", file_path, e);
                        }
                        None
                    }
                }
            })
            .flatten()
            .collect();

        batch_matches.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

        if let Some(limit) = remaining_limit {
            batch_matches.truncate(limit);
            remaining_limit = Some(limit.saturating_sub(batch_matches.len()));
        }

        total_matches += batch_matches.len();

        if collect {
            all_matches.extend(batch_matches);
        }
        // collect=false: streaming placeholder for future large-repo optimization
    }

    Ok((total_matches, all_matches))
}

// ---------------------------------------------------------------------------
// Debug mode
// ---------------------------------------------------------------------------

pub fn run_debug(ctx: &RunContext, files: &[String], xpath_expr: &NormalizedXpath) -> Result<(), Box<dyn std::error::Error>> {
    let mut remaining_limit = ctx.limit;

    for file_path in files {
        if remaining_limit == Some(0) {
            break;
        }

        let mut result = match parse_to_documents(
            std::path::Path::new(file_path),
            ctx.lang.as_deref(),
            ctx.tree_mode,
            ctx.ignore_whitespace,
            ctx.parse_depth,
        ) {
            Ok(r) => r,
            Err(e) => {
                if ctx.verbose {
                    eprintln!("warning: {}: {}", file_path, e);
                }
                continue;
            }
        };

        match result.query(xpath_expr.as_str()) {
            Ok(matches) if !matches.is_empty() => {
                let matches: Vec<_> = if let Some(limit) = remaining_limit {
                    let take = limit.min(matches.len());
                    remaining_limit = Some(limit - take);
                    matches.into_iter().take(take).collect()
                } else {
                    matches
                };

                let highlights: HashSet<(u32, u32)> = matches
                    .iter()
                    .map(|m| (m.line, m.column))
                    .collect();

                let doc_node = result.documents.document_node(result.doc_handle).unwrap();
                let render_opts = RenderOptions::new()
                    .with_color(ctx.use_color)
                    .with_meta(true)
                    .with_max_depth(ctx.depth)
                    .with_highlights(highlights)
                    .with_pretty_print(!ctx.no_pretty);
                let output = render_document(result.documents.xot(), doc_node, &render_opts);
                print!("{}", output);
            }
            Ok(_) => {}
            Err(e) => {
                if ctx.verbose {
                    eprintln!("warning: {}: query error: {}", file_path, e);
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Batch rule execution
// ---------------------------------------------------------------------------

/// A match tagged with its originating rule, ready for report building.
pub struct RuleMatch {
    pub rule_index: usize,
    pub m: Match,
}

/// Precomputed per-rule state: the glob matcher and the XPath expression.
struct CompiledRule {
    glob: GlobMatcher,
    xpath: NormalizedXpath,
}

/// Check if a rule's language matches the file's detected language.
///
/// Returns `true` if:
/// - The rule has no explicit language (uses file extension detection)
/// - The rule's effective language matches the file's detected language
///
/// This prevents rules from matching files of incompatible languages,
/// fixing the mixed-language hang issue where e.g. a markdown rule
/// would incorrectly apply to JavaScript files.
fn rule_language_matches_file(
    ruleset: &RuleSet,
    rule_idx: usize,
    file_path: &str,
) -> bool {
    let rule = &ruleset.rules[rule_idx];
    let effective_lang = ruleset.effective_language(rule);
    
    match effective_lang {
        // No language specified → rule uses auto-detection, always matches
        None => true,
        // Language specified → must match file's detected language
        Some(rule_lang) => {
            let file_lang = detect_language(file_path);
            // Handle language aliases (e.g., "js" vs "javascript")
            normalize_language(rule_lang) == normalize_language(file_lang)
        }
    }
}

/// Normalize language names to handle aliases.
fn normalize_language(lang: &str) -> &str {
    match lang {
        "js" => "javascript",
        "ts" => "typescript",
        "py" => "python",
        "rb" => "ruby",
        "rs" => "rust",
        "cs" => "csharp",
        "md" => "markdown",
        "yml" => "yaml",
        "sh" => "bash",
        _ => lang,
    }
}

/// Execute all rules in a `RuleSet` against a list of files.
///
/// For each file, rules are grouped by their effective language. Files are
/// parsed once per language group, ensuring rules are evaluated against
/// correctly-parsed ASTs. This handles mixed-language configs (e.g., rules
/// for both JavaScript and Markdown in the same config file).
///
/// `verbose` controls whether parse/query warnings are printed to stderr.
pub fn run_rules(
    ruleset: &RuleSet,
    files: &[String],
    tree_mode: Option<tractor_core::TreeMode>,
    ignore_whitespace: bool,
    parse_depth: Option<usize>,
    verbose: bool,
    filters: &[&dyn ResultFilter],
) -> Result<Vec<RuleMatch>, Box<dyn std::error::Error>> {
    // Compile glob matchers for each rule upfront.
    let compiled: Vec<CompiledRule> = ruleset
        .rules
        .iter()
        .map(|rule| {
            let glob = ruleset.glob_matcher(rule)?;
            Ok(CompiledRule {
                glob,
                xpath: rule.xpath.clone(),
            })
        })
        .collect::<Result<Vec<_>, tractor_core::rule::GlobError>>()?;

    // Process files in parallel. Each file may be parsed multiple times
    // if rules require different languages.
    let results: Vec<Vec<RuleMatch>> = files
        .par_iter()
        .filter_map(|file_path| {
            // Determine which rules apply to this file based on globs AND language.
            let applicable: Vec<usize> = compiled
                .iter()
                .enumerate()
                .filter(|(i, cr)| {
                    cr.glob.matches(file_path) && rule_language_matches_file(ruleset, *i, file_path)
                })
                .map(|(i, _)| i)
                .collect();

            if applicable.is_empty() {
                return None;
            }

            // Group applicable rules by their effective language.
            // This allows us to parse once per language and run all
            // rules for that language against the parsed result.
            let mut rules_by_lang: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
            for &rule_idx in &applicable {
                let rule = &ruleset.rules[rule_idx];
                let lang = ruleset.effective_language(rule);
                rules_by_lang.entry(lang).or_default().push(rule_idx);
            }

            let mut file_matches = Vec::new();

            // Process each language group: parse once, run all rules
            for (lang_override, rule_indices) in rules_by_lang {
                // Resolve tree_mode from the first rule in this language group
                let first_rule = &ruleset.rules[rule_indices[0]];
                let effective_tree_mode = ruleset.effective_tree_mode(first_rule).or(tree_mode);

                let mut result = match parse_to_documents(
                    std::path::Path::new(file_path),
                    lang_override,
                    effective_tree_mode,
                    ignore_whitespace,
                    parse_depth,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        if verbose {
                            eprintln!("warning: {}: {}", file_path, e);
                        }
                        continue;
                    }
                };

                // Run all rules for this language against the parsed result
                for rule_idx in rule_indices {
                    match result.query(compiled[rule_idx].xpath.as_str()) {
                        Ok(matches) => {
                            for m in matches {
                                file_matches.push(RuleMatch {
                                    rule_index: rule_idx,
                                    m,
                                });
                            }
                        }
                        Err(e) => {
                            if verbose {
                                eprintln!(
                                    "warning: {}: rule '{}' query error: {}",
                                    file_path, ruleset.rules[rule_idx].id, e
                                );
                            }
                        }
                    }
                }
            }

            // Apply result filters at the query engine level.
            if !filters.is_empty() {
                file_matches.retain(|rm| filters.iter().all(|f| f.include(&rm.m)));
            }

            if file_matches.is_empty() {
                None
            } else {
                Some(file_matches)
            }
        })
        .collect();

    // Flatten and sort by file, line, column for stable output.
    let mut all_matches: Vec<RuleMatch> = results.into_iter().flatten().collect();
    all_matches.sort_by(|a, b| {
        (&a.m.file, a.m.line, a.m.column).cmp(&(&b.m.file, b.m.line, b.m.column))
    });

    Ok(all_matches)
}

// ---------------------------------------------------------------------------
// Report post-processing helpers
// ---------------------------------------------------------------------------

/// Project a report to only contain the fields requested by the view.
///
/// The executor populates all content fields. This function prunes
/// fields that are not in the view, ensuring renderers see `None`
/// for unselected fields (matching the behaviour of `match_to_report_match`).
pub fn project_report(report: &mut Report, view: &ViewSet) {
    for m in report.all_matches_mut() {
        // Fatal diagnostics (broken XPath, bad config) always keep their fields —
        // the user needs to see why their query failed regardless of -v settings.
        // Error/Warning matches from user rules are normal output, subject to view.
        let is_diagnostic = matches!(m.severity, Some(Severity::Fatal));
        if !is_diagnostic {
            // Map/Array nodes are always kept — they're the only representation for data formats.
            // For other nodes, keep when tree/lines/source is selected (needed for rendering).
            let keep_tree = match &m.tree {
                Some(node) if matches!(
                    node,
                    tractor_core::xpath::XmlNode::Map { .. }
                        | tractor_core::xpath::XmlNode::Array { .. }
                ) => true,
                _ => view.has(ViewField::Tree) || view.has(ViewField::Lines) || view.has(ViewField::Source),
            };
            if !keep_tree {
                m.tree = None;
            }
            if !view.has(ViewField::Value) {
                m.value = None;
            }
            if !view.has(ViewField::Source) {
                m.source = None;
            }
            if !view.has(ViewField::Lines) {
                m.lines = None;
            }
            if !view.has(ViewField::Reason) {
                m.reason = None;
            }
            if !view.has(ViewField::Severity) {
                m.severity = None;
            }
            if !view.has(ViewField::Status) {
                m.status = None;
            }
        }
    }
}

/// Apply a CLI-level message template (`-m`) to all matches in a report.
///
/// This overwrites any existing message (e.g. from rule-level templates).
/// Placeholders: `{file}`, `{line}`, `{col}`, `{value}`.
pub fn apply_message_template(report: &mut Report, template: &str) {
    if !template.contains('{') {
        // Static template — same for every match.
        let msg = template.to_string();
        for m in report.all_matches_mut() {
            m.message = Some(msg.clone());
        }
        return;
    }

    for m in report.all_matches_mut() {
        m.message = Some(
            template
                .replace("{file}", &tractor_core::output::normalize_path(&m.file))
                .replace("{line}", &m.line.to_string())
                .replace("{col}", &m.column.to_string())
                .replace("{value}", m.value.as_deref().unwrap_or(""))
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_language_aliases() {
        assert_eq!(normalize_language("js"), "javascript");
        assert_eq!(normalize_language("javascript"), "javascript");
        assert_eq!(normalize_language("ts"), "typescript");
        assert_eq!(normalize_language("typescript"), "typescript");
        assert_eq!(normalize_language("py"), "python");
        assert_eq!(normalize_language("python"), "python");
        assert_eq!(normalize_language("rb"), "ruby");
        assert_eq!(normalize_language("ruby"), "ruby");
        assert_eq!(normalize_language("rs"), "rust");
        assert_eq!(normalize_language("rust"), "rust");
        assert_eq!(normalize_language("cs"), "csharp");
        assert_eq!(normalize_language("csharp"), "csharp");
        assert_eq!(normalize_language("md"), "markdown");
        assert_eq!(normalize_language("markdown"), "markdown");
        assert_eq!(normalize_language("yml"), "yaml");
        assert_eq!(normalize_language("yaml"), "yaml");
        assert_eq!(normalize_language("sh"), "bash");
        assert_eq!(normalize_language("bash"), "bash");
        // Unknown languages pass through
        assert_eq!(normalize_language("go"), "go");
        assert_eq!(normalize_language("java"), "java");
    }

    #[test]
    fn test_rule_language_matches_file_no_language_specified() {
        // When no language is specified, rule should match any file
        let mut ruleset = RuleSet::new();
        ruleset.add(tractor_core::rule::Rule::new("test", "//any"));
        
        assert!(rule_language_matches_file(&ruleset, 0, "test.js"));
        assert!(rule_language_matches_file(&ruleset, 0, "test.rs"));
        assert!(rule_language_matches_file(&ruleset, 0, "test.md"));
        assert!(rule_language_matches_file(&ruleset, 0, "test.unknown"));
    }

    #[test]
    fn test_rule_language_matches_file_with_language() {
        // When language is specified, only matching files should match
        let mut ruleset = RuleSet::new();
        let rule = tractor_core::rule::Rule::new("test", "//any")
            .with_language("javascript");
        ruleset.add(rule);
        
        assert!(rule_language_matches_file(&ruleset, 0, "test.js"));
        assert!(!rule_language_matches_file(&ruleset, 0, "test.rs"));
        assert!(!rule_language_matches_file(&ruleset, 0, "test.md"));
    }

    #[test]
    fn test_rule_language_matches_file_with_alias() {
        // Language aliases should work
        let mut ruleset = RuleSet::new();
        let rule = tractor_core::rule::Rule::new("test", "//any")
            .with_language("js");  // alias for javascript
        ruleset.add(rule);
        
        assert!(rule_language_matches_file(&ruleset, 0, "test.js"));
        assert!(!rule_language_matches_file(&ruleset, 0, "test.rs"));
    }

    #[test]
    fn test_rule_language_matches_file_with_default_language() {
        // Default language on ruleset should be used
        let mut ruleset = RuleSet::new();
        ruleset.default_language = Some("markdown".to_string());
        ruleset.add(tractor_core::rule::Rule::new("test", "//any"));
        
        assert!(rule_language_matches_file(&ruleset, 0, "test.md"));
        assert!(!rule_language_matches_file(&ruleset, 0, "test.js"));
    }

    #[test]
    fn test_rule_language_overrides_default() {
        // Rule language should override default
        let mut ruleset = RuleSet::new();
        ruleset.default_language = Some("markdown".to_string());
        let rule = tractor_core::rule::Rule::new("test", "//any")
            .with_language("javascript");
        ruleset.add(rule);
        
        assert!(rule_language_matches_file(&ruleset, 0, "test.js"));
        assert!(!rule_language_matches_file(&ruleset, 0, "test.md"));
    }
}
