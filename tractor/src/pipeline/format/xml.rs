use tractor_core::{report::Report, normalize_path, render_xml_string, RenderOptions};
use super::options::{ViewField, ViewSet};

pub fn render_xml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    use tractor_core::report::ReportKind;

    // Tree fragments inside <tree> are built without color. The entire report is colorized
    // in one pass at the end via render_xml_string — keeping coloring at the serialization layer.
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    // Summary: always present for check/test reports (structural, not view-gated).
    // For query reports, only include if explicitly requested via -v summary.
    let show_summary = if matches!(report.kind, ReportKind::Query) {
        view.has(ViewField::Summary)
    } else {
        true
    };

    let show_tree     = view.has(ViewField::Tree);
    let show_value    = view.has(ViewField::Value);
    let show_source   = view.has(ViewField::Source);
    let show_lines    = view.has(ViewField::Lines);
    let show_reason   = view.has(ViewField::Reason);
    let show_severity = view.has(ViewField::Severity);

    let mut body = String::new();
    body.push_str("<report>\n");

    if show_summary {
        if let Some(ref summary) = report.summary {
            body.push_str("  <summary>\n");
            body.push_str(&format!("    <passed>{}</passed>\n", summary.passed));
            body.push_str(&format!("    <total>{}</total>\n", summary.total));
            body.push_str(&format!("    <files>{}</files>\n", summary.files_affected));
            body.push_str(&format!("    <errors>{}</errors>\n", summary.errors));
            body.push_str(&format!("    <warnings>{}</warnings>\n", summary.warnings));
            if let Some(ref expected) = summary.expected {
                body.push_str(&format!("    <expected>{}</expected>\n", escape(expected)));
            }
            body.push_str("  </summary>\n");
        }
    }

    if !report.matches.is_empty() {
        body.push_str("  <matches>\n");
        for rm in &report.matches {
            append_match(&mut body, rm, show_tree, show_value, show_source, show_lines, show_reason, show_severity, "    ", &tree_opts);
        }
        body.push_str("  </matches>\n");
    }
    if let Some(ref groups) = report.groups {
        body.push_str("  <groups>\n");
        for g in groups {
            body.push_str(&format!("    <group file=\"{}\">\n", escape_attr(&g.file)));
            for rm in &g.matches {
                append_match(&mut body, rm, show_tree, show_value, show_source, show_lines, show_reason, show_severity, "      ", &tree_opts);
            }
            body.push_str("    </group>\n");
        }
        body.push_str("  </groups>\n");
    }

    body.push_str("</report>\n");

    // Colorize the whole report XML in one pass via the unified XML renderer.
    if render_opts.use_color {
        let color_opts = RenderOptions::new()
            .with_color(true)
            .with_locations(render_opts.include_locations)
            .with_pretty_print(true);
        let colored = render_xml_string(&body, &color_opts);
        format!("\x1b[2m<?xml version=\"1.0\" encoding=\"UTF-8\"?>\x1b[0m\n{}", colored)
    } else {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", body)
    }
}

fn append_match(
    out: &mut String,
    rm: &tractor_core::report::ReportMatch,
    show_tree: bool,
    show_value: bool,
    show_source: bool,
    show_lines: bool,
    show_reason: bool,
    show_severity: bool,
    indent: &str,
    render_opts: &RenderOptions,
) {
    let m    = &rm.inner;
    let file = escape_attr(&normalize_path(&m.file));
    out.push_str(&format!("{}<match file=\"{}\" line=\"{}\" column=\"{}\"", indent, file, m.line, m.column));
    if m.end_line != m.line || m.end_column != m.column {
        out.push_str(&format!(" end_line=\"{}\" end_column=\"{}\"", m.end_line, m.end_column));
    }
    out.push_str(">\n");

    let inner = &format!("{}  ", indent);
    let deep  = &format!("{}    ", indent);

    if show_value {
        out.push_str(&format!("{}<value>{}</value>\n", inner, escape(&m.value)));
    }
    if show_source {
        out.push_str(&format!("{}<source>{}</source>\n", inner, escape(&m.extract_source_snippet())));
    }
    if show_lines {
        out.push_str(&format!("{}<lines>\n", inner));
        for line in m.get_source_lines_range() {
            out.push_str(&format!("{}<line>{}</line>\n", inner, escape(line.trim_end_matches('\r'))));
        }
        out.push_str(&format!("{}</lines>\n", inner));
    }
    if let Some(ref message) = rm.message {
        out.push_str(&format!("{}<message>{}</message>\n", inner, escape(message)));
    }
    if show_reason {
        if let Some(ref reason) = rm.reason {
            out.push_str(&format!("{}<reason>{}</reason>\n", inner, escape(reason)));
        }
    }
    if show_severity {
        if let Some(severity) = rm.severity {
            out.push_str(&format!("{}<severity>{}</severity>\n", inner, severity.as_str()));
        }
    }
    if let Some(ref rule_id) = rm.rule_id {
        out.push_str(&format!("{}<rule-id>{}</rule-id>\n", inner, escape(rule_id)));
    }
    // Tree is always last — it's the bulkiest field
    if show_tree {
        if let Some(ref frag) = m.xml_fragment {
            // render_opts has use_color=false here; the outer colorization pass handles it
            let rendered = render_xml_string(frag, render_opts);
            out.push_str(&format!("{}<tree>\n", inner));
            for line in rendered.lines() {
                out.push_str(deep);
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&format!("{}</tree>\n", inner));
        }
    }

    out.push_str(&format!("{}</match>\n", indent));
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}
