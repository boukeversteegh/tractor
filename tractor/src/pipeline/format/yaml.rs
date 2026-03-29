use serde_json::Value;
use tractor_core::{report::Report, RenderOptions};
use super::options::{ViewField, ViewSet};
use super::json::{render_results_json, emit_report_metadata};

pub fn render_yaml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut root = serde_json::Map::new();

    let show_totals = if report.success.is_some() {
        true
    } else {
        view.has(ViewField::Summary)
    };
    if show_totals {
        emit_report_metadata(&mut root, report);
    }

    // Render results
    if !report.results.is_empty() {
        let results_json = render_results_json(&report.results, view, render_opts);
        if !results_json.is_empty() {
            root.insert("results".into(), Value::Array(results_json));
        }
    }
    if let Some(ref group) = report.group {
        root.insert("group".into(), serde_json::json!(group));
    }

    serde_yaml::to_string(&Value::Object(root)).unwrap_or_else(|_| "{}\n".to_string())
}
