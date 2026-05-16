//! Generic variant-blind walker over the three tree families.
//!
//! `SyntaxTree`, `DataTree`, and `SqlTree` each generate their own
//! `metadata.generated.rs` with the same accessor signatures —
//! [`WalkerTree`] formalises that contract so a single generic walker
//! can render any of them to XML / JSON / etc.
//!
//! Pre-trait, each tree had its own hand-written walker (~150–700 LOC
//! per output format per tree). With the trait, one [`render_to_xot`]
//! and one [`render_to_json`] cover all three trees.
//!
//! ## What the trait abstracts
//!
//! - **Variant identity** — [`element_name_of`](WalkerTree::element_name_of)
//!   returns the runtime element name (or `None` for inline pass-through
//!   variants like `Inline` / `Skip`).
//! - **Markers** — [`flags_of`](WalkerTree::flags_of) yields the
//!   variant's modifier / discriminator markers as empty-element
//!   children with source-position metadata.
//! - **Children** — [`children_of`](WalkerTree::children_of) walks the
//!   variant's tree-child fields in source order. Sort happens in the
//!   accessor; the walker consumes the result verbatim.
//! - **Source ranges** — [`range_of`](WalkerTree::range_of),
//!   [`span_of`](WalkerTree::span_of), and
//!   [`scalar_text_of`](WalkerTree::scalar_text_of) drive the
//!   gap-text and leaf-text emission rules.
//! - **Per-context naming** — [`display_name_for`](WalkerTree::display_name_for)
//!   is the per-tree overlay (per-language for SyntaxTree, per-format
//!   for DataTree). Default returns the variant name unchanged.

#![cfg(feature = "native")]

use crate::tree::types::{ByteRange, Marker, Span};

/// Tree contract consumed by the generic walker. Implemented by each
/// of the three tree families (`SyntaxTree`, `DataTree`, `SqlTree`)
/// in terms of its own `metadata_generated` accessors.
pub trait WalkerTree: Sized {
    /// Element name for this node, or `None` for inline-pass-through
    /// variants (`Inline` / `Skip`). Borrowed from the tree so
    /// open-set discriminator strings (`SimpleStatement.element_name`,
    /// `DataTree::Element.name`, ...) can be returned without
    /// allocation.
    fn element_name_of(&self) -> Option<&str>;

    /// Marker children (modifier flags, kind discriminators,
    /// per-variant markers). Each `Marker` carries its source span
    /// for `line` / `column` attributes on the rendered empty
    /// element.
    fn flags_of(&self) -> Vec<Marker>;

    /// Direct tree children in source order. Source-sorted by the
    /// generated accessor — the walker treats the order as authoritative.
    fn children_of(&self) -> Vec<&Self>;

    /// Source byte range of this node. Used for gap-text emission and
    /// verbatim-source recovery.
    fn range_of(&self) -> ByteRange;

    /// Source-location span. Used for `line` / `column` attributes
    /// on the rendered element.
    fn span_of(&self) -> Span;

    /// Stored text for scalar-leaf variants (`Name` / `Int` / `String`
    /// / ...). `None` for compound variants. The walker uses this to
    /// emit leaf content for synthetic (non-anchored) nodes — anchored
    /// leaves slice source instead.
    fn scalar_text_of(&self) -> Option<&str>;

    /// Per-context display name (language vocabulary for SyntaxTree,
    /// format vocabulary for DataTree, ...). Default returns `name`
    /// unchanged; trees with per-context naming override to consult
    /// their override table.
    fn display_name_for<'a>(name: &'a str, context: Option<&str>) -> &'a str {
        let _ = context;
        name
    }
}

impl WalkerTree for crate::tree::SyntaxTree {
    fn element_name_of(&self) -> Option<&str> {
        crate::tree::syntax::metadata_generated::element_name_of(self)
    }
    fn flags_of(&self) -> Vec<Marker> {
        crate::tree::syntax::metadata_generated::flags_of(self)
    }
    fn children_of(&self) -> Vec<&Self> {
        crate::tree::syntax::metadata_generated::children_of(self)
    }
    fn range_of(&self) -> ByteRange {
        crate::tree::syntax::metadata_generated::range_of(self)
    }
    fn span_of(&self) -> Span {
        crate::tree::syntax::metadata_generated::span_of(self)
    }
    fn scalar_text_of(&self) -> Option<&str> {
        crate::tree::syntax::metadata_generated::scalar_text_of(self)
    }
    fn display_name_for<'a>(name: &'a str, context: Option<&str>) -> &'a str {
        crate::tree::syntax::element_naming::element_name_for_lang(name, context)
    }
}

