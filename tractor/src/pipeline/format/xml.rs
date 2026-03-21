use tractor_core::{report::Report, normalize_path, render_xml_string, render_xml_node, RenderOptions};
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

    let mut body = String::new();
    body.push_str("<report>\n");

    if show_summary {
        if let Some(ref summary) = report.summary {
            body.push_str("  <summary>\n");
            if matches!(report.kind, ReportKind::Set) {
                body.push_str(&format!("    <total>{}</total>\n", summary.total));
                body.push_str(&format!("    <files>{}</files>\n", summary.files_affected));
                body.push_str(&format!("    <updated>{}</updated>\n", summary.errors));
                body.push_str(&format!("    <unchanged>{}</unchanged>\n", summary.warnings));
            } else {
                body.push_str(&format!("    <passed>{}</passed>\n", summary.passed));
                body.push_str(&format!("    <total>{}</total>\n", summary.total));
                body.push_str(&format!("    <files>{}</files>\n", summary.files_affected));
                body.push_str(&format!("    <errors>{}</errors>\n", summary.errors));
                body.push_str(&format!("    <warnings>{}</warnings>\n", summary.warnings));
                if let Some(ref expected) = summary.expected {
                    body.push_str(&format!("    <expected>{}</expected>\n", escape(expected)));
                }
            }
            body.push_str("  </summary>\n");
        }
    }

    if !report.matches.is_empty() {
        body.push_str("  <matches>\n");
        for rm in &report.matches {
            append_match(&mut body, rm, view, "    ", &tree_opts);
        }
        body.push_str("  </matches>\n");
    }
    if let Some(ref groups) = report.groups {
        body.push_str("  <groups>\n");
        for g in groups {
            body.push_str(&format!("    <group file=\"{}\">\n", escape_attr(&g.file)));
            for rm in &g.matches {
                append_match(&mut body, rm, view, "      ", &tree_opts);
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
            .with_meta(render_opts.include_meta)
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
    view: &ViewSet,
    indent: &str,
    render_opts: &RenderOptions,
) {
    let file_str = normalize_path(&rm.file);
    // Only include line/column attributes when they carry meaningful position info (non-zero)
    let has_position = rm.line > 0;
    if file_str.is_empty() {
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

    // Iterate ViewSet for declaration order
    for field in &view.fields {
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
                        out.push_str(&format!("{}<line>{}</line>\n", inner, escape(line)));
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
                    // render_opts has use_color=false here; the outer colorization pass handles it
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
            // File/Line/Column are attributes, not child elements; Summary/Count/Schema handled elsewhere
            _ => {}
        }
    }

    // message and rule_id always emitted when present (annotations, not view-gated)
    if let Some(ref message) = rm.message {
        out.push_str(&format!("{}<message>{}</message>\n", inner, escape(message)));
    }
    if let Some(ref rule_id) = rm.rule_id {
        out.push_str(&format!("{}<rule-id>{}</rule-id>\n", inner, escape(rule_id)));
    }

    out.push_str(&format!("{}</match>\n", indent));
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}
