//! YAML source emitter for [`DataTree`] — produces canonical YAML
//! text with optional span tracking for splice-based mutation.
//!
//! Mirrors [`crate::render::yaml`] but reads the typed [`DataTree`]
//! tree directly (no XmlNode intermediate). The span map keys each
//! [`DataTree`] node by its `span()` (line, column) and records the
//! byte range of its rendered value.

#![cfg(feature = "native")]

use std::collections::HashMap;

use crate::tree::data::DataTree;

/// `(line, col) → (rendered_start, rendered_end)` byte range map.
pub type DataSpanMap = HashMap<(u32, u32), (usize, usize)>;

/// Options for YAML output formatting.
#[derive(Debug, Clone)]
pub struct YamlRenderOptions {
    pub indent: String,
    pub newline: String,
    pub indent_level: usize,
}

impl Default for YamlRenderOptions {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            newline: "\n".to_string(),
            indent_level: 0,
        }
    }
}

impl YamlRenderOptions {
    fn indented(&self) -> Self {
        Self {
            indent: self.indent.clone(),
            newline: self.newline.clone(),
            indent_level: self.indent_level + 1,
        }
    }

    fn current_indent(&self) -> String {
        self.indent.repeat(self.indent_level)
    }
}

/// Render a [`DataTree`] to YAML source text and a span map.
pub fn render_yaml_with_spans(
    ir: &DataTree,
    opts: &YamlRenderOptions,
) -> (String, DataSpanMap) {
    let mut buf = String::new();
    let mut spans = DataSpanMap::new();
    render_top(ir, opts, &mut buf, &mut spans);
    if !buf.ends_with('\n') {
        buf.push_str(&opts.newline);
    }
    (buf, spans)
}

/// Convenience: render YAML without keeping the span map.
pub fn render_yaml(ir: &DataTree, opts: &YamlRenderOptions) -> String {
    render_yaml_with_spans(ir, opts).0
}

fn render_top(
    ir: &DataTree,
    opts: &YamlRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    match ir {
        DataTree::Document { children, .. } => {
            // Stream of documents (multi-doc YAML): each child is itself
            // a Document, emit `---` between them.
            let is_stream = !children.is_empty()
                && children.iter().all(|c| matches!(c, DataTree::Document { .. }));
            if is_stream {
                for (i, doc) in children.iter().enumerate() {
                    if i > 0 {
                        buf.push_str("---");
                        buf.push_str(&opts.newline);
                    }
                    render_top(doc, opts, buf, spans);
                }
            } else {
                let payload = children
                    .iter()
                    .find(|c| !matches!(c, DataTree::Comment { .. }));
                if let Some(child) = payload {
                    render_value(child, opts, buf, true, spans);
                }
            }
        }
        _ => {
            render_value(ir, opts, buf, true, spans);
        }
    }
}

