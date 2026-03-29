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
        if let Some(ref totals) = report.totals {
            let mut s = serde_json::Map::new();
            if let Some(passed) = report.passed {
                s.insert("passed".into(), serde_json::json!(passed));
            }
            s.insert("results".into(), serde_json::json!(totals.results));
            s.insert("files".into(),   serde_json::json!(totals.files));
            if totals.errors > 0 {
                s.insert("errors".into(), serde_json::json!(totals.errors));
            }
            if totals.warnings > 0 {
                s.insert("warnings".into(), serde_json::json!(totals.warnings));
            }
            if totals.updated > 0 {
                s.insert("updated".into(), serde_json::json!(totals.updated));
            }
            if totals.unchanged > 0 {
                s.insert("unchanged".into(), serde_json::json!(totals.unchanged));
            }
            if let Some(ref expected) = report.expected {
                s.insert("expected".into(), serde_json::json!(expected));
            }
            if let Some(ref query) = report.query {
                s.insert("query".into(), serde_json::json!(query));
            }
            root.insert("summary".into(), serde_json::json!(s));
        }
    }

    // Run report: emit sub-reports as "operations" array
    if let Some(ref ops) = report.operations {
        let ops_yaml: Vec<Value> = ops.iter().map(|sub| {
            let sub_str = render_yaml_report(sub, view, render_opts);
            let sub_obj: serde_json::Map<String, Value> = serde_yaml::from_str(&sub_str).unwrap_or_default();
            // Put "kind" first by building a new ordered map
            let mut ordered = serde_json::Map::new();
            ordered.insert("kind".into(), serde_json::json!(sub.kind.as_str()));
            ordered.extend(sub_obj);
            Value::Object(ordered)
        }).collect();
        root.insert("operations".into(), Value::Array(ops_yaml));
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
