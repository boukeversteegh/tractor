//! IR → Xot rendering.
//!
//! Mechanical translation: given an [`Ir`] tree and the original
//! `source` string, build the corresponding Xot tree. No decisions
//! live here — every shape choice is encoded in the IR variants.
//!
//! ## Invariants
//! For every IR node `n` rendered to XML element `E`:
//!
//! 1. **Text recovery.** XPath `string(E)` (concatenation of all
//!    descendant text in document order) equals `source[n.range]`.
//!    This makes `[.='foo()']` matching by literal source text a
//!    valid query.
//! 2. **Source attributes.** `E` carries `line`, `column`, `end_line`,
//!    `end_column` matching `n.span`.
//! 3. **No source loss.** Every byte of `source` covered by the root
//!    IR ends up in *some* descendant text node of the root element,
//!    in source order.
//!
//! ## How gap text works
//! For a container IR node with byte range `[P_start, P_end)` and
//! source-derived children with ranges `[c0..c1) [c2..c3) ...` (in
//! source order):
//!
//! - Pre-first-child gap: `source[P_start .. c0]` — emitted as text
//!   inside `E` before child 0's element.
//! - Inter-child gap: `source[c1 .. c2]` — emitted between children.
//! - Trailing gap: `source[c_last_end .. P_end]` — emitted after the
//!   last child.
//!
//! Synthetic IR (markers like `<access/>`, slot wrappers like `<left>`)
//! is emitted at variant-determined positions and contributes zero
//! text. It does not participate in gap calculation.

use xot::{Node as XotNode, Xot};

use super::types::{AccessSegment, ByteRange, Ir, Span};

