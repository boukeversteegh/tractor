use serde_json::Value;
use tractor_core::{report::Report, report::ReportKind, RenderOptions};
use super::options::{GroupBy, ViewField, ViewSet};
use super::json::{match_to_value, render_results_json, emit_report_metadata};

pub fn render_yaml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut root = serde_json::Map::new();

    let show_totals = if matches!(report.kind, ReportKind::Query) {
        view.has(ViewField::Summary)
    } else {
        true
    };
    if show_totals {
        emit_report_metadata(&mut root, report);
    }

    // Render results from the new structure if populated, otherwise fall back to old fields
    if !report.results.is_empty() {
        let results_json = render_results_json(&report.results, view, render_opts);
        if !results_json.is_empty() {
            root.insert("results".into(), Value::Array(results_json));
        }
        if let Some(ref group) = report.group {
            root.insert("group".into(), serde_json::json!(group));
        }
    } else {
        // Fallback to old fields
        if let Some(ref ops) = report.operations {
            let ops_yaml: Vec<Value> = ops.iter().map(|sub| {
                let sub_str = render_yaml_report(sub, view, render_opts);
                let sub_obj: serde_json::Map<String, Value> = serde_yaml::from_str(&sub_str).unwrap_or_default();
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
                    .map(|rm| match_to_value(rm, view, render_opts, GroupBy::File))
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
    }

    serde_yaml::to_string(&Value::Object(root)).unwrap_or_else(|_| "{}\n".to_string())
}
