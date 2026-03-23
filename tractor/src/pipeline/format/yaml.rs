use serde_json::Value;
use tractor_core::{report::Report, report::ReportKind, RenderOptions};
use super::options::{GroupBy, ViewField, ViewSet};
use super::json::match_to_value;

pub fn render_yaml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut root = serde_json::Map::new();

    // Summary: always present for check/test reports (structural, not view-gated).
    // For query reports, only include if explicitly requested via -v summary.
    let show_summary = if matches!(report.kind, ReportKind::Query) {
        view.has(ViewField::Summary)
    } else {
        true
    };
    if show_summary {
        if let Some(ref summary) = report.summary {
            let summary_val = if matches!(report.kind, ReportKind::Set) {
                serde_json::json!({
                    "total":     summary.total,
                    "files":     summary.files_affected,
                    "updated":   summary.errors,
                    "unchanged": summary.warnings,
                })
            } else {
                serde_json::json!({
                    "passed":   summary.passed,
                    "total":    summary.total,
                    "files":    summary.files_affected,
                    "errors":   summary.errors,
                    "warnings": summary.warnings,
                })
            };
            root.insert("summary".into(), summary_val);
        }
    }

    if !report.matches.is_empty() {
        let matches_yaml: Vec<Value> = report.matches.iter()
            .map(|rm| match_to_value(rm, view, render_opts, GroupBy::None))
            .collect();
        root.insert("matches".into(), Value::Array(matches_yaml));
    }

    if let Some(ref groups) = report.groups {
        let groups_yaml: Vec<Value> = groups.iter().map(|g| {
            let group_matches: Vec<Value> = g.matches.iter()
                // file is on the group — omit it from individual matches
                .map(|rm| match_to_value(rm, view, render_opts, GroupBy::File))
                // skip empty match objects (e.g. stdout mode with only Output in view)
                .filter(|v| !v.as_object().map(|o| o.is_empty()).unwrap_or(false))
                .collect();
            let mut group_obj = serde_json::Map::new();
            if !g.file.is_empty() {
                group_obj.insert("file".into(), serde_json::json!(g.file));
            }
            if view.has(ViewField::Output) {
                if let Some(ref content) = g.output {
                    group_obj.insert("output".into(), serde_json::json!(content));
                }
            }
            if !group_matches.is_empty() {
                group_obj.insert("matches".into(), Value::Array(group_matches));
            }
            Value::Object(group_obj)
        }).collect();
        root.insert("groups".into(), Value::Array(groups_yaml));
    }

    serde_yaml::to_string(&Value::Object(root)).unwrap_or_else(|_| "{}\n".to_string())
}
