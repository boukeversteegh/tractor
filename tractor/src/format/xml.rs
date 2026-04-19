use tractor::{report::{Report, ResultItem}, normalize_path, render_xml_string, render_xml_node, RenderOptions};
use super::options::{ViewField, ViewSet};
use super::shared::{render_fields_for_match, should_emit_command, should_emit_file, should_emit_rule_id, should_show_totals};

pub fn render_xml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions, dimensions: &[&str]) -> String {
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    let mut body = String::new();
    body.push_str("<report>\n");

    if should_show_totals(report, view) {
        append_summary(&mut body, report, "  ");
    }
    if let Some(ref schema) = report.schema {
        body.push_str(&format!("  <schema>{}</schema>\n", escape(schema)));
    }

    // Top-level captured outputs — honest view of the report model.
    // Any file-bound outputs that matched a file-group have been moved
    // into their group during `with_grouping`; what remains here is
    // genuinely ungrouped output (stdin payloads or orphans).
    if !report.outputs.is_empty() {
        append_outputs(&mut body, &report.outputs, "  ");
    }

    // Render results
    if let Some(ref group) = report.group {
        body.push_str(&format!("  <group-by>{}</group-by>\n", escape(group)));
    }
    if !report.results.is_empty() {
        // Render into a temp buffer first so we can skip the <results> wrapper
        // entirely when the view selects no per-match fields (e.g. `-v summary`
        // or `-v query`). json/yaml skip empty objects automatically via serde;
        // xml needs to do it explicitly to stay consistent across formats.
        let mut results_body = String::new();
        render_xml_results(&mut results_body, &report.results, view, "    ", &tree_opts, dimensions);
        if !results_body.is_empty() {
            body.push_str("  <results>\n");
            body.push_str(&results_body);
            body.push_str("  </results>\n");
        }
    }

    body.push_str("</report>\n");

    // Colorize the whole report XML in one pass via the unified XML renderer.
    // Always use with_meta(true) here: the report body already contains only
    // the attributes it wants. The meta filter would incorrectly strip report
    // attributes like "line" and "column" from <match> elements.
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

pub fn append_match(
    out: &mut String,
    rm: &tractor::report::ReportMatch,
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
            out.push_str(&format!("{}<match line=\"{}\" column=\"{}\"", indent, rm.line, rm.column));
        } else {
            out.push_str(&format!("{}<match", indent));
        }
    } else if has_position {
        out.push_str(&format!("{}<match file=\"{}\" line=\"{}\" column=\"{}\"", indent, escape_attr(&file_str), rm.line, rm.column));
    } else {
        out.push_str(&format!("{}<match file=\"{}\"", indent, escape_attr(&file_str)));
    }
    if has_position && (rm.end_line != rm.line || rm.end_column != rm.column) {
        out.push_str(&format!(" end_line=\"{}\" end_column=\"{}\"", rm.end_line, rm.end_column));
    }
    out.push_str(">\n");

    let inner = &format!("{}  ", indent);
    let deep  = &format!("{}    ", indent);

    let (view_fields, extra_fields) = render_fields_for_match(view, rm);
    let all_fields: Vec<ViewField> = view_fields.into_iter().chain(extra_fields).collect();

    for field in &all_fields {
        match field {
            ViewField::Value => {
                if let Some(ref v) = rm.value {
                    out.push_str(&format!("{}<value>{}</value>\n", inner, escape(v)));
                }
            }
            ViewField::Source => {
                if let Some(ref s) = rm.source {
                    out.push_str(&format!("{}<source>{}</source>\n", inner, escape(s)));
                }
            }
            ViewField::Lines => {
                if let Some(ref ls) = rm.lines {
                    out.push_str(&format!("{}<lines>\n", inner));
                    for line in ls {
                        out.push_str(&format!("{}<line>{}</line>\n", deep, escape(line)));
                    }
                    out.push_str(&format!("{}</lines>\n", inner));
                }
            }
            ViewField::Reason => {
                if let Some(ref reason) = rm.reason {
                    out.push_str(&format!("{}<reason>{}</reason>\n", inner, escape(reason)));
                }
            }
            ViewField::Severity => {
                if let Some(severity) = rm.severity {
                    out.push_str(&format!("{}<severity>{}</severity>\n", inner, severity.as_str()));
                }
            }
            ViewField::Status => {
                if let Some(ref status) = rm.status {
                    out.push_str(&format!("{}<status>{}</status>\n", inner, escape(status)));
                }
            }
            ViewField::Output => {
                if let Some(ref output) = rm.output {
                    out.push_str(&format!("{}<output>{}</output>\n", inner, escape(output)));
                }
            }
            ViewField::Tree => {
                if let Some(ref node) = rm.tree {
                    let rendered = render_xml_node(node, render_opts);
                    out.push_str(&format!("{}<tree>\n", inner));
                    for line in rendered.lines() {
                        out.push_str(deep);
                        out.push_str(line);
                        out.push('\n');
                    }
                    out.push_str(&format!("{}</tree>\n", inner));
                }
            }
            ViewField::Origin => {
                if rm.file.is_empty() {
                    if let Some(origin) = rm.origin {
                        out.push_str(&format!("{}<origin>{}</origin>\n", inner, origin.as_str()));
                    }
                }
            }
            _ => {}
        }
    }

    if should_emit_command(rm, view, skip_dims) {
        out.push_str(&format!("{}<command>{}</command>\n", inner, escape(&rm.command)));
    }
    if let Some(ref message) = rm.message {
        out.push_str(&format!("{}<message>{}</message>\n", inner, escape(message)));
    }
    if should_emit_rule_id(rm, skip_dims) {
        out.push_str(&format!("{}<rule-id>{}</rule-id>\n", inner, escape(rm.rule_id.as_deref().unwrap())));
    }

    out.push_str(&format!("{}</match>\n", indent));
}

