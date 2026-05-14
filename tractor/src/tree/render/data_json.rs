//! JSON source emitter for [`DataTree`] — produces canonical JSON text
//! with optional span tracking for splice-based mutation.
//!
//! Mirrors [`crate::render::json`] but reads the typed [`DataTree`] tree
//! directly (no XmlNode intermediate). The span map keys each
//! [`DataTree`] node by its `span()` (line, column) and records the
//! byte range of its rendered value, so callers like
//! [`crate::mutation::xpath_upsert`] can splice byte regions of a
//! re-rendered tree back into the original source.

#![cfg(feature = "native")]

use crate::tree::data::DataTree;
use super::data_common::{DataRenderOptions, DataSpanMap};

/// Legacy alias for the shared [`DataRenderOptions`]. Kept so existing
/// callers (e.g. [`crate::mutation::xpath_upsert`]) compile against
/// the renamed type without churn.
pub type JsonRenderOptions = DataRenderOptions;

/// Re-export of [`DataSpanMap`] under the old name. Both renderers
/// produce the same span-map shape.
#[allow(dead_code)]
pub type JsonDataSpanMap = DataSpanMap;

/// Render a [`DataTree`] to JSON source text and a span map.
///
/// The trailing newline is appended after the final `}` / `]` /
/// scalar, matching [`crate::render::json::render_node_tracked`].
pub fn render_json_with_spans(
    tree: &DataTree,
    opts: &JsonRenderOptions,
) -> (String, DataSpanMap) {
    let mut buf = String::new();
    let mut spans = DataSpanMap::new();
    render_value(tree, opts, &mut buf, &mut spans);
    buf.push_str(&opts.newline);
    (buf, spans)
}

/// Convenience: render JSON without keeping the span map.
pub fn render_json(tree: &DataTree, opts: &JsonRenderOptions) -> String {
    render_json_with_spans(tree, opts).0
}

fn render_value(
    tree: &DataTree,
    opts: &JsonRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    let start = buf.len();
    match tree {
        DataTree::Document { children, .. } => {
            // Top-level document is a transparent wrapper. Render the
            // first non-comment child as the JSON value; everything
            // else is dropped (JSON has no comments).
            let payload = children
                .iter()
                .find(|c| !matches!(c, DataTree::Comment { .. }));
            if let Some(child) = payload {
                render_value(child, opts, buf, spans);
            } else {
                buf.push_str("null");
            }
        }
        DataTree::Mapping { pairs, .. } => render_object(pairs, opts, buf, spans),
        DataTree::Sequence { items, .. } => render_array(items, opts, buf, spans),
        DataTree::String { value, .. } => emit_string(value, buf),
        DataTree::Number { text, .. } => buf.push_str(text),
        DataTree::Bool { value, .. } => {
            buf.push_str(if *value { "true" } else { "false" });
        }
        DataTree::Null { .. } => buf.push_str("null"),
        DataTree::Pair { value, .. } => {
            // Top-level Pair without an enclosing mapping — render the
            // value alone. (Shouldn't normally happen; defensive.)
            render_value(value, opts, buf, spans);
        }
        DataTree::Section { children, .. } | DataTree::Directive { children, .. } => {
            // Sections and directives don't have a JSON projection;
            // render as a mapping over their child pairs.
            let pairs: Vec<&DataTree> = children
                .iter()
                .filter(|c| matches!(c, DataTree::Pair { .. }))
                .collect();
            if pairs.is_empty() {
                buf.push_str("{}");
            } else {
                let owned: Vec<DataTree> = pairs.iter().map(|p| (*p).clone()).collect();
                render_object(&owned, opts, buf, spans);
            }
        }
        DataTree::Comment { .. } => {
            // JSON has no comments; emit nothing.
            return;
        }
        DataTree::Element { children, .. } => {
            // Markdown-style synthetic element — render children in
            // sequence as a JSON array.
            render_array(children, opts, buf, spans);
        }
        DataTree::Unknown { .. } => buf.push_str("null"),
    }
    let end = buf.len();
    if end > start {
        let span = tree.span();
        spans.insert((span.line, span.column), (start, end));
    }
}

