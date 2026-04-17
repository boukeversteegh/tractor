use serde_json::Value;
use tractor::{report::Report, RenderOptions};
use super::options::{ViewSet};
use super::json::{emit_report_metadata, outputs_to_json, render_results_json};
use super::shared::should_show_totals;

pub fn render_yaml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions, dimensions: &[&str]) -> String {
    let mut root = serde_json::Map::new();

    if should_show_totals(report, view) {
        emit_report_metadata(&mut root, report);
    }

    if !report.outputs.is_empty() {
        root.insert("outputs".into(), outputs_to_json(&report.outputs));
    }

    // Group dimension (before results)
    if let Some(ref group) = report.group {
        root.insert("group".into(), serde_json::json!(group));
    }

    // Render results
    if !report.results.is_empty() {
        let results_json = render_results_json(&report.results, view, render_opts, dimensions);
        if !results_json.is_empty() {
            root.insert("results".into(), Value::Array(results_json));
        }
    }

    serde_yaml::to_string(&Value::Object(root)).unwrap_or_else(|_| "{}\n".to_string())
}
