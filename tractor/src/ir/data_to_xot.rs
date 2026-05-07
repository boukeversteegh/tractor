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
        DataIr::Directive { flavor, children, span, .. } => {
            let node = element(xot, "directive", *span);
            xot.append(parent, node)?;
            // Marker child for the flavor (`<yaml/>` / `<tag/>` / `<reserved/>`).
            let m = element(xot, flavor, *span);
            xot.append(node, m)?;
            // Each pair `key=value` renders as `<key>value</key>`.
            for c in children {
                if let DataIr::Pair { key, value, .. } = c {
                    if let Some(k) = scalar_text(key, source) {
                        let safe = sanitize_xml_name(k);
                        let kn = element(xot, &safe, key.span());
                        xot.append(node, kn)?;
                        if let Some(v) = scalar_text(value, source) {
                            let t = xot.new_text(&v);
                            xot.append(kn, t)?;
                        }
                    }
                }
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

/// Render a [`DataIr`] tree using the **data-branch** projection:
/// pair keys become element names rather than `<property><key>...`
/// wrappers. Used for TOML / INI / Markdown which don't have a
/// distinct "syntax" XML view.
///
/// `[database]` + `host = "localhost"` becomes
/// `<database><host>localhost</host></database>`.
///
/// Element-name sanitization: keys may contain characters not valid
/// in XML names (dots, dashes, leading digits). We sanitize by
/// replacing offending characters with `_` and prepending `_` if
/// the first character is a digit.
pub fn render_data_to_xot_keyed(
    xot: &mut Xot,
    parent: XotNode,
    ir: &DataIr,
    source: &str,
) -> Result<XotNode, xot::Error> {
    match ir {
        DataIr::Document { children, span, .. } => {
            let node = element(xot, "document", *span);
            xot.append(parent, node)?;
            for c in children {
                render_data_to_xot_keyed(xot, node, c, source)?;
            }
            Ok(node)
        }
        DataIr::Section { name, children, span, .. } => {
            // Section name (typically Scalar(String)) becomes the
            // element name. Sanitize for XML.
            let element_name = scalar_text(name, source).map(sanitize_xml_name)
                .unwrap_or_else(|| "section".to_string());
            let node = element(xot, &element_name, *span);
            xot.append(parent, node)?;
            for c in children {
                render_data_to_xot_keyed(xot, node, c, source)?;
            }
            Ok(node)
        }
        DataIr::Pair { key, value, span, .. } => {
            // Key text → element name; value renders as the
            // element's content (text for scalars, nested for
            // Mapping/Sequence).
            let element_name = scalar_text(key, source).map(sanitize_xml_name)
                .unwrap_or_else(|| "pair".to_string());
            let node = element(xot, &element_name, *span);
            xot.append(parent, node)?;
            render_keyed_value(xot, node, value, source)?;
            Ok(node)
        }
        DataIr::Mapping { pairs, span, .. } => {
            // Top-level mapping (no enclosing Section) — render as
            // <document>-equivalent inline. Each pair gets its key
            // as element name.
            let node = element(xot, "object", *span);
            xot.append(parent, node)?;
            for p in pairs {
                render_data_to_xot_keyed(xot, node, p, source)?;
            }
            Ok(node)
        }
        DataIr::Sequence { items, span, .. } => {
            // Top-level sequence (no enclosing Pair) — wrap in
            // <array>. Inner items get <item> wrapping.
            let node = element(xot, "array", *span);
            xot.append(parent, node)?;
            for item in items {
                let item_node = element(xot, "item", item.span());
                xot.append(node, item_node)?;
                render_keyed_value(xot, item_node, item, source)?;
            }
            Ok(node)
        }
        DataIr::String { value, span, .. } => {
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
            let node = element(xot, "bool", *span);
            xot.append(parent, node)?;
            let t = xot.new_text(if *value { "true" } else { "false" });
            xot.append(node, t)?;
            Ok(node)
        }
        DataIr::Null { span, .. } => {
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
        DataIr::Directive { flavor, children, span, .. } => {
            let node = element(xot, "directive", *span);
            xot.append(parent, node)?;
            // Marker child for the flavor (`<yaml/>` / `<tag/>` / `<reserved/>`).
            let m = element(xot, flavor, *span);
            xot.append(node, m)?;
            // Each pair `key=value` renders as `<key>value</key>`.
            for c in children {
                if let DataIr::Pair { key, value, .. } = c {
                    if let Some(k) = scalar_text(key, source) {
                        let safe = sanitize_xml_name(k);
                        let kn = element(xot, &safe, key.span());
                        xot.append(node, kn)?;
                        if let Some(v) = scalar_text(value, source) {
                            let t = xot.new_text(&v);
                            xot.append(kn, t)?;
                        }
                    }
                }
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

/// Render a Pair's value: scalars become text content of the parent
/// (no nested element wrapping); Mappings/Sequences nest as usual.
fn render_keyed_value(
    xot: &mut Xot,
    parent: XotNode,
    value: &DataIr,
    source: &str,
) -> Result<(), xot::Error> {
    match value {
        DataIr::String { value, .. } => {
            if !value.is_empty() {
                let t = xot.new_text(value);
                xot.append(parent, t)?;
            }
        }
        DataIr::Number { text, .. } => {
            let t = xot.new_text(text);
            xot.append(parent, t)?;
        }
        DataIr::Bool { value, .. } => {
            let t = xot.new_text(if *value { "true" } else { "false" });
            xot.append(parent, t)?;
        }
        DataIr::Null { .. } => { /* leave parent empty */ }
        DataIr::Mapping { pairs, .. } => {
            for p in pairs {
                render_data_to_xot_keyed(xot, parent, p, source)?;
            }
        }
        DataIr::Sequence { items, .. } => {
            // Each array element gets an `<item>` wrapper so the
            // shape is `<key><item>v1</item><item>v2</item></key>`.
            // JSON projection then collects them as an array.
            for item in items {
                let item_node = element(xot, "item", item.span());
                xot.append(parent, item_node)?;
                render_keyed_value(xot, item_node, item, source)?;
            }
        }
        // Recursive nesting for unusual cases.
        other => {
            render_data_to_xot_keyed(xot, parent, other, source)?;
        }
    }
    Ok(())
}

/// Pull the textual content out of a scalar DataIr (for use as a
/// keyed element name). Returns None for non-scalar IRs.
fn scalar_text<'a>(ir: &'a DataIr, source: &'a str) -> Option<String> {
    match ir {
        DataIr::String { value, .. } => Some(value.clone()),
        DataIr::Number { text, .. } => Some(text.clone()),
        DataIr::Bool { value, .. } => Some(value.to_string()),
        DataIr::Null { .. } => Some("null".to_string()),
        // Allow any leaf-ish IR by sliding source bytes.
        _ => Some(ir.range().slice(source).trim().to_string()),
    }
}

/// Sanitize a string for use as an XML element name. Replaces
/// invalid characters with `_` and prepends `_` if the name starts
/// with a digit. Empty strings become `_`.
fn sanitize_xml_name(raw: String) -> String {
    if raw.is_empty() {
        return "_".to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    let first = chars.next().unwrap();
    if first.is_ascii_digit() {
        out.push('_');
    }
    if is_xml_name_start_char(first) {
        out.push(first);
    } else {
        out.push('_');
    }
    for c in chars {
        if is_xml_name_char(c) {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

fn is_xml_name_start_char(c: char) -> bool {
    c == '_'
        || c == ':'
        || c.is_ascii_alphabetic()
        || (c as u32 > 0x7F)
}

fn is_xml_name_char(c: char) -> bool {
    is_xml_name_start_char(c) || c.is_ascii_digit() || c == '-' || c == '.'
}

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