/// Render an [`Ir`] tree as a child of `parent` in the given Xot
/// document. Returns the root node of the rendered subtree.
///
/// `source` must be the same string the IR was lowered from.
pub fn render_to_xot(
    xot: &mut Xot,
    parent: XotNode,
    ir: &Ir,
    source: &str,
) -> Result<XotNode, xot::Error> {
    match ir {
        Ir::Module { children, range, span } => {
            let node = element(xot, "module", *span);
            xot.append(parent, node)?;
            render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Expression { inner, range, span } => {
            let node = element(xot, "expression", *span);
            xot.append(parent, node)?;
            // <expression> is a Principle #15 host wrapper. Its byte
            // range typically equals (or contains) the inner's range.
            // Emit pre-gap, inner, trailing-gap so trailing newlines /
            // semicolons attached to the statement are preserved.
            render_with_gaps(xot, node, source, *range, std::slice::from_ref(inner.as_ref()),
                |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ()),
            )?;
            Ok(node)
        }
        Ir::Access { receiver, segments, range, span } => {
            let object = element(xot, "object", *span);
            xot.append(parent, object)?;
            // Synthetic `<access/>` marker — first child, zero text
            // contribution. It is *not* part of the source-order
            // walk, so we emit it before any source-derived children.
            let access = element(xot, "access", Span::point(span.line, span.column));
            xot.append(object, access)?;
            // Receiver — first source-derived child. Pre-receiver gap
            // is `source[range.start .. receiver.range.start]`.
            let receiver_range = receiver.range();
            emit_gap(xot, object, source, range.start, receiver_range.start)?;
            render_to_xot(xot, object, receiver, source)?;
            // Segments — right-nested. The first segment is a child of
            // <object>; deeper segments are children of the previous
            // segment.
            let mut cursor = receiver_range.end;
            render_segments_chain(xot, object, segments, &mut cursor, source)?;
            // Trailing gap inside <object>.
            emit_gap(xot, object, source, cursor, range.end)?;
            Ok(object)
        }
        Ir::Assign { targets, type_annotation, op_text, op_range, op_markers, values, range, span } => {
            let node = element(xot, "assign", *span);
            xot.append(parent, node)?;

            // <left><expression>...</expression>... </left> — each
            // target wrapped in an <expression> host, in source order.
            let left_node = element(xot, "left", *span);
            xot.append(node, left_node)?;
            // Compute left's source range from first to last target.
            let left_range = if let (Some(first), Some(last)) = (targets.first(), targets.last()) {
                ByteRange::new(first.range().start, last.range().end)
            } else {
                ByteRange::empty_at(range.start)
            };
            // Pre-target gap inside <left> (typically empty).
            let mut cursor = left_range.start;
            for t in targets {
                let tr = t.range();
                emit_gap(xot, left_node, source, cursor, tr.start)?;
                let expr = element(xot, "expression", t.span());
                xot.append(left_node, expr)?;
                render_to_xot(xot, expr, t, source)?;
                cursor = tr.end;
            }
            emit_gap(xot, left_node, source, cursor, left_range.end)?;

            // Gap from end-of-left to start-of-type (if any) or op.
            let post_left_end = left_range.end;
            let next_start = type_annotation.as_ref().map(|t| t.range().start)
                .unwrap_or(op_range.start);
            emit_gap(xot, node, source, post_left_end, next_start)?;

            // <type>...</type> — type annotation if present.
            let post_type_end = if let Some(t) = type_annotation {
                let tr = t.range();
                let type_node = element(xot, "type", t.span());
                xot.append(node, type_node)?;
                render_with_gaps(xot, type_node, source, tr,
                    std::slice::from_ref(t.as_ref()),
                    |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ()))?;
                emit_gap(xot, node, source, tr.end, op_range.start)?;
                tr.end
            } else {
                post_left_end
            };
            let _ = post_type_end;

            // <op>{op_text}{markers}</op>
            if !op_text.is_empty() {
                let op_node = element(xot, "op", *span);
                xot.append(node, op_node)?;
                let t = xot.new_text(op_text);
                xot.append(op_node, t)?;
                for marker in op_markers {
                    let m = element(xot, marker, *span);
                    xot.append(op_node, m)?;
                }
            }

            // Gap from op to right.
            let right_range = if let (Some(first), Some(last)) = (values.first(), values.last()) {
                Some(ByteRange::new(first.range().start, last.range().end))
            } else {
                None
            };
            if let Some(rr) = right_range {
                emit_gap(xot, node, source, op_range.end, rr.start)?;
                let right_node = element(xot, "right", *span);
                xot.append(node, right_node)?;
                let mut cursor = rr.start;
                for v in values {
                    let vr = v.range();
                    emit_gap(xot, right_node, source, cursor, vr.start)?;
                    let expr = element(xot, "expression", v.span());
                    xot.append(right_node, expr)?;
                    render_to_xot(xot, expr, v, source)?;
                    cursor = vr.end;
                }
                emit_gap(xot, right_node, source, cursor, rr.end)?;
                // Trailing gap after right inside <assign>.
                emit_gap(xot, node, source, rr.end, range.end)?;
            } else {
                // Pure type-only declaration — trailing gap after op
                // (or after type if no op).
                let after = if !op_text.is_empty() { op_range.end } else { post_type_end };
                emit_gap(xot, node, source, after, range.end)?;
            }
            Ok(node)
        }
        Ir::Import { has_alias, children, range, span } => {
            let node = element(xot, "import", *span);
            xot.append(parent, node)?;
            if *has_alias {
                let m = element(xot, "alias", Span::point(span.line, span.column));
                xot.append(node, m)?;
            }
            render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::From { relative, path, imports, range, span } => {
            let node = element(xot, "from", *span);
            xot.append(parent, node)?;
            if *relative {
                let m = element(xot, "relative", Span::point(span.line, span.column));
                xot.append(node, m)?;
            }
            // Source-derived children in source order: path first (if
            // any), then imports.
            let mut order: Vec<&Ir> = Vec::new();
            if let Some(p) = path { order.push(p.as_ref()); }
            for i in imports { order.push(i); }
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::FromImport { has_alias, name, alias, range, span } => {
            let node = element(xot, "import", *span);
            xot.append(parent, node)?;
            if *has_alias {
                let m = element(xot, "alias", Span::point(span.line, span.column));
                xot.append(node, m)?;
            }
            let mut order: Vec<&Ir> = vec![name.as_ref()];
            if let Some(a) = alias { order.push(a.as_ref()); }
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Path { segments, range, span } => {
            let node = element(xot, "path", *span);
            xot.append(parent, node)?;
            render_with_gaps(xot, node, source, *range, segments, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Aliased { inner, range, span } => {
            let node = element(xot, "aliased", *span);
            xot.append(parent, node)?;
            render_with_gaps(xot, node, source, *range, std::slice::from_ref(inner.as_ref()),
                |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ()),
            )?;
            Ok(node)
        }
        Ir::Call { callee, arguments, range, span } => {
            let node = element(xot, "call", *span);
            xot.append(parent, node)?;
            let mut order: Vec<&Ir> = Vec::with_capacity(1 + arguments.len());
            order.push(callee.as_ref());
            for arg in arguments { order.push(arg); }
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Binary { op_text, op_marker, op_range, left, right, range, span } => {
            let node = element(xot, "binary", *span);
            xot.append(parent, node)?;

            // <left><expression>...</expression></left>
            // <left>'s byte coverage equals left.range; it has no
            // own source contribution. <expression> wrapper inherits
            // the same range.
            let left_range = left.range();
            // Pre-left gap (typically empty for binary).
            emit_gap(xot, node, source, range.start, left_range.start)?;
            let left_slot = element(xot, "left", left.span());
            xot.append(node, left_slot)?;
            let left_expr = element(xot, "expression", left.span());
            xot.append(left_slot, left_expr)?;
            render_to_xot(xot, left_expr, left, source)?;

            // Gap between left and op (e.g. " " in "a + b").
            emit_gap(xot, node, source, left_range.end, op_range.start)?;

            // <op>{op_text}<{op_marker}/></op>
            // op_range covers the operator symbol; emit op_text as a
            // literal text leaf inside <op>, then the marker element.
            let op_node = element(xot, "op", Span::point(span.line, span.column));
            xot.append(node, op_node)?;
            if !op_text.is_empty() {
                let t = xot.new_text(op_text);
                xot.append(op_node, t)?;
            }
            let marker_node = element(xot, op_marker, Span::point(span.line, span.column));
            xot.append(op_node, marker_node)?;

            // Gap between op and right.
            let right_range = right.range();
            emit_gap(xot, node, source, op_range.end, right_range.start)?;

            // <right><expression>...</expression></right>
            let right_slot = element(xot, "right", right.span());
            xot.append(node, right_slot)?;
            let right_expr = element(xot, "expression", right.span());
            xot.append(right_slot, right_expr)?;
            render_to_xot(xot, right_expr, right, source)?;

            // Trailing gap.
            emit_gap(xot, node, source, right_range.end, range.end)?;

            Ok(node)
        }
        Ir::Unary { op_text, op_marker, op_range, operand, range, span } => {
            let node = element(xot, "unary", *span);
            xot.append(parent, node)?;

            // Pre-op gap.
            emit_gap(xot, node, source, range.start, op_range.start)?;

            // <op>{op_text}<{op_marker}/></op>
            let op_node = element(xot, "op", Span::point(span.line, span.column));
            xot.append(node, op_node)?;
            if !op_text.is_empty() {
                let t = xot.new_text(op_text);
                xot.append(op_node, t)?;
            }
            let marker_node = element(xot, op_marker, Span::point(span.line, span.column));
            xot.append(op_node, marker_node)?;

            // Gap between op and operand.
            let operand_range = operand.range();
            emit_gap(xot, node, source, op_range.end, operand_range.start)?;

            // Operand untagged (no <expression> host — Python convention).
            render_to_xot(xot, node, operand, source)?;

            emit_gap(xot, node, source, operand_range.end, range.end)?;

            Ok(node)
        }

        // ----- Atoms — emit source[range] as the leaf text. ---------
        Ir::Name { range, span } => leaf(xot, parent, "name", source, *range, *span),
        Ir::Int { range, span } => leaf(xot, parent, "int", source, *range, *span),
        Ir::Float { range, span } => leaf(xot, parent, "float", source, *range, *span),
        Ir::String { range, span } => leaf(xot, parent, "string", source, *range, *span),
        Ir::True { range, span } => leaf(xot, parent, "true", source, *range, *span),
        Ir::False { range, span } => leaf(xot, parent, "false", source, *range, *span),
        Ir::None { range, span } => leaf(xot, parent, "none", source, *range, *span),

        Ir::Inline { children, range, span: _ } => {
            // Inline contributes no element of its own. Children render
            // at the parent level; gap text from the inline's range
            // wraps them.
            render_with_gaps(xot, parent, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(parent)
        }
        Ir::Unknown { kind, range, span } => {
            let node = element(xot, "unknown", *span);
            let kind_attr = xot.add_name("kind");
            xot.attributes_mut(node).insert(kind_attr, kind.clone());
            // Emit source[range] as the leaf text so XPath
            // `string(.)` parity holds even for un-handled kinds.
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
// Access-chain rendering
// ---------------------------------------------------------------------------

/// Render an access-chain segment list right-nested into `host`.
/// `cursor` points at the current source-position cursor (just past
/// the last source-derived child emitted in `host`); on return it
/// points at the end of the deepest segment processed in this call.
///
/// For each segment, we:
/// 1. Emit gap from `cursor` to `segment.range.start` (typically empty
///    — segments touch their predecessors directly).
/// 2. Create the `<member>` / `<index>` element.
/// 3. Render the segment's own content with internal gaps.
/// 4. If there are deeper segments, render them *inside* this
///    segment's element (right-nesting).
fn render_segments_chain(
    xot: &mut Xot,
    host: XotNode,
    segments: &[AccessSegment],
    cursor: &mut u32,
    source: &str,
) -> Result<(), xot::Error> {
    let Some((first, rest)) = segments.split_first() else { return Ok(()) };
    let seg_range = first.range();
    // Gap before this segment in the host.
    emit_gap(xot, host, source, *cursor, seg_range.start)?;

    let segment_node = match first {
        AccessSegment::Member { property_range, property_span, range: _, span } => {
            let node = element(xot, "member", *span);
            xot.append(host, node)?;
            // Internal gap from segment-start to property-name (the `.`).
            emit_gap(xot, node, source, seg_range.start, property_range.start)?;
            // Property name leaf.
            leaf(xot, node, "name", source, *property_range, *property_span)?;
            // Inner cursor advances past the property.
            let mut inner_cursor = property_range.end;
            // Render any deeper segments inside this <member>.
            render_segments_chain(xot, node, rest, &mut inner_cursor, source)?;
            // Trailing gap inside this <member>, up to its range end.
            // For `a.b.c`, segment 0's range is `.b`; the trailing gap
            // is from `b`-end to `.b`-end = empty. For chains where
            // this segment is the deepest, inner_cursor == property
            // end; gap to seg_range.end may include trailing
            // whitespace.
            emit_gap(xot, node, source, inner_cursor, seg_range.end)?;
            *cursor = if rest.is_empty() {
                seg_range.end
            } else {
                // Deeper segments may extend beyond seg_range.end if
                // their own ranges do; we expose the deepest cursor
                // observed.
                std::cmp::max(seg_range.end, inner_cursor)
            };
            node
        }
        AccessSegment::Index { indices, range: _, span } => {
            let node = element(xot, "index", *span);
            xot.append(host, node)?;
            // Indices live inside [...]; emit source-order with gaps.
            let inner_refs: Vec<&Ir> = indices.iter().collect();
            render_with_gaps(xot, node, source, seg_range, &inner_refs,
                |xot, parent, &child| render_to_xot(xot, parent, child, source).map(|_| ()),
            )?;
            // Index segments do not currently chain deeper segments
            // INSIDE themselves; deeper chain steps would be siblings
            // (further AccessSegment entries). For the slice we cover,
            // index is always the last segment.
            let mut inner_cursor = seg_range.end;
            render_segments_chain(xot, node, rest, &mut inner_cursor, source)?;
            *cursor = inner_cursor;
            node
        }
    };
    let _ = segment_node;
    Ok(())
}

// ---------------------------------------------------------------------------
// Generic gap-aware rendering
// ---------------------------------------------------------------------------

/// Render a sequence of source-order children inside `container`,
/// inserting gap text from `source` between them based on byte ranges.
///
/// `container_range` is the source range covered by `container`.
/// Children are visited in given order, which is assumed to be source
/// order. The renderer:
///
/// - emits `source[container_range.start .. children[0].range.start]`
///   as pre-first-child gap text,
/// - calls `render_child(xot, container, &children[i])` for each
///   child,
/// - emits `source[children[i].range.end .. children[i+1].range.start]`
///   between consecutive children,
/// - emits `source[children[last].range.end .. container_range.end]`
///   as trailing gap text.
fn render_with_gaps<C, F>(
    xot: &mut Xot,
    container: XotNode,
    source: &str,
    container_range: ByteRange,
    children: &[C],
    mut render_child: F,
) -> Result<(), xot::Error>
where
    C: HasRange,
    F: FnMut(&mut Xot, XotNode, &C) -> Result<(), xot::Error>,
{
    let mut cursor = container_range.start;
    for child in children {
        let child_range = child.range();
        emit_gap(xot, container, source, cursor, child_range.start)?;
        render_child(xot, container, child)?;
        cursor = child_range.end;
    }
    emit_gap(xot, container, source, cursor, container_range.end)?;
    Ok(())
}

/// Trait so `render_with_gaps` can take either `&Ir` or `&&Ir`.
trait HasRange {
    fn range(&self) -> ByteRange;
}

impl HasRange for Ir {
    fn range(&self) -> ByteRange { Ir::range(self) }
}

impl<T: HasRange> HasRange for &T {
    fn range(&self) -> ByteRange { (*self).range() }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn element(xot: &mut Xot, name: &str, span: Span) -> XotNode {
    let name_id = xot.add_name(name);
    let node = xot.new_element(name_id);
    set_span_attrs(xot, node, span);
    node
}

fn leaf(
    xot: &mut Xot,
    parent: XotNode,
    name: &str,
    source: &str,
    range: ByteRange,
    span: Span,
) -> Result<XotNode, xot::Error> {
    let node = element(xot, name, span);
    let text = range.slice(source);
    if !text.is_empty() {
        let text_node = xot.new_text(text);
        xot.append(node, text_node)?;
    }
    xot.append(parent, node)?;
    Ok(node)
}

/// Emit `source[start..end]` as a text node child of `container` if
/// the range is non-empty.
fn emit_gap(
    xot: &mut Xot,
    container: XotNode,
    source: &str,
    start: u32,
    end: u32,
) -> Result<(), xot::Error> {
    if end > start {
        let text = &source[start as usize..end as usize];
        if !text.is_empty() {
            let t = xot.new_text(text);
            xot.append(container, t)?;
        }
    }
    Ok(())
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
