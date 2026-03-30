use serde_json::Value;
use tractor_core::{report::Report, RenderOptions};
use super::options::{ViewField, ViewSet};
use super::json::{render_results_json, emit_report_metadata};

pub fn render_yaml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions, dimensions: &[&str]) -> String {
    let mut root = serde_json::Map::new();

    let show_totals = if report.success.is_some() {
        true
    } else {
        view.has(ViewField::Totals)
    };
    if show_totals {
        emit_report_metadata(&mut root, report);
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