/// Render results list recursively as XML.
pub fn render_xml_results(
    out: &mut String,
    items: &[ResultItem],
    view: &ViewSet,
    indent: &str,
    tree_opts: &RenderOptions,
    dimensions: &[&str],
) {
    let inner = format!("{}  ", indent);
    for item in items {
        match item {
            ResultItem::Match(rm) => {
                if view.has_per_match_fields() || rm.message.is_some() {
                    append_match(out, rm, view, indent, tree_opts, dimensions);
                }
            }
            ResultItem::Group(sub) => {
                // Build group element with hoisted attributes
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
                out.push_str(&format!("{}<group{}>\n", indent, attrs));
                // Sub-group's own grouping dimension
                if let Some(ref group) = sub.group {
                    out.push_str(&format!("{}<group-by>{}</group-by>\n", inner, escape(group)));
                }
                // Group-level captured outputs — unconditional honest view.
                if !sub.outputs.is_empty() {
                    append_group_outputs(out, &sub.outputs, sub.file.as_deref(), &inner);
                }
                // Recurse — this group's children skip the same field that was hoisted
                // to create this group. If this group has sub-grouping, that applies too.
                render_xml_results(out, &sub.results, view, &inner, tree_opts, dimensions);
                out.push_str(&format!("{}</group>\n", indent));
            }
        }
    }
}

/// Render a list of captured outputs as `<outputs><output file="...">...</output>...</outputs>`.
/// Content is XML-escaped but newlines are preserved literally, matching how
/// `<lines><line>` is rendered elsewhere.
fn append_outputs(out: &mut String, outputs: &[tractor::report::ReportOutput], indent: &str) {
    let inner = format!("{}  ", indent);
    out.push_str(&format!("{}<outputs>\n", indent));
    for captured in outputs {
        match &captured.file {
            Some(file) => out.push_str(&format!("{}<output file=\"{}\">", inner, escape_attr(file))),
            None => out.push_str(&format!("{}<output>", inner)),
        }
        out.push_str(&escape(&captured.content));
        out.push_str("</output>\n");
    }
    out.push_str(&format!("{}</outputs>\n", indent));
}

fn append_group_outputs(
    out: &mut String,
    outputs: &[tractor::report::ReportOutput],
    group_file: Option<&str>,
    indent: &str,
) {
    if group_file.is_some() && outputs.len() == 1 {
        let captured = &outputs[0];
        match &captured.file {
            Some(file) => out.push_str(&format!("{}<output file=\"{}\">", indent, escape_attr(file))),
            None => out.push_str(&format!("{}<output>", indent)),
        }
        out.push_str(&escape(&captured.content));
        out.push_str("</output>\n");
        return;
    }

    append_outputs(out, outputs, indent);
}

/// Append the `<summary>…</summary>` container with the report's
/// success / totals / expected / query fields. Returns early without
/// emitting anything if none of those fields are present, keeping the
/// container purely structural (no empty `<summary/>`).
pub fn append_summary(body: &mut String, report: &tractor::report::Report, indent: &str) {
    let has_any = report.success.is_some()
        || report.totals.is_some()
        || report.expected.is_some()
        || report.query.is_some();
    if !has_any {
        return;
    }

    let inner = format!("{}  ", indent);
    body.push_str(&format!("{}<summary>\n", indent));
    if let Some(passed) = report.success {
        body.push_str(&format!("{}<success>{}</success>\n", inner, passed));
    }
    if let Some(ref totals) = report.totals {
        append_totals(body, totals, &inner);
    }
    if let Some(ref expected) = report.expected {
        body.push_str(&format!("{}<expected>{}</expected>\n", inner, escape(expected)));
    }
    if let Some(ref query) = report.query {
        body.push_str(&format!("{}<query>{}</query>\n", inner, escape(query.as_str())));
    }
    body.push_str(&format!("{}</summary>\n", indent));
}

/// Append a `<totals>` element with all non-zero fields.
pub fn append_totals(body: &mut String, totals: &tractor::report::Totals, indent: &str) {
    let inner = format!("{}  ", indent);
    body.push_str(&format!("{}<totals>\n", indent));
    body.push_str(&format!("{}<results>{}</results>\n", inner, totals.results));
    body.push_str(&format!("{}<files>{}</files>\n", inner, totals.files));
    if totals.fatals > 0 {
        body.push_str(&format!("{}<fatals>{}</fatals>\n", inner, totals.fatals));
    }
    if totals.errors > 0 {
        body.push_str(&format!("{}<errors>{}</errors>\n", inner, totals.errors));
    }
    if totals.warnings > 0 {
        body.push_str(&format!("{}<warnings>{}</warnings>\n", inner, totals.warnings));
    }
    if totals.infos > 0 {
        body.push_str(&format!("{}<infos>{}</infos>\n", inner, totals.infos));
    }
    if totals.updated > 0 {
        body.push_str(&format!("{}<updated>{}</updated>\n", inner, totals.updated));
    }
    if totals.unchanged > 0 {
        body.push_str(&format!("{}<unchanged>{}</unchanged>\n", inner, totals.unchanged));
    }
    body.push_str(&format!("{}</totals>\n", indent));
}

/// Escape text for use inside an XML element.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}
