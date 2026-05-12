//! TOML tree-sitter CST → [`DataIr`] lowering.
//!
//! TOML's natural projection is the data branch (key-as-element-
//! name): `[database]\nhost = "localhost"` becomes
//! `<database><host>localhost</host></database>`. The renderer
//! `render_data_to_xot_keyed` provides that shape; this lowering
//! produces a structural `DataIr` tree that maps cleanly onto it.
//!
//! Mapping:
//!   - `document` → [`DataIr::Document`] containing pairs +
//!     section children
//!   - `table` → [`DataIr::Section`] (one per `[name]` header)
//!   - `pair` → [`DataIr::Pair`] (key = bare/dotted/quoted key
//!     string; value = scalar / array / inline_table)
//!   - scalar kinds → [`DataIr::String`] / [`DataIr::Number`] /
//!     [`DataIr::Bool`]
//!   - `array` → [`DataIr::Sequence`]
//!   - `inline_table` → [`DataIr::Mapping`]

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::data::DataIr;
use super::lower_helpers::{range_of, span_of, text_of};
use super::types::{ByteRange, Span};

pub fn lower_toml_data_root(root: TsNode<'_>, source: &str) -> DataIr {
    lower_node(root, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> DataIr {
    let range = range_of(node);
    let span = span_of(node);

    match node.kind() {
        "document" => DataIr::Document {
            children: collapse_array_of_tables(lower_named_children(node, source)),
            range,
            span,
        },
        "table" | "table_array_element" => {
            // First named child is the bare / dotted / quoted key
            // (the table header); remaining children are pairs.
            let mut cursor = node.walk();
            let mut named = node.named_children(&mut cursor);
            let header = match named.next() {
                Some(h) => h,
                None => return DataIr::Unknown {
                    kind: "empty table".to_string(),
                    range,
                    span,
                },
            };

            // Lower the body pairs once so we can wrap them in
            // nested sections if the header is dotted.
            let mut body: Vec<DataIr> = Vec::new();
            let mut c2 = node.walk();
            for child in node.named_children(&mut c2).skip(1) {
                body.push(lower_node(child, source));
            }

            // Collect the header's segments. A `dotted_key` has
            // multiple `bare_key` / `quoted_key` named children;
            // a plain `bare_key` is a single segment.
            let segments = collect_header_segments(header, source);

            // Build nested DataIr::Section from inside out so
            // `[a.b.c]` becomes Section(a, [Section(b, [Section(c, body)])]).
            let array_of_tables = node.kind() == "table_array_element";
            let mut current = if array_of_tables {
                // `[[name]]` — the deepest section's body wraps in a
                // single-item Sequence so a later post-pass /
                // renderer can recognize it as an array element.
                // Mark the section's name with the special marker
                // by wrapping body in a Sequence below; for now,
                // emit a Section whose first child is a marker
                // pseudo-pair `__array_of_tables__ = true`.
                let mut tagged_body = Vec::with_capacity(body.len() + 1);
                tagged_body.push(DataIr::Pair {
                    key: Box::new(DataIr::String {
                        value: "__array_of_tables__".to_string(),
                        range,
                        span,
                    }),
                    value: Box::new(DataIr::Bool {
                        value: true,
                        range,
                        span,
                    }),
                    range,
                    span,
                });
                tagged_body.extend(body);
                tagged_body
            } else {
                body
            };
            // Walk segments outermost-first; build Sections inside-out.
            for (idx, seg) in segments.iter().enumerate().rev() {
                let seg_ir = DataIr::String {
                    value: seg.0.clone(),
                    range: seg.1,
                    span: seg.2,
                };
                if idx == segments.len() - 1 {
                    current = vec![DataIr::Section {
                        name: Box::new(seg_ir),
                        children: std::mem::take(&mut current),
                        range,
                        span,
                    }];
                } else {
                    current = vec![DataIr::Section {
                        name: Box::new(seg_ir),
                        children: std::mem::take(&mut current),
                        range,
                        span,
                    }];
                }
            }
            // `current` is the outermost Section wrapped in a
            // singleton Vec.
            current.into_iter().next().unwrap_or(DataIr::Unknown {
                kind: "empty section".to_string(),
                range,
                span,
            })
        }
        "pair" => {
            let mut cursor = node.walk();
            let mut named = node.named_children(&mut cursor);
            let key = named.next();
            let value = named.next();
            match (key, value) {
                (Some(k), Some(v)) => DataIr::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(lower_node(v, source)),
                    range,
                    span,
                },
                _ => DataIr::Unknown {
                    kind: "pair (missing key/value)".to_string(),
                    range,
                    span,
                },
            }
        }
        "bare_key" | "quoted_key" => {
            // Key text used as an element name in the keyed
            // renderer. Strip surrounding quotes for quoted_key.
            let raw = text_of(node, source);
            let value = if node.kind() == "quoted_key" {
                strip_quotes(&raw)
            } else {
                raw
            };
            DataIr::String { value, range, span }
        }
        "dotted_key" => {
            // Dotted key like `foo.bar.baz` — flatten to a single
            // string for now (the keyed renderer can split if
            // needed later).
            DataIr::String {
                value: text_of(node, source),
                range,
                span,
            }
        }
        "string" => DataIr::String {
            value: strip_quotes(&text_of(node, source)),
            range,
            span,
        },
        "integer" | "float" => DataIr::Number {
            text: text_of(node, source),
            range,
            span,
        },
        "boolean" => {
            let value = text_of(node, source).trim() == "true";
            DataIr::Bool { value, range, span }
        }
        "local_date" | "local_time" | "local_date_time" | "offset_date_time" => {
            // Treat dates / times as strings for now — they're
            // valid TOML scalars but tree-sitter tags them
            // separately. Future work could give them a dedicated
            // variant.
            DataIr::String {
                value: text_of(node, source),
                range,
                span,
            }
        }
        "array" => DataIr::Sequence {
            items: lower_named_children(node, source),
            range,
            span,
        },
        "inline_table" => DataIr::Mapping {
            pairs: lower_named_children(node, source),
            range,
            span,
        },
        "comment" => {
            let raw = text_of(node, source);
            let text = raw.strip_prefix('#').map(|s| s.trim_start().to_string())
                .unwrap_or(raw);
            DataIr::Comment {
                text,
                leading: true,
                trailing: false,
                range,
                span,
            }
        }
        other => DataIr::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// Collapse adjacent `[[name]]` table-array sections into a single
/// `Section { name, children: [Sequence([item1, item2, ...])] }`.
///
/// Each `[[name]]` lowers to a `Section` whose body starts with the
/// `__array_of_tables__: true` pseudo-pair. Consecutive sections
/// with the same name and that marker collapse into one outer
/// section whose body is a single Sequence of inner item-bodies.
fn collapse_array_of_tables(children: Vec<DataIr>) -> Vec<DataIr> {
    let mut out: Vec<DataIr> = Vec::new();
    let mut pending: Option<(String, ByteRange, Span, Vec<Vec<DataIr>>, ByteRange, Span)> = None;
    for child in children {
        if let Some((name, accum_range, accum_span)) = aot_key(&child) {
            // Pull the inner body (drop the marker pseudo-pair).
            let inner_body = match &child {
                DataIr::Section { children, .. } => children
                    .iter()
                    .filter(|c| !matches!(c, DataIr::Pair { key, value, .. }
                        if matches!(key.as_ref(), DataIr::String { value: k, .. } if k == "__array_of_tables__")
                            && matches!(value.as_ref(), DataIr::Bool { .. })))
                    .cloned()
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };
            match pending.take() {
                Some((p_name, p_name_range, p_name_span, mut bodies, p_range, _p_span))
                    if p_name == name =>
                {
                    bodies.push(inner_body);
                    let new_range = ByteRange::new(p_range.start, accum_range.end);
                    pending = Some((p_name, p_name_range, p_name_span, bodies, new_range, accum_span));
                }
                Some(prev) => {
                    out.push(emit_aot_section(prev));
                    pending = Some((name, accum_range, accum_span, vec![inner_body], accum_range, accum_span));
                }
                None => {
                    pending = Some((name, accum_range, accum_span, vec![inner_body], accum_range, accum_span));
                }
            }
        } else {
            if let Some(p) = pending.take() {
                out.push(emit_aot_section(p));
            }
            out.push(child);
        }
    }
    if let Some(p) = pending.take() {
        out.push(emit_aot_section(p));
    }
    out
}

/// If `ir` is a `[[name]]` array-of-tables Section (i.e. starts
/// with `__array_of_tables__: true`), return its name + ranges.
fn aot_key(ir: &DataIr) -> Option<(String, ByteRange, Span)> {
    let DataIr::Section { name, children, .. } = ir else { return None };
    let DataIr::String { value: name_str, range, span } = name.as_ref() else { return None };
    let first = children.first()?;
    let DataIr::Pair { key, value, .. } = first else { return None };
    let DataIr::String { value: key_str, .. } = key.as_ref() else { return None };
    if key_str != "__array_of_tables__" { return None; }
    let DataIr::Bool { value: true, .. } = value.as_ref() else { return None };
    Some((name_str.clone(), *range, *span))
}

/// Build a single Section that wraps a Sequence of accumulated
/// inner bodies. The resulting shape renders as
/// `<name><item>...</item><item>...</item></name>` under the keyed
/// renderer.
fn emit_aot_section(
    state: (String, ByteRange, Span, Vec<Vec<DataIr>>, ByteRange, Span),
) -> DataIr {
    let (name, name_range, name_span, bodies, full_range, full_span) = state;
    let items: Vec<DataIr> = bodies
        .into_iter()
        .map(|b| DataIr::Mapping {
            pairs: b,
            range: full_range,
            span: full_span,
        })
        .collect();
    let seq = DataIr::Sequence {
        items,
        range: full_range,
        span: full_span,
    };
    DataIr::Section {
        name: Box::new(DataIr::String {
            value: name,
            range: name_range,
            span: name_span,
        }),
        children: vec![seq],
        range: full_range,
        span: full_span,
    }
}

/// Pull the ordered list of `(text, range, span)` segments out of a
/// table header (`bare_key` / `quoted_key` / `dotted_key`). For a
/// plain bare key this is a single-element vec; for dotted keys it
/// expands to one entry per dot-separated segment.
fn collect_header_segments(
    node: TsNode<'_>,
    source: &str,
) -> Vec<(String, ByteRange, Span)> {
    match node.kind() {
        "dotted_key" => {
            let mut out = Vec::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                out.extend(collect_header_segments(child, source));
            }
            out
        }
        "quoted_key" => {
            let raw = text_of(node, source);
            vec![(strip_quotes(&raw), range_of(node), span_of(node))]
        }
        _ => vec![(text_of(node, source), range_of(node), span_of(node))],
    }
}

fn lower_named_children(node: TsNode<'_>, source: &str) -> Vec<DataIr> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .map(|c| lower_node(c, source))
        .collect()
}

fn strip_quotes(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        raw.to_string()
    }
}