fn render_value(
    ir: &DataTree,
    opts: &YamlRenderOptions,
    buf: &mut String,
    at_top: bool,
    spans: &mut DataSpanMap,
) {
    let start = buf.len();
    match ir {
        DataTree::Mapping { pairs, .. } => {
            if pairs.iter().all(|p| !matches!(p, DataTree::Pair { .. })) {
                buf.push_str("{}");
            } else if at_top {
                render_mapping(pairs, opts, buf, spans);
            } else {
                buf.push_str(&opts.newline);
                render_mapping(pairs, &opts.indented(), buf, spans);
            }
        }
        DataTree::Sequence { items, .. } => {
            if items.is_empty() {
                buf.push_str("[]");
            } else if at_top {
                render_sequence(items, opts, buf, spans);
            } else {
                buf.push_str(&opts.newline);
                render_sequence(items, &opts.indented(), buf, spans);
            }
        }
        DataTree::String { value, .. } => emit_scalar_string(value, buf),
        DataTree::Number { text, .. } => buf.push_str(text),
        DataTree::Bool { value, .. } => {
            buf.push_str(if *value { "true" } else { "false" });
        }
        DataTree::Null { .. } => buf.push_str("null"),
        DataTree::Pair { value, .. } => {
            // Bare Pair as a value — defensive; render value alone.
            render_value(value, opts, buf, at_top, spans);
        }
        DataTree::Document { .. } => {
            // Nested document — render via render_top.
            render_top(ir, opts, buf, spans);
        }
        DataTree::Section { children, .. } | DataTree::Directive { children, .. } => {
            let pairs: Vec<&DataTree> = children
                .iter()
                .filter(|c| matches!(c, DataTree::Pair { .. }))
                .collect();
            if pairs.is_empty() {
                buf.push_str("{}");
            } else {
                let owned: Vec<DataTree> = pairs.iter().map(|p| (*p).clone()).collect();
                if at_top {
                    render_mapping(&owned, opts, buf, spans);
                } else {
                    buf.push_str(&opts.newline);
                    render_mapping(&owned, &opts.indented(), buf, spans);
                }
            }
        }
        DataTree::Comment { .. } => return,
        DataTree::Element { children, .. } => {
            if children.is_empty() {
                buf.push_str("[]");
            } else if at_top {
                render_sequence(children, opts, buf, spans);
            } else {
                buf.push_str(&opts.newline);
                render_sequence(children, &opts.indented(), buf, spans);
            }
        }
        DataTree::Unknown { .. } => buf.push_str("null"),
    }
    let end = buf.len();
    if end > start {
        let span = ir.span();
        spans.insert((span.line, span.column), (start, end));
    }
}

fn render_mapping(
    pairs: &[DataTree],
    opts: &YamlRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    let indent = opts.current_indent();

    for pair in pairs {
        let DataTree::Pair { key, value, .. } = pair else { continue };

        let key_text = scalar_text(key);
        let key_str = yaml_quote_key(&key_text);

        match value.as_ref() {
            DataTree::Mapping { pairs: inner, .. } if inner.iter().any(|p| matches!(p, DataTree::Pair { .. })) => {
                buf.push_str(&indent);
                buf.push_str(&key_str);
                buf.push(':');
                buf.push_str(&opts.newline);
                let block_start = buf.len();
                render_mapping(inner, &opts.indented(), buf, spans);
                record_value_span(value, block_start, buf.len(), spans);
            }
            DataTree::Sequence { items, .. } if !items.is_empty() => {
                buf.push_str(&indent);
                buf.push_str(&key_str);
                buf.push(':');
                buf.push_str(&opts.newline);
                let block_start = buf.len();
                render_sequence(items, &opts.indented(), buf, spans);
                record_value_span(value, block_start, buf.len(), spans);
            }
            _ => {
                buf.push_str(&indent);
                buf.push_str(&key_str);
                buf.push_str(": ");
                let value_start = buf.len();
                render_inline_value(value, buf);
                record_value_span(value, value_start, buf.len(), spans);
                buf.push_str(&opts.newline);
            }
        }
    }
}

fn render_sequence(
    items: &[DataTree],
    opts: &YamlRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    let indent = opts.current_indent();

    for item in items {
        buf.push_str(&indent);
        buf.push_str("- ");
        match item {
            DataTree::Mapping { pairs, .. } if pairs.iter().any(|p| matches!(p, DataTree::Pair { .. })) => {
                // Inline first pair on the same line as `-`, rest indented.
                render_sequence_mapping_item(pairs, opts, buf, spans);
            }
            DataTree::Sequence { items: inner, .. } if !inner.is_empty() => {
                buf.push_str(&opts.newline);
                let block_start = buf.len();
                render_sequence(inner, &opts.indented(), buf, spans);
                record_value_span(item, block_start, buf.len(), spans);
            }
            _ => {
                let value_start = buf.len();
                render_inline_value(item, buf);
                record_value_span(item, value_start, buf.len(), spans);
                buf.push_str(&opts.newline);
            }
        }
    }
}

