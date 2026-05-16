//! TOML tree-sitter CST → [`DataTree`] lowering.
//!
//! TOML's natural projection is the data branch (key-as-element-
//! name): `[database]\nhost = "localhost"` becomes
//! `<database><host>localhost</host></database>`. The renderer
//! `render_data_to_xot_keyed` provides that shape; this lowering
//! produces a structural `DataTree` tree that maps cleanly onto it.
//!
//! Mapping:
//!   - `document` → [`DataTree::Document`] containing pairs +
//!     section children
//!   - `table` → [`DataTree::Section`] (one per `[name]` header)
//!   - `pair` → [`DataTree::Pair`] (key = bare/dotted/quoted key
//!     string; value = scalar / array / inline_table)
//!   - scalar kinds → [`DataTree::String`] / [`DataTree::Number`] /
//!     [`DataTree::Bool`]
//!   - `array` → [`DataTree::Sequence`]
//!   - `inline_table` → [`DataTree::Mapping`]


use crate::raw::RawNode;

use crate::tree::DataTree;
use crate::tree::lower_helpers::{range_of, span_of, text_of};
use crate::tree::types::{ByteRange, QuoteStyle, Span};

pub fn lower_toml_data_root(root: &RawNode, source: &str) -> DataTree {
    lower_node(root, source)
}

