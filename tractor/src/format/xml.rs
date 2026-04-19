use tractor::{report::{Report, ResultItem}, normalize_path, render_xml_string, render_xml_node, RenderOptions};
use super::options::{ViewField, ViewSet, Projection};
use super::shared::{render_fields_for_match, should_emit_command, should_emit_file, should_emit_rule_id, should_show_totals};

pub fn render_xml_report(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    projection: Projection,
    single: bool,
) -> String {
    let body = match projection {
        Projection::Report => render_full_report(report, view, render_opts, dimensions),
        Projection::Results => render_results_projection(report, view, render_opts, dimensions, single),
        Projection::Summary => render_summary_projection(report),
        Projection::Totals  => render_totals_projection(report),
        Projection::Count   => render_count_projection(report),
        Projection::Schema  => render_schema_projection(report),
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            render_per_match_projection(report, view, render_opts, projection, single)
        }
    };

    let xml_header = if render_opts.use_color {
        format!("\x1b[2m<?xml version=\"1.0\" encoding=\"UTF-8\"?>\x1b[0m\n")
    } else {
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n".to_string()
    };

    if render_opts.use_color {
        let color_opts = RenderOptions::new()
            .with_color(true)
            .with_meta(true)
            .with_pretty_print(true);
        let colored = render_xml_string(&body, &color_opts);
        format!("{}{}", xml_header, colored)
    } else {
        format!("{}{}", xml_header, body)
    }
}

// ---------------------------------------------------------------------------
// Full report (default -p report)
// ---------------------------------------------------------------------------

fn render_full_report(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    let mut body = String::new();
    body.push_str("<report>\n");

    // Summary section (wrapping what used to be top-level fields)
    if should_show_totals(report, view) {
        body.push_str("  <summary>\n");
        if let Some(passed) = report.success {
            body.push_str(&format!("    <success>{}</success>\n", passed));
        }
        if let Some(ref totals) = report.totals {
            body.push_str("    <totals>\n");
            body.push_str(&format!("      <results>{}</results>\n", totals.results));
            body.push_str(&format!("      <files>{}</files>\n", totals.files));
            if totals.fatals > 0 { body.push_str(&format!("      <fatals>{}</fatals>\n", totals.fatals)); }
            if totals.errors > 0 { body.push_str(&format!("      <errors>{}</errors>\n", totals.errors)); }
            if totals.warnings > 0 { body.push_str(&format!("      <warnings>{}</warnings>\n", totals.warnings)); }
            if totals.infos > 0 { body.push_str(&format!("      <infos>{}</infos>\n", totals.infos)); }
            if totals.updated > 0 { body.push_str(&format!("      <updated>{}</updated>\n", totals.updated)); }
            if totals.unchanged > 0 { body.push_str(&format!("      <unchanged>{}</unchanged>\n", totals.unchanged)); }
            body.push_str("    </totals>\n");
        }
        if let Some(ref expected) = report.expected {
            body.push_str(&format!("    <expected>{}</expected>\n", escape(expected)));
        }
        if let Some(ref query) = report.query {
            body.push_str(&format!("    <query>{}</query>\n", escape(query.as_str())));
        }
        body.push_str("  </summary>\n");
    }

    // Schema element (when computed)
    if let Some(ref schema) = report.schema {
        body.push_str(&format!("  <schema>{}</schema>\n", escape(schema)));
    }

    // Top-level captured outputs
    if !report.outputs.is_empty() {
        append_outputs(&mut body, &report.outputs, "  ");
    }

    // Grouping dimension
    if let Some(ref group) = report.group {
        body.push_str(&format!("  <group-by>{}</group-by>\n", escape(group)));
    }

    // Results
    if !report.results.is_empty() {
        let mut results_body = String::new();
        render_xml_results(&mut results_body, &report.results, view, "    ", &tree_opts, dimensions);
        if !results_body.is_empty() {
            body.push_str("  <results>\n");
            body.push_str(&results_body);
            body.push_str("  </results>\n");
        }
    }

    body.push_str("</report>\n");
    body
}