fn render_sequence_mapping_item(
    pairs: &[DataTree],
    opts: &YamlRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    let inner = opts.indented();
    let indent = inner.current_indent();
    let mut first = true;

    for pair in pairs {
        let DataTree::Pair { key, value, .. } = pair else { continue };
        let key_text = scalar_text(key);
        let key_str = yaml_quote_key(&key_text);

        if !first {
            buf.push_str(&indent);
        }
        buf.push_str(&key_str);
        match value.as_ref() {
            DataTree::Mapping { pairs: inner_pairs, .. } if inner_pairs.iter().any(|p| matches!(p, DataTree::Pair { .. })) => {
                buf.push(':');
                buf.push_str(&opts.newline);
                let block_start = buf.len();
                render_mapping(inner_pairs, &inner.indented(), buf, spans);
                record_value_span(value, block_start, buf.len(), spans);
            }
            DataTree::Sequence { items, .. } if !items.is_empty() => {
                buf.push(':');
                buf.push_str(&opts.newline);
                let block_start = buf.len();
                render_sequence(items, &inner.indented(), buf, spans);
                record_value_span(value, block_start, buf.len(), spans);
            }
            _ => {
                buf.push_str(": ");
                let value_start = buf.len();
                render_inline_value(value, buf);
                record_value_span(value, value_start, buf.len(), spans);
                buf.push_str(&opts.newline);
            }
        }
        first = false;
    }
}

fn render_inline_value(ir: &DataTree, buf: &mut String) {
    match ir {
        DataTree::String { value, .. } => emit_scalar_string(value, buf),
        DataTree::Number { text, .. } => buf.push_str(text),
        DataTree::Bool { value, .. } => {
            buf.push_str(if *value { "true" } else { "false" });
        }
        DataTree::Null { .. } => buf.push_str("null"),
        DataTree::Mapping { pairs, .. } if pairs.iter().all(|p| !matches!(p, DataTree::Pair { .. })) => {
            buf.push_str("{}");
        }
        DataTree::Sequence { items, .. } if items.is_empty() => {
            buf.push_str("[]");
        }
        DataTree::Mapping { .. } | DataTree::Sequence { .. } => {
            // Should never reach here — block render path handles
            // non-empty composite values. Fallback: empty braces.
            buf.push_str("{}");
        }
        DataTree::Pair { value, .. } => render_inline_value(value, buf),
        DataTree::Comment { .. }
        | DataTree::Document { .. }
        | DataTree::Section { .. }
        | DataTree::Directive { .. }
        | DataTree::Element { .. }
        | DataTree::Unknown { .. } => buf.push_str("null"),
    }
}

fn record_value_span(
    value: &DataTree,
    start: usize,
    end: usize,
    spans: &mut DataSpanMap,
) {
    if end > start {
        let span = value.span();
        spans.insert((span.line, span.column), (start, end));
    }
}

fn scalar_text(ir: &DataTree) -> String {
    match ir {
        DataTree::String { value, .. } => value.clone(),
        DataTree::Number { text, .. } => text.clone(),
        DataTree::Bool { value, .. } => {
            if *value { "true".to_string() } else { "false".to_string() }
        }
        DataTree::Null { .. } => "null".to_string(),
        _ => String::new(),
    }
}

fn emit_scalar_string(value: &str, buf: &mut String) {
    if needs_yaml_quoting(value) {
        yaml_quote_string(value, buf);
    } else {
        buf.push_str(value);
    }
}

fn needs_yaml_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    match s {
        "true" | "false" | "True" | "False" | "TRUE" | "FALSE"
        | "yes" | "no" | "Yes" | "No" | "YES" | "NO"
        | "on" | "off" | "On" | "Off" | "ON" | "OFF"
        | "null" | "Null" | "NULL" | "~" => return true,
        _ => {}
    }
    if s.contains(':') || s.contains('#') || s.contains('\n')
        || s.contains('"') || s.contains('\'')
        || s.starts_with('&') || s.starts_with('*')
        || s.starts_with('!') || s.starts_with('|')
        || s.starts_with('>') || s.starts_with('%')
        || s.starts_with('@') || s.starts_with('`')
        || s.starts_with('{') || s.starts_with('}')
        || s.starts_with('[') || s.starts_with(']')
        || s.starts_with(',') || s.starts_with('?')
        || s.starts_with('-') || s.starts_with(' ')
        || s.ends_with(' ')
    {
        return true;
    }
    false
}