impl WalkerTree for crate::tree::DataTree {
    fn element_name_of(&self) -> Option<&str> {
        crate::tree::data::metadata_generated::element_name_of(self)
    }
    fn flags_of(&self) -> Vec<Marker> {
        crate::tree::data::metadata_generated::flags_of(self)
    }
    fn children_of(&self) -> Vec<&Self> {
        crate::tree::data::metadata_generated::children_of(self)
    }
    fn range_of(&self) -> ByteRange {
        crate::tree::data::metadata_generated::range_of(self)
    }
    fn span_of(&self) -> Span {
        crate::tree::data::metadata_generated::span_of(self)
    }
    fn scalar_text_of(&self) -> Option<&str> {
        crate::tree::data::metadata_generated::scalar_text_of(self)
    }
}

impl WalkerTree for crate::tree::sql::SqlTree {
    fn element_name_of(&self) -> Option<&str> {
        crate::tree::sql::metadata_generated::element_name_of(self)
    }
    fn flags_of(&self) -> Vec<Marker> {
        crate::tree::sql::metadata_generated::flags_of(self)
    }
    fn children_of(&self) -> Vec<&Self> {
        crate::tree::sql::metadata_generated::children_of(self)
    }
    fn range_of(&self) -> ByteRange {
        crate::tree::sql::metadata_generated::range_of(self)
    }
    fn span_of(&self) -> Span {
        crate::tree::sql::metadata_generated::span_of(self)
    }
    fn scalar_text_of(&self) -> Option<&str> {
        crate::tree::sql::metadata_generated::scalar_text_of(self)
    }
}

// =============================================================================
// Generic walker — XML projection (`to_xot`).
// =============================================================================

use xot::{Node as XotNode, Xot};

/// Generic variant-blind walker that renders any [`WalkerTree`] to
/// XML. Same shape as the original `tree/syntax/to_xot.rs`:
///
/// 1. Look up the element name via [`WalkerTree::element_name_of`].
///    `None` ⇒ no wrapper element; inline children at parent.
/// 2. Collect [`WalkerTree::flags_of`] markers and the variant-blind
///    [`WalkerTree::children_of`] list.
/// 3. Merge markers + children, sort by source position, weave in
///    gap text from `source` between siblings.
/// 4. Recurse.
///
/// **No per-variant rendering knowledge lives here.** Every shape
/// decision is encoded in the typed tree at lowering time and exposed
/// via the trait. If the rendered XML differs from the desired shape,
/// the fix is in lowering or in the variant definition — never here.
pub fn render_walker_to_xot<T: WalkerTree>(
    xot: &mut Xot,
    parent: XotNode,
    tree: &T,
    source: &str,
    context: Option<&str>,
) -> Result<XotNode, xot::Error> {
    match tree.element_name_of() {
        Some(tag) => {
            let display = T::display_name_for(tag, context).to_string();
            render_element(xot, parent, tree, source, &display, context)
        }
        None => {
            render_inline_into(xot, parent, tree, source, context)?;
            Ok(parent)
        }
    }
}

fn render_element<T: WalkerTree>(
    xot: &mut Xot,
    parent: XotNode,
    tree: &T,
    source: &str,
    name: &str,
    context: Option<&str>,
) -> Result<XotNode, xot::Error> {
    let name_id = xot.add_name(name);
    let node = xot.new_element(name_id);
    xot.append(parent, node)?;
    set_span_attrs(xot, node, tree.span_of());

    render_body(xot, node, tree, source, context)?;
    Ok(node)
}

