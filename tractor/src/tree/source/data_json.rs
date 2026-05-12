//! JSON source emitter for [`DataIr`] — produces canonical JSON text
//! with optional span tracking for splice-based mutation.
//!
//! Mirrors [`crate::render::json`] but reads the typed [`DataIr`] tree
//! directly (no XmlNode intermediate). The span map keys each
//! [`DataIr`] node by its `span()` (line, column) and records the
//! byte range of its rendered value, so callers like
//! [`crate::mutation::xpath_upsert`] can splice byte regions of a
//! re-rendered tree back into the original source.

#![cfg(feature = "native")]

use std::collections::HashMap;

use crate::tree::data::DataIr;

/// `(line, col) → (rendered_start, rendered_end)` byte range map.
///
/// Same shape as [`crate::render::SpanMap`] so callers can lookup
/// using the same `(line, column)` keys they already harvest from the
/// IR's xot rendering.
pub type DataSpanMap = HashMap<(u32, u32), (usize, usize)>;

/// Options for JSON output formatting.
#[derive(Debug, Clone)]
pub struct JsonRenderOptions {
    /// Indentation string (e.g. two spaces, four spaces, tab).
    pub indent: String,
    /// Newline string (`"\n"` or `"\r\n"`).
    pub newline: String,
    /// Current indentation level (caller normally passes 0).
    pub indent_level: usize,
}

impl Default for JsonRenderOptions {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            newline: "\n".to_string(),
            indent_level: 0,
        }
    }
}

impl JsonRenderOptions {
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

/// Render a [`DataIr`] to JSON source text and a span map.
///
/// The trailing newline is appended after the final `}` / `]` /
/// scalar, matching [`crate::render::json::render_node_tracked`].
pub fn render_json_with_spans(
    ir: &DataIr,
    opts: &JsonRenderOptions,
) -> (String, DataSpanMap) {
    let mut buf = String::new();
    let mut spans = DataSpanMap::new();
    render_value(ir, opts, &mut buf, &mut spans);
    buf.push_str(&opts.newline);
    (buf, spans)
}

/// Convenience: render JSON without keeping the span map.
pub fn render_json(ir: &DataIr, opts: &JsonRenderOptions) -> String {
    render_json_with_spans(ir, opts).0
}

fn render_value(
    ir: &DataIr,
    opts: &JsonRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    let start = buf.len();
    match ir {
        DataIr::Document { children, .. } => {
            // Top-level document is a transparent wrapper. Render the
            // first non-comment child as the JSON value; everything
            // else is dropped (JSON has no comments).
            let payload = children
                .iter()
                .find(|c| !matches!(c, DataIr::Comment { .. }));
            if let Some(child) = payload {
                render_value(child, opts, buf, spans);
            } else {
                buf.push_str("null");
            }
        }
        DataIr::Mapping { pairs, .. } => render_object(pairs, opts, buf, spans),
        DataIr::Sequence { items, .. } => render_array(items, opts, buf, spans),
        DataIr::String { value, .. } => emit_string(value, buf),
        DataIr::Number { text, .. } => buf.push_str(text),
        DataIr::Bool { value, .. } => {
            buf.push_str(if *value { "true" } else { "false" });
        }
        DataIr::Null { .. } => buf.push_str("null"),
        DataIr::Pair { value, .. } => {
            // Top-level Pair without an enclosing mapping — render the
            // value alone. (Shouldn't normally happen; defensive.)
            render_value(value, opts, buf, spans);
        }
        DataIr::Section { children, .. } | DataIr::Directive { children, .. } => {
            // Sections and directives don't have a JSON projection;
            // render as a mapping over their child pairs.
            let pairs: Vec<&DataIr> = children
                .iter()
                .filter(|c| matches!(c, DataIr::Pair { .. }))
                .collect();
            if pairs.is_empty() {
                buf.push_str("{}");
            } else {
                let owned: Vec<DataIr> = pairs.iter().map(|p| (*p).clone()).collect();
                render_object(&owned, opts, buf, spans);
            }
        }
        DataIr::Comment { .. } => {
            // JSON has no comments; emit nothing.
            return;
        }
        DataIr::Element { children, .. } => {
            // Markdown-style synthetic element — render children in
            // sequence as a JSON array.
            render_array(children, opts, buf, spans);
        }
        DataIr::Unknown { .. } => buf.push_str("null"),
    }
    let end = buf.len();
    if end > start {
        let span = ir.span();
        spans.insert((span.line, span.column), (start, end));
    }
}

fn render_object(
    pairs: &[DataIr],
    opts: &JsonRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    let real: Vec<&DataIr> = pairs
        .iter()
        .filter(|p| matches!(p, DataIr::Pair { .. }))
        .collect();

    if real.is_empty() {
        buf.push_str("{}");
        return;
    }

    let inner = opts.indented();
    let inner_indent = inner.current_indent();
    let outer_indent = opts.current_indent();

    buf.push('{');

    for (i, pair) in real.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        buf.push_str(&opts.newline);
        buf.push_str(&inner_indent);

        if let DataIr::Pair { key, value, .. } = pair {
            // Key text — keys are typically String scalars; fall back
            // to range-slice text for unusual key types.
            let key_text = scalar_text(key);
            buf.push('"');
            buf.push_str(&escape_json_string(&key_text));
            buf.push_str("\": ");
            render_value(value, &inner, buf, spans);
        }
    }

    buf.push_str(&opts.newline);
    buf.push_str(&outer_indent);
    buf.push('}');
}

