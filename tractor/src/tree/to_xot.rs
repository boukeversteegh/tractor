//! Tree → XML projection — **variant-blind walk**.
//!
//! The renderer is a single mechanical pre-order walk:
//!
//! 1. Look up the element name via [`element_name_of`].
//!    `None` ⇒ no wrapper element; inline children at parent.
//! 2. Collect [`flags_of`] markers and the variant-blind
//!    [`SyntaxTree::children`] list.
//! 3. Merge markers + children, sort by source position, weave in
//!    gap text from `source` between siblings.
//! 4. Recurse.
//!
//! **No per-variant rendering knowledge lives in this file.** Every
//! shape decision is encoded in the typed tree at lowering time and
//! exposed mechanically via the generated accessors. If the rendered
//! XML differs from the desired shape, the fix is in lowering or in
//! the variant definition — never here.
//!
//! See `tractor/src/bin/gen_walker.rs` for the codegen and
//! `docs/design-walker-codegen.md` for the architectural rationale.

#![cfg(feature = "native")]

use xot::{Node as XotNode, Xot};

use super::render_generated::{element_name_of, flags_of};
use super::types::{ByteRange, Marker, Span, SyntaxTree};

/// Render `tree` under `parent` and return the appended wrapper
/// element (or `parent` itself when `tree` has no wrapper — `Inline`
/// at the top level inlines its children directly into `parent`).
pub fn render_to_xot(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    match element_name_of(tree) {
        Some(name) => render_element(xot, parent, tree, source, name),
        None => {
            render_inline_into(xot, parent, tree, source)?;
            Ok(parent)
        }
    }
}

/// Append a `<{name}>...</{name}>` element under `parent` for `tree`.
/// Body is the variant-blind merge of markers + children + gap text.
fn render_element(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
    name: &str,
) -> Result<XotNode, xot::Error> {
    let name_id = xot.add_name(name);
    let node = xot.new_element(name_id);
    xot.append(parent, node)?;
    set_span_attrs(xot, node, tree.span());

    render_body(xot, node, tree, source)?;
    Ok(node)
}

/// Body of an element: markers + children sorted by source position,
/// gap text from `source` between them, source slice as text content
/// for anchored leaves.
fn render_body(
    xot: &mut Xot,
    node: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<(), xot::Error> {
    let parent_range = tree.range();
    let anchored = parent_range.is_anchored();

    let markers = flags_of(tree);
    let children = tree.children();

    // Combine markers and children into a single source-sorted stream.
    // Each item carries its sort position and end-of-range so the
    // gap-text computation can advance the cursor uniformly.
    let mut items: Vec<RenderItem> = Vec::with_capacity(markers.len() + children.len());
    for m in &markers {
        items.push(RenderItem::Marker(*m));
    }
    for c in &children {
        items.push(RenderItem::Child(*c));
    }
    // Stable sort so markers placed at the same byte offset as a child
    // keep their relative order (markers first when emitted by the
    // generated `flags_of`, which lists modifiers before extra_markers).
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
            RenderItem::Marker(m) => {
                emit_marker(xot, node, m)?;
            }
            RenderItem::Child(child) => {
                if let Some(child_name) = element_name_of(child) {
                    render_element(xot, node, child, source, child_name)?;
                } else {
                    render_inline_into(xot, node, child, source)?;
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

    // Non-anchored leaf: no source to slice, but the variant carries
    // a `text` field (Name, Int, String, ...). Emit it as the leaf
    // text content so synthetic nodes still produce visible output.
    if markers.is_empty() && children.is_empty() && !anchored {
        if let Some(text) = tree.scalar_text() {
            if !text.is_empty() {
                emit_text(xot, node, text)?;
            }
        }
    }

    Ok(())
}

/// Inline-render a child with no wrapper element (Inline / Skip).
/// Children stream into `parent` directly; gap text uses the inline
/// node's own range to keep source-text correlation correct.
fn render_inline_into(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<(), xot::Error> {
    render_body(xot, parent, tree, source)
}

fn emit_text(
    xot: &mut Xot,
    parent: XotNode,
    text: &str,
) -> Result<(), xot::Error> {
    if text.is_empty() {
        return Ok(());
    }
    let tn = xot.new_text(text);
    xot.append(parent, tn)?;
    Ok(())
}

fn emit_marker(
    xot: &mut Xot,
    parent: XotNode,
    marker: &Marker,
) -> Result<(), xot::Error> {
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
enum RenderItem<'a> {
    Marker(Marker),
    Child(&'a SyntaxTree),
}

impl<'a> RenderItem<'a> {
    fn range(&self) -> ByteRange {
        match self {
            RenderItem::Marker(m) => m.range,
            RenderItem::Child(t) => t.range(),
        }
    }

    fn range_start(&self) -> u32 {
        self.range().start
    }

    fn range_end(&self) -> u32 {
        self.range().end
    }

    fn sort_key(&self) -> u32 {
        self.range_start()
    }
}