fn render_body<T: WalkerTree>(
    xot: &mut Xot,
    node: XotNode,
    tree: &T,
    source: &str,
    context: Option<&str>,
) -> Result<(), xot::Error> {
    let parent_range = tree.range_of();
    let anchored = parent_range.is_anchored();

    let markers = tree.flags_of();
    let children = tree.children_of();

    let mut items: Vec<RenderItem<T>> = Vec::with_capacity(markers.len() + children.len());
    for m in &markers {
        items.push(RenderItem::Marker(*m));
    }
    for c in &children {
        items.push(RenderItem::Child(*c));
    }
    items.sort_by_key(|i| i.sort_key());

    let mut cursor: u32 = parent_range.start;
    for item in &items {
        let item_start = item.range_start();
        if anchored {
            let from = cursor;
            let to = item_start.max(cursor);
            if to > from {
                emit_text(xot, node, &source[from as usize..to as usize])?;
            }
        }
        match item {
            RenderItem::Marker(m) => emit_marker(xot, node, m)?,
            RenderItem::Child(child) => {
                if let Some(tag) = child.element_name_of() {
                    let display = T::display_name_for(tag, context).to_string();
                    render_element(xot, node, *child, source, &display, context)?;
                } else {
                    render_inline_into(xot, node, *child, source, context)?;
                }
            }
        }
        let end = item.range_end();
        if end > cursor {
            cursor = end;
        }
    }

    if anchored && parent_range.end > cursor {
        emit_text(xot, node, &source[cursor as usize..parent_range.end as usize])?;
    }

    if markers.is_empty() && children.is_empty() && !anchored {
        if let Some(text) = tree.scalar_text_of() {
            if !text.is_empty() {
                emit_text(xot, node, text)?;
            }
        }
    }

    Ok(())
}

fn render_inline_into<T: WalkerTree>(
    xot: &mut Xot,
    parent: XotNode,
    tree: &T,
    source: &str,
    context: Option<&str>,
) -> Result<(), xot::Error> {
    render_body(xot, parent, tree, source, context)
}

fn emit_text(xot: &mut Xot, parent: XotNode, text: &str) -> Result<(), xot::Error> {
    if text.is_empty() {
        return Ok(());
    }
    let tn = xot.new_text(text);
    xot.append(parent, tn)?;
    Ok(())
}

fn emit_marker(xot: &mut Xot, parent: XotNode, marker: &Marker) -> Result<(), xot::Error> {
    let name_id = xot.add_name(marker.name);
    let elem = xot.new_element(name_id);
    xot.append(parent, elem)?;
    set_span_attrs(xot, elem, marker.span);
    Ok(())
}

fn set_span_attrs(xot: &mut Xot, node: XotNode, span: Span) {
    let line_id = xot.add_name("line");
    let column_id = xot.add_name("column");
    let end_line_id = xot.add_name("end_line");
    let end_column_id = xot.add_name("end_column");
    let mut attrs = xot.attributes_mut(node);
    attrs.insert(line_id, span.line.to_string());
    attrs.insert(column_id, span.column.to_string());
    attrs.insert(end_line_id, span.end_line.to_string());
    attrs.insert(end_column_id, span.end_column.to_string());
    if span.id != 0 {
        drop(attrs);
        let id_id = xot.add_name("id");
        xot.attributes_mut(node).insert(id_id, span.id.to_string());
    }
}

#[derive(Clone, Copy)]
enum RenderItem<'a, T> {
    Marker(Marker),
    Child(&'a T),
}

impl<'a, T: WalkerTree> RenderItem<'a, T> {
    fn range(&self) -> ByteRange {
        match self {
            RenderItem::Marker(m) => m.range,
            RenderItem::Child(t) => t.range_of(),
        }
    }
    fn range_start(&self) -> u32 { self.range().start }
    fn range_end(&self) -> u32 { self.range().end }
    fn sort_key(&self) -> u32 { self.range_start() }
}

// =============================================================================
// Generic walker — JSON projection (`to_json`).
// =============================================================================

use std::collections::BTreeMap;
use serde_json::{Map, Value};

const KEY_TYPE: &str = "$type";
const KEY_CHILDREN: &str = "$children";

