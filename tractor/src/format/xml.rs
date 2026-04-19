use tractor::{
    format_schema_tree,
    normalize_path,
    render_xml_node,
    render_xml_string,
    report::{Report, ReportMatch, ReportOutput, ResultItem, Summary, Totals},
    RenderOptions,
};

use super::options::{ViewField, ViewSet};
use super::shared::{
    render_fields_for_match, should_emit_command, should_emit_file, should_emit_rule_id,
    should_show_totals,
};
use super::{Projection, ProjectionRenderError};

pub fn render_xml_report(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    render_xml_output(report, view, render_opts, dimensions, Projection::Report, false)
        .expect("report rendering should not fail")
}

pub fn render_xml_output(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    projection: Projection,
    single: bool,
) -> Result<String, ProjectionRenderError> {
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    let body = match projection {
        Projection::Report => render_xml_report_body(report, view, &tree_opts, dimensions),
        Projection::Results => render_xml_results_projection(report, view, &tree_opts, dimensions, single)?,
        Projection::Summary => render_xml_summary(report),
        Projection::Totals => render_xml_totals(report),
        Projection::Count => format!(
            "<count>{}</count>\n",
            report.totals.as_ref().map(|totals| totals.results).unwrap_or(0)
        ),
        Projection::Schema => format!("<schema>{}</schema>\n", escape(&schema_to_string(report, render_opts))),
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            render_xml_field_projection(report, &tree_opts, projection, single)?
        }
    };

    Ok(finish_xml_output(body, render_opts))
}

fn render_xml_report_body(
    report: &Report,
    view: &ViewSet,
    tree_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    let mut body = String::new();
    body.push_str("<report>\n");

    if should_show_totals(report, view) {
        let summary = render_xml_summary_inner(Summary::from_report(report), "    ");
        if !summary.is_empty() {
            body.push_str("  <summary>\n");
            body.push_str(&summary);
            body.push_str("  </summary>\n");
        }
    }

    if report.schema.is_some() {
        body.push_str(&format!("  <schema>{}</schema>\n", escape(&schema_to_string(report, tree_opts))));
    }

    if !report.outputs.is_empty() {
        append_outputs(&mut body, &report.outputs, "  ");
    }

    if let Some(ref group) = report.group {
        body.push_str(&format!("  <group-by>{}</group-by>\n", escape(group)));
    }
    if !report.results.is_empty() {
        let mut results_body = String::new();
        render_xml_results(&mut results_body, &report.results, view, "    ", tree_opts, dimensions);
        if !results_body.is_empty() {
            body.push_str("  <results>\n");
            body.push_str(&results_body);
            body.push_str("  </results>\n");
        }
    }

    body.push_str("</report>\n");
    body
}

fn render_xml_results_projection(
    report: &Report,
    view: &ViewSet,
    tree_opts: &RenderOptions,
    dimensions: &[&str],
    single: bool,
) -> Result<String, ProjectionRenderError> {
    if single {
        let Some(first) = report.results.first() else {
            return Err(ProjectionRenderError::EmptySingle);
        };
        let mut out = String::new();
        match first {
            ResultItem::Match(rm) => append_match(&mut out, rm, view, "", tree_opts, dimensions),
            ResultItem::Group(group) => append_group(&mut out, group, view, "", tree_opts, dimensions),
        }
        return Ok(out);
    }

    if report.results.is_empty() {
        return Ok("<results/>\n".to_string());
    }

    let mut body = String::new();
    render_xml_results(&mut body, &report.results, view, "  ", tree_opts, dimensions);
    Ok(format!("<results>\n{body}</results>\n"))
}

fn render_xml_field_projection(
    report: &Report,
    tree_opts: &RenderOptions,
    projection: Projection,
    single: bool,
) -> Result<String, ProjectionRenderError> {
    if single {
        report
            .all_matches()
            .into_iter()
            .find_map(|rm| match_field_bare(rm, tree_opts, projection))
            .ok_or(ProjectionRenderError::EmptySingle)
    } else {
        let projected: Vec<String> = report
            .all_matches()
            .into_iter()
            .filter_map(|rm| project_match_field_xml(rm, tree_opts, projection))
            .collect();

        if projected.is_empty() {
            return Ok("<results/>\n".to_string());
        }

        let mut out = String::from("<results>\n");
        for item in projected {
            out.push_str(&indent_xml_fragment(&item, "  "));
        }
        out.push_str("</results>\n");
        Ok(out)
    }
}