fn yaml_quote_string(s: &str, buf: &mut String) {
    buf.push('"');
    for c in s.chars() {
        match c {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                buf.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => buf.push(c),
        }
    }
    buf.push('"');
}

fn yaml_quote_key(key: &str) -> String {
    if needs_yaml_quoting(key) {
        let mut out = String::new();
        yaml_quote_string(key, &mut out);
        out
    } else {
        key.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::lower_yaml_data_root;

    fn lower(src: &str) -> DataTree {
        let language = tree_sitter_yaml::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(src, None).unwrap();
        lower_yaml_data_root(tree.root_node(), src)
    }

    #[test]
    fn flat_mapping() {
        let src = "name: Alice\nage: 30\n";
        let ir = lower(src);
        let out = render_yaml(&ir, &YamlRenderOptions::default());
        assert_eq!(out, "name: Alice\nage: 30\n");
    }

    #[test]
    fn nested_mapping() {
        let src = "db:\n  host: localhost\n  port: 5432\n";
        let ir = lower(src);
        let out = render_yaml(&ir, &YamlRenderOptions::default());
        assert!(out.contains("db:"));
        assert!(out.contains("  host: localhost"));
        assert!(out.contains("  port: 5432"));
    }

    #[test]
    fn sequence_value() {
        let src = "tags:\n  - a\n  - b\n";
        let ir = lower(src);
        let out = render_yaml(&ir, &YamlRenderOptions::default());
        assert!(out.contains("tags:"));
        assert!(out.contains("  - a"));
        assert!(out.contains("  - b"));
    }

    #[test]
    fn boolean_string_quoting() {
        let src = "name: Alice\n";
        let mut ir = lower(src);
        // Replace value with the string "true" — must be quoted.
        let target = ir.find_at_offset_mut(6).unwrap();
        target
            .set_scalar("true", crate::tree::data::ScalarKind::String)
            .unwrap();
        let out = render_yaml(&ir, &YamlRenderOptions::default());
        assert!(
            out.contains("\"true\""),
            "expected quoted boolean string, got: {}",
            out,
        );
    }

    #[test]
    fn span_map_records_scalar_value_span() {
        let src = "name: Alice\n";
        let ir = lower(src);
        let (out, spans) = render_yaml_with_spans(&ir, &YamlRenderOptions::default());

        // Find the value of the only pair via byte offset (start of "Alice")
        let value_node = ir.find_at_offset(6).unwrap();
        let span = value_node.span();
        let key = (span.line, span.column);
        let (start, end) = spans
            .get(&key)
            .unwrap_or_else(|| panic!("no span for {:?}; spans={:?}", key, spans));
        assert_eq!(&out[*start..*end], "Alice");
    }

    #[test]
    fn mutation_then_render_roundtrips_via_span_map() {
        let src = "name: Alice\nage: 30\n";
        let mut ir = lower(src);

        let value_offset = 6;
        let pre = ir.find_at_offset(value_offset).unwrap();
        let pre_span = pre.span();
        let pre_range = pre.range();

        ir.find_at_offset_mut(value_offset)
            .unwrap()
            .set_scalar("Bob", crate::tree::data::ScalarKind::String)
            .unwrap();

        let (rendered, spans) = render_yaml_with_spans(&ir, &YamlRenderOptions::default());
        let key = (pre_span.line, pre_span.column);
        let (rs, re) = spans.get(&key).unwrap();
        let new_value_bytes = &rendered[*rs..*re];

        let mut new_source = src.to_string();
        new_source.replace_range(
            pre_range.start as usize..pre_range.end as usize,
            new_value_bytes,
        );
        assert_eq!(new_source, "name: Bob\nage: 30\n");
    }
}