/// Generic variant-blind walker that renders any [`WalkerTree`] to
/// `serde_json::Value`. Mirrors the XML walker's structure but emits
/// JSON keys instead of XML elements; same source-of-truth metadata
/// drives both.
///
/// ## Output shape
///
/// - **Scalar leaves** (no children, no flags, has stored text) →
///   bare JSON string.
/// - **Inline / Skip** (no element name) → render children inline at
///   parent.
/// - **Structural nodes** → JSON object with:
///   - `$type`: element name (omitted when the parent's key already
///     conveys the type).
///   - One boolean field per flag (`"async": true`).
///   - One child group per distinct child element name; singleton
///     children are keyed directly (`"body": { … }`); multiple
///     same-named children promote to a JSON array under that key
///     (`"imports": [{…}, {…}]`).
pub fn render_walker_to_json<T: WalkerTree>(
    tree: &T,
    source: &str,
    context: Option<&str>,
) -> Value {
    render_json(tree, source, context, /*strip_type=*/ false)
}

fn render_json<T: WalkerTree>(
    tree: &T,
    source: &str,
    context: Option<&str>,
    strip_type: bool,
) -> Value {
    let tag = match tree.element_name_of() {
        Some(t) => t,
        None => return render_json_inline(tree, source, context),
    };
    let display = T::display_name_for(tag, context).to_string();

    let flags = tree.flags_of();
    let children = tree.children_of();

    if children.is_empty() && flags.is_empty() {
        if let Some(text) = leaf_text_for(tree, source) {
            return Value::String(text);
        }
    }

    let mut obj: Map<String, Value> = Map::new();
    if !strip_type {
        obj.insert(KEY_TYPE.to_string(), Value::String(display.clone()));
    }

    for marker in &flags {
        obj.insert(marker.name.to_string(), Value::Bool(true));
    }

    let mut groups: BTreeMap<String, Vec<&T>> = BTreeMap::new();
    let mut inline_overflow: Vec<&T> = Vec::new();
    for child in &children {
        push_json_child::<T>(child, context, &mut groups, &mut inline_overflow);
    }

    for (key, items) in groups {
        if items.len() == 1 {
            obj.insert(key, render_json(items[0], source, context, /*strip_type=*/ true));
        } else {
            let arr: Vec<Value> = items
                .iter()
                .map(|item| render_json(*item, source, context, /*strip_type=*/ true))
                .collect();
            obj.insert(key, Value::Array(arr));
        }
    }

    if !inline_overflow.is_empty() {
        let mut existing = obj
            .remove(KEY_CHILDREN)
            .and_then(|v| match v {
                Value::Array(a) => Some(a),
                _ => None,
            })
            .unwrap_or_default();
        for item in inline_overflow {
            existing.push(render_json(item, source, context, /*strip_type=*/ false));
        }
        obj.insert(KEY_CHILDREN.to_string(), Value::Array(existing));
    }

    Value::Object(obj)
}

fn render_json_inline<T: WalkerTree>(
    tree: &T,
    source: &str,
    context: Option<&str>,
) -> Value {
    let children = tree.children_of();
    match children.len() {
        0 => Value::Null,
        1 => render_json(children[0], source, context, /*strip_type=*/ false),
        _ => Value::Array(
            children
                .iter()
                .map(|c| render_json(*c, source, context, /*strip_type=*/ false))
                .collect(),
        ),
    }
}

fn push_json_child<'a, T: WalkerTree>(
    child: &'a T,
    context: Option<&str>,
    groups: &mut BTreeMap<String, Vec<&'a T>>,
    inline_overflow: &mut Vec<&'a T>,
) {
    match child.element_name_of() {
        Some(tag) => {
            let display = T::display_name_for(tag, context).to_string();
            groups.entry(display).or_default().push(child);
        }
        None => {
            for grand in child.children_of() {
                push_json_child::<T>(grand, context, groups, inline_overflow);
            }
        }
    }
}

fn leaf_text_for<T: WalkerTree>(tree: &T, source: &str) -> Option<String> {
    if let Some(text) = tree.scalar_text_of() {
        return Some(text.to_string());
    }
    let range = tree.range_of();
    if range.is_anchored() && !range.is_empty() {
        return Some(range.slice(source).to_string());
    }
    None
}
