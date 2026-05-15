//! tree → Xot rendering.
//!
//! Mechanical translation: given an [`SyntaxTree`] tree and the original
//! `source` string, build the corresponding Xot tree. No decisions
//! live here — every shape choice is encoded in the tree variants.
//!
//! ## Invariants
//! For every tree node `n` rendered to XML element `E`:
//!
//! 1. **Text recovery.** XPath `string(E)` (concatenation of all
//!    descendant text in document order) equals `source[n.range]`.
//!    This makes `[.='foo()']` matching by literal source text a
//!    valid query.
//! 2. **Source attributes.** `E` carries `line`, `column`, `end_line`,
//!    `end_column` matching `n.span`.
//! 3. **No source loss.** Every byte of `source` covered by the root
//!    tree ends up in *some* descendant text node of the root element,
//!    in source order.
//!
//! ## How gap text works
//! For a container tree node with byte range `[P_start, P_end)` and
//! source-derived children with ranges `[c0..c1) [c2..c3) ...` (in
//! source order):
//!
//! - Pre-first-child gap: `source[P_start .. c0]` — emitted as text
//!   inside `E` before child 0's element.
//! - Inter-child gap: `source[c1 .. c2]` — emitted between children.
//! - Trailing gap: `source[c_last_end .. P_end]` — emitted after the
//!   last child.
//!
//! Synthetic tree (markers like `<access/>`, slot wrappers like `<left>`)
//! is emitted at variant-determined positions and contributes zero
//! text. It does not participate in gap calculation.

use xot::{Node as XotNode, Xot};

use super::types::{AccessReceiver, AccessSegment, ByteRange, LambdaBody, SyntaxTree, ParamKind, Span};

/// Render an [`SyntaxTree`] tree as a child of `parent` in the given Xot
/// document. Returns the root node of the rendered subtree.
///
/// `source` must be the same string the tree was lowered from.
pub fn render_to_xot(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    // Walker-eligible variants render through the variant-blind
    // accessors in `tree::walker`. The per-variant `render_tree_*`
    // arms below survive only for variants whose tree shape still
    // depends on renderer-side synthesis (slot / op wrappers being
    // lifted slice-by-slice).
    use super::walker::{walker_eligibility, render_generic, WalkerEligibility};
    if walker_eligibility(tree) == WalkerEligibility::Eligible {
        return render_generic(xot, parent, tree, source);
    }
    match tree {
        SyntaxTree::Module { .. } => render_tree_module(xot, parent, tree, source),
        SyntaxTree::Expression { .. } => render_tree_expression(xot, parent, tree, source),
        SyntaxTree::Access { .. } => render_tree_access(xot, parent, tree, source),
        SyntaxTree::Tuple { .. } => render_tree_tuple(xot, parent, tree, source),
        SyntaxTree::List { .. } => render_tree_list(xot, parent, tree, source),
        SyntaxTree::Set { .. } => render_tree_set(xot, parent, tree, source),
        SyntaxTree::Dictionary { .. } => render_tree_dictionary(xot, parent, tree, source),
        SyntaxTree::Pair { .. } => render_tree_pair(xot, parent, tree, source),
        SyntaxTree::GenericType { .. } => render_tree_generic_type(xot, parent, tree, source),
        SyntaxTree::Comparison { .. } => render_tree_comparison(xot, parent, tree, source),
        SyntaxTree::If { .. } => render_tree_if(xot, parent, tree, source),
        SyntaxTree::ElseIf { .. } => render_tree_else_if(xot, parent, tree, source),
        SyntaxTree::Else { .. } => render_tree_else(xot, parent, tree, source),
        SyntaxTree::For { .. } => render_tree_for(xot, parent, tree, source),
        SyntaxTree::While { .. } => render_tree_while(xot, parent, tree, source),
        SyntaxTree::Foreach { .. } => render_tree_foreach(xot, parent, tree, source),
        SyntaxTree::CFor { .. } => render_tree_cfor(xot, parent, tree, source),
        SyntaxTree::DoWhile { .. } => render_tree_do_while(xot, parent, tree, source),
        SyntaxTree::FieldWrap { .. } => render_tree_field_wrap(xot, parent, tree, source),
        SyntaxTree::SimpleStatement { .. } => render_tree_simple_statement(xot, parent, tree, source),
        SyntaxTree::Try { .. } => render_tree_try(xot, parent, tree, source),
        SyntaxTree::ExceptHandler { .. } => render_tree_except_handler(xot, parent, tree, source),
        SyntaxTree::TypeAlias { .. } => render_tree_type_alias(xot, parent, tree, source),
        SyntaxTree::KeywordArgument { .. } => render_tree_keyword_argument(xot, parent, tree, source),
        SyntaxTree::ListSplat { .. } => render_tree_list_splat(xot, parent, tree, source),
        SyntaxTree::DictSplat { .. } => render_tree_dict_splat(xot, parent, tree, source),
        SyntaxTree::Ternary { .. } => render_tree_ternary(xot, parent, tree, source),
        SyntaxTree::ObjectCreation { .. } => render_tree_object_creation(xot, parent, tree, source),
        SyntaxTree::Lambda { .. } => render_tree_lambda(xot, parent, tree, source),
        SyntaxTree::Break { span, range } => {
            let node = element(xot, "break", *span);
            xot.append(parent, node)?;
            emit_gap(xot, node, source, range.start, range.end)?;
            Ok(node)
        }
        SyntaxTree::Continue { span, range } => {
            let node = element(xot, "continue", *span);
            xot.append(parent, node)?;
            emit_gap(xot, node, source, range.start, range.end)?;
            Ok(node)
        }
        SyntaxTree::Function { .. } => render_tree_function(xot, parent, tree, source),
        SyntaxTree::Class { .. } => render_tree_class(xot, parent, tree, source),
        SyntaxTree::Body { .. } => render_tree_body(xot, parent, tree, source),
        SyntaxTree::Parameter { .. } => render_tree_parameter(xot, parent, tree, source),
        SyntaxTree::Skip { range: _, span: _ } => {
            // Source-range consumer that emits nothing. Parent's
            // `render_with_gaps` still advances its cursor past
            // `range.end`, so the gap before the next sibling
            // skips the consumed bytes (used by T-SQL to swallow
            // anonymous keyword children without leaking them as
            // gap text).
            Ok(parent)
        }
        SyntaxTree::PositionalSeparator { range, span } => {
            leaf(xot, parent, "positional", source, *range, *span)
        }
        SyntaxTree::KeywordSeparator { range, span } => {
            leaf(xot, parent, "keyword", source, *range, *span)
        }
        SyntaxTree::Decorator { .. } => render_tree_decorator(xot, parent, tree, source),
        SyntaxTree::Returns { .. } => render_tree_returns(xot, parent, tree, source),
        SyntaxTree::Generic { .. } => render_tree_generic(xot, parent, tree, source),
        SyntaxTree::TypeParameter { .. } => render_tree_type_parameter(xot, parent, tree, source),
        SyntaxTree::Return { .. } => render_tree_return(xot, parent, tree, source),
        SyntaxTree::Comment { .. } => render_tree_comment(xot, parent, tree, source),
        SyntaxTree::Assign { .. } => render_tree_assign(xot, parent, tree, source),
        SyntaxTree::Import { .. } => render_tree_import(xot, parent, tree, source),
        SyntaxTree::From { .. } => render_tree_from(xot, parent, tree, source),
        SyntaxTree::FromImport { .. } => render_tree_from_import(xot, parent, tree, source),
        SyntaxTree::Path { .. } => render_tree_path(xot, parent, tree, source),
        SyntaxTree::Aliased { .. } => render_tree_aliased(xot, parent, tree, source),
        SyntaxTree::Call { .. } => render_tree_call(xot, parent, tree, source),
        SyntaxTree::Binary { .. } => render_tree_binary(xot, parent, tree, source),
        SyntaxTree::Unary { .. } => render_tree_unary(xot, parent, tree, source),

        // ----- Atoms — emit source[range] as the leaf text. ---------
        SyntaxTree::Name { range, span, .. } => leaf(xot, parent, "name", source, *range, *span),
        SyntaxTree::Atom { element_name, range, span, .. } => leaf(xot, parent, element_name, source, *range, *span),
        SyntaxTree::Int { range, span, .. } => leaf(xot, parent, "int", source, *range, *span),
        SyntaxTree::Float { range, span, .. } => leaf(xot, parent, "float", source, *range, *span),
        SyntaxTree::String { range, span, .. } => leaf(xot, parent, "string", source, *range, *span),
        SyntaxTree::True { range, span, .. } => leaf(xot, parent, "true", source, *range, *span),
        SyntaxTree::False { range, span, .. } => leaf(xot, parent, "false", source, *range, *span),
        SyntaxTree::None { range, span, .. } => leaf(xot, parent, "none", source, *range, *span),
        SyntaxTree::Null { range, span, .. } => leaf(xot, parent, "null", source, *range, *span),
        SyntaxTree::Enum { .. } => render_tree_enum(xot, parent, tree, source),
        SyntaxTree::EnumMember { .. } => render_tree_enum_member(xot, parent, tree, source),
        SyntaxTree::Property { .. } => render_tree_property(xot, parent, tree, source),
        SyntaxTree::Accessor { .. } => render_tree_accessor(xot, parent, tree, source),
        SyntaxTree::Constructor { .. } => render_tree_constructor(xot, parent, tree, source),
        SyntaxTree::Using { .. } => render_tree_using(xot, parent, tree, source),
        SyntaxTree::Namespace { .. } => render_tree_namespace(xot, parent, tree, source),
        SyntaxTree::Variable { .. } => render_tree_variable(xot, parent, tree, source),
        SyntaxTree::Is { .. } => render_tree_is(xot, parent, tree, source),
        SyntaxTree::Cast { .. } => render_tree_cast(xot, parent, tree, source),

        SyntaxTree::Inline { .. } => render_tree_inline(xot, parent, tree, source),
        SyntaxTree::Unknown { .. } => render_tree_unknown(xot, parent, tree, source),
        SyntaxTree::Raw { .. } => render_tree_raw(xot, parent, tree, source),
    }
}