fn match_field_bare(
    rm: &ReportMatch,
    tree_opts: &RenderOptions,
    projection: Projection,
) -> Option<String> {
    match projection {
        Projection::Tree => rm
            .tree
            .as_ref()
            .map(|node| ensure_xml_fragment_newline(render_xml_node(node, tree_opts))),
        Projection::Value => rm
            .value
            .as_ref()
            .map(|value| format!("<value>{}</value>\n", escape(value))),
        Projection::Source => rm
            .source
            .as_ref()
            .map(|source| format!("<source>{}</source>\n", escape(source))),
        Projection::Lines => rm.lines.as_ref().map(|lines| {
            let mut out = String::from("<lines>\n");
            for line in lines {
                out.push_str(&format!("  <line>{}</line>\n", escape(line)));
            }
            out.push_str("</lines>\n");
            out
        }),
        _ => None,
    }
}

fn project_match_field_xml(
    rm: &ReportMatch,
    tree_opts: &RenderOptions,
    projection: Projection,
) -> Option<String> {
    match projection {
        Projection::Tree => rm
            .tree
            .as_ref()
            .map(|node| wrap_projection_element("tree", &render_xml_node(node, tree_opts))),
        Projection::Value | Projection::Source | Projection::Lines => {
            match_field_bare(rm, tree_opts, projection)
        }
        _ => None,
    }
}

fn render_xml_summary(report: &Report) -> String {
    let inner = render_xml_summary_inner(Summary::from_report(report), "  ");
    if inner.is_empty() {
        "<summary/>\n".to_string()
    } else {
        format!("<summary>\n{inner}</summary>\n")
    }
}

fn render_xml_summary_inner(summary: Summary<'_>, indent: &str) -> String {
    let mut rendered = String::new();
    if let Some(passed) = summary.success {
        rendered.push_str(&format!("{indent}<success>{passed}</success>\n"));
    }
    if let Some(totals) = summary.totals {
        rendered.push_str(&render_xml_totals_inner(totals, indent));
    }
    if let Some(expected) = summary.expected {
        rendered.push_str(&format!("{indent}<expected>{}</expected>\n", escape(expected)));
    }
    if let Some(query) = summary.query {
        rendered.push_str(&format!("{indent}<query>{}</query>\n", escape(query.as_str())));
    }
    rendered
}

fn render_xml_totals(report: &Report) -> String {
    match report.totals.as_ref() {
        Some(totals) => render_xml_totals_inner(totals, ""),
        None => "<totals/>\n".to_string(),
    }
}

fn render_xml_totals_inner(totals: &Totals, indent: &str) -> String {
    let inner = format!("{indent}  ");
    let mut out = String::new();
    out.push_str(&format!("{indent}<totals>\n"));
    out.push_str(&format!("{inner}<results>{}</results>\n", totals.results));
    out.push_str(&format!("{inner}<files>{}</files>\n", totals.files));
    if totals.fatals > 0 {
        out.push_str(&format!("{inner}<fatals>{}</fatals>\n", totals.fatals));
    }
    if totals.errors > 0 {
        out.push_str(&format!("{inner}<errors>{}</errors>\n", totals.errors));
    }
    if totals.warnings > 0 {
        out.push_str(&format!("{inner}<warnings>{}</warnings>\n", totals.warnings));
    }
    if totals.infos > 0 {
        out.push_str(&format!("{inner}<infos>{}</infos>\n", totals.infos));
    }
    if totals.updated > 0 {
        out.push_str(&format!("{inner}<updated>{}</updated>\n", totals.updated));
    }
    if totals.unchanged > 0 {
        out.push_str(&format!("{inner}<unchanged>{}</unchanged>\n", totals.unchanged));
    }
    out.push_str(&format!("{indent}</totals>\n"));
    out
}

