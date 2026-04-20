use tractor::{report::Report, RenderOptions};
use super::options::{ViewSet};
use super::json::project_json_value;
use super::{Projection, ProjectionRenderError};

pub fn render_yaml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions, dimensions: &[&str]) -> String {
    render_yaml_output(report, view, render_opts, dimensions, Projection::Report, false)
        .expect("report rendering should not fail")
}

pub fn render_yaml_output(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    projection: Projection,
    single: bool,
) -> Result<String, ProjectionRenderError> {
    let value = project_json_value(report, view, render_opts, dimensions, projection, single)?;
    Ok(serde_yaml::to_string(&value).unwrap_or_else(|_| "{}\n".to_string()))
}