// ---------------------------------------------------------------------------
// Projection: -p results
// ---------------------------------------------------------------------------

fn render_results_projection(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    single: bool,
) -> String {
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    let matches = report.all_matches();

    if single {
        // Single match bare (no <results> wrapper)
        if let Some(rm) = matches.first() {
            let mut out = String::new();
            append_match(&mut out, rm, view, "", &tree_opts, dimensions);
            return out;
        }
        return String::new();
    }

    let mut body = String::new();
    let mut results_body = String::new();
    render_xml_results(&mut results_body, &report.results, view, "  ", &tree_opts, dimensions);
    body.push_str("<results>\n");
    body.push_str(&results_body);
    body.push_str("</results>\n");
    body
}

// ---------------------------------------------------------------------------
// Projection: -p summary
// ---------------------------------------------------------------------------

fn render_summary_projection(report: &Report) -> String {
    let mut body = String::new();
    body.push_str("<summary>\n");
    if let Some(passed) = report.success {
        body.push_str(&format!("  <success>{}</success>\n", passed));
    }
    if let Some(ref totals) = report.totals {
        body.push_str("  <totals>\n");
        body.push_str(&format!("    <results>{}</results>\n", totals.results));
        body.push_str(&format!("    <files>{}</files>\n", totals.files));
        if totals.fatals > 0 { body.push_str(&format!("    <fatals>{}</fatals>\n", totals.fatals)); }
        if totals.errors > 0 { body.push_str(&format!("    <errors>{}</errors>\n", totals.errors)); }
        if totals.warnings > 0 { body.push_str(&format!("    <warnings>{}</warnings>\n", totals.warnings)); }
        if totals.infos > 0 { body.push_str(&format!("    <infos>{}</infos>\n", totals.infos)); }
        if totals.updated > 0 { body.push_str(&format!("    <updated>{}</updated>\n", totals.updated)); }
        if totals.unchanged > 0 { body.push_str(&format!("    <unchanged>{}</unchanged>\n", totals.unchanged)); }
        body.push_str("  </totals>\n");
    }
    if let Some(ref expected) = report.expected {
        body.push_str(&format!("  <expected>{}</expected>\n", escape(expected)));
    }
    if let Some(ref query) = report.query {
        body.push_str(&format!("  <query>{}</query>\n", escape(query.as_str())));
    }
    body.push_str("</summary>\n");
    body
}

// ---------------------------------------------------------------------------
// Projection: -p totals
// ---------------------------------------------------------------------------

fn render_totals_projection(report: &Report) -> String {
    let mut body = String::new();
    if let Some(ref totals) = report.totals {
        body.push_str("<totals>\n");
        body.push_str(&format!("  <results>{}</results>\n", totals.results));
        body.push_str(&format!("  <files>{}</files>\n", totals.files));
        if totals.fatals > 0 { body.push_str(&format!("  <fatals>{}</fatals>\n", totals.fatals)); }
        if totals.errors > 0 { body.push_str(&format!("  <errors>{}</errors>\n", totals.errors)); }
        if totals.warnings > 0 { body.push_str(&format!("  <warnings>{}</warnings>\n", totals.warnings)); }
        if totals.infos > 0 { body.push_str(&format!("  <infos>{}</infos>\n", totals.infos)); }
        if totals.updated > 0 { body.push_str(&format!("  <updated>{}</updated>\n", totals.updated)); }
        if totals.unchanged > 0 { body.push_str(&format!("  <unchanged>{}</unchanged>\n", totals.unchanged)); }
        body.push_str("</totals>\n");
    }
    body
}

// ---------------------------------------------------------------------------
// Projection: -p count
// ---------------------------------------------------------------------------

fn render_count_projection(report: &Report) -> String {
    let count = report.totals.as_ref().map_or(0, |t| t.results);
    format!("<count>{}</count>\n", count)
}

// ---------------------------------------------------------------------------
// Projection: -p schema
// ---------------------------------------------------------------------------