fn finish_xml_output(body: String, render_opts: &RenderOptions) -> String {
    if render_opts.use_color {
        let color_opts = RenderOptions::new()
            .with_color(true)
            .with_meta(true)
            .with_pretty_print(true);
        let colored = render_xml_string(&body, &color_opts);
        format!("\x1b[2m<?xml version=\"1.0\" encoding=\"UTF-8\"?>\x1b[0m\n{}", colored)
    } else {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", body)
    }
}

fn schema_to_string(report: &Report, render_opts: &RenderOptions) -> String {
    report
        .schema
        .as_ref()
        .map(|schema| format_schema_tree(schema, render_opts.max_depth.or(Some(4)), false))
        .unwrap_or_default()
}

fn append_match(
    out: &mut String,
    rm: &ReportMatch,
    view: &ViewSet,
    indent: &str,
    render_opts: &RenderOptions,
    skip_dims: &[&str],
) {
    let file_str = normalize_path(&rm.file);
    let show_file = should_emit_file(rm, skip_dims);
    let has_position = rm.line > 0;
    if !show_file {
        if has_position {
            out.push_str(&format!(
                "{indent}<match line=\"{}\" column=\"{}\"",
                rm.line, rm.column
            ));
        } else {
            out.push_str(&format!("{indent}<match"));
        }
    } else if has_position {
        out.push_str(&format!(
            "{indent}<match file=\"{}\" line=\"{}\" column=\"{}\"",
            escape_attr(&file_str),
            rm.line,
            rm.column
        ));
    } else {
        out.push_str(&format!(
            "{indent}<match file=\"{}\"",
            escape_attr(&file_str)
        ));
    }
    if has_position && (rm.end_line != rm.line || rm.end_column != rm.column) {
        out.push_str(&format!(
            " end_line=\"{}\" end_column=\"{}\"",
            rm.end_line, rm.end_column
        ));
    }
    out.push_str(">\n");

    let inner = format!("{indent}  ");
    let deep = format!("{indent}    ");

    let (view_fields, extra_fields) = render_fields_for_match(view, rm);
    let all_fields: Vec<ViewField> = view_fields.into_iter().chain(extra_fields).collect();

    for field in &all_fields {
        match field {
            ViewField::Value => {
                if let Some(ref v) = rm.value {
                    out.push_str(&format!("{inner}<value>{}</value>\n", escape(v)));
                }
            }
            ViewField::Source => {
                if let Some(ref s) = rm.source {
                    out.push_str(&format!("{inner}<source>{}</source>\n", escape(s)));
                }
            }
            ViewField::Lines => {
                if let Some(ref ls) = rm.lines {
                    out.push_str(&format!("{inner}<lines>\n"));
                    for line in ls {
                        out.push_str(&format!("{deep}<line>{}</line>\n", escape(line)));
                    }
                    out.push_str(&format!("{inner}</lines>\n"));
                }
            }
            ViewField::Reason => {
                if let Some(ref reason) = rm.reason {
                    out.push_str(&format!("{inner}<reason>{}</reason>\n", escape(reason)));
                }
            }
            ViewField::Severity => {
                if let Some(severity) = rm.severity {
                    out.push_str(&format!("{inner}<severity>{}</severity>\n", severity.as_str()));
                }
            }
            ViewField::Status => {
                if let Some(ref status) = rm.status {
                    out.push_str(&format!("{inner}<status>{}</status>\n", escape(status)));
                }
            }
            ViewField::Output => {
                if let Some(ref output) = rm.output {
                    out.push_str(&format!("{inner}<output>{}</output>\n", escape(output)));
                }
            }
            ViewField::Tree => {
                if let Some(ref node) = rm.tree {
                    let rendered = render_xml_node(node, render_opts);
                    out.push_str(&format!("{inner}<tree>\n"));
                    for line in rendered.lines() {
                        out.push_str(&deep);
                        out.push_str(line);
                        out.push('\n');
                    }
                    out.push_str(&format!("{inner}</tree>\n"));
                }
            }
            ViewField::Origin => {
                if rm.file.is_empty() {
                    if let Some(origin) = rm.origin {
                        out.push_str(&format!("{inner}<origin>{}</origin>\n", origin.as_str()));
                    }
                }
            }
            _ => {}
        }
    }

    if should_emit_command(rm, view, skip_dims) {
        out.push_str(&format!("{inner}<command>{}</command>\n", escape(&rm.command)));
    }
    if let Some(ref message) = rm.message {
        out.push_str(&format!("{inner}<message>{}</message>\n", escape(message)));
    }
    if should_emit_rule_id(rm, skip_dims) {
        out.push_str(&format!(
            "{inner}<rule-id>{}</rule-id>\n",
            escape(rm.rule_id.as_deref().unwrap())
        ));
    }

    out.push_str(&format!("{indent}</match>\n"));
}