fn lower_node(node: &RawNode, source: &str) -> DataTree {
    let range = range_of(node);
    let span = span_of(node);

    match node.kind() {
        "document" => DataTree::Document {
            children: collapse_array_of_tables(lower_named_children(node, source)),
            range,
            span,
        },
        "table" | "table_array_element" => {
            // First named child is the bare / dotted / quoted key
            // (the table header); remaining children are pairs.
            let mut named = node.named_children();
            let header = match named.next() {
                Some(h) => h,
                None => return DataTree::Unknown {
                    kind: "empty table".to_string(),
                    range,
                    span,
                },
            };

            // Lower the body pairs once so we can wrap them in
            // nested sections if the header is dotted.
            let mut body: Vec<DataTree> = Vec::new();
            for child in node.named_children().skip(1) {
                body.push(lower_node(child, source));
            }

            // Collect the header's segments. A `dotted_key` has
            // multiple `bare_key` / `quoted_key` named children;
            // a plain `bare_key` is a single segment.
            let segments = collect_header_segments(header, source);

            // Build nested DataTree::Section from inside out so
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
                tagged_body.push(DataTree::Pair {
                    key: Box::new(DataTree::String {
                        value: "__array_of_tables__".to_string(),
                        quote_style: QuoteStyle::Plain,
                        range,
                        span,
                    }),
                    value: Box::new(DataTree::Bool {
                        value: true,
                        text: "true".to_string(),
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
                let seg_ir = DataTree::String {
                    value: seg.0.clone(),
                    quote_style: QuoteStyle::Plain,
                    range: seg.1,
                    span: seg.2,
                };
                if idx == segments.len() - 1 {
                    current = vec![DataTree::Section {
                        name: Box::new(seg_ir),
                        children: std::mem::take(&mut current),
                        range,
                        span,
                    }];
                } else {
                    current = vec![DataTree::Section {
                        name: Box::new(seg_ir),
                        children: std::mem::take(&mut current),
                        range,
                        span,
                    }];
                }
            }
            // `current` is the outermost Section wrapped in a
            // singleton Vec.
            current.into_iter().next().unwrap_or(DataTree::Unknown {
                kind: "empty section".to_string(),
                range,
                span,
            })
        }
        "pair" => {
            let mut named = node.named_children();
            let key = named.next();
            let value = named.next();
            match (key, value) {
                (Some(k), Some(v)) => DataTree::Pair {
                    key: Box::new(lower_node(k, source)),
                    value: Box::new(lower_node(v, source)),
                    range,
                    span,
                },
                _ => DataTree::Unknown {
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
            let (value, quote_style) = if node.kind() == "quoted_key" {
                (strip_quotes(&raw), QuoteStyle::Double)
            } else {
                (raw, QuoteStyle::Plain)
            };
            DataTree::String { value, quote_style, range, span }
        }
        "dotted_key" => {
            // Dotted key like `foo.bar.baz` — flatten to a single
            // string for now (the keyed renderer can split if
            // needed later).
            DataTree::String {
                value: text_of(node, source),
                quote_style: QuoteStyle::Plain,
                range,
                span,
            }
        }
        "string" => DataTree::String {
            value: strip_quotes(&text_of(node, source)),
            quote_style: QuoteStyle::Double,
            range,
            span,
        },
        "integer" | "float" => DataTree::Number {
            text: text_of(node, source),
            range,
            span,
        },
        "boolean" => {
            let raw = text_of(node, source);
            let value = raw.trim() == "true";
            DataTree::Bool { value, text: raw, range, span }
        }
        "local_date" | "local_time" | "local_date_time" | "offset_date_time" => {
            // Treat dates / times as strings for now — they're
            // valid TOML scalars but tree-sitter tags them
            // separately. Future work could give them a dedicated
            // variant.
            DataTree::String {
                value: text_of(node, source),
                quote_style: QuoteStyle::Plain,
                range,
                span,
            }
        }
        "array" => DataTree::Sequence {
            items: lower_named_children(node, source),
            range,
            span,
        },
        "inline_table" => DataTree::Mapping {
            pairs: lower_named_children(node, source),
            range,
            span,
        },
        "comment" => {
            let raw = text_of(node, source);
            let text = raw.strip_prefix('#').map(|s| s.trim_start().to_string())
                .unwrap_or(raw);
            DataTree::Comment {
                text,
                leading: crate::tree::types::Flag::implicit_at(span.line, span.column),
                trailing: crate::tree::types::Flag::Off,
                range,
                span,
            }
        }
        other => DataTree::Unknown {
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
fn collapse_array_of_tables(children: Vec<DataTree>) -> Vec<DataTree> {
    let mut out: Vec<DataTree> = Vec::new();
    let mut pending: Option<(String, ByteRange, Span, Vec<Vec<DataTree>>, ByteRange, Span)> = None;
    for child in children {
        if let Some((name, accum_range, accum_span)) = aot_key(&child) {
            // Pull the inner body (drop the marker pseudo-pair).
            let inner_body = match &child {
                DataTree::Section { children, .. } => children
                    .iter()
                    .filter(|c| !matches!(c, DataTree::Pair { key, value, .. }
                        if matches!(key.as_ref(), DataTree::String { value: k, .. } if k == "__array_of_tables__")
                            && matches!(value.as_ref(), DataTree::Bool { .. })))
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

/// If `tree` is a `[[name]]` array-of-tables Section (i.e. starts
/// with `__array_of_tables__: true`), return its name + ranges.
fn aot_key(tree: &DataTree) -> Option<(String, ByteRange, Span)> {
    let DataTree::Section { name, children, .. } = tree else { return None };
    let DataTree::String { value: name_str, range, span, .. } = name.as_ref() else { return None };
    let first = children.first()?;
    let DataTree::Pair { key, value, .. } = first else { return None };
    let DataTree::String { value: key_str, .. } = key.as_ref() else { return None };
    if key_str != "__array_of_tables__" { return None; }
    let DataTree::Bool { value: true, .. } = value.as_ref() else { return None };
    Some((name_str.clone(), *range, *span))
}

/// Build a single Section that wraps a Sequence of accumulated
/// inner bodies. The resulting shape renders as
/// `<name><item>...</item><item>...</item></name>` under the keyed
/// renderer.
fn emit_aot_section(
    state: (String, ByteRange, Span, Vec<Vec<DataTree>>, ByteRange, Span),
) -> DataTree {
    let (name, name_range, name_span, bodies, full_range, full_span) = state;
    let items: Vec<DataTree> = bodies
        .into_iter()
        .map(|b| DataTree::Mapping {
            pairs: b,
            range: full_range,
            span: full_span,
        })
        .collect();
    let seq = DataTree::Sequence {
        items,
        range: full_range,
        span: full_span,
    };
    DataTree::Section {
        name: Box::new(DataTree::String {
            value: name,
            quote_style: QuoteStyle::Plain,
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
    node: &RawNode,
    source: &str,
) -> Vec<(String, ByteRange, Span)> {
    match node.kind() {
        "dotted_key" => {
            let mut out = Vec::new();
            for child in node.named_children() {
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

fn lower_named_children(node: &RawNode, source: &str) -> Vec<DataTree> {
    node.named_children()
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