fn render_array(
    items: &[DataIr],
    opts: &JsonRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    if items.is_empty() {
        buf.push_str("[]");
        return;
    }

    let inner = opts.indented();
    let inner_indent = inner.current_indent();
    let outer_indent = opts.current_indent();

    buf.push('[');

    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        buf.push_str(&opts.newline);
        buf.push_str(&inner_indent);
        render_value(item, &inner, buf, spans);
    }

    buf.push_str(&opts.newline);
    buf.push_str(&outer_indent);
    buf.push(']');
}

fn scalar_text(ir: &DataIr) -> String {
    match ir {
        DataIr::String { value, .. } => value.clone(),
        DataIr::Number { text, .. } => text.clone(),
        DataIr::Bool { value, .. } => {
            if *value { "true".to_string() } else { "false".to_string() }
        }
        DataIr::Null { .. } => "null".to_string(),
        _ => String::new(),
    }
}

fn emit_string(value: &str, buf: &mut String) {
    buf.push('"');
    buf.push_str(&escape_json_string(value));
    buf.push('"');
}

fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::lower_json_data_root;

    fn lower(src: &str) -> DataIr {
        let language = tree_sitter_json::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(src, None).unwrap();
        lower_json_data_root(tree.root_node(), src)
    }

    #[test]
    fn flat_object_renders_canonically() {
        let ir = lower(r#"{"name": "Alice", "age": 30}"#);
        let out = render_json(&ir, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn nested_object() {
        let ir = lower(r#"{"db": {"host": "localhost", "port": 5432}}"#);
        let out = render_json(&ir, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["db"]["host"], "localhost");
        assert_eq!(parsed["db"]["port"], 5432);
    }

    #[test]
    fn array_with_mixed_scalars() {
        let ir = lower(r#"{"tags": ["a", "b", "c"]}"#);
        let out = render_json(&ir, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["tags"][0], "a");
        assert_eq!(parsed["tags"][2], "c");
    }

    #[test]
    fn bool_and_null() {
        let ir = lower(r#"{"a": true, "b": false, "c": null}"#);
        let out = render_json(&ir, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["a"], true);
        assert_eq!(parsed["b"], false);
        assert!(parsed["c"].is_null());
    }

    #[test]
    fn string_escaping() {
        // Bytes: `{"msg": "hello"}` — value's opening quote is at 8.
        let ir = lower(r#"{"msg": "hello"}"#);
        let mut ir = ir;
        let target = ir.find_at_offset_mut(8).unwrap();
        target
            .set_scalar("hi \"world\"\nbye", crate::tree::data::ScalarKind::String)
            .unwrap();
        let out = render_json(&ir, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["msg"], "hi \"world\"\nbye");
    }

    #[test]
    fn span_map_records_value_range_keyed_by_value_span() {
        let src = r#"{"name": "Alice"}"#;
        let ir = lower(src);
        let (out, spans) = render_json_with_spans(&ir, &JsonRenderOptions::default());

        // The String "Alice" has span at the original source position
        // of its opening quote (line 1, column 10 — 1-based column).
        // Look up the value span directly.
        let value_node = ir.find_at_offset(9).unwrap();
        let span = value_node.span();
        let key = (span.line, span.column);
        let (start, end) = spans.get(&key).unwrap_or_else(|| {
            panic!("no span recorded for {:?}; spans: {:?}", key, spans)
        });
        assert_eq!(&out[*start..*end], "\"Alice\"");
    }

    #[test]
    fn span_map_handles_nested_object_value() {
        let src = r#"{"db": {"host": "localhost"}}"#;
        let ir = lower(src);
        let (out, spans) = render_json_with_spans(&ir, &JsonRenderOptions::default());

        // The inner Mapping {"host": "localhost"} is the value of
        // the outer "db" pair. Find it via offset (the opening brace
        // of the inner object is at byte 7).
        let inner = ir.find_at_offset(7).unwrap();
        let span = inner.span();
        let key = (span.line, span.column);
        let (start, end) = spans
            .get(&key)
            .unwrap_or_else(|| panic!("no span for inner mapping at {:?}", key));
        // The recorded slice should start with `{` and end with `}`.
        assert!(out[*start..*end].starts_with('{'));
        assert!(out[*start..*end].ends_with('}'));
        assert!(out[*start..*end].contains("\"host\""));
    }

    #[test]
    fn mutation_then_render_roundtrips_via_span_map() {
        // End-to-end S4B-Z2 contract: mutate a value via DataIr
        // primitives, re-render, and use the span map to splice the
        // new bytes back into the original source.
        let src = r#"{"name": "Alice", "age": 30}"#;
        let mut ir = lower(src);

        // Locate value's original span before mutating (the offsets
        // are preserved through `set_scalar`).
        let value_offset_in_src = 9; // opening quote of "Alice"
        let pre = ir.find_at_offset(value_offset_in_src).unwrap();
        let pre_span = pre.span();
        let pre_range = pre.range();

        ir.find_at_offset_mut(value_offset_in_src)
            .unwrap()
            .set_scalar("Bob", crate::tree::data::ScalarKind::String)
            .unwrap();

        let (rendered, spans) = render_json_with_spans(&ir, &JsonRenderOptions::default());
        let key = (pre_span.line, pre_span.column);
        let (rs, re) = spans.get(&key).unwrap();
        let new_value_bytes = &rendered[*rs..*re];

        let mut new_source = src.to_string();
        new_source.replace_range(
            pre_range.start as usize..pre_range.end as usize,
            new_value_bytes,
        );
        // The splice should turn "Alice" into "Bob".
        assert_eq!(new_source, r#"{"name": "Bob", "age": 30}"#);
    }
}
