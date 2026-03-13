use serde_json::Value;
use tractor_core::{report::Report, report::ReportKind, RenderOptions};
use super::options::{ViewField, ViewSet};
use super::json::{match_to_value, MatchFlags};

pub fn render_yaml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut root = serde_json::Map::new();
    root.insert("kind".into(), serde_json::json!(format!("{:?}", report.kind).to_lowercase()));

    if view.has(ViewField::Summary) {
        if let Some(ref summary) = report.summary {
            if !matches!(report.kind, ReportKind::Query) {
                root.insert("summary".into(), serde_json::json!({
                    "passed":   summary.passed,
                    "total":    summary.total,
                    "files":    summary.files_affected,
                    "errors":   summary.errors,
                    "warnings": summary.warnings,
                }));
            }
        }
    }

    let match_flags = MatchFlags::from_view(view);

    let matches_yaml: Vec<Value> = report.matches.iter()
        .map(|rm| match_to_value(rm, &match_flags, render_opts))
        .collect();
    if !matches_yaml.is_empty() {
        root.insert("matches".into(), Value::Array(matches_yaml));
    }

    if let Some(ref groups) = report.groups {
        let groups_yaml: Vec<Value> = groups.iter().map(|g| {
            let group_matches: Vec<Value> = g.matches.iter()
                .map(|rm| match_to_value(rm, &match_flags, render_opts))
                .collect();
            serde_json::json!({ "file": g.file, "matches": group_matches })
        }).collect();
        root.insert("groups".into(), Value::Array(groups_yaml));
    }

    serde_yaml::to_string(&Value::Object(root)).unwrap_or_else(|_| "{}\n".to_string())
}