fn render_schema_projection(report: &Report) -> String {
    if let Some(ref schema) = report.schema {
        format!("<schema>{}</schema>\n", escape(schema))
    } else {
        "<schema/>\n".to_string()
    }
}

// ---------------------------------------------------------------------------
// Projection: per-match (-p tree, -p value, -p source, -p lines)
// ---------------------------------------------------------------------------

fn render_per_match_projection(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    projection: Projection,
    single: bool,
) -> String {
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    let matches = report.all_matches();

    if single {
        if let Some(rm) = matches.first() {
            return render_per_match_field_bare(rm, projection, &tree_opts);
        }
        return String::new();
    }

    // Multi-match: wrap in <results>
    let mut body = String::new();
    body.push_str("<results>\n");
    for rm in &matches {
        let field_xml = render_per_match_field_wrapped(rm, projection, "  ", &tree_opts);
        body.push_str(&field_xml);
    }
    body.push_str("</results>\n");
    body
}

/// Render the projection field from a match as a bare element (for --single).
fn render_per_match_field_bare(
    rm: &tractor::report::ReportMatch,
    projection: Projection,
    render_opts: &RenderOptions,
) -> String {
    match projection {
        Projection::Tree => {
            if let Some(ref node) = rm.tree {
                let rendered = render_xml_node(node, render_opts);
                if rendered.ends_with('\n') { rendered } else { format!("{}\n", rendered) }
            } else {
                String::new()
            }
        }
        Projection::Value => {
            if let Some(ref v) = rm.value {
                format!("<value>{}</value>\n", escape(v))
            } else {
                String::new()
            }
        }
        Projection::Source => {
            if let Some(ref s) = rm.source {
                format!("<source>{}</source>\n", escape(s))
            } else {
                String::new()
            }
        }
        Projection::Lines => {
            if let Some(ref ls) = rm.lines {
                let mut out = String::new();
                out.push_str("<lines>\n");
                for line in ls {
                    out.push_str(&format!("  <line>{}</line>\n", escape(line)));
                }
                out.push_str("</lines>\n");
                out
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Render the projection field from a match, wrapped in its element tag (for multi-match list).
fn render_per_match_field_wrapped(
    rm: &tractor::report::ReportMatch,
    projection: Projection,
    indent: &str,
    render_opts: &RenderOptions,
) -> String {
    let inner = format!("{}  ", indent);
    let deep  = format!("{}    ", indent);
    match projection {
        Projection::Tree => {
            if let Some(ref node) = rm.tree {
                let rendered = render_xml_node(node, render_opts);
                let mut out = format!("{}<tree>\n", indent);
                for line in rendered.lines() {
                    out.push_str(&inner);
                    out.push_str(line);
                    out.push('\n');
                }
                out.push_str(&format!("{}</tree>\n", indent));
                out
            } else {
                String::new()
            }
        }
        Projection::Value => {
            if let Some(ref v) = rm.value {
                format!("{}<value>{}</value>\n", indent, escape(v))
            } else {
                String::new()
            }
        }
        Projection::Source => {
            if let Some(ref s) = rm.source {
                format!("{}<source>{}</source>\n", indent, escape(s))
            } else {
                String::new()
            }
        }
        Projection::Lines => {
            if let Some(ref ls) = rm.lines {
                let mut out = format!("{}<lines>\n", indent);
                for line in ls {
                    out.push_str(&format!("{}<line>{}</line>\n", inner, escape(line)));
                }
                out.push_str(&format!("{}</lines>\n", indent));
                let _ = deep;
                out
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Match rendering (full match with fields)
// ---------------------------------------------------------------------------

fn append_match(
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
fn render_xml_results(
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
                if let Some(ref group) = sub.group {
                    out.push_str(&format!("{}<group-by>{}</group-by>\n", inner, escape(group)));
                }
                if !sub.outputs.is_empty() {
                    append_group_outputs(out, &sub.outputs, sub.file.as_deref(), &inner);
                }
                render_xml_results(out, &sub.results, view, &inner, tree_opts, dimensions);
                out.push_str(&format!("{}</group>\n", indent));
            }
        }
    }
}

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

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}