// ---------------------------------------------------------------------------
// Per-arm renderers (out-of-line)
// ---------------------------------------------------------------------------
//
// Each `render_tree_<variant>` mirrors one arm of `render_to_xot`'s match.
// They're `#[inline(never)]` so the compiler doesn't fold them back into
// the dispatcher's frame — that's the whole point: the dispatcher's
// match becomes a thin jump table rather than a wide-frame function
// reserving stack space worst-case across every arm. Recursive
// tree walks at depth 20-30+ now run on default 2 MiB thread stacks
// instead of overflowing.

#[inline(never)]
fn render_tree_assign(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Assign { targets, type_annotation, op_text, op_range, op_markers, values, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "assign", *span);
    xot.append(parent, node)?;

    // <left>...</left> — each target is pre-wrapped in `<expression>`
    // at lowering, so the renderer just emits the slot wrapper and
    // delegates rendering. Lifting the slot wrapper itself into the
    // tree is deferred to a later slice.
    let left_node = element(xot, "left", *span);
    xot.append(node, left_node)?;
    let left_range = if let (Some(first), Some(last)) = (targets.first(), targets.last()) {
        ByteRange::new(first.range().start, last.range().end)
    } else {
        ByteRange::empty_at(range.start)
    };
    let mut cursor = left_range.start;
    for t in targets {
        let tr = t.range();
        emit_gap(xot, left_node, source, cursor, tr.start)?;
        render_to_xot(xot, left_node, t, source)?;
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
    // field on SyntaxTree::Assign is now unused.
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
            // Lowering wraps each value in `SyntaxTree::Expression`
            // (P2). Renderer just renders what's there.
            render_to_xot(xot, right_node, v, source)?;
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

#[inline(never)]
fn render_tree_class(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Class { kind, modifiers, decorators, name, generics, bases, where_clauses, body, range, span } = tree
        else { unreachable!() };
    let node = element(xot, kind, *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    // Source-order children: decorators, name, generic items
    // (flat siblings, not wrapped in outer `<generic>`),
    // bases (wrapped in `<extends><type>...`), where clauses,
    // body.
    #[derive(Clone, Copy)]
    enum CSlot<'a> {
        Decor(&'a SyntaxTree),
        Name(&'a SyntaxTree),
        Generics(&'a SyntaxTree),
        Base(&'a SyntaxTree),
        Where(&'a SyntaxTree),
        Body(&'a SyntaxTree),
    }
    let mut order: Vec<CSlot> = Vec::new();
    for d in decorators { order.push(CSlot::Decor(d)); }
    order.push(CSlot::Name(name));
    for it in generics { order.push(CSlot::Generics(it)); }
    for b in bases { order.push(CSlot::Base(b)); }
    for w in where_clauses { order.push(CSlot::Where(w)); }
    order.push(CSlot::Body(body));
    order.sort_by_key(|s| match s {
        CSlot::Decor(i) | CSlot::Name(i) | CSlot::Generics(i)
        | CSlot::Base(i) | CSlot::Where(i) | CSlot::Body(i) => i.range().start,
    });
    let mut cursor = range.start;
    for slot in &order {
        let inner: &SyntaxTree = match slot {
            CSlot::Decor(i) | CSlot::Name(i) | CSlot::Generics(i)
            | CSlot::Base(i) | CSlot::Where(i) | CSlot::Body(i) => i,
        };
        let cr = inner.range();
        emit_gap(xot, node, source, cursor, cr.start)?;
        if matches!(slot, CSlot::Where(_)) {
            // C# where-clause source bytes flow through as gap text
            // under `<class>` — no `<where>` element. The constraint
            // structure is already merged into `<generic>` items by
            // `fold_csharp_where_clauses_into_generics` during
            // lowering, so the structural query path doesn't need
            // a separate `<where>` element.
            emit_gap(xot, node, source, cr.start, cr.end)?;
        } else if matches!(slot, CSlot::Base(_)) {
            // Lowering wraps each base via `wrap_extends()` (P1) —
            // produces either `<extends><type>...</type></extends>`
            // or `<implements>...</implements>` as a tree node.
            // Renderer renders what's there.
            render_to_xot(xot, node, inner, source)?;
        } else {
            render_to_xot(xot, node, inner, source)?;
        }
        cursor = cr.end;
    }
    emit_gap(xot, node, source, cursor, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_except_handler(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::ExceptHandler { kind, type_target, binding, filter, body, range, span } = tree
        else { unreachable!() };
    let node = element(xot, kind, *span);
    xot.append(parent, node)?;
    // Slots pre-shaped at lowering — type_target via `wrap_type()`,
    // binding via `wrap_slot("as")`, filter via `wrap_slot("filter")`.
    // Renderer walks them in source order.
    let mut order: Vec<&SyntaxTree> = Vec::new();
    if let Some(t) = type_target { order.push(t.as_ref()); }
    if let Some(b) = binding { order.push(b.as_ref()); }
    if let Some(f) = filter { order.push(f.as_ref()); }
    order.push(body.as_ref());
    order.sort_by_key(|i| i.range().start);
    let mut cursor = range.start;
    for inner in &order {
        let cr = inner.range();
        emit_gap(xot, node, source, cursor, cr.start)?;
        render_to_xot(xot, node, inner, source)?;
        cursor = cr.end;
    }
    emit_gap(xot, node, source, cursor, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_for(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::For { is_async, targets, iterables, body, else_body, range, span } = tree
        else { unreachable!() };
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
    // Lowering pre-wraps each target / iterable in `<expression>`;
    // the renderer just emits the slot wrappers + delegates rendering.
    // Lifting `<left>` / `<right>` slot wrappers themselves into the
    // tree is deferred to a later slice (Assign and For share the
    // pattern).
    emit_gap(xot, node, source, range.start, left_range.start)?;
    let left_slot = element(xot, "left", *span);
    xot.append(node, left_slot)?;
    let mut cursor = left_range.start;
    for t in targets {
        let tr = t.range();
        emit_gap(xot, left_slot, source, cursor, tr.start)?;
        render_to_xot(xot, left_slot, t, source)?;
        cursor = tr.end;
    }
    emit_gap(xot, left_slot, source, cursor, left_range.end)?;
    emit_gap(xot, node, source, left_range.end, right_range.start)?;
    let right_slot = element(xot, "right", *span);
    xot.append(node, right_slot)?;
    let mut cursor = right_range.start;
    for i in iterables {
        let ir2 = i.range();
        emit_gap(xot, right_slot, source, cursor, ir2.start)?;
        render_to_xot(xot, right_slot, i, source)?;
        cursor = ir2.end;
    }
    emit_gap(xot, right_slot, source, cursor, right_range.end)?;
    // Body + else (else_body pre-wrapped in `<else>` at lowering via
    // `wrap_clause("else")`).
    let br = body.range();
    emit_gap(xot, node, source, right_range.end, br.start)?;
    render_to_xot(xot, node, body, source)?;
    if let Some(e) = else_body {
        let er = e.range();
        emit_gap(xot, node, source, br.end, er.start)?;
        render_to_xot(xot, node, e, source)?;
        emit_gap(xot, node, source, er.end, range.end)?;
    } else {
        emit_gap(xot, node, source, br.end, range.end)?;
    }
    Ok(node)
}

#[inline(never)]
fn render_tree_if(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::If { condition, body, else_branch, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "if", *span);
    xot.append(parent, node)?;
    // Lowering pre-wraps condition via `wrap_slot("condition")`.
    let cr = condition.range();
    emit_gap(xot, node, source, range.start, cr.start)?;
    render_to_xot(xot, node, condition, source)?;
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
            SyntaxTree::ElseIf { condition: ec, body: eb, else_branch: deeper, span: es, range: er } => {
                let elseif = element(xot, "else_if", *es);
                xot.append(node, elseif)?;
                // Lowering pre-wraps condition via `wrap_slot("condition")`.
                let ecr = ec.range();
                emit_gap(xot, elseif, source, er.start, ecr.start)?;
                render_to_xot(xot, elseif, ec, source)?;
                let ebr = eb.range();
                emit_gap(xot, elseif, source, ecr.end, ebr.start)?;
                render_to_xot(xot, elseif, eb, source)?;
                emit_gap(xot, elseif, source, ebr.end, er.end)?;
                cursor = er.end;
                next = deeper.as_ref().map(|b| b.as_ref());
            }
            SyntaxTree::Else { body: eb, span: es, range: er } => {
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

#[inline(never)]
fn render_tree_foreach(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Foreach { type_ann, target, iterable, body, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "foreach", *span);
    xot.append(parent, node)?;
    let in_marker = element(xot, "in", *span);
    xot.append(node, in_marker)?;
    // All slots pre-shaped in lowering: type_ann via `wrap_type()`,
    // target / iterable via `wrap_slot("left" / "right")`. Renderer
    // emits them in source order with gap text between.
    let mut order: Vec<&SyntaxTree> = Vec::new();
    if let Some(t) = type_ann { order.push(t.as_ref()); }
    order.push(target.as_ref());
    order.push(iterable.as_ref());
    order.push(body.as_ref());
    order.sort_by_key(|i| i.range().start);
    let mut cursor = range.start;
    for inner in &order {
        let cr = inner.range();
        emit_gap(xot, node, source, cursor, cr.start)?;
        render_to_xot(xot, node, inner, source)?;
        cursor = cr.end;
    }
    emit_gap(xot, node, source, cursor, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_field_wrap(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::FieldWrap { wrapper, inner, range, span } = tree else { unreachable!() };
    // When the wrapper is `name`, collapse the inner to a flat text
    // leaf — `<name>` is text-only by contract. Works for bare
    // identifiers (SyntaxTree::Name), dotted paths (SyntaxTree::Path), generic types
    // (SyntaxTree::GenericType), and any other inner: we emit the full source
    // slice as the text content. Mirrors the imperative pipeline's
    // `name_wrapper`.
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
    // Tree shape is final (P1): lowering decides whether to construct
    // FieldWrap or pass the bare wrapper-emitting inner directly. The
    // renderer marshals whatever's in the tree.
    let node = element(xot, wrapper, *span);
    xot.append(parent, node)?;
    let ir_range = inner.range();
    emit_gap(xot, node, source, range.start, ir_range.start)?;
    // Lowering wraps value/condition inner in `SyntaxTree::Expression`
    // (P2). Renderer renders the tree as-is — no conditional wrap.
    render_to_xot(xot, node, inner, source)?;
    emit_gap(xot, node, source, ir_range.end, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_ternary(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Ternary { condition, if_true, if_false, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "ternary", *span);
    xot.append(parent, node)?;
    // Lowering pre-wraps each slot via `wrap_slot("condition" / "then"
    // / "else")` — the renderer walks in source order and renders
    // what's there.
    let mut order: Vec<&SyntaxTree> = vec![condition.as_ref(), if_true.as_ref(), if_false.as_ref()];
    order.sort_by_key(|i| i.range().start);
    let mut cursor = range.start;
    for inner in &order {
        let cr = inner.range();
        emit_gap(xot, node, source, cursor, cr.start)?;
        render_to_xot(xot, node, inner, source)?;
        cursor = cr.end;
    }
    emit_gap(xot, node, source, cursor, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_try(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Try { try_body, handlers, else_body, finally_body, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "try", *span);
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = vec![try_body.as_ref()];
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
            render_to_xot(xot, node, child, source)?;
        }
        cursor = cr.end;
    }
    emit_gap(xot, node, source, cursor, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_lambda(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Lambda { modifiers, parameters, body, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "lambda", *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    let mut order: Vec<&SyntaxTree> = Vec::new();
    for p in parameters { order.push(p); }
    order.push(body.inner());
    order.sort_by_key(|c| c.range().start);

    let is_block_body = matches!(body, LambdaBody::Block(_));
    let mut cursor = range.start;
    for child in &order {
        let cr = child.range();
        emit_gap(xot, node, source, cursor, cr.start)?;
        if std::ptr::eq(*child, body.inner()) {
            if is_block_body {
                render_to_xot(xot, node, child, source)?;
            } else {
                let val = element(xot, "value", child.span());
                xot.append(node, val)?;
                let expr = element(xot, "expression", child.span());
                xot.append(val, expr)?;
                render_to_xot(xot, expr, child, source)?;
            }
        } else {
            render_to_xot(xot, node, child, source)?;
        }
        cursor = cr.end;
    }
    emit_gap(xot, node, source, cursor, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_function(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Function { element_name, modifiers, decorators, name, generics, parameters, returns, throws, body, range, span } = tree
        else { unreachable!() };
    let node = element(xot, element_name, *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or_else(|| Span::point(span.line, span.column)));
        xot.append(node, m)?;
    }
    let mut order: Vec<&SyntaxTree> = Vec::new();
    for d in decorators { order.push(d); }
    order.push(name.as_ref());
    for it in generics { order.push(it); }
    for p in parameters { order.push(p); }
    if let Some(r) = returns { order.push(r.as_ref()); }
    for t in throws { order.push(t); }
    if let Some(b) = body { order.push(b.as_ref()); }
    order.sort_by_key(|c| c.range().start);
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_parameter(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Parameter { kind, extra_markers, modifiers, name, type_ann, default, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "parameter", *span);
    xot.append(parent, node)?;
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
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    for marker in extra_markers.iter() {
        let m = element(xot, marker.name, marker.span);
        xot.append(node, m)?;
    }
    #[derive(Clone, Copy)]
    enum Slot<'a> { Name(&'a SyntaxTree), Type(&'a SyntaxTree), Default(&'a SyntaxTree) }
    let mut order: Vec<Slot> = vec![Slot::Name(name)];
    if let Some(t) = type_ann { order.push(Slot::Type(t)); }
    if let Some(d) = default { order.push(Slot::Default(d)); }
    order.sort_by_key(|s| match s {
        Slot::Name(i) | Slot::Type(i) | Slot::Default(i) => i.range().start,
    });

    let mut cursor = range.start;
    for slot in &order {
        let inner: &SyntaxTree = match slot {
            Slot::Name(i) | Slot::Type(i) | Slot::Default(i) => i,
        };
        let ir_range = inner.range();
        emit_gap(xot, node, source, cursor, ir_range.start)?;
        match slot {
            Slot::Name(_) => {
                render_to_xot(xot, node, inner, source)?;
            }
            Slot::Type(_) => {
                // Lowering wraps via `wrap_type()` (P1). Renderer
                // renders what's there.
                render_to_xot(xot, node, inner, source)?;
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

#[inline(never)]
fn render_tree_variable(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Variable { element_name, modifiers, decorators, type_ann, name, value, range, span } = tree
        else { unreachable!() };
    let node = element(xot, element_name, *span);
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = Vec::new();
    for d in decorators { order.push(d); }
    if let Some(t) = type_ann { order.push(t.as_ref()); }
    order.push(name.as_ref());
    if let Some(v) = value { order.push(v.inner.as_ref()); }
    order.sort_by_key(|c| c.range().start);
    let mut cursor = range.start;
    for c in &order {
        let cr = c.range();
        emit_gap(xot, node, source, cursor, cr.start)?;
        if let Some(t) = type_ann {
            if std::ptr::eq(*c, t.as_ref()) {
                // Lowering produces typed-wrapped trees via
                // `wrap_type()` (P1). Renderer just renders what's
                // there.
                render_to_xot(xot, node, *c, source)?;
                cursor = cr.end;
                continue;
            }
        }
        if let Some(v) = value {
            if std::ptr::eq(*c, v.inner.as_ref()) {
                // Lowering wraps initializers in `<value>` via the
                // appropriate construction (P1). Renderer renders
                // what's there.
                let val = element(xot, "value", c.span());
                xot.append(node, val)?;
                render_to_xot(xot, val, *c, source)?;
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

#[inline(never)]
fn render_tree_comparison(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Comparison { left, op_text, op_marker, op_range, right, range, span } = tree
        else { unreachable!() };
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

#[inline(never)]
fn render_tree_while(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::While { condition, body, else_body, range, span } = tree else { unreachable!() };
    let node = element(xot, "while", *span);
    xot.append(parent, node)?;
    // Lowering pre-wraps condition via `wrap_slot("condition")`.
    let cr = condition.range();
    emit_gap(xot, node, source, range.start, cr.start)?;
    render_to_xot(xot, node, condition, source)?;
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

#[inline(never)]
fn render_tree_cfor(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::CFor { initializer, condition, updates, body, range, span } = tree else { unreachable!() };
    let node = element(xot, "for", *span);
    xot.append(parent, node)?;
    let mut header: Vec<(usize, &SyntaxTree, u8)> = Vec::new();
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
    let br = body.range();
    emit_gap(xot, node, source, cursor, br.start)?;
    render_to_xot(xot, node, body, source)?;
    emit_gap(xot, node, source, br.end, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_do_while(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::DoWhile { body, condition, range, span } = tree else { unreachable!() };
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

#[inline(never)]
fn render_tree_body(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Body { children, pass_only, block_wrap, range, span } = tree else { unreachable!() };
    let node = element(xot, "body", *span);
    xot.append(parent, node)?;
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

#[inline(never)]
fn render_tree_binary(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Binary { element_name, op_text, op_marker, op_range, left, right, range, span } = tree
        else { unreachable!() };
    let node = element(xot, element_name, *span);
    xot.append(parent, node)?;
    // Lowering pre-wraps left/right as `<left><expression>...</expression></left>`
    // (and same for right) — see `SyntaxTree::wrap_slot`. The renderer
    // walks what's there + synthesises the `<op>` element (lifting op
    // synthesis into the tree is deferred to a later slice).
    let left_range = left.range();
    emit_gap(xot, node, source, range.start, left_range.start)?;
    render_to_xot(xot, node, left, source)?;
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
    let right_range = right.range();
    emit_gap(xot, node, source, op_range.end, right_range.start)?;
    render_to_xot(xot, node, right, source)?;
    emit_gap(xot, node, source, right_range.end, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_unary(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Unary { op_text, op_marker, op_range, operand, extra_markers, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "unary", *span);
    xot.append(parent, node)?;
    for marker in extra_markers.iter() {
        let m = element(xot, marker.name, marker.span);
        xot.append(node, m)?;
    }
    let operand_range = operand.range();
    let is_postfix = op_range.start >= operand_range.end;
    if is_postfix {
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

#[inline(never)]
fn render_tree_return(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Return { value, range, span } = tree else { unreachable!() };
    let node = element(xot, "return", *span);
    xot.append(parent, node)?;
    // Lowering pre-wraps the value in `<expression>` host(s) — for an
    // Inline value, each child is already wrapped. The renderer just
    // walks what's there, emitting source-anchored gap text.
    if let Some(v) = value {
        let children = std::slice::from_ref(v.as_ref());
        render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
            render_to_xot(xot, parent, child, source).map(|_| ())
        })?;
    } else {
        emit_gap(xot, node, source, range.start, range.end)?;
    }
    Ok(node)
}

#[inline(never)]
fn render_tree_type_alias(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::TypeAlias { name, type_params, value, range, span } = tree else { unreachable!() };
    let node = element(xot, "alias", *span);
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = vec![name.as_ref()];
    if let Some(p) = type_params { order.push(p.as_ref()); }
    order.push(value.as_ref());
    order.sort_by_key(|c| c.range().start);
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

#[inline(never)]
fn render_tree_access(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Access { receiver, segments, range, span } = tree else { unreachable!() };
    let object = element(xot, "object", *span);
    xot.append(parent, object)?;
    let access = element(xot, "access", Span::point(span.line, span.column));
    xot.append(object, access)?;
    let receiver_range = receiver.range();
    let receiver_span = receiver.span();
    emit_gap(xot, object, source, range.start, receiver_range.start)?;
    if let Some(kw) = receiver.keyword_element() {
        // Empty marker element (MarkerOnly shape) + sibling text node
        // so XPath text recovery on <object> still yields the keyword
        // bytes from source.
        let m = element(xot, kw, receiver_span);
        xot.append(object, m)?;
        let t = xot.new_text(receiver_range.slice(source));
        xot.append(object, t)?;
    } else if let AccessReceiver::Instance(t) = receiver {
        render_to_xot(xot, object, t, source)?;
    }
    let mut cursor = receiver_range.end;
    render_segments_chain(xot, object, segments, &mut cursor, source)?;
    emit_gap(xot, object, source, cursor, range.end)?;
    Ok(object)
}

#[inline(never)]
fn render_tree_generic_type(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::GenericType { name, params, range, span } = tree else { unreachable!() };
    let node = element(xot, "type", *span);
    xot.append(parent, node)?;
    let g = element(xot, "generic", *span);
    xot.append(node, g)?;
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

#[inline(never)]
fn render_tree_else_if(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::ElseIf { condition, body, else_branch, range, span } = tree else { unreachable!() };
    let node = element(xot, "else_if", *span);
    xot.append(parent, node)?;
    // Lowering pre-wraps condition via `wrap_slot("condition")`.
    let cr = condition.range();
    emit_gap(xot, node, source, range.start, cr.start)?;
    render_to_xot(xot, node, condition, source)?;
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

#[inline(never)]
fn render_tree_returns(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Returns { type_ann, range, span } = tree else { unreachable!() };
    let node = element(xot, "returns", *span);
    xot.append(parent, node)?;
    let tr = type_ann.range();
    emit_gap(xot, node, source, range.start, tr.start)?;
    // Lowering produces typed-wrapped trees via `wrap_type()` (P1).
    // Renderer renders what's there.
    render_to_xot(xot, node, type_ann, source)?;
    emit_gap(xot, node, source, tr.end, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_is(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Is { value, type_target, range, span } = tree else { unreachable!() };
    let node = element(xot, "is", *span);
    xot.append(parent, node)?;
    let vr = value.range();
    emit_gap(xot, node, source, range.start, vr.start)?;
    let left_slot = element(xot, "left", value.span());
    xot.append(node, left_slot)?;
    let left_expr = element(xot, "expression", value.span());
    xot.append(left_slot, left_expr)?;
    render_to_xot(xot, left_expr, value, source)?;
    let tr = type_target.range();
    emit_gap(xot, node, source, vr.end, tr.start)?;
    let right_slot = element(xot, "right", type_target.span());
    xot.append(node, right_slot)?;
    let right_expr = element(xot, "expression", type_target.span());
    xot.append(right_slot, right_expr)?;
    let type_el = element(xot, "type", type_target.span());
    xot.append(right_expr, type_el)?;
    render_to_xot(xot, type_el, type_target, source)?;
    emit_gap(xot, node, source, tr.end, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_cast(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Cast { type_ann, value, range, span } = tree else { unreachable!() };
    let node = element(xot, "cast", *span);
    xot.append(parent, node)?;
    let tr = type_ann.range();
    emit_gap(xot, node, source, range.start, tr.start)?;
    let type_el = element(xot, "type", type_ann.span());
    xot.append(node, type_el)?;
    render_to_xot(xot, type_el, type_ann, source)?;
    let vr = value.range();
    emit_gap(xot, node, source, tr.end, vr.start)?;
    let value_el = element(xot, "value", value.span());
    xot.append(node, value_el)?;
    let expr_el = element(xot, "expression", value.span());
    xot.append(value_el, expr_el)?;
    render_to_xot(xot, expr_el, value, source)?;
    emit_gap(xot, node, source, vr.end, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_inline(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Inline { children, list_name, range, span: _ } = tree else { unreachable!() };
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

#[inline(never)]
fn render_tree_comment(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Comment { leading, trailing, range, span } = tree else { unreachable!() };
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

#[inline(never)]
fn render_tree_import(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Import { has_alias, children, range, span } = tree else { unreachable!() };
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

#[inline(never)]
fn render_tree_from(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::From { relative, path, imports, range, span } = tree else { unreachable!() };
    let node = element(xot, "from", *span);
    xot.append(parent, node)?;
    if *relative {
        let m = element(xot, "relative", Span::point(span.line, span.column));
        xot.append(node, m)?;
    }
    let mut order: Vec<&SyntaxTree> = Vec::new();
    if let Some(p) = path { order.push(p.as_ref()); }
    for i in imports { order.push(i); }
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_from_import(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::FromImport { has_alias, name, alias, range, span } = tree else { unreachable!() };
    let node = element(xot, "import", *span);
    xot.append(parent, node)?;
    if *has_alias {
        let m = element(xot, "alias", Span::point(span.line, span.column));
        xot.append(node, m)?;
    }
    let mut order: Vec<&SyntaxTree> = vec![name.as_ref()];
    if let Some(a) = alias { order.push(a.as_ref()); }
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_path(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Path { segments, range, span } = tree else { unreachable!() };
    let node = element(xot, "path", *span);
    xot.append(parent, node)?;
    render_with_gaps(xot, node, source, *range, segments, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_aliased(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Aliased { inner, range, span } = tree else { unreachable!() };
    let node = element(xot, "aliased", *span);
    xot.append(parent, node)?;
    render_with_gaps(xot, node, source, *range, std::slice::from_ref(inner.as_ref()),
        |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ()),
    )?;
    Ok(node)
}

#[inline(never)]
fn render_tree_call(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Call { callee, arguments, range, span } = tree else { unreachable!() };
    let node = element(xot, "call", *span);
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = Vec::with_capacity(1 + arguments.len());
    order.push(callee.as_ref());
    for arg in arguments { order.push(arg); }
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_module(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Module { element_name, children, range, span } = tree else { unreachable!() };
    let node = element(xot, element_name, *span);
    xot.append(parent, node)?;
    render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_expression(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Expression { inner, marker, range, span } = tree else { unreachable!() };
    let node = element(xot, "expression", *span);
    xot.append(parent, node)?;
    if let Some(m) = marker {
        let marker_node = element(xot, m, *span);
        xot.append(node, marker_node)?;
    }
    render_with_gaps(xot, node, source, *range, std::slice::from_ref(inner.as_ref()),
        |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ()),
    )?;
    Ok(node)
}

#[inline(never)]
fn render_tree_tuple(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Tuple { children, range, span } = tree else { unreachable!() };
    let node = element(xot, "tuple", *span);
    xot.append(parent, node)?;
    render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_list(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::List { children, range, span } = tree else { unreachable!() };
    let node = element(xot, "list", *span);
    xot.append(parent, node)?;
    let m = element(xot, "literal", *span);
    xot.append(node, m)?;
    render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_set(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Set { children, range, span } = tree else { unreachable!() };
    let node = element(xot, "set", *span);
    xot.append(parent, node)?;
    let m = element(xot, "literal", *span);
    xot.append(node, m)?;
    render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_dictionary(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Dictionary { pairs, range, span } = tree else { unreachable!() };
    let node = element(xot, "dict", *span);
    xot.append(parent, node)?;
    let m = element(xot, "literal", *span);
    xot.append(node, m)?;
    render_with_gaps(xot, node, source, *range, pairs, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_pair(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Pair { key, value, range, span } = tree else { unreachable!() };
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

#[inline(never)]
fn render_tree_else(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Else { body, range, span } = tree else { unreachable!() };
    let node = element(xot, "else", *span);
    xot.append(parent, node)?;
    let br = body.range();
    emit_gap(xot, node, source, range.start, br.start)?;
    render_to_xot(xot, node, body, source)?;
    emit_gap(xot, node, source, br.end, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_simple_statement(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::SimpleStatement { element_name, modifiers, extra_markers, children, range, span } = tree
        else { unreachable!() };
    let node = element(xot, element_name, *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    for marker in extra_markers.iter() {
        let m = element(xot, marker.name, marker.span);
        xot.append(node, m)?;
    }
    render_with_gaps(xot, node, source, *range, children, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_keyword_argument(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::KeywordArgument { name, value, range, span } = tree else { unreachable!() };
    let node = element(xot, "keyword", *span);
    xot.append(parent, node)?;
    let nr = name.range();
    let vr = value.range();
    emit_gap(xot, node, source, range.start, nr.start)?;
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

#[inline(never)]
fn render_tree_list_splat(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::ListSplat { inner, range, span } = tree else { unreachable!() };
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

#[inline(never)]
fn render_tree_dict_splat(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::DictSplat { inner, range, span } = tree else { unreachable!() };
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

#[inline(never)]
fn render_tree_object_creation(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::ObjectCreation { type_target, arguments, initializer, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "new", *span);
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = Vec::new();
    if let Some(t) = type_target { order.push(t.as_ref()); }
    for a in arguments { order.push(a); }
    if let Some(i) = initializer { order.push(i.as_ref()); }
    order.sort_by_key(|c| c.range().start);
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_decorator(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Decorator { inner, range, span } = tree else { unreachable!() };
    let node = element(xot, "decorator", *span);
    xot.append(parent, node)?;
    render_with_gaps(xot, node, source, *range, std::slice::from_ref(inner.as_ref()),
        |xot, parent, child| render_to_xot(xot, parent, child, source).map(|_| ())
    )?;
    Ok(node)
}

#[inline(never)]
fn render_tree_generic(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Generic { items, range, span } = tree else { unreachable!() };
    let node = element(xot, "generic", *span);
    xot.append(parent, node)?;
    render_with_gaps(xot, node, source, *range, items, |xot, parent, child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_type_parameter(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::TypeParameter { name, constraint, range, span } = tree else { unreachable!() };
    let node = element(xot, "type", *span);
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = vec![name.as_ref()];
    if let Some(c) = constraint { order.push(c.as_ref()); }
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_enum(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Enum { modifiers, decorators, name, underlying_type, members, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "enum", *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    let mut order: Vec<&SyntaxTree> = Vec::new();
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

#[inline(never)]
fn render_tree_enum_member(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::EnumMember { decorators, name, value, range, span } = tree else { unreachable!() };
    let node = element(xot, "constant", *span);
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = Vec::new();
    for d in decorators { order.push(d); }
    order.push(name.as_ref());
    if let Some(v) = value { order.push(v.as_ref()); }
    order.sort_by_key(|c| c.range().start);
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_property(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Property { modifiers, decorators, type_ann, name, accessors, value, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "property", *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    // Slot-tagged ordering mirrors `render_tree_parameter` /
    // `render_tree_variable`: `type_ann` wraps in `<type>`, `value`
    // wraps in `<value>`, decorators / accessors / name render as
    // themselves. Without these wrappers the type leaked as a bare
    // `<name>` sibling and the initializer expression was rendered
    // unwrapped — both are Principle #5 / #15 violations.
    #[derive(Clone, Copy)]
    enum Slot<'a> { Decorator(&'a SyntaxTree), Type(&'a SyntaxTree), Name(&'a SyntaxTree), Accessor(&'a SyntaxTree), Value(&'a SyntaxTree) }
    let mut order: Vec<Slot> = Vec::new();
    for d in decorators { order.push(Slot::Decorator(d)); }
    if let Some(t) = type_ann { order.push(Slot::Type(t.as_ref())); }
    order.push(Slot::Name(name.as_ref()));
    for a in accessors { order.push(Slot::Accessor(a)); }
    if let Some(v) = value { order.push(Slot::Value(v.as_ref())); }
    order.sort_by_key(|s| match s {
        Slot::Decorator(i) | Slot::Type(i) | Slot::Name(i)
        | Slot::Accessor(i) | Slot::Value(i) => i.range().start,
    });

    let mut cursor = range.start;
    for slot in &order {
        let inner: &SyntaxTree = match slot {
            Slot::Decorator(i) | Slot::Type(i) | Slot::Name(i)
            | Slot::Accessor(i) | Slot::Value(i) => i,
        };
        let ir_range = inner.range();
        emit_gap(xot, node, source, cursor, ir_range.start)?;
        match slot {
            Slot::Decorator(_) | Slot::Name(_) | Slot::Accessor(_) => {
                render_to_xot(xot, node, inner, source)?;
            }
            Slot::Type(_) => {
                // Lowering wraps via `wrap_type()` (P1). Renderer
                // renders what's there.
                render_to_xot(xot, node, inner, source)?;
            }
            Slot::Value(_) => {
                let val = element(xot, "value", inner.span());
                xot.append(node, val)?;
                render_to_xot(xot, val, inner, source)?;
            }
        }
        cursor = ir_range.end;
    }
    emit_gap(xot, node, source, cursor, range.end)?;
    Ok(node)
}

#[inline(never)]
fn render_tree_accessor(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Accessor { modifiers, kind, body, range, span } = tree else { unreachable!() };
    let node = element(xot, kind, *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
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

#[inline(never)]
fn render_tree_constructor(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Constructor { modifiers, decorators, name, parameters, body, range, span } = tree
        else { unreachable!() };
    let node = element(xot, "constructor", *span);
    xot.append(parent, node)?;
    for (marker, marker_span) in modifiers.markers_with_spans() {
        let m = element(xot, marker, marker_span.unwrap_or(*span));
        xot.append(node, m)?;
    }
    let mut order: Vec<&SyntaxTree> = Vec::new();
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

#[inline(never)]
fn render_tree_using(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Using { is_static, alias, path, range, span } = tree else { unreachable!() };
    let _ = is_static;
    let node = element(xot, "import", *span);
    xot.append(parent, node)?;
    let mut order: Vec<&SyntaxTree> = Vec::new();
    order.push(path.as_ref());
    if let Some(a) = alias { order.push(a.as_ref()); }
    order.sort_by_key(|c| c.range().start);
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_namespace(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Namespace { name, children, file_scoped, range, span } = tree else { unreachable!() };
    let node = element(xot, "namespace", *span);
    xot.append(parent, node)?;
    if *file_scoped {
        let m = element(xot, "file", *span);
        xot.append(node, m)?;
    }
    let mut order: Vec<&SyntaxTree> = vec![name.as_ref()];
    for c in children { order.push(c); }
    order.sort_by_key(|c| c.range().start);
    render_with_gaps(xot, node, source, *range, &order, |xot, parent, &child| {
        render_to_xot(xot, parent, child, source).map(|_| ())
    })?;
    Ok(node)
}

#[inline(never)]
fn render_tree_unknown(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Unknown { kind, range, span } = tree else { unreachable!() };
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

/// Render a `SyntaxTree::Raw` passthrough node — `<{kind}>{children…}
/// </{kind}>`. Leaves (no children) include the source slice inline.
///
/// Used for languages without a semantic lowering (HTML, CSS, C,
/// C++, bash, scala, lua, haskell, ocaml, r, julia). The shape is
/// deliberately raw: no field-wrapping, no marker injection, no
/// per-language hooks. Whatever structure callers query has to live
/// either in tree-sitter's kind hierarchy or in a real
/// `lower_<lang>_root`.
#[inline(never)]
fn render_tree_raw(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let SyntaxTree::Raw { kind, is_named, children, range, span } = tree else { unreachable!() };

    // Anonymous Raw nodes are tree-sitter tokens (punctuation,
    // keywords, operators). They have no enclosing element — just
    // their source text in place. Only `lower_raw_passthrough_all`
    // produces them; `lower_raw_passthrough` filters them out.
    if !is_named {
        let text = range.slice(source);
        if !text.is_empty() {
            let t = xot.new_text(text);
            xot.append(parent, t)?;
        }
        return Ok(parent);
    }

    let node = element(xot, kind, *span);
    xot.append(parent, node)?;
    if children.is_empty() {
        // Leaf: include the source slice as text. Matches the
        // imperative `XotBuilder`'s leaf shape for un-typed kinds and
        // preserves `string()` round-trip for typed-passthrough
        // queries.
        let text = range.slice(source);
        if !text.is_empty() {
            let t = xot.new_text(text);
            xot.append(node, t)?;
        }
    } else {
        // Emit gap text between sibling children so `string()`
        // round-trip recovers inter-token whitespace. Imperative
        // `XotBuilder` did this between each pair of CST children;
        // we replicate it here using each child's byte range.
        let mut cursor = range.start as usize;
        for child in children {
            let child_start = child.range().start as usize;
            if child_start > cursor {
                let gap = source.get(cursor..child_start).unwrap_or("");
                if !gap.is_empty() {
                    let t = xot.new_text(gap);
                    xot.append(node, t)?;
                }
            }
            render_to_xot(xot, node, child, source)?;
            cursor = child.range().end as usize;
        }
        // Trailing gap to the end of this node's range (e.g. closing
        // punctuation not represented as a child).
        let end = range.end as usize;
        if end > cursor {
            let tail = source.get(cursor..end).unwrap_or("");
            if !tail.is_empty() {
                let t = xot.new_text(tail);
                xot.append(node, t)?;
            }
        }
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
                let inner_refs: Vec<&SyntaxTree> = arguments.iter().collect();
                let arg_range = ByteRange::new(name_range.end, seg_range.end);
                render_with_gaps(xot, node, source, arg_range, &inner_refs,
                    |xot, parent, &child| render_to_xot(xot, parent, child, source).map(|_| ()),
                )?;
            } else {
                let inner_refs: Vec<&SyntaxTree> = arguments.iter().collect();
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

/// Trait so `render_with_gaps` can take either `&SyntaxTree` or `&&SyntaxTree`.
trait HasRange {
    fn range(&self) -> ByteRange;
}

impl HasRange for SyntaxTree {
    fn range(&self) -> ByteRange { SyntaxTree::range(self) }
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
    // Slice 2 (editable trees): expose the typed-tree NodeId on xot so
    // XPath matches can recover the corresponding SyntaxTree node via
    // `find_by_id`. Omitted when id is 0 (the unassigned sentinel) to
    // keep the attribute set clean for synthetic / pre-`assign_ids` paths.
    let id_name = (span.id != 0).then(|| xot.add_name("id"));
    let mut attrs = xot.attributes_mut(node);
    attrs.insert(line, span.line.to_string());
    attrs.insert(column, span.column.to_string());
    attrs.insert(end_line, span.end_line.to_string());
    attrs.insert(end_column, span.end_column.to_string());
    if let Some(id_name) = id_name {
        attrs.insert(id_name, span.id.to_string());
    }
}