fn render_xml_results(
    out: &mut String,
    items: &[ResultItem],
    view: &ViewSet,
    indent: &str,
    tree_opts: &RenderOptions,
    dimensions: &[&str],
) {
    for item in items {
        match item {
            ResultItem::Match(rm) => {
                if view.has_per_match_fields() || rm.message.is_some() {
                    append_match(out, rm, view, indent, tree_opts, dimensions);
                }
            }
            ResultItem::Group(sub) => append_group(out, sub, view, indent, tree_opts, dimensions),
        }
    }
}

fn append_group(
    out: &mut String,
    sub: &Report,
    view: &ViewSet,
    indent: &str,
    tree_opts: &RenderOptions,
    dimensions: &[&str],
) {
    let inner = format!("{indent}  ");
    let mut attrs = String::new();
    if let Some(ref file) = sub.file {
        attrs.push_str(&format!(" file=\"{}\"", escape_attr(file)));
    }
    if let Some(ref command) = sub.command {
        attrs.push_str(&format!(" command=\"{}\"", escape_attr(command)));
    }
    if let Some(ref rule_id) = sub.rule_id {
        attrs.push_str(&format!(" rule-id=\"{}\"", escape_attr(rule_id)));
    }
    out.push_str(&format!("{indent}<group{attrs}>\n"));
    if let Some(ref group) = sub.group {
        out.push_str(&format!("{inner}<group-by>{}</group-by>\n", escape(group)));
    }
    if !sub.outputs.is_empty() {
        append_group_outputs(out, &sub.outputs, sub.file.as_deref(), &inner);
    }
    render_xml_results(out, &sub.results, view, &inner, tree_opts, dimensions);
    out.push_str(&format!("{indent}</group>\n"));
}

fn append_outputs(out: &mut String, outputs: &[ReportOutput], indent: &str) {
    let inner = format!("{indent}  ");
    out.push_str(&format!("{indent}<outputs>\n"));
    for captured in outputs {
        match &captured.file {
            Some(file) => out.push_str(&format!("{inner}<output file=\"{}\">", escape_attr(file))),
            None => out.push_str(&format!("{inner}<output>")),
        }
        out.push_str(&escape(&captured.content));
        out.push_str("</output>\n");
    }
    out.push_str(&format!("{indent}</outputs>\n"));
}

fn append_group_outputs(
    out: &mut String,
    outputs: &[ReportOutput],
    group_file: Option<&str>,
    indent: &str,
) {
    if group_file.is_some() && outputs.len() == 1 {
        let captured = &outputs[0];
        match &captured.file {
            Some(file) => out.push_str(&format!("{indent}<output file=\"{}\">", escape_attr(file))),
            None => out.push_str(&format!("{indent}<output>")),
        }
        out.push_str(&escape(&captured.content));
        out.push_str("</output>\n");
        return;
    }

    append_outputs(out, outputs, indent);
}

fn wrap_projection_element(name: &str, body: &str) -> String {
    let body = body.trim_end();
    if body.is_empty() {
        return format!("<{name}/>\n");
    }

    let mut out = String::new();
    out.push_str(&format!("<{name}>\n"));
    for line in body.lines() {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&format!("</{name}>\n"));
    out
}

fn indent_xml_fragment(fragment: &str, indent: &str) -> String {
    let mut out = String::new();
    for line in fragment.lines() {
        out.push_str(indent);
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn ensure_xml_fragment_newline(mut fragment: String) -> String {
    if !fragment.ends_with('\n') {
        fragment.push('\n');
    }
    fragment
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}
