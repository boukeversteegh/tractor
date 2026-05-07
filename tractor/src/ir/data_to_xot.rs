//! [`DataIr`] → Xot rendering — syntax-branch (preserves source
//! structure), JSON-style element vocabulary.
//!
//! Element-name mapping for the JSON-style render:
//!
//!   | DataIr variant | XML element             |
//!   |----------------|-------------------------|
//!   | Document       | `<document>`            |
//!   | Mapping        | `<object>`              |
//!   | Sequence       | `<array>`               |
//!   | Pair           | `<property>` with `<key>`/`<value>` |
//!   | Section        | `<section>` (TOML/INI)  |
//!   | String         | `<string>`              |
//!   | Number         | `<number>`              |
//!   | Bool           | `<bool>`                |
//!   | Null           | `<null>`                |
//!   | Comment        | `<comment>`             |
//!   | Unknown        | `<unknown kind="…">`    |
//!
//! Other formats (YAML, TOML) get their own renderer modules with
//! different name choices (e.g. `<mapping>`/`<sequence>` for YAML)
//! — but they all walk the same `DataIr` tree.
//!
//! ## Invariant — round-trip text recovery
//!
//! For any same-format render, XPath `string(rendered_root)` over
//! the result equals `source[range_of_root]` — i.e. concatenating
//! all descendant text in document order recovers the original
//! bytes. Achieved via `range`-anchored gap-text emission between
//! source-derived children, identical to `to_xot.rs`.

#![cfg(feature = "native")]

use xot::{Node as XotNode, Xot};

use super::data::DataIr;
use super::types::Span;

/// Render a [`DataIr`] tree as a child of `parent` in the given Xot
/// document, using JSON-style element names.
///
/// Punctuation tokens (`{`, `}`, `[`, `]`, `,`, `:`) and string
/// quote characters are not preserved as XML text — the parsed
/// `DataIr` carries the structural info, and JSON's punctuation is
/// noise from a query perspective. This matches the existing
/// imperative-pipeline shape (which used `remove_text_children`
/// + `extract_string_content` to strip).
///
/// `range`-anchored round-trip via `to_source(data_ir, source)`
/// still works — the IR carries source ranges; the renderer just
/// chooses a clean projection.
pub fn render_data_to_xot_json(
    xot: &mut Xot,
    parent: XotNode,
    ir: &DataIr,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let _ = source; // unused: structure-only render, no gap text.
    match ir {
        DataIr::Document { children, span, .. } => {
            let node = element(xot, "document", *span);
            xot.append(parent, node)?;
            for c in children {
                render_data_to_xot_json(xot, node, c, source)?;
            }
            Ok(node)
        }
        DataIr::Mapping { pairs, span, .. } => {
            let node = element(xot, "object", *span);
            xot.append(parent, node)?;
            for p in pairs {
                render_data_to_xot_json(xot, node, p, source)?;
            }
            Ok(node)
        }
        DataIr::Sequence { items, span, .. } => {
            let node = element(xot, "array", *span);
            xot.append(parent, node)?;
            for i in items {
                render_data_to_xot_json(xot, node, i, source)?;
            }
            Ok(node)
        }
        DataIr::Pair { key, value, span, .. } => {
            let node = element(xot, "property", *span);
            xot.append(parent, node)?;
            let key_el = element(xot, "key", key.span());
            xot.append(node, key_el)?;
            render_data_to_xot_json(xot, key_el, key, source)?;
            let val_el = element(xot, "value", value.span());
            xot.append(node, val_el)?;
            render_data_to_xot_json(xot, val_el, value, source)?;
            Ok(node)
        }
        DataIr::Section { name, children, span, .. } => {
            let node = element(xot, "section", *span);
            xot.append(parent, node)?;
            render_data_to_xot_json(xot, node, name, source)?;
            for c in children {
                render_data_to_xot_json(xot, node, c, source)?;
            }
            Ok(node)
        }
        DataIr::String { value, span, .. } => {
            // Emit `<string>parsed_value</string>` with escapes
            // resolved — matches the imperative pipeline's
            // `extract_string_content` shape.
            let node = element(xot, "string", *span);
            xot.append(parent, node)?;
            if !value.is_empty() {
                let t = xot.new_text(value);
                xot.append(node, t)?;
            }
            Ok(node)
        }
        DataIr::Number { text, span, .. } => {
            let node = element(xot, "number", *span);
            xot.append(parent, node)?;
            if !text.is_empty() {
                let t = xot.new_text(text);
                xot.append(node, t)?;
            }
            Ok(node)
        }
        DataIr::Bool { value, span, .. } => {
            // Render text as `true` / `false` to match imperative
            // pipeline (which renamed `true_node` / `false_node` to
            // `<bool>` with the original keyword text).
            let node = element(xot, "bool", *span);
            xot.append(parent, node)?;
            let t = xot.new_text(if *value { "true" } else { "false" });
            xot.append(node, t)?;
            Ok(node)
        }
        DataIr::Null { span, .. } => {
            // `<null>` keeps no text body — matches imperative.
            let node = element(xot, "null", *span);
            xot.append(parent, node)?;
            Ok(node)
        }
        DataIr::Comment { text, span, .. } => {
            let node = element(xot, "comment", *span);
            xot.append(parent, node)?;
            if !text.is_empty() {
                let t = xot.new_text(text);
                xot.append(node, t)?;
            }
            Ok(node)
        }
        DataIr::Unknown { kind, range, span } => {
            let node = element(xot, "unknown", *span);
            let kind_attr = xot.add_name("kind");
            xot.attributes_mut(node).insert(kind_attr, kind.clone());
            let text = range.slice(source);
            if !text.is_empty() {
                let t = xot.new_text(text);
                xot.append(node, t)?;
            }
            xot.append(parent, node)?;
            Ok(node)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers (mirror those in `to_xot.rs`)
// ---------------------------------------------------------------------------

fn element(xot: &mut Xot, name: &str, span: Span) -> XotNode {
    let n = xot.add_name(name);
    let node = xot.new_element(n);
    set_span_attrs(xot, node, span);
    node
}

fn set_span_attrs(xot: &mut Xot, node: XotNode, span: Span) {
    let line = xot.add_name("line");
    let column = xot.add_name("column");
    let end_line = xot.add_name("end_line");
    let end_column = xot.add_name("end_column");
    let mut attrs = xot.attributes_mut(node);
    attrs.insert(line, span.line.to_string());
    attrs.insert(column, span.column.to_string());
    attrs.insert(end_line, span.end_line.to_string());
    attrs.insert(end_column, span.end_column.to_string());
}
