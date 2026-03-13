use tractor_core::{report::Report, normalize_path, render_xml_string, RenderOptions};
use super::options::{ViewField, ViewSet};

pub fn render_xml_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    use tractor_core::report::ReportKind;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<report>\n");

    // Summary: always present for check/test reports (structural, not view-gated).
    // For query reports, only include if explicitly requested via -v summary.
    let show_summary = if matches!(report.kind, ReportKind::Query) {
        view.has(ViewField::Summary)
    } else {
        true
    };
    if show_summary {
        if let Some(ref summary) = report.summary {
            out.push_str("  <summary>\n");
            out.push_str(&format!("    <passed>{}</passed>\n", summary.passed));
            out.push_str(&format!("    <total>{}</total>\n", summary.total));
            out.push_str(&format!("    <files>{}</files>\n", summary.files_affected));
            out.push_str(&format!("    <errors>{}</errors>\n", summary.errors));
            out.push_str(&format!("    <warnings>{}</warnings>\n", summary.warnings));
            if let Some(ref expected) = summary.expected {
                out.push_str(&format!("    <expected>{}</expected>\n", escape(expected)));
            }
            out.push_str("  </summary>\n");
        }
    }

    let show_tree     = view.has(ViewField::Tree);
    let show_value    = view.has(ViewField::Value);
    let show_source   = view.has(ViewField::Source);
    let show_lines    = view.has(ViewField::Lines);
    let show_reason   = view.has(ViewField::Reason);
    let show_severity = view.has(ViewField::Severity);

    if !report.matches.is_empty() {
        out.push_str("  <matches>\n");
        for rm in &report.matches {
            append_match(&mut out, rm, show_tree, show_value, show_source, show_lines, show_reason, show_severity, "    ", render_opts);
        }
        out.push_str("  </matches>\n");
    }
    if let Some(ref groups) = report.groups {
        out.push_str("  <groups>\n");
        for g in groups {
            out.push_str(&format!("    <group file=\"{}\">\n", escape_attr(&g.file)));
            for rm in &g.matches {
                append_match(&mut out, rm, show_tree, show_value, show_source, show_lines, show_reason, show_severity, "      ", render_opts);
            }
            out.push_str("    </group>\n");
        }
        out.push_str("  </groups>\n");
    }

    out.push_str("</report>\n");
    out
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
