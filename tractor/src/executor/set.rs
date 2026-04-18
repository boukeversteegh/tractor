//! Set operation: ensure values exist at specified XPaths.

use tractor::report::{ReportBuilder, ReportMatch, ReportOutput};
use tractor::tree_mode::TreeMode;
use tractor::{parse_string_to_documents, Match};
use tractor::xpath_upsert::upsert_typed;

use crate::input::filter::ResultFilter;
use crate::input::Source;

use super::{ExecuteOptions, filter_refs, match_to_report_match};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A set operation: ensure values exist at specified XPaths.
///
/// Virtual inline sources share the same `Vec<Source>` as disk files.
/// Write-mode is automatically routed to Capture for virtual sources
/// (nothing to write back to disk).
pub struct SetOperation {
    /// Pre-resolved unified input list.
    pub sources: Vec<Source>,
    /// Pre-built result filters.
    pub filters: Vec<Box<dyn ResultFilter>>,
    /// Mappings to apply.
    pub mappings: Vec<SetMapping>,
    /// Tree mode override for parsing diagnostics.
    pub tree_mode: Option<TreeMode>,
    /// Maximum number of matches to update per mapping.
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes while collecting diagnostics.
    pub ignore_whitespace: bool,
    /// How transformed content should be applied.
    pub write_mode: SetWriteMode,
    /// How detailed the diagnostic report should be.
    pub report_mode: SetReportMode,
}

/// A single xpath → value mapping for set operations.
#[derive(Debug, Clone)]
pub struct SetMapping {
    pub xpath: String,
    pub value: String,
    pub value_kind: Option<String>,
}

/// Write policy for set operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetWriteMode {
    InPlace,
    Verify,
    Capture,
}

