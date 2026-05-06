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

use super::types::{AccessSegment, ByteRange, Ir, ParamKind, Span};

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
        Ir::Module { element_name, children, range, span } => {
            let node = element(xot, element_name, *span);
            xot.append(parent, node)?;
            render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Expression { inner, marker, range, span } => {
            let node = element(xot, "expression", *span);
            xot.append(parent, node)?;
            // Optional marker child first (e.g. `<non_null/>` for
            // `obj!`, `<await/>` for `await x`). Empty element, no
            // text contribution — XPath text-recovery unaffected.
            if let Some(m) = marker {
                let marker_node = element(xot, m, *span);
                xot.append(node, marker_node)?;
            }
            // <expression> is a Principle #15 host wrapper. Its byte
            // range typically equals (or contains) the inner's range.
            // Emit pre-gap, inner, trailing-gap so trailing tokens
            // (e.g. the `!` for non-null) are preserved.
            render_with_gaps(xot, node, source, *range, std::slice::from_ref(inner.as_ref()),
                |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ()),
            )?;
            Ok(node)
        }
        Ir::Access { receiver, segments, range, span } => {
            let object = element(xot, "object", *span);
            xot.append(parent, object)?;
            let access = element(xot, "access", Span::point(span.line, span.column));
            xot.append(object, access)?;
            // Synthetic `<base/>` / `<this/>` markers when the receiver
            // is the corresponding C# keyword — `base.Method()` /
            // `this.X` queryable as `//object[base]` / `//object[this]`.
            // The receiver's source text identifies the keyword.
            let receiver_range = receiver.range();
            let recv_text = receiver_range.slice(source);
            if matches!(receiver.as_ref(), Ir::Name { .. }) {
                if recv_text == "base" {
                    let m = element(xot, "base", Span::point(span.line, span.column));
                    xot.append(object, m)?;
                } else if recv_text == "this" {
                    let m = element(xot, "this", Span::point(span.line, span.column));
                    xot.append(object, m)?;
                }
            }
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
        Ir::Tuple { children, range, span } => {
            let node = element(xot, "tuple", *span);
            xot.append(parent, node)?;
            render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::List { children, range, span } => {
            let node = element(xot, "list", *span);
            xot.append(parent, node)?;
            // <literal/> marker — first child.
            let m = element(xot, "literal", *span);
            xot.append(node, m)?;
            render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Set { children, range, span } => {
            let node = element(xot, "set", *span);
            xot.append(parent, node)?;
            let m = element(xot, "literal", *span);
            xot.append(node, m)?;
            render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Dictionary { pairs, range, span } => {
            // Python's vocabulary uses short `dict` (TractorNode::Dict).
            // The IR variant is `Ir::Dictionary` for clarity in code,
            // but the wire name matches the imperative pipeline's
            // `Rename(Dict)` on `dictionary` CST kind.
            let node = element(xot, "dict", *span);
            xot.append(parent, node)?;
            let m = element(xot, "literal", *span);
            xot.append(node, m)?;
            render_with_gaps(xot, node, source, *range, pairs, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Pair { key, value, range, span } => {
            let node = element(xot, "pair", *span);
            xot.append(parent, node)?;
            let kr = key.range();
            let vr = value.range();
            emit_gap(xot, node, source, range.start, kr.start)?;
            render_to_xot(xot, node, key, source)?;
            emit_gap(xot, node, source, kr.end, vr.start)?;
            render_to_xot(xot, node, value, source)?;
            emit_gap(xot, node, source, vr.end, range.end)?;
            Ok(node)
        }
        Ir::GenericType { name, params, range, span } => {
            // <type[generic]>
            let node = element(xot, "type", *span);
            xot.append(parent, node)?;
            let g = element(xot, "generic", *span);
            xot.append(node, g)?;
            // Source-derived children: name, then each param wrapped
            // in <type>.
            let name_range = name.range();
            emit_gap(xot, node, source, range.start, name_range.start)?;
            render_to_xot(xot, node, name, source)?;
            let mut cursor = name_range.end;
            for p in params {
                let pr = p.range();
                emit_gap(xot, node, source, cursor, pr.start)?;
                let type_el = element(xot, "type", p.span());
                xot.append(node, type_el)?;
                render_to_xot(xot, type_el, p, source)?;
                cursor = pr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::Comparison { left, op_text, op_marker, op_range, right, range, span } => {
            // Python `comparison_operator` — renders as `<compare>`
            // with flat `<name>`/`<int>` siblings + an `<op>`
            // wrapper carrying the operator markers. The imperative
            // pipeline produces this shape; differs from
            // `<binary><left>/<right>` Java/C#-style by deliberate
            // design choice (Python's comparison chains and operators
            // historically render flat).
            let node = element(xot, "compare", *span);
            xot.append(parent, node)?;
            let lr = left.range();
            emit_gap(xot, node, source, range.start, lr.start)?;
            render_to_xot(xot, node, left, source)?;

            emit_gap(xot, node, source, lr.end, op_range.start)?;
            let op_node = element(xot, "op", *span);
            xot.append(node, op_node)?;
            if !op_text.is_empty() {
                let t = xot.new_text(op_text);
                xot.append(op_node, t)?;
            }
            let _ = op_marker;
            crate::transform::operators::add_operator_markers(xot, op_node, op_text)
                .map_err(|e| xot::Error::Io(format!("op marker: {e}")))?;

            let rr = right.range();
            emit_gap(xot, node, source, op_range.end, rr.start)?;
            render_to_xot(xot, node, right, source)?;
            emit_gap(xot, node, source, rr.end, range.end)?;
            Ok(node)
        }
        Ir::If { condition, body, else_branch, range, span } => {
            let node = element(xot, "if", *span);
            xot.append(parent, node)?;
            let cr = condition.range();
            emit_gap(xot, node, source, range.start, cr.start)?;
            let cond_slot = element(xot, "condition", condition.span());
            xot.append(node, cond_slot)?;
            let cond_expr = element(xot, "expression", condition.span());
            xot.append(cond_slot, cond_expr)?;
            render_to_xot(xot, cond_expr, condition, source)?;
            let br = body.range();
            emit_gap(xot, node, source, cr.end, br.start)?;
            render_to_xot(xot, node, body, source)?;
            // Flatten the else-if chain: emit `<else_if>` / `<else>`
            // siblings under the same `<if>` parent rather than
            // recursively nesting them. Matches the imperative
            // pipeline's `collapse_else_if_chain` post-pass.
            let mut cursor = br.end;
            let mut next = else_branch.as_ref().map(|b| b.as_ref());
            while let Some(branch) = next {
                let br_range = branch.range();
                emit_gap(xot, node, source, cursor, br_range.start)?;
                match branch {
                    Ir::ElseIf { condition: ec, body: eb, else_branch: deeper, span: es, range: er } => {
                        let elseif = element(xot, "else_if", *es);
                        xot.append(node, elseif)?;
                        let ecr = ec.range();
                        emit_gap(xot, elseif, source, er.start, ecr.start)?;
                        let cs = element(xot, "condition", ec.span());
                        xot.append(elseif, cs)?;
                        let ce = element(xot, "expression", ec.span());
                        xot.append(cs, ce)?;
                        render_to_xot(xot, ce, ec, source)?;
                        let ebr = eb.range();
                        emit_gap(xot, elseif, source, ecr.end, ebr.start)?;
                        render_to_xot(xot, elseif, eb, source)?;
                        emit_gap(xot, elseif, source, ebr.end, er.end)?;
                        cursor = er.end;
                        next = deeper.as_ref().map(|b| b.as_ref());
                    }
                    Ir::Else { body: eb, span: es, range: er } => {
                        let el = element(xot, "else", *es);
                        xot.append(node, el)?;
                        let ebr = eb.range();
                        emit_gap(xot, el, source, er.start, ebr.start)?;
                        render_to_xot(xot, el, eb, source)?;
                        emit_gap(xot, el, source, ebr.end, er.end)?;
                        cursor = er.end;
                        next = None;
                    }
                    _ => {
                        render_to_xot(xot, node, branch, source)?;
                        cursor = br_range.end;
                        next = None;
                    }
                }
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::ElseIf { condition, body, else_branch, range, span } => {
            let node = element(xot, "else_if", *span);
            xot.append(parent, node)?;
            let cr = condition.range();
            emit_gap(xot, node, source, range.start, cr.start)?;
            let cond_slot = element(xot, "condition", condition.span());
            xot.append(node, cond_slot)?;
            let cond_expr = element(xot, "expression", condition.span());
            xot.append(cond_slot, cond_expr)?;
            render_to_xot(xot, cond_expr, condition, source)?;
            let br = body.range();
            emit_gap(xot, node, source, cr.end, br.start)?;
            render_to_xot(xot, node, body, source)?;
            if let Some(e) = else_branch {
                let er = e.range();
                emit_gap(xot, node, source, br.end, er.start)?;
                render_to_xot(xot, node, e, source)?;
                emit_gap(xot, node, source, er.end, range.end)?;
            } else {
                emit_gap(xot, node, source, br.end, range.end)?;
            }
            Ok(node)
        }
        Ir::Else { body, range, span } => {
            let node = element(xot, "else", *span);
            xot.append(parent, node)?;
            let br = body.range();
            emit_gap(xot, node, source, range.start, br.start)?;
            render_to_xot(xot, node, body, source)?;
            emit_gap(xot, node, source, br.end, range.end)?;
            Ok(node)
        }
        Ir::For { is_async, targets, iterables, body, else_body, range, span } => {
            let node = element(xot, "for", *span);
            xot.append(parent, node)?;
            if *is_async {
                let m = element(xot, "async", *span);
                xot.append(node, m)?;
            }
            // Source-order: <left>{targets}</left>, <right>{iters}</right>, body, else?
            let left_range = if let (Some(f), Some(l)) = (targets.first(), targets.last()) {
                ByteRange::new(f.range().start, l.range().end)
            } else {
                ByteRange::empty_at(range.start)
            };
            let right_range = if let (Some(f), Some(l)) = (iterables.first(), iterables.last()) {
                ByteRange::new(f.range().start, l.range().end)
            } else {
                ByteRange::empty_at(range.start)
            };
            // Pre-left gap
            emit_gap(xot, node, source, range.start, left_range.start)?;
            let left_slot = element(xot, "left", *span);
            xot.append(node, left_slot)?;
            let mut cursor = left_range.start;
            for t in targets {
                let tr = t.range();
                emit_gap(xot, left_slot, source, cursor, tr.start)?;
                let expr = element(xot, "expression", t.span());
                xot.append(left_slot, expr)?;
                render_to_xot(xot, expr, t, source)?;
                cursor = tr.end;
            }
            emit_gap(xot, left_slot, source, cursor, left_range.end)?;
            // Gap between left and right
            emit_gap(xot, node, source, left_range.end, right_range.start)?;
            // Right slot
            let right_slot = element(xot, "right", *span);
            xot.append(node, right_slot)?;
            let mut cursor = right_range.start;
            for i in iterables {
                let ir2 = i.range();
                emit_gap(xot, right_slot, source, cursor, ir2.start)?;
                let expr = element(xot, "expression", i.span());
                xot.append(right_slot, expr)?;
                render_to_xot(xot, expr, i, source)?;
                cursor = ir2.end;
            }
            emit_gap(xot, right_slot, source, cursor, right_range.end)?;
            // Body + else
            let br = body.range();
            emit_gap(xot, node, source, right_range.end, br.start)?;
            render_to_xot(xot, node, body, source)?;
            if let Some(e) = else_body {
                let er = e.range();
                emit_gap(xot, node, source, br.end, er.start)?;
                let else_node = element(xot, "else", e.span());
                xot.append(node, else_node)?;
                render_to_xot(xot, else_node, e, source)?;
                emit_gap(xot, node, source, er.end, range.end)?;
            } else {
                emit_gap(xot, node, source, br.end, range.end)?;
            }
            Ok(node)
        }
        Ir::While { condition, body, else_body, range, span } => {
            let node = element(xot, "while", *span);
            xot.append(parent, node)?;
            let cr = condition.range();
            emit_gap(xot, node, source, range.start, cr.start)?;
            let cond_slot = element(xot, "condition", condition.span());
            xot.append(node, cond_slot)?;
            let cond_expr = element(xot, "expression", condition.span());
            xot.append(cond_slot, cond_expr)?;
            render_to_xot(xot, cond_expr, condition, source)?;
            let br = body.range();
            emit_gap(xot, node, source, cr.end, br.start)?;
            render_to_xot(xot, node, body, source)?;
            if let Some(e) = else_body {
                let er = e.range();
                emit_gap(xot, node, source, br.end, er.start)?;
                let else_node = element(xot, "else", e.span());
                xot.append(node, else_node)?;
                render_to_xot(xot, else_node, e, source)?;
                emit_gap(xot, node, source, er.end, range.end)?;
            } else {
                emit_gap(xot, node, source, br.end, range.end)?;
            }
            Ok(node)
        }
        Ir::Foreach { type_ann, target, iterable, body, range, span } => {
            let node = element(xot, "foreach", *span);
            xot.append(parent, node)?;
            // `<in/>` marker for the `in` keyword in `foreach (T x in coll)`.
            let in_marker = element(xot, "in", *span);
            xot.append(node, in_marker)?;
            // Source-order children: type? target iterable body. The
            // header `(... in ...)` punctuation lives in gap text.
            // type → <type>, target → <left><expression>,
            // iterable → <right><expression>, body → as-is.
            #[derive(Clone, Copy)]
            enum Slot<'a> { Type(&'a Ir), Target(&'a Ir), Iter(&'a Ir), Body(&'a Ir) }
            let mut order: Vec<Slot> = Vec::new();
            if let Some(t) = type_ann { order.push(Slot::Type(t)); }
            order.push(Slot::Target(target));
            order.push(Slot::Iter(iterable));
            order.push(Slot::Body(body));
            order.sort_by_key(|s| match s {
                Slot::Type(i) | Slot::Target(i) | Slot::Iter(i) | Slot::Body(i) => i.range().start,
            });
            let mut cursor = range.start;
            for slot in &order {
                let inner: &Ir = match slot {
                    Slot::Type(i) | Slot::Target(i) | Slot::Iter(i) | Slot::Body(i) => i,
                };
                let cr = inner.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                match slot {
                    Slot::Type(_) => {
                        let t = element(xot, "type", inner.span());
                        xot.append(node, t)?;
                        render_to_xot(xot, t, inner, source)?;
                    }
                    Slot::Target(_) => {
                        let slot_el = element(xot, "left", inner.span());
                        xot.append(node, slot_el)?;
                        let expr = element(xot, "expression", inner.span());
                        xot.append(slot_el, expr)?;
                        render_to_xot(xot, expr, inner, source)?;
                    }
                    Slot::Iter(_) => {
                        let slot_el = element(xot, "right", inner.span());
                        xot.append(node, slot_el)?;
                        let expr = element(xot, "expression", inner.span());
                        xot.append(slot_el, expr)?;
                        render_to_xot(xot, expr, inner, source)?;
                    }
                    Slot::Body(_) => {
                        render_to_xot(xot, node, inner, source)?;
                    }
                }
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::CFor { initializer, condition, updates, body, range, span } => {
            let node = element(xot, "for", *span);
            xot.append(parent, node)?;
            // Render header parts (init / cond / updates) in source
            // order, then the body. The imperative pipeline wraps
            // condition in `<condition><expression>`; init/updates
            // ride in bare.
            let mut header: Vec<(usize, &Ir, u8)> = Vec::new();
            if let Some(i) = initializer { header.push((i.range().start as usize, i.as_ref(), 0)); }
            if let Some(c) = condition { header.push((c.range().start as usize, c.as_ref(), 1)); }
            for u in updates { header.push((u.range().start as usize, u, 2)); }
            header.sort_by_key(|(p, _, _)| *p);
            let mut cursor = range.start;
            for (_, child, kind) in &header {
                let cr = child.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                match *kind {
                    1 => {
                        let slot = element(xot, "condition", child.span());
                        xot.append(node, slot)?;
                        let expr = element(xot, "expression", child.span());
                        xot.append(slot, expr)?;
                        render_to_xot(xot, expr, child, source)?;
                    }
                    _ => {
                        render_to_xot(xot, node, child, source)?;
                    }
                }
                cursor = cr.end;
            }
            // Body — always rendered after header.
            let br = body.range();
            emit_gap(xot, node, source, cursor, br.start)?;
            render_to_xot(xot, node, body, source)?;
            emit_gap(xot, node, source, br.end, range.end)?;
            Ok(node)
        }
        Ir::DoWhile { body, condition, range, span } => {
            let node = element(xot, "do", *span);
            xot.append(parent, node)?;
            let br = body.range();
            emit_gap(xot, node, source, range.start, br.start)?;
            render_to_xot(xot, node, body, source)?;
            let cr = condition.range();
            emit_gap(xot, node, source, br.end, cr.start)?;
            let cond_slot = element(xot, "condition", condition.span());
            xot.append(node, cond_slot)?;
            let cond_expr = element(xot, "expression", condition.span());
            xot.append(cond_slot, cond_expr)?;
            render_to_xot(xot, cond_expr, condition, source)?;
            emit_gap(xot, node, source, cr.end, range.end)?;
            Ok(node)
        }
        Ir::FieldWrap { wrapper, inner, range, span } => {
            // When the wrapper is `name`, collapse the inner to a flat
            // text leaf — `<name>` is text-only by contract. Works for
            // bare identifiers (Ir::Name), dotted paths (Ir::Path),
            // generic types (Ir::GenericType), and any other inner: we
            // emit the full source slice as the text content.
            // Mirrors the imperative pipeline's `name_wrapper`.
            if *wrapper == "name" {
                let node = element(xot, "name", *span);
                xot.append(parent, node)?;
                let text = inner.range().slice(source);
                if !text.is_empty() {
                    let t = xot.new_text(text);
                    xot.append(node, t)?;
                }
                return Ok(node);
            }
            // Skip the wrap entirely if the inner already produces
            // an element of the same name — avoids `<X><X>...</X></X>`
            // double-nesting (mirrors the imperative pipeline's
            // post-transform deduplication for body/type/name slots).
            let inner_already_emits_wrapper = match (*wrapper, inner.as_ref()) {
                ("body", Ir::Body { .. }) => true,
                ("type", Ir::GenericType { .. }) => true,
                ("type", Ir::SimpleStatement { element_name: "type", .. }) => true,
                ("name", Ir::SimpleStatement { element_name: "name", .. }) => true,
                _ => false,
            };
            if inner_already_emits_wrapper {
                let ir = inner.range();
                emit_gap(xot, parent, source, range.start, ir.start)?;
                let inner_node = render_to_xot(xot, parent, inner, source)?;
                emit_gap(xot, parent, source, ir.end, range.end)?;
                return Ok(inner_node);
            }
            let node = element(xot, wrapper, *span);
            xot.append(parent, node)?;
            let ir = inner.range();
            emit_gap(xot, node, source, range.start, ir.start)?;
            // Slot wrappers for value-producing positions (Principle #15).
            // `<value>` always contains an `<expression>` host so XPath
            // queries like `//value/expression/...` work uniformly.
            let target = if matches!(*wrapper, "value" | "condition")
                && !matches!(inner.as_ref(), Ir::Expression { .. })
            {
                let expr = element(xot, "expression", *span);
                xot.append(node, expr)?;
                expr
            } else {
                node
            };
            render_to_xot(xot, target, inner, source)?;
            emit_gap(xot, node, source, ir.end, range.end)?;
            Ok(node)
        }
        Ir::SimpleStatement { element_name, modifiers, extra_markers, children, range, span } => {
            let node = element(xot, element_name, *span);
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            for marker in *extra_markers {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Try { try_body, handlers, else_body, finally_body, range, span } => {
            let node = element(xot, "try", *span);
            xot.append(parent, node)?;
            // Source-order: try_body, handlers in source order,
            // else_body (Python only, optional), finally_body (optional).
            let mut order: Vec<&Ir> = vec![try_body.as_ref()];
            for h in handlers { order.push(h); }
            if let Some(e) = else_body { order.push(e.as_ref()); }
            if let Some(f) = finally_body { order.push(f.as_ref()); }
            order.sort_by_key(|c| c.range().start);
            let mut cursor = range.start;
            for child in &order {
                let cr = child.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                if std::ptr::eq(*child, try_body.as_ref()) {
                    render_to_xot(xot, node, child, source)?;
                } else if else_body.as_ref().map_or(false, |e| std::ptr::eq(*child, e.as_ref())) {
                    let el = element(xot, "else", child.span());
                    xot.append(node, el)?;
                    render_to_xot(xot, el, child, source)?;
                } else if finally_body.as_ref().map_or(false, |f| std::ptr::eq(*child, f.as_ref())) {
                    let fin = element(xot, "finally", child.span());
                    xot.append(node, fin)?;
                    render_to_xot(xot, fin, child, source)?;
                } else {
                    // Handler — already self-rendered as <except>/<catch>.
                    render_to_xot(xot, node, child, source)?;
                }
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::ExceptHandler { kind, type_target, binding, filter, body, range, span } => {
            let node = element(xot, kind, *span);
            xot.append(parent, node)?;
            // Python's `except [Type [as Name]]:` renders the
            // type+binding pair as `<value><expression>[<as>]...`,
            // with `<as>` wrapping ONLY when a binding is present.
            // Detect the python shape by element name "except" and
            // wrap accordingly. Other languages (C# catch) keep the
            // older slot layout: `<type>` + bare binding name.
            let python_shape = *kind == "except";
            #[derive(Clone, Copy)]
            enum Slot<'a> { Type(&'a Ir), Bind(&'a Ir), Filter(&'a Ir), Body(&'a Ir) }
            let mut order: Vec<Slot> = Vec::new();
            if let Some(t) = type_target { order.push(Slot::Type(t)); }
            if let Some(b) = binding { order.push(Slot::Bind(b)); }
            if let Some(f) = filter { order.push(Slot::Filter(f)); }
            order.push(Slot::Body(body));
            order.sort_by_key(|s| match s {
                Slot::Type(i) | Slot::Bind(i) | Slot::Filter(i) | Slot::Body(i) => i.range().start,
            });
            // Python: if both type and binding present, wrap them
            // together in <value><expression><as>...</as></expression></value>.
            // If only type (no binding), wrap in <value><expression>.
            // Otherwise fall back to the slot-style layout.
            if python_shape {
                if let Some(t) = type_target {
                    let cr = t.range();
                    emit_gap(xot, node, source, range.start, cr.start)?;
                    let outer_end = binding.as_ref().map(|b| b.range().end).unwrap_or(cr.end);
                    let value = element(xot, "value", t.span());
                    xot.append(node, value)?;
                    let expr = element(xot, "expression", t.span());
                    xot.append(value, expr)?;
                    if let Some(b) = binding {
                        let as_el = element(xot, "as", t.span());
                        xot.append(expr, as_el)?;
                        render_to_xot(xot, as_el, t, source)?;
                        emit_gap(xot, as_el, source, cr.end, b.range().start)?;
                        render_to_xot(xot, as_el, b, source)?;
                    } else {
                        render_to_xot(xot, expr, t, source)?;
                    }
                    let mut cursor = outer_end;
                    if let Some(f) = filter {
                        let fr = f.range();
                        emit_gap(xot, node, source, cursor, fr.start)?;
                        let fil = element(xot, "filter", f.span());
                        xot.append(node, fil)?;
                        let fexpr = element(xot, "expression", f.span());
                        xot.append(fil, fexpr)?;
                        render_to_xot(xot, fexpr, f, source)?;
                        cursor = fr.end;
                    }
                    let br = body.range();
                    emit_gap(xot, node, source, cursor, br.start)?;
                    render_to_xot(xot, node, body, source)?;
                    emit_gap(xot, node, source, br.end, range.end)?;
                    return Ok(node);
                }
                // No type — fall through to slot-style (bare except).
            }
            let mut cursor = range.start;
            for slot in &order {
                let inner: &Ir = match slot {
                    Slot::Type(i) | Slot::Bind(i) | Slot::Filter(i) | Slot::Body(i) => i,
                };
                let cr = inner.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                match slot {
                    Slot::Type(_) => {
                        let t = element(xot, "type", inner.span());
                        xot.append(node, t)?;
                        render_to_xot(xot, t, inner, source)?;
                    }
                    Slot::Bind(_) => {
                        render_to_xot(xot, node, inner, source)?;
                    }
                    Slot::Filter(_) => {
                        let f = element(xot, "filter", inner.span());
                        xot.append(node, f)?;
                        let expr = element(xot, "expression", inner.span());
                        xot.append(f, expr)?;
                        render_to_xot(xot, expr, inner, source)?;
                    }
                    Slot::Body(_) => {
                        render_to_xot(xot, node, inner, source)?;
                    }
                }
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::TypeAlias { name, type_params, value, range, span } => {
            let node = element(xot, "alias", *span);
            xot.append(parent, node)?;
            // Source-order: name, type_params (if any), value.
            // The `type` keyword + `=` live in gap text.
            let mut order: Vec<&Ir> = vec![name.as_ref()];
            if let Some(p) = type_params { order.push(p.as_ref()); }
            order.push(value.as_ref());
            order.sort_by_key(|c| c.range().start);
            // Wrap name in <left>, value in <right> with <type> wrappers.
            let mut cursor = range.start;
            for child in &order {
                let cr = child.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                if std::ptr::eq(*child, name.as_ref()) {
                    let left = element(xot, "left", child.span());
                    xot.append(node, left)?;
                    let type_el = element(xot, "type", child.span());
                    xot.append(left, type_el)?;
                    render_to_xot(xot, type_el, child, source)?;
                } else if type_params.as_ref().map_or(false, |p| std::ptr::eq(*child, p.as_ref())) {
                    render_to_xot(xot, node, child, source)?;
                } else {
                    let right = element(xot, "right", child.span());
                    xot.append(node, right)?;
                    let type_el = element(xot, "type", child.span());
                    xot.append(right, type_el)?;
                    render_to_xot(xot, type_el, child, source)?;
                }
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::KeywordArgument { name, value, range, span } => {
            let node = element(xot, "keyword", *span);
            xot.append(parent, node)?;
            let nr = name.range();
            let vr = value.range();
            emit_gap(xot, node, source, range.start, nr.start)?;
            // Name goes in <name> ... not bare. Actually existing
            // pipeline shape: <keyword><name>x</name>=<value><expression>...</expression></value></keyword>.
            // For now, render name as Ir::Name (it'll wrap as <name>),
            // and value as <value><expression>...</expression></value>.
            render_to_xot(xot, node, name, source)?;
            emit_gap(xot, node, source, nr.end, vr.start)?;
            let val = element(xot, "value", value.span());
            xot.append(node, val)?;
            let expr = element(xot, "expression", value.span());
            xot.append(val, expr)?;
            render_to_xot(xot, expr, value, source)?;
            emit_gap(xot, node, source, vr.end, range.end)?;
            Ok(node)
        }
        Ir::ListSplat { inner, range, span } => {
            // `<spread>` container with a `<list/>` discriminator
            // marker — matches the imperative pipeline's
            // `RenameWithMarker(Spread, List)` shape on `list_splat`.
            // (`<splat/>` is reserved as a MarkerOnly name in some
            // language vocabularies, so an element-named `<splat>`
            // would trip `marker-only-no-element-children`.)
            let node = element(xot, "spread", *span);
            xot.append(parent, node)?;
            let m = element(xot, "list", *span);
            xot.append(node, m)?;
            let ir_range = inner.range();
            emit_gap(xot, node, source, range.start, ir_range.start)?;
            render_to_xot(xot, node, inner, source)?;
            emit_gap(xot, node, source, ir_range.end, range.end)?;
            Ok(node)
        }
        Ir::DictSplat { inner, range, span } => {
            let node = element(xot, "spread", *span);
            xot.append(parent, node)?;
            let m = element(xot, "dict", *span);
            xot.append(node, m)?;
            let ir_range = inner.range();
            emit_gap(xot, node, source, range.start, ir_range.start)?;
            render_to_xot(xot, node, inner, source)?;
            emit_gap(xot, node, source, ir_range.end, range.end)?;
            Ok(node)
        }
        Ir::Ternary { condition, if_true, if_false, range, span } => {
            let node = element(xot, "ternary", *span);
            xot.append(parent, node)?;
            // Slot layout: <condition><expression>{cond}</expression></condition>
            // {if_true wrapped in <expression>}
            // <else><expression>{if_false}</expression></else>
            // The slot wrapper element is determined by *which* child
            // we're emitting, regardless of source order.
            #[derive(Clone, Copy)]
            enum Slot<'a> { Cond(&'a Ir), True(&'a Ir), False(&'a Ir) }
            let mut order: Vec<Slot> = vec![
                Slot::Cond(condition),
                Slot::True(if_true),
                Slot::False(if_false),
            ];
            order.sort_by_key(|s| match s {
                Slot::Cond(i) | Slot::True(i) | Slot::False(i) => i.range().start,
            });
            let mut cursor = range.start;
            for slot in &order {
                let inner: &Ir = match slot {
                    Slot::Cond(i) | Slot::True(i) | Slot::False(i) => i,
                };
                let cr = inner.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                match slot {
                    Slot::Cond(_) => {
                        let cs = element(xot, "condition", inner.span());
                        xot.append(node, cs)?;
                        let expr = element(xot, "expression", inner.span());
                        xot.append(cs, expr)?;
                        render_to_xot(xot, expr, inner, source)?;
                    }
                    Slot::True(_) => {
                        let then = element(xot, "then", inner.span());
                        xot.append(node, then)?;
                        // No `<expression>` wrapper — Python's
                        // imperative pipeline emits the inner directly
                        // as a `<then>` child. C# tests use the same
                        // flat shape (`<then>` directly contains the
                        // value or expression-shaped element).
                        render_to_xot(xot, then, inner, source)?;
                    }
                    Slot::False(_) => {
                        let el = element(xot, "else", inner.span());
                        xot.append(node, el)?;
                        render_to_xot(xot, el, inner, source)?;
                    }
                }
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::ObjectCreation { type_target, arguments, initializer, range, span } => {
            let node = element(xot, "new", *span);
            xot.append(parent, node)?;
            // Source-order children: type? args... initializer?
            // The `new` keyword + parens + braces live in gap text.
            let mut order: Vec<&Ir> = Vec::new();
            if let Some(t) = type_target { order.push(t.as_ref()); }
            for a in arguments { order.push(a); }
            if let Some(i) = initializer { order.push(i.as_ref()); }
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Lambda { modifiers, parameters, body, range, span } => {
            let node = element(xot, "lambda", *span);
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            // Source-order children: parameters then body. The `=>`
            // token + parens (when present) live in gap text.
            let mut order: Vec<&Ir> = Vec::new();
            for p in parameters { order.push(p); }
            order.push(body.as_ref());
            order.sort_by_key(|c| c.range().start);

            let is_block_body = matches!(body.as_ref(), Ir::Body { .. });
            let mut cursor = range.start;
            for child in &order {
                let cr = child.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                if std::ptr::eq(*child, body.as_ref()) {
                    if is_block_body {
                        // Block-bodied — render as <body> directly.
                        render_to_xot(xot, node, child, source)?;
                    } else {
                        // Expression-bodied — wrap in <value><expression>.
                        let val = element(xot, "value", child.span());
                        xot.append(node, val)?;
                        let expr = element(xot, "expression", child.span());
                        xot.append(val, expr)?;
                        render_to_xot(xot, expr, child, source)?;
                    }
                } else {
                    // Parameter — render as-is (Ir::Parameter handles its own wrapping).
                    render_to_xot(xot, node, child, source)?;
                }
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::Break { range, span } => {
            let node = element(xot, "break", *span);
            xot.append(parent, node)?;
            emit_gap(xot, node, source, range.start, range.end)?;
            Ok(node)
        }
        Ir::Continue { range, span } => {
            let node = element(xot, "continue", *span);
            xot.append(parent, node)?;
            emit_gap(xot, node, source, range.start, range.end)?;
            Ok(node)
        }
        Ir::Function { element_name, modifiers, decorators, name, generics, parameters, returns, body, range, span } => {
            let node = element(xot, element_name, *span);
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, Span::point(span.line, span.column));
                xot.append(node, m)?;
            }
            // Generics expand to flat `<generic>` siblings (not wrapped
            // in an outer `<generic>` container) — matches the
            // imperative pipeline's flat-list shape (Principle #12).
            let mut order: Vec<&Ir> = Vec::new();
            for d in decorators { order.push(d); }
            order.push(name.as_ref());
            if let Some(g) = generics {
                if let Ir::Generic { items, .. } = g.as_ref() {
                    for it in items { order.push(it); }
                } else {
                    order.push(g.as_ref());
                }
            }
            for p in parameters { order.push(p); }
            if let Some(r) = returns { order.push(r.as_ref()); }
            if let Some(b) = body { order.push(b.as_ref()); }
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Class { kind, modifiers, decorators, name, generics, bases, where_clauses, body, range, span } => {
            let node = element(xot, kind, *span);
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            // Source-order children: decorators, name, generic items
            // (flat siblings, not wrapped in outer `<generic>`),
            // bases (wrapped in `<extends><type>...`), where clauses,
            // body.
            #[derive(Clone, Copy)]
            enum CSlot<'a> {
                Decor(&'a Ir),
                Name(&'a Ir),
                Generics(&'a Ir),
                Base(&'a Ir),
                Where(&'a Ir),
                Body(&'a Ir),
            }
            let mut order: Vec<CSlot> = Vec::new();
            for d in decorators { order.push(CSlot::Decor(d)); }
            order.push(CSlot::Name(name));
            if let Some(g) = generics {
                if let Ir::Generic { items, .. } = g.as_ref() {
                    for it in items { order.push(CSlot::Generics(it)); }
                } else {
                    order.push(CSlot::Generics(g));
                }
            }
            for b in bases { order.push(CSlot::Base(b)); }
            for w in where_clauses { order.push(CSlot::Where(w)); }
            order.push(CSlot::Body(body));
            order.sort_by_key(|s| match s {
                CSlot::Decor(i) | CSlot::Name(i) | CSlot::Generics(i)
                | CSlot::Base(i) | CSlot::Where(i) | CSlot::Body(i) => i.range().start,
            });
            let mut cursor = range.start;
            for slot in &order {
                let inner: &Ir = match slot {
                    CSlot::Decor(i) | CSlot::Name(i) | CSlot::Generics(i)
                    | CSlot::Base(i) | CSlot::Where(i) | CSlot::Body(i) => i,
                };
                let cr = inner.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                if matches!(slot, CSlot::Base(_)) {
                    // Bases wrap in `<extends><type>...</type></extends>`
                    // — when the inner is already a type-shaped IR
                    // (GenericType produces its own `<type>`), don't
                    // double-wrap. When the inner is itself an
                    // `<implements>` SimpleStatement (Java's
                    // interface base), emit it as-is without the
                    // `<extends>` wrap.
                    let inner_already_wrapped = matches!(
                        inner,
                        Ir::SimpleStatement { element_name: "implements", .. }
                            | Ir::SimpleStatement { element_name: "extends", .. }
                    );
                    if inner_already_wrapped {
                        render_to_xot(xot, node, inner, source)?;
                    } else {
                        let ext = element(xot, "extends", inner.span());
                        xot.append(node, ext)?;
                        let already_typed = matches!(inner,
                            Ir::GenericType { .. }
                                | Ir::SimpleStatement { element_name: "type", .. }
                        );
                        if already_typed {
                            render_to_xot(xot, ext, inner, source)?;
                        } else {
                            let t = element(xot, "type", inner.span());
                            xot.append(ext, t)?;
                            render_to_xot(xot, t, inner, source)?;
                        }
                    }
                } else {
                    render_to_xot(xot, node, inner, source)?;
                }
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::Body { children, pass_only, block_wrap, range, span } => {
            let node = element(xot, "body", *span);
            xot.append(parent, node)?;
            // Optional inner `<block>` element so the rendered shape
            // is `<body><block>{stmts}</block></body>` (C#).
            let target = if *block_wrap {
                let block = element(xot, "block", *span);
                xot.append(node, block)?;
                block
            } else {
                node
            };
            if *pass_only {
                let m = element(xot, "pass", *span);
                xot.append(target, m)?;
                emit_gap(xot, target, source, range.start, range.end)?;
            } else {
                render_with_gaps(xot, target, source, *range, children, |xot, parent, child| {
                    render_to_xot(xot, parent, child, source).map(|_| ())
                })?;
            }
            Ok(node)
        }
        Ir::Parameter { kind, extra_markers, modifiers, name, type_ann, default, range, span } => {
            let node = element(xot, "parameter", *span);
            xot.append(parent, node)?;
            // Marker for *args / **kwargs — first child, empty.
            match kind {
                ParamKind::Args => {
                    let m = element(xot, "args", *span);
                    xot.append(node, m)?;
                }
                ParamKind::Kwargs => {
                    let m = element(xot, "kwargs", *span);
                    xot.append(node, m)?;
                }
                ParamKind::Regular => {}
            }
            // Access / modifier markers (TS constructor parameter shorthand:
            // `private readonly id: number`, etc.).
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            for marker in *extra_markers {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            // Source-ordered children: type / name / default. Python
            // and C# put these in different orders (`x: int = 5` vs
            // `int x = 5`); sorting by range avoids duplicating bytes
            // in gap text.
            #[derive(Clone, Copy)]
            enum Slot<'a> { Name(&'a Ir), Type(&'a Ir), Default(&'a Ir) }
            let mut order: Vec<Slot> = vec![Slot::Name(name)];
            if let Some(t) = type_ann { order.push(Slot::Type(t)); }
            if let Some(d) = default { order.push(Slot::Default(d)); }
            order.sort_by_key(|s| match s {
                Slot::Name(i) | Slot::Type(i) | Slot::Default(i) => i.range().start,
            });

            let mut cursor = range.start;
            for slot in &order {
                let inner: &Ir = match slot {
                    Slot::Name(i) | Slot::Type(i) | Slot::Default(i) => i,
                };
                let ir_range = inner.range();
                emit_gap(xot, node, source, cursor, ir_range.start)?;
                match slot {
                    Slot::Name(_) => {
                        render_to_xot(xot, node, inner, source)?;
                    }
                    Slot::Type(_) => {
                        let type_el = element(xot, "type", inner.span());
                        xot.append(node, type_el)?;
                        render_to_xot(xot, type_el, inner, source)?;
                    }
                    Slot::Default(_) => {
                        let val = element(xot, "value", inner.span());
                        xot.append(node, val)?;
                        let expr = element(xot, "expression", inner.span());
                        xot.append(val, expr)?;
                        render_to_xot(xot, expr, inner, source)?;
                    }
                }
                cursor = ir_range.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::PositionalSeparator { range, span } => {
            leaf(xot, parent, "positional", source, *range, *span)
        }
        Ir::KeywordSeparator { range, span } => {
            leaf(xot, parent, "keyword", source, *range, *span)
        }
        Ir::Decorator { inner, range, span } => {
            let node = element(xot, "decorator", *span);
            xot.append(parent, node)?;
            render_with_gaps(xot, node, source, *range, std::slice::from_ref(inner.as_ref()),
                |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ())
            )?;
            Ok(node)
        }
        Ir::Returns { type_ann, range, span } => {
            let node = element(xot, "returns", *span);
            xot.append(parent, node)?;
            // <returns> wraps the type annotation in <type>, unless
            // the inner already produces a `<type>` element (e.g. an
            // `Ir::GenericType` or a `SimpleStatement<type>`) — in
            // which case we'd double-wrap.
            let tr = type_ann.range();
            emit_gap(xot, node, source, range.start, tr.start)?;
            let already_typed = matches!(
                type_ann.as_ref(),
                Ir::GenericType { .. }
                    | Ir::SimpleStatement { element_name: "type", .. }
                    | Ir::SimpleStatement { element_name: "predicate", .. }
            );
            if already_typed {
                render_to_xot(xot, node, type_ann, source)?;
            } else {
                let type_el = element(xot, "type", type_ann.span());
                xot.append(node, type_el)?;
                render_to_xot(xot, type_el, type_ann, source)?;
            }
            emit_gap(xot, node, source, tr.end, range.end)?;
            Ok(node)
        }
        Ir::Generic { items, range, span } => {
            let node = element(xot, "generic", *span);
            xot.append(parent, node)?;
            render_with_gaps(xot, node, source, *range, items, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::TypeParameter { name, constraint, range, span } => {
            // Renders as <type><name>...</name></type>.
            let node = element(xot, "type", *span);
            xot.append(parent, node)?;
            let mut order: Vec<&Ir> = vec![name.as_ref()];
            if let Some(c) = constraint { order.push(c.as_ref()); }
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Return { value, range, span } => {
            let node = element(xot, "return", *span);
            xot.append(parent, node)?;
            // `return value` wraps value in <expression> host;
            // `return a, b, c` (Ir::Inline value) wraps each item in
            // its own <expression> so the post-pass can tag them with
            // `list="expressions"` for JSON projection.
            if let Some(v) = value {
                let vr = v.range();
                emit_gap(xot, node, source, range.start, vr.start)?;
                if let Ir::Inline { children, .. } = v.as_ref() {
                    let mut cursor = vr.start;
                    for c in children {
                        let cr = c.range();
                        emit_gap(xot, node, source, cursor, cr.start)?;
                        let expr = element(xot, "expression", c.span());
                        xot.append(node, expr)?;
                        render_to_xot(xot, expr, c, source)?;
                        cursor = cr.end;
                    }
                    emit_gap(xot, node, source, cursor, vr.end)?;
                } else {
                    let expr = element(xot, "expression", v.span());
                    xot.append(node, expr)?;
                    render_to_xot(xot, expr, v, source)?;
                }
                emit_gap(xot, node, source, vr.end, range.end)?;
            } else {
                emit_gap(xot, node, source, range.start, range.end)?;
            }
            Ok(node)
        }
        Ir::Comment { leading, trailing, range, span } => {
            let node = element(xot, "comment", *span);
            xot.append(parent, node)?;
            if *leading {
                let m = element(xot, "leading", *span);
                xot.append(node, m)?;
            }
            if *trailing {
                let m = element(xot, "trailing", *span);
                xot.append(node, m)?;
            }
            let text = range.slice(source);
            if !text.is_empty() {
                let t = xot.new_text(text);
                xot.append(node, t)?;
            }
            Ok(node)
        }
        Ir::Assign { .. } => render_ir_assign(xot, parent, ir, source),
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
        Ir::Binary { element_name, op_text, op_marker, op_range, left, right, range, span } => {
            let node = element(xot, element_name, *span);
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

            let op_node = element(xot, "op", Span::point(span.line, span.column));
            xot.append(node, op_node)?;
            if !op_text.is_empty() {
                let t = xot.new_text(op_text);
                xot.append(op_node, t)?;
            }
            let _ = op_marker;
            crate::transform::operators::add_operator_markers(xot, op_node, op_text)
                .map_err(|e| xot::Error::Io(format!("op marker: {e}")))?;

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
        Ir::Unary { op_text, op_marker, op_range, operand, extra_markers, range, span } => {
            let node = element(xot, "unary", *span);
            xot.append(parent, node)?;
            for marker in *extra_markers {
                let m = element(xot, marker, Span::point(span.line, span.column));
                xot.append(node, m)?;
            }

            let operand_range = operand.range();
            let is_postfix = op_range.start >= operand_range.end;

            if is_postfix {
                // Operand first, then `<op>`. e.g. `i++`.
                emit_gap(xot, node, source, range.start, operand_range.start)?;
                render_to_xot(xot, node, operand, source)?;
                emit_gap(xot, node, source, operand_range.end, op_range.start)?;
                let op_node = element(xot, "op", Span::point(span.line, span.column));
                xot.append(node, op_node)?;
                if !op_text.is_empty() {
                    let t = xot.new_text(op_text);
                    xot.append(op_node, t)?;
                }
                let _ = op_marker;
                crate::transform::operators::add_operator_markers(xot, op_node, op_text)
                    .map_err(|e| xot::Error::Io(format!("op marker: {e}")))?;
                emit_gap(xot, node, source, op_range.end, range.end)?;
            } else {
                // Prefix: pre-op gap, op, gap, operand, trailing gap.
                emit_gap(xot, node, source, range.start, op_range.start)?;

                let op_node = element(xot, "op", Span::point(span.line, span.column));
                xot.append(node, op_node)?;
                if !op_text.is_empty() {
                    let t = xot.new_text(op_text);
                    xot.append(op_node, t)?;
                }
                let _ = op_marker;
                crate::transform::operators::add_operator_markers(xot, op_node, op_text)
                    .map_err(|e| xot::Error::Io(format!("op marker: {e}")))?;

                emit_gap(xot, node, source, op_range.end, operand_range.start)?;

                render_to_xot(xot, node, operand, source)?;

                emit_gap(xot, node, source, operand_range.end, range.end)?;
            }

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
        Ir::Null { range, span } => leaf(xot, parent, "null", source, *range, *span),
        Ir::Enum { modifiers, decorators, name, underlying_type, members, range, span } => {
            let node = element(xot, "enum", *span);
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            let mut order: Vec<&Ir> = Vec::new();
            for d in decorators { order.push(d); }
            order.push(name.as_ref());
            if let Some(t) = underlying_type { order.push(t.as_ref()); }
            for me in members { order.push(me); }
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::EnumMember { decorators, name, value, range, span } => {
            let node = element(xot, "constant", *span);
            xot.append(parent, node)?;
            let mut order: Vec<&Ir> = Vec::new();
            for d in decorators { order.push(d); }
            order.push(name.as_ref());
            if let Some(v) = value { order.push(v.as_ref()); }
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Property { modifiers, decorators, type_ann, name, accessors, value, range, span } => {
            let node = element(xot, "property", *span);
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            let mut order: Vec<&Ir> = Vec::new();
            for d in decorators { order.push(d); }
            if let Some(t) = type_ann { order.push(t.as_ref()); }
            order.push(name.as_ref());
            for a in accessors { order.push(a); }
            if let Some(v) = value { order.push(v.as_ref()); }
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Accessor { modifiers, kind, body, range, span } => {
            let node = element(xot, kind, *span);  // <get/>, <set/>, <init/>
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            if let Some(b) = body {
                let br = b.range();
                emit_gap(xot, node, source, range.start, br.start)?;
                render_to_xot(xot, node, b, source)?;
                emit_gap(xot, node, source, br.end, range.end)?;
            } else {
                emit_gap(xot, node, source, range.start, range.end)?;
            }
            Ok(node)
        }
        Ir::Constructor { modifiers, decorators, name, parameters, body, range, span } => {
            let node = element(xot, "constructor", *span);
            xot.append(parent, node)?;
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            let mut order: Vec<&Ir> = Vec::new();
            for d in decorators { order.push(d); }
            order.push(name.as_ref());
            for p in parameters { order.push(p); }
            order.push(body.as_ref());
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Using { is_static, alias, path, range, span } => {
            // C# `using_directive` renders as <import> (matching the
            // imperative pipeline). Block-scoped `using_statement` is
            // a separate kind handled via SimpleStatement "using".
            // The `static` keyword is captured in gap text for parity;
            // the `is_static` field stays on the IR for mutation.
            let _ = is_static;
            let node = element(xot, "import", *span);
            xot.append(parent, node)?;
            let mut order: Vec<&Ir> = Vec::new();
            order.push(path.as_ref());
            if let Some(a) = alias { order.push(a.as_ref()); }
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Namespace { name, children, file_scoped, range, span } => {
            let node = element(xot, "namespace", *span);
            xot.append(parent, node)?;
            // `<file/>` marker for `namespace Foo;` form — the C#
            // post_transform's unify_file_scoped_namespace looks for
            // this marker to fold following siblings into the body.
            if *file_scoped {
                let m = element(xot, "file", *span);
                xot.append(node, m)?;
            }
            let mut order: Vec<&Ir> = vec![name.as_ref()];
            for c in children { order.push(c); }
            order.sort_by_key(|c| c.range().start);
            render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            Ok(node)
        }
        Ir::Variable { element_name, modifiers, decorators, type_ann, name, value, range, span } => {
            let node = element(xot, element_name, *span);
            for marker in modifiers.marker_names() {
                let m = element(xot, marker, *span);
                xot.append(node, m)?;
            }
            xot.append(parent, node)?;
            // Source order: decorators, type (optional), name, value (optional).
            let mut order: Vec<&Ir> = Vec::new();
            for d in decorators { order.push(d); }
            if let Some(t) = type_ann { order.push(t.as_ref()); }
            order.push(name.as_ref());
            if let Some(v) = value { order.push(v.as_ref()); }
            order.sort_by_key(|c| c.range().start);
            // Wrap type_ann in <type> when present, but only if the
            // inner doesn't already produce its own <type> outer
            // (e.g. Ir::GenericType, or a SimpleStatement with
            // element_name == "type"). Otherwise we'd render
            // <type><type>...</type></type> which breaks XPath text
            // queries like `//type[.='List<string>']`.
            let mut cursor = range.start;
            for c in &order {
                let cr = c.range();
                emit_gap(xot, node, source, cursor, cr.start)?;
                if let Some(t) = type_ann {
                    if std::ptr::eq(*c, t.as_ref()) {
                        let already_typed = matches!(*c,
                            Ir::GenericType { .. }
                                | Ir::SimpleStatement { element_name: "type", .. }
                        );
                        if already_typed {
                            render_to_xot(xot, node, *c, source)?;
                        } else {
                            let type_el = element(xot, "type", c.span());
                            xot.append(node, type_el)?;
                            render_to_xot(xot, type_el, c, source)?;
                        }
                        cursor = cr.end;
                        continue;
                    }
                }
                render_to_xot(xot, node, *c, source)?;
                cursor = cr.end;
            }
            emit_gap(xot, node, source, cursor, range.end)?;
            Ok(node)
        }
        Ir::Is { value, type_target, range, span } => {
            let node = element(xot, "is", *span);
            xot.append(parent, node)?;
            // <left><expression>{value}</expression></left>
            let vr = value.range();
            emit_gap(xot, node, source, range.start, vr.start)?;
            let left_slot = element(xot, "left", value.span());
            xot.append(node, left_slot)?;
            let left_expr = element(xot, "expression", value.span());
            xot.append(left_slot, left_expr)?;
            render_to_xot(xot, left_expr, value, source)?;
            // Gap between value and type (`is` keyword + spaces).
            let tr = type_target.range();
            emit_gap(xot, node, source, vr.end, tr.start)?;
            // <right><expression><type>{type_target}</type></expression></right>
            let right_slot = element(xot, "right", type_target.span());
            xot.append(node, right_slot)?;
            let right_expr = element(xot, "expression", type_target.span());
            xot.append(right_slot, right_expr)?;
            let type_el = element(xot, "type", type_target.span());
            xot.append(right_expr, type_el)?;
            render_to_xot(xot, type_el, type_target, source)?;
            // Trailing.
            emit_gap(xot, node, source, tr.end, range.end)?;
            Ok(node)
        }
        Ir::Cast { type_ann, value, range, span } => {
            let node = element(xot, "cast", *span);
            xot.append(parent, node)?;
            // <type>...</type>
            let tr = type_ann.range();
            emit_gap(xot, node, source, range.start, tr.start)?;
            let type_el = element(xot, "type", type_ann.span());
            xot.append(node, type_el)?;
            render_to_xot(xot, type_el, type_ann, source)?;
            // <value><expression>...</expression></value>
            let vr = value.range();
            emit_gap(xot, node, source, tr.end, vr.start)?;
            let value_el = element(xot, "value", value.span());
            xot.append(node, value_el)?;
            let expr_el = element(xot, "expression", value.span());
            xot.append(value_el, expr_el)?;
            render_to_xot(xot, expr_el, value, source)?;
            // Trailing gap after value.
            emit_gap(xot, node, source, vr.end, range.end)?;
            Ok(node)
        }

        Ir::Inline { children, list_name, range, span: _ } => {
            // Inline contributes no element of its own. Children render
            // at the parent level; gap text from the inline's range
            // wraps them. When `list_name` is set, every direct
            // element child gets a `list="X"` attribute — matches the
            // imperative pipeline's `distribute_list` post-pass and
            // enables plural-key JSON projection.
            let before: Vec<XotNode> = xot.children(parent).collect();
            render_with_gaps(xot, parent, source, *range, children, |xot, parent, child| {
                render_to_xot(xot, parent, child, source).map(|_| ())
            })?;
            if let Some(list) = list_name {
                let list_attr = xot.add_name("list");
                let new_children: Vec<XotNode> = xot.children(parent)
                    .filter(|c| !before.contains(c))
                    .collect();
                for c in new_children {
                    if xot.element(c).is_some() {
                        xot.attributes_mut(c).insert(list_attr, list.to_string());
                    }
                }
            }
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
// Per-arm renderers (out-of-line)
// ---------------------------------------------------------------------------
//
// Each `render_ir_<variant>` mirrors one arm of `render_to_xot`'s match.
// They're `#[inline(never)]` so the compiler doesn't fold them back into
// the dispatcher's frame — that's the whole point: the dispatcher's
// match becomes a thin jump table rather than a wide-frame function
// reserving stack space worst-case across every arm. Recursive
// IR walks at depth 20-30+ now run on default 2 MiB thread stacks
// instead of overflowing.

#[inline(never)]
fn render_ir_assign(
    xot: &mut Xot,
    parent: XotNode,
    ir: &Ir,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let Ir::Assign { targets, type_annotation, op_text, op_range, op_markers, values, range, span } = ir
        else { unreachable!() };
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

    // <op>{op_text}{markers}</op> — markers come from the
    // canonical OPERATOR_MARKERS table (shared with the
    // imperative pipeline) keyed by op_text. The `op_markers`
    // field on Ir::Assign is now unused.
    let _ = op_markers;
    if !op_text.is_empty() {
        let op_node = element(xot, "op", *span);
        xot.append(node, op_node)?;
        let t = xot.new_text(op_text);
        xot.append(op_node, t)?;
        crate::transform::operators::add_operator_markers(xot, op_node, op_text)
            .map_err(|e| xot::Error::Io(format!("op marker: {e}")))?;
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
            // Don't double-wrap when the value already
            // produces an `<expression>` host (Ir::Expression
            // / await / non-null markers).
            if matches!(v, Ir::Expression { .. }) {
                render_to_xot(xot, right_node, v, source)?;
            } else {
                let expr = element(xot, "expression", v.span());
                xot.append(right_node, expr)?;
                render_to_xot(xot, expr, v, source)?;
            }
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
        AccessSegment::Member { property_range, property_span, optional, range: _, span } => {
            let node = element(xot, "member", *span);
            xot.append(host, node)?;
            // <optional/> empty marker — first child if conditional
            // (`?.`). No text contribution; XPath text-recovery
            // unaffected.
            if *optional {
                let m = element(xot, "optional", *span);
                xot.append(node, m)?;
            }
            // Internal gap from segment-start to property-name (the `.`
            // or `?.`).
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
            // For multi-arg indexers (`arr[1, 2, 3]`), wrap each
            // index in `<argument>` so the post_transform's
            // `("index", "argument")` tag pair tags them with
            // `list="arguments"`. Single-index keeps the bare child
            // (matches Python's `<index><int>0</int></index>` shape).
            let wrap = indices.len() > 1;
            let mut cursor_pos = seg_range.start;
            for idx in indices {
                let r = idx.range();
                emit_gap(xot, node, source, cursor_pos, r.start)?;
                if wrap {
                    let arg = element(xot, "argument", idx.span());
                    xot.append(node, arg)?;
                    render_to_xot(xot, arg, idx, source)?;
                } else {
                    render_to_xot(xot, node, idx, source)?;
                }
                cursor_pos = r.end;
            }
            emit_gap(xot, node, source, cursor_pos, seg_range.end)?;
            let mut inner_cursor = seg_range.end;
            render_segments_chain(xot, node, rest, &mut inner_cursor, source)?;
            *cursor = inner_cursor;
            node
        }
        AccessSegment::Call { name, name_span, arguments, range: _, span } => {
            let node = element(xot, "call", *span);
            xot.append(host, node)?;
            // Optional `<name>Method</name>` first child when the
            // call absorbed the preceding member's property name.
            if let (Some(name_range), Some(ns)) = (*name, *name_span) {
                emit_gap(xot, node, source, seg_range.start, name_range.start)?;
                leaf(xot, node, "name", source, name_range, ns)?;
                emit_gap(xot, node, source, name_range.end, seg_range.end.min(name_range.end))?;
                let inner_refs: Vec<&Ir> = arguments.iter().collect();
                let arg_range = ByteRange::new(name_range.end, seg_range.end);
                render_with_gaps(xot, node, source, arg_range, &inner_refs,
                    |xot, parent, &child| render_to_xot(xot, parent, child, source).map(|_| ()),
                )?;
            } else {
                let inner_refs: Vec<&Ir> = arguments.iter().collect();
                render_with_gaps(xot, node, source, seg_range, &inner_refs,
                    |xot, parent, &child| render_to_xot(xot, parent, child, source).map(|_| ()),
                )?;
            }
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