fn render_object(
    pairs: &[DataTree],
    opts: &JsonRenderOptions,
    buf: &mut String,
    spans: &mut DataSpanMap,
) {
    let real: Vec<&DataTree> = pairs
        .iter()
        .filter(|p| matches!(p, DataTree::Pair { .. }))
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

        if let DataTree::Pair { key, value, .. } = pair {
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
    items: &[DataTree],
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

fn scalar_text(tree: &DataTree) -> String {
    match tree {
        DataTree::String { value, .. } => value.clone(),
        DataTree::Number { text, .. } => text.clone(),
        DataTree::Bool { value, .. } => {
            if *value { "true".to_string() } else { "false".to_string() }
        }
        DataTree::Null { .. } => "null".to_string(),
        _ => String::new(),
    }
}

fn emit_string(value: &str, buf: &mut String) {
    // Delegates to the shared [`write_quoted_scalar`] primitive
    // (Layer A) with JSON's escape table. Hard-codes `Double` because
    // JSON only allows `"..."` strings — `quote_style` on a DataTree
    // emitted as JSON is honoured only when round-trip-rendering YAML
    // with the YAML emitter.
    use crate::tree::render::common::write_quoted_scalar;
    use crate::tree::types::QuoteStyle;
    write_quoted_scalar(value, &QuoteStyle::Double, escape_json_string, buf);
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

    fn lower(src: &str) -> DataTree {
        let language = tree_sitter_json::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(src, None).unwrap();
        lower_json_data_root(tree.root_node(), src)
    }

    #[test]
    fn flat_object_renders_canonically() {
        let tree = lower(r#"{"name": "Alice", "age": 30}"#);
        let out = render_json(&tree, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn nested_object() {
        let tree = lower(r#"{"db": {"host": "localhost", "port": 5432}}"#);
        let out = render_json(&tree, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["db"]["host"], "localhost");
        assert_eq!(parsed["db"]["port"], 5432);
    }

    #[test]
    fn array_with_mixed_scalars() {
        let tree = lower(r#"{"tags": ["a", "b", "c"]}"#);
        let out = render_json(&tree, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["tags"][0], "a");
        assert_eq!(parsed["tags"][2], "c");
    }

    #[test]
    fn bool_and_null() {
        let tree = lower(r#"{"a": true, "b": false, "c": null}"#);
        let out = render_json(&tree, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["a"], true);
        assert_eq!(parsed["b"], false);
        assert!(parsed["c"].is_null());
    }

    #[test]
    fn string_escaping() {
        // Bytes: `{"msg": "hello"}` — value's opening quote is at 8.
        let tree = lower(r#"{"msg": "hello"}"#);
        let mut tree = tree;
        let target = tree.find_at_offset_mut(8).unwrap();
        target
            .set_scalar("hi \"world\"\nbye", crate::tree::data::ScalarKind::String)
            .unwrap();
        let out = render_json(&tree, &JsonRenderOptions::default());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["msg"], "hi \"world\"\nbye");
    }

    #[test]
    fn span_map_records_value_range_keyed_by_value_span() {
        let src = r#"{"name": "Alice"}"#;
        let tree = lower(src);
        let (out, spans) = render_json_with_spans(&tree, &JsonRenderOptions::default());

        // The String "Alice" has span at the original source position
        // of its opening quote (line 1, column 10 — 1-based column).
        // Look up the value span directly.
        let value_node = tree.find_at_offset(9).unwrap();
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
        let tree = lower(src);
        let (out, spans) = render_json_with_spans(&tree, &JsonRenderOptions::default());

        // The inner Mapping {"host": "localhost"} is the value of
        // the outer "db" pair. Find it via offset (the opening brace
        // of the inner object is at byte 7).
        let inner = tree.find_at_offset(7).unwrap();
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
        // End-to-end S4B-Z2 contract: mutate a value via DataTree
        // primitives, re-render, and use the span map to splice the
        // new bytes back into the original source.
        let src = r#"{"name": "Alice", "age": 30}"#;
        let mut tree = lower(src);

        // Locate value's original span before mutating (the offsets
        // are preserved through `set_scalar`).
        let value_offset_in_src = 9; // opening quote of "Alice"
        let pre = tree.find_at_offset(value_offset_in_src).unwrap();
        let pre_span = pre.span();
        let pre_range = pre.range();

        tree.find_at_offset_mut(value_offset_in_src)
            .unwrap()
            .set_scalar("Bob", crate::tree::data::ScalarKind::String)
            .unwrap();

        let (rendered, spans) = render_json_with_spans(&tree, &JsonRenderOptions::default());
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