/// Diagnostic detail level for set operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetReportMode {
    PerMatch,
    PerFile,
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_set(
    op: &SetOperation,
    _options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    if op.mappings.is_empty() {
        return Ok(());
    }

    let filter_refs = filter_refs(&op.filters);

    for source in &op.sources {
        let content = source.read()?;
        // Virtual sources can't be written back to disk — auto-route any
        // non-Verify write mode through Capture so the mutated content
        // surfaces in the report instead of silently vanishing.
        let effective_write_mode = if source.is_virtual()
            && matches!(op.write_mode, SetWriteMode::InPlace)
        {
            SetWriteMode::Capture
        } else {
            op.write_mode
        };

        let filters_for_source: &[&dyn ResultFilter] = if source.is_virtual() {
            &[]
        } else {
            &filter_refs
        };

        let outcome = execute_set_target(
            source,
            &content,
            op,
            effective_write_mode,
            filters_for_source,
        )?;

        if outcome.changed
            && matches!(effective_write_mode, SetWriteMode::InPlace)
            && !source.is_virtual()
        {
            std::fs::write(source.path.as_str(), &outcome.content)?;
        }
        if matches!(op.write_mode, SetWriteMode::Verify) && outcome.changed {
            report.fail();
        }
        report.add_all(outcome.diagnostics);
        if let Some(output) = outcome.output {
            report.add_output(output);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

struct SetTargetOutcome {
    content: String,
    diagnostics: Vec<ReportMatch>,
    output: Option<ReportOutput>,
    changed: bool,
}

fn execute_set_target(
    source: &Source,
    content: &str,
    op: &SetOperation,
    effective_write_mode: SetWriteMode,
    filters: &[&dyn ResultFilter],
) -> Result<SetTargetOutcome, Box<dyn std::error::Error>> {
    let file_label = source.path_str();
    let lang = source.language.as_str();
    let mut current = content.to_string();
    let mut diagnostics = Vec::new();
    let mut changed = false;

    for mapping in &op.mappings {
        let before_matches = if matches!(op.report_mode, SetReportMode::PerMatch) {
            query_set_matches(&current, file_label, lang, mapping, op, filters)?
        } else {
            Vec::new()
        };

        let result = apply_set_mapping(&current, file_label, lang, mapping, op, filters, &before_matches)?;
        let was_modified = result.source != current;
        changed |= was_modified;

        if matches!(op.report_mode, SetReportMode::PerMatch) {
            let mut report_matches = if !result.matches.is_empty() {
                result.matches
            } else if was_modified {
                query_set_matches(&result.source, file_label, lang, mapping, op, filters)?
            } else {
                before_matches
            };

            let status = if was_modified { "updated" } else { "unchanged" };
            diagnostics.extend(report_matches.drain(..).map(|m| {
                let mut rm = match_to_report_match(m, "set");
                rm.status = Some(status.to_string());
                rm.reason = Some(mapping.xpath.clone());
                rm
            }));
        }

        current = result.source;
    }

    if matches!(op.report_mode, SetReportMode::PerFile) {
        diagnostics.push(ReportMatch {
            file: file_label.to_string(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            command: "set".to_string(),
            tree: None,
            value: None,
            source: None,
            lines: None,
            reason: None,
            severity: None,
            message: None,
            origin: None,
            rule_id: None,
            status: Some(if changed { "updated" } else { "unchanged" }.to_string()),
            output: None,
        });
    }

    // Pathless inline sources produce outputs with `file: None` (preserving
    // the prior "no filename in output" signal for -s with no positional
    // path). Everything else — disk files and virtual paths — carries its
    // path through.
    let output_file = if source.is_pathless() {
        None
    } else {
        Some(file_label.to_string())
    };

    let output = if matches!(effective_write_mode, SetWriteMode::Capture) {
        Some(ReportOutput {
            file: output_file,
            content: current.clone(),
        })
    } else {
        None
    };

    Ok(SetTargetOutcome {
        content: current,
        diagnostics,
        output,
        changed,
    })
}

struct SetMappingResult {
    source: String,
    matches: Vec<Match>,
}

fn apply_set_mapping(
    source: &str,
    file_label: &str,
    lang: &str,
    mapping: &SetMapping,
    op: &SetOperation,
    filters: &[&dyn ResultFilter],
    before_matches: &[Match],
) -> Result<SetMappingResult, Box<dyn std::error::Error>> {
    match upsert_typed(
        source,
        lang,
        &mapping.xpath,
        &mapping.value,
        op.limit,
        mapping.value_kind.as_deref(),
    ) {
        Ok(result) => Ok(SetMappingResult {
            source: result.source,
            matches: result.matches.into_iter().map(|mut m| {
                m.file = file_label.to_string();
                m
            }).collect(),
        }),
        Err(tractor::xpath_upsert::UpsertError::UnsupportedLanguage(_)) => {
            if mapping.value_kind.as_deref().is_some_and(|kind| kind != "string") {
                return Err(format!(
                    "set fallback only supports string replacements for unsupported languages ({})",
                    file_label,
                ).into());
            }

            let fallback_matches = if before_matches.is_empty() {
                query_set_matches(source, file_label, lang, mapping, op, filters)?
            } else {
                before_matches.to_vec()
            };

            if fallback_matches.is_empty() {
                return Err(format!(
                    "cannot create missing path '{}' for unsupported language '{}'",
                    mapping.xpath, lang,
                ).into());
            }

            let updated = tractor::apply_set_to_string(source, &fallback_matches, &mapping.value)?;
            Ok(SetMappingResult {
                source: updated,
                matches: fallback_matches,
            })
        }
        Err(err) => Err(err.into()),
    }
}

fn query_set_matches(
    source: &str,
    file_label: &str,
    lang: &str,
    mapping: &SetMapping,
    op: &SetOperation,
    filters: &[&dyn ResultFilter],
) -> Result<Vec<Match>, Box<dyn std::error::Error>> {
    let mut result = parse_string_to_documents(
        source,
        lang,
        file_label.to_string(),
        op.tree_mode,
        op.ignore_whitespace,
    )?;
    let mut matches = result.query(&mapping.xpath)?;
    if !filters.is_empty() {
        matches.retain(|m| filters.iter().all(|f| f.include(m)));
    }
    if let Some(limit) = op.limit {
        matches.truncate(limit);
    }
    Ok(matches)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tractor::report::ReportBuilder;
    use tractor::NormalizedPath;
    use crate::executor::{Operation, ExecuteOptions, execute};

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

    fn disk_source(path: &str) -> Source {
        let np = NormalizedPath::absolute(path);
        let lang = tractor::detect_language(np.as_str()).to_string();
        Source::disk(np, lang)
    }

    fn string_mapping(xpath: &str, value: &str) -> SetMapping {
        SetMapping {
            xpath: xpath.into(),
            value: value.into(),
            value_kind: Some("string".into()),
        }
    }

    fn set_operation(path: String, mappings: Vec<SetMapping>, write_mode: SetWriteMode) -> Operation {
        Operation::Set(SetOperation {
            sources: vec![disk_source(&path)],
            filters: vec![],
            mappings,
            tree_mode: None,
            limit: None,
            ignore_whitespace: false,
            write_mode,
            report_mode: SetReportMode::PerMatch,
        })
    }

    fn set_inline_operation(
        lang: &str,
        source: &str,
        mappings: Vec<SetMapping>,
        write_mode: SetWriteMode,
    ) -> Operation {
        let inline = Source::inline_pathless(
            lang,
            std::sync::Arc::new(source.to_string()),
        );
        Operation::Set(SetOperation {
            sources: vec![inline],
            filters: vec![],
            mappings,
            tree_mode: None,
            limit: None,
            ignore_whitespace: false,
            write_mode,
            report_mode: SetReportMode::PerMatch,
        })
    }

    /// Helper: execute operations and build a report.
    fn run(ops: &[Operation]) -> tractor::report::Report {
        let mut builder = ReportBuilder::new();
        execute(ops, &ExecuteOptions::default(), &mut builder).unwrap();
        builder.build()
    }

    #[test]
    fn set_updates_json_value() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "old"}}"#);
        let ops = vec![set_operation(
            path.clone(),
            vec![string_mapping("//database/host", "new-host")],
            SetWriteMode::InPlace,
        )];
        let report = run(&ops);
        assert!(report.success.unwrap());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "file should contain new value: {}", content);
        assert!(!content.contains("old"), "file should not contain old value: {}", content);
    }

    #[test]
    fn set_creates_missing_node() {
        let (_dir, path) = temp_json_file(r#"{"database": {}}"#);
        let ops = vec![set_operation(
            path.clone(),
            vec![string_mapping("//database/host", "localhost")],
            SetWriteMode::InPlace,
        )];
        let report = run(&ops);
        assert!(report.success.unwrap());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("localhost"), "missing node should be created: {}", content);
    }

    #[test]
    fn set_multiple_mappings() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "old", "port": 1234}}"#);
        let ops = vec![set_operation(
            path.clone(),
            vec![
                string_mapping("//database/host", "new-host"),
                string_mapping("//database/port", "5432"),
            ],
            SetWriteMode::InPlace,
        )];
        let report = run(&ops);
        assert!(report.success.unwrap());
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
        let ops = vec![set_operation(
            path.clone(),
            vec![string_mapping("//database/host", "localhost")],
            SetWriteMode::InPlace,
        )];
        let report = run(&ops);
        assert!(report.success.unwrap());
        assert_eq!(report.all_matches()[0].status.as_deref(), Some("unchanged"));
    }

    #[test]
    fn verify_detects_drift() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "wrong"}}"#);
        let ops = vec![set_operation(
            path.clone(),
            vec![string_mapping("//database/host", "correct")],
            SetWriteMode::Verify,
        )];
        let report = run(&ops);
        assert!(!report.success.unwrap(), "verify should detect drift");
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
        let ops = vec![set_operation(
            path.clone(),
            vec![string_mapping("//database/host", "correct")],
            SetWriteMode::Verify,
        )];
        let report = run(&ops);
        assert!(report.success.unwrap(), "verify should pass when values are in sync");
    }

    #[test]
    fn set_capture_inline_source_emits_output() {
        let ops = vec![set_inline_operation(
            "yaml",
            "database:\n  host: localhost\n  port: 5432\n",
            vec![string_mapping("//database/host", "db.example.com")],
            SetWriteMode::Capture,
        )];
        let report = run(&ops);
        assert!(report.success.unwrap());
        assert_eq!(report.outputs.len(), 1);
        assert!(report.outputs[0].file.is_none());
        assert!(report.outputs[0].content.contains("db.example.com"));
        assert_eq!(report.all_matches()[0].status.as_deref(), Some("updated"));
    }

    #[test]
    fn set_capture_files_emits_file_outputs() {
        let (_dir_a, path_a) = temp_yaml_file("database:\n  host: a\n");
        let (_dir_b, path_b) = temp_yaml_file("database:\n  host: b\n");
        let ops = vec![Operation::Set(SetOperation {
            sources: vec![disk_source(&path_a), disk_source(&path_b)],
            filters: vec![],
            mappings: vec![string_mapping("//database/host", "db.example.com")],
            tree_mode: None,
            limit: None,
            ignore_whitespace: false,
            write_mode: SetWriteMode::Capture,
            report_mode: SetReportMode::PerMatch,
        })];
        let report = run(&ops);
        assert!(report.success.unwrap());
        assert_eq!(report.outputs.len(), 2);
        let files: std::collections::HashSet<_> = report.outputs.iter()
            .filter_map(|output| output.file.as_deref())
            .collect();
        assert!(files.contains(tractor::normalize_path(&path_a).as_str()));
        assert!(files.contains(tractor::normalize_path(&path_b).as_str()));
        assert!(report.outputs.iter().all(|output| output.content.contains("db.example.com")));
        let content_a = std::fs::read_to_string(&path_a).unwrap();
        let content_b = std::fs::read_to_string(&path_b).unwrap();
        assert!(content_a.contains("host: a"), "capture mode should not write files");
        assert!(content_b.contains("host: b"), "capture mode should not write files");
    }

    #[test]
    fn set_yaml_updates_value() {
        let (_dir, path) = temp_yaml_file("database:\n  host: old\n  port: 5432\n");
        let ops = vec![set_operation(
            path.clone(),
            vec![string_mapping("//database/host", "new-host")],
            SetWriteMode::InPlace,
        )];
        let report = run(&ops);
        assert!(report.success.unwrap());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "yaml host should be updated: {}", content);
        assert!(content.contains("5432"), "yaml port should be preserved: {}", content);
    }
}
