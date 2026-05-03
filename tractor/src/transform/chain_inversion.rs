//! Chain inversion — convert right-deep operator-precedence trees
//! into left-deep nested-`<object>` shape that mirrors the developer
//! mental model.
//!
//! ## Background
//!
//! Source `a.b.c.d()` parses right-deep (operator precedence): the
//! outermost element is the LAST operation (the call), with the
//! receiver chain nested inside as `<callee>`/`<object>` slots.
//! Developers reading the source think left-to-right: "start with
//! `a`, then access `.b`, then `.c`, then call `.d()`."
//!
//! Iter 232 design (`todo/40-chain-inversion-design.md`) chose a
//! NESTED inverted shape — `<object>` wrapper around the receiver
//! and a left-deep step spine. Choice ratified by the user mid-
//! iter-234 conversation; declaration-call query symmetry was the
//! deciding factor (`//class[name='Foo']/method[name='bar']` and
//! `//chain[name='Foo']/call[name='bar']` read identically).
//!
//! ## Output shape
//!
//! For `console.stdout.write()`:
//!
//! ```xml
//! <object>
//!   <access/>                      <!-- marker: this is an access chain
//!                                       (distinguishes from object
//!                                       literals that share <object>) -->
//!   <name>console</name>          <!-- receiver -->
//!   <member>                       <!-- step 1: .stdout -->
//!     <name>stdout</name>
//!     <call>                        <!-- step 2 (terminal): .write() -->
//!       <name>write</name>
//!     </call>
//!   </member>
//! </object>
//! ```
//!
//! Receiver (first child of `<object>`) is any expression element
//! (typically `<name>`, but could be `<cast>`, `<paren>`, `<call>`
//! for result-invocation). Each subsequent step is `<member>`,
//! `<call>`, or `<subscript>` and nests subsequent steps as its
//! LAST child.
//!
//! ## Module status
//!
//! Iter 235: emit primitive + unit tests only. Extract and the
//! per-language adaptors land in iters 236+.

use xot::{Xot, Node as XotNode};

use super::helpers::*;

// =============================================================================
// SEGMENT IR
// =============================================================================

/// One link in a chain, in source order (leftmost-first).
///
/// The receiver is `Receiver`; subsequent links are `Member`,
/// `Call`, or `Subscript`.
#[derive(Debug)]
pub enum ChainSegment {
    /// Leftmost: the receiver. Holds the existing receiver node
    /// (a bare `<name>`, a `<cast>`, a `<paren>`, etc.). The node
    /// is consumed (detached + re-inserted) by `emit_chain`.
    Receiver(XotNode),
    /// `.X` access. `name_node` is the `<name>` element holding X.
    Member { name_node: XotNode, markers: Vec<XotNode> },
    /// `.X(args)` method call. `name_node` is None for result-
    /// invocation forms like `f()(args)`.
    Call {
        name_node: Option<XotNode>,
        args: Vec<XotNode>,
        markers: Vec<XotNode>,
    },
    /// `[index]` subscript access.
    Subscript { index_node: XotNode, markers: Vec<XotNode> },
}

// =============================================================================
// EMIT
// =============================================================================

/// Build the inverted `<object>` tree from a segment list.
///
/// Returns the new `<object>` element. The caller is responsible for
/// inserting it into the document at the desired location.
///
/// Pre-conditions:
///   - `segments` has at least 2 entries (one receiver + one step).
///     A 1-entry list is meaningless (just the receiver alone) —
///     the caller should leave such cases untouched.
///   - The first entry is `ChainSegment::Receiver`. Subsequent
///     entries are step variants (Member / Call / Subscript).
///   - All node references are detached from any prior parent
///     before this function runs (the function will append them
///     to the new tree).
///
/// Source-location: copied from the receiver node onto `<object>`,
/// and from each segment's primary node onto the step element.
pub fn emit_chain(
    xot: &mut Xot,
    segments: Vec<ChainSegment>,
) -> Result<XotNode, xot::Error> {
    assert!(segments.len() >= 2, "emit_chain requires receiver + ≥1 step");
    let mut iter = segments.into_iter();
    let receiver = match iter.next().expect("non-empty") {
        ChainSegment::Receiver(node) => node,
        _ => panic!("first segment must be Receiver"),
    };

    // Wrapper element is `<object>`. The `[access]` marker
    // distinguishes the runtime member-access shape from object
    // literals that share the same element name (TS/JS `{a: 1}`
    // also emit `<object>`). Structural predicates can also
    // disambiguate (`object[member]` vs `object[pair]`), but the
    // marker makes the distinction queryable directly:
    // `//object[access]` finds chains; `//object[not(access)]`
    // finds literals.
    let object_id = xot.add_name("object");
    let object = xot.new_element(object_id);
    copy_source_location(xot, receiver, object);

    let access_id = xot.add_name("access");
    let access = xot.new_element(access_id);
    xot.append(object, access)?;

    xot.append(object, receiver)?;

    // Each step is appended as the LAST child of the previous
    // step; the first step is appended directly to `<object>`.
    let mut anchor = object;
    for segment in iter {
        let step = build_step(xot, segment)?;
        xot.append(anchor, step)?;
        anchor = step;
    }

    Ok(object)
}

fn build_step(
    xot: &mut Xot,
    segment: ChainSegment,
) -> Result<XotNode, xot::Error> {
    match segment {
        ChainSegment::Receiver(_) => {
            panic!("Receiver is only valid as the first segment");
        }
        ChainSegment::Member { name_node, markers } => {
            let id = xot.add_name("member");
            let step = xot.new_element(id);
            copy_source_location(xot, name_node, step);
            for marker in markers {
                xot.append(step, marker)?;
            }
            xot.append(step, name_node)?;
            Ok(step)
        }
        ChainSegment::Call { name_node, args, markers } => {
            let id = xot.add_name("call");
            let step = xot.new_element(id);
            if let Some(name) = name_node {
                copy_source_location(xot, name, step);
            }
            for marker in markers {
                xot.append(step, marker)?;
            }
            if let Some(name) = name_node {
                xot.append(step, name)?;
            }
            for arg in args {
                xot.append(step, arg)?;
            }
            Ok(step)
        }
        ChainSegment::Subscript { index_node, markers } => {
            let id = xot.add_name("subscript");
            let step = xot.new_element(id);
            copy_source_location(xot, index_node, step);
            for marker in markers {
                xot.append(step, marker)?;
            }
            xot.append(step, index_node)?;
            Ok(step)
        }
    }
}

// =============================================================================
// EXTRACT
// =============================================================================
//
// Canonical right-deep input shape:
//
//   <member>                            -- one access step (rightmost)
//     <object>RECEIVER</object>           -- receiver subtree
//     <property><name>X</name></property> -- the .X access
//   </member>
//
//   <call>                              -- one invocation step
//     CALLEE                              -- first element child:
//                                          (a) <member>...</member> for method call
//                                          (b) <call>...</call> for result-invocation
//                                          (c) any other element for top-level call
//     <argument>...</argument>*           -- args follow as siblings
//   </call>
//
// `<object>` and `<property>` are field-slot wrappers; their content
// is the actual receiver / access expression. The outermost element
// (the chain root) corresponds to the LAST source token in operator-
// precedence order, with each child step nested deeper in source-
// order direction.
//
// For a non-chain expression (just a bare identifier, an isolated
// `f(args)` top-level call, etc.), extract_chain returns segments
// that don't form a useful inverted chain — the caller should not
// emit a `<object>` wrapper for fewer than 2 segments.

/// Walk a right-deep chain rooted at `node` and produce a segment
/// list in source order (leftmost-first).
///
/// The function is non-mutating: it returns references to existing
/// nodes in the input tree. The caller (typically
/// `invert_chain_nesting` in iter 237) is responsible for
/// detaching the originals before passing them to `emit_chain`.
///
/// For inputs that don't match the canonical shape, the function
/// degrades gracefully: any element that isn't `<member>` or
/// `<call>` is pushed as a `Receiver` segment and recursion stops.
pub fn extract_chain(xot: &Xot, node: XotNode) -> Vec<ChainSegment> {
    let mut out = Vec::new();
    walk_chain(xot, node, &mut out);
    out
}

fn walk_chain(xot: &Xot, node: XotNode, out: &mut Vec<ChainSegment>) {
    let element_name = get_element_name(xot, node);
    match element_name.as_deref() {
        Some("member") => walk_member(xot, node, out),
        Some("call") => walk_call(xot, node, out),
        _ => {
            // Base case: this is the leftmost receiver.
            out.push(ChainSegment::Receiver(node));
        }
    }
}

fn walk_member(xot: &Xot, node: XotNode, out: &mut Vec<ChainSegment>) {
    let object_slot = find_named_child(xot, node, "object");
    let property_slot = find_named_child(xot, node, "property");

    // Without an `<object>` slot the member element isn't a
    // canonical chain link — it's a self-contained expression in
    // some other context. Treat the whole node as an opaque
    // receiver rather than producing a Member segment with no
    // preceding receiver.
    let object_inner = object_slot.and_then(|slot| first_element_child(xot, slot));
    if object_inner.is_none() {
        out.push(ChainSegment::Receiver(node));
        return;
    }

    // Recurse into the receiver first so earlier links land before
    // this access in the segment list.
    walk_chain(xot, object_inner.unwrap(), out);

    let name_node = property_slot
        .and_then(|p| first_element_child(xot, p))
        .unwrap_or(node);
    let markers = collect_markers(xot, node);
    out.push(ChainSegment::Member { name_node, markers });
}

fn walk_call(xot: &Xot, node: XotNode, out: &mut Vec<ChainSegment>) {
    // First non-marker element child is the callee.
    let callee = first_non_marker_element_child(xot, node);
    let args = collect_call_args(xot, node, callee);
    let markers = collect_markers(xot, node);

    match callee {
        Some(c) => match get_element_name(xot, c).as_deref() {
            Some("member") => {
                // Method call: recurse into the member's receiver,
                // then push the call segment with the property's
                // name as the method name.
                let object_slot = find_named_child(xot, c, "object");
                if let Some(slot) = object_slot {
                    if let Some(inner) = first_element_child(xot, slot) {
                        walk_chain(xot, inner, out);
                    }
                }
                let property_slot = find_named_child(xot, c, "property");
                let method_name = property_slot
                    .and_then(|p| first_element_child(xot, p));
                out.push(ChainSegment::Call {
                    name_node: method_name,
                    args,
                    markers,
                });
            }
            Some("call") => {
                // Nested call as receiver: `f()(args)` where the
                // inner `f()` is itself a complete chain. Recurse
                // into the inner call (treats it as a sub-chain),
                // then push the outer call as a result-invocation
                // (no name).
                walk_chain(xot, c, out);
                out.push(ChainSegment::Call {
                    name_node: None,
                    args,
                    markers,
                });
            }
            _ => {
                // Bare-name callee or other simple form. This is
                // a self-contained call (no receiver chain to
                // unfold). Treat the WHOLE node as an opaque
                // Receiver — preserves the call shape verbatim
                // when it sits inside a larger chain (e.g. Java's
                // `getClass().getSimpleName()` where the inner
                // `getClass()` is the chain's receiver).
                out.push(ChainSegment::Receiver(node));
            }
        },
        None => {
            // No callee element — degenerate input.
            out.push(ChainSegment::Receiver(node));
        }
    }
}

// ---- Helpers ----------------------------------------------------------

fn find_named_child(xot: &Xot, parent: XotNode, name: &str) -> Option<XotNode> {
    xot.children(parent)
        .find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some(name)
        })
}

fn first_element_child(xot: &Xot, parent: XotNode) -> Option<XotNode> {
    xot.children(parent).find(|&c| xot.element(c).is_some())
}

fn first_non_marker_element_child(xot: &Xot, parent: XotNode) -> Option<XotNode> {
    xot.children(parent).find(|&c| {
        xot.element(c).is_some() && !is_marker_element(xot, c)
    })
}

/// A marker is an empty self-closing element (no element children
/// and no text content) — typically `<optional/>`, `<async/>`,
/// `<prefix/>`, etc.
fn is_marker_element(xot: &Xot, node: XotNode) -> bool {
    if xot.element(node).is_none() {
        return false;
    }
    !xot.children(node).any(|c| xot.element(c).is_some() || xot.text_str(c).is_some())
}

/// Collect marker children (self-closing, no content) of `node`.
fn collect_markers(xot: &Xot, node: XotNode) -> Vec<XotNode> {
    xot.children(node)
        .filter(|&c| is_marker_element(xot, c))
        .collect()
}

/// Collect `<argument>` (or any non-callee, non-marker, non-slot)
/// element children of a `<call>`. The callee is the first
/// non-marker element child; everything after it (excluding slot
/// wrappers like `<object>`/`<property>` if any leak through) is
/// considered an argument.
fn collect_call_args(
    xot: &Xot,
    call_node: XotNode,
    callee: Option<XotNode>,
) -> Vec<XotNode> {
    xot.children(call_node)
        .filter(|&c| {
            xot.element(c).is_some()
                && !is_marker_element(xot, c)
                && Some(c) != callee
        })
        .collect()
}

// =============================================================================
// PRE-PASS: wrap_flat_call_member (for Tier B / C languages)
// =============================================================================

/// Normalise a flat `<call>` shape — `<object>RECV</object>` +
/// bare `<name>METHOD</name>` siblings — into the canonical
/// right-deep input shape:
///
///   `<call><member><object>RECV</object><property><name>METHOD</name></property></member>...args</call>`
///
/// Use as a pre-pass before `invert_chains_in_tree` for languages
/// whose tree-sitter grammars emit method invocations as a flat
/// call (Java's `method_invocation`, Ruby's `call`, …) — both have
/// the receiver in `<object>` and the method name as a bare
/// `<name>` sibling rather than nested under a `<member>` callee.
///
/// Idempotent: skips calls that don't have both an `<object>`
/// slot and a bare `<name>` sibling. Top-level `f(args)` calls
/// (no receiver, no `<object>` slot) are left alone.
pub fn wrap_flat_call_member(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut calls: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("call")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut calls);
    for call in calls {
        let object_slot = xot.children(call).find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("object")
        });
        let object_slot = match object_slot {
            Some(o) => o,
            None => continue,
        };
        // The method name is the first `<name>` AFTER the <object>
        // slot (avoid grabbing names inside arguments).
        let mut name_node: Option<XotNode> = None;
        let mut found_object = false;
        for c in xot.children(call) {
            if xot.element(c).is_none() {
                continue;
            }
            if c == object_slot {
                found_object = true;
                continue;
            }
            if !found_object {
                continue;
            }
            if get_element_name(xot, c).as_deref() == Some("name") {
                name_node = Some(c);
                break;
            }
        }
        let name_node = match name_node {
            Some(n) => n,
            None => continue,
        };

        // Build <property><name>X</name></property>.
        let property_id = xot.add_name("property");
        let property = xot.new_element(property_id);
        copy_source_location(xot, name_node, property);
        xot.detach(name_node)?;
        xot.append(property, name_node)?;

        // Build <member><object>RECV</object><property>NAME</property></member>.
        let member_id = xot.add_name("member");
        let member = xot.new_element(member_id);
        copy_source_location(xot, object_slot, member);
        xot.insert_before(object_slot, member)?;
        xot.detach(object_slot)?;
        xot.append(member, object_slot)?;
        xot.append(member, property)?;
    }
    Ok(())
}

// =============================================================================
// TREE-WIDE: invert_chains_in_tree
// =============================================================================

/// Walk the tree under `root`, find every chain root, and invert
/// each in place.
///
/// A "chain root" is the OUTERMOST `<member>` or `<call>` of a
/// chain — the element whose parent is NOT another chain step:
///
/// - `<member>` is a chain root iff its parent is neither
///   `<object>` (which would mean this member is a receiver-step
///   inside an enclosing member/call) nor `<call>` (which would
///   mean this member is the callee of an enclosing call).
/// - `<call>` is a chain root iff its parent is not `<object>`
///   (which would mean this call is the receiver of an enclosing
///   member/call).
///
/// Visiting top-down is safe: `invert_chain_nesting` walks the
/// full subtree of each root and consumes the entire chain in one
/// extraction, so nested chains never need a second pass — they're
/// flattened into the same segment list as the outer chain.
///
/// Use this from per-language post-transforms after the per-kind
/// dispatcher has produced the right-deep canonical shape.
pub fn invert_chains_in_tree(
    xot: &mut Xot,
    root: XotNode,
) -> Result<(), xot::Error> {
    let mut roots: Vec<XotNode> = Vec::new();
    collect_chain_roots(xot, root, &mut roots);
    for chain_root in roots {
        // Skip nodes that have already been replaced (their parent
        // is None after a prior detach in this loop). This can
        // happen when an outer chain root absorbed an inner one
        // during the same call, then we encounter the inner one
        // here.
        if xot.parent(chain_root).is_some() {
            invert_chain_nesting(xot, chain_root)?;
        }
    }
    Ok(())
}

fn collect_chain_roots(
    xot: &Xot,
    node: XotNode,
    out: &mut Vec<XotNode>,
) {
    if xot.element(node).is_some() && is_chain_root(xot, node) {
        out.push(node);
    }
    for child in xot.children(node) {
        if xot.element(child).is_some() {
            collect_chain_roots(xot, child, out);
        }
    }
}

fn is_chain_root(xot: &Xot, node: XotNode) -> bool {
    let element_name = match get_element_name(xot, node) {
        Some(n) => n,
        None => return false,
    };
    let parent = match xot.parent(node) {
        Some(p) => p,
        None => return false,
    };
    let parent_name = get_element_name(xot, parent);
    match element_name.as_str() {
        "member" => {
            // Not a chain root when the <member> is acting as a
            // receiver/callee inside an enclosing chain step:
            // - parent=<object>: this <member> is a receiver inside
            //   another <member>/<call>. Skip; the outer chain
            //   walker absorbs it.
            // - parent=<call> AND we are the FIRST element child:
            //   we're the callee. Skip.
            // A <member> that is a NON-first child of <call> is an
            // argument expression — it IS its own chain root and
            // must be inverted (covers the Go shape where member-
            // access expressions sit as bare arguments next to a
            // top-level bare-name callee).
            match parent_name.as_deref() {
                Some("object") => false,
                Some("call") => {
                    // Callee = first non-marker element child.
                    let first = first_non_marker_element_child(xot, parent);
                    first != Some(node)
                }
                _ => true,
            }
        }
        "call" => {
            // Not a chain root if it's a receiver (parent=<object>).
            !matches!(parent_name.as_deref(), Some("object"))
        }
        _ => false,
    }
}

// =============================================================================
// ROUND-TRIP: invert_chain_nesting
// =============================================================================

/// In-place: replace `node` (a right-deep `<member>`/`<call>` chain
/// root) with its inverted nested-`<object>` equivalent.
///
/// Pipeline:
/// 1. `extract_chain(xot, node)` — produce the segment list IR.
/// 2. If fewer than 2 segments OR the only step segment is a
///    nameless top-level Call (i.e. `f(x)` with no chain), leave
///    `node` untouched. Wrapping a non-chain in `<object>` adds
///    noise without informational value.
/// 3. Detach every node referenced in the segment list from its
///    current parent (the original chain tree is now hollow).
/// 4. `emit_chain(xot, segments)` — build the new `<object>`.
/// 5. Insert the new `<object>` at `node`'s position; detach
///    `node`. Source-location is already threaded onto the new
///    chain by `emit_chain`.
///
/// Returns the new `<object>` node on success, or `Ok(None)` if the
/// input wasn't a useful chain (and was left untouched).
pub fn invert_chain_nesting(
    xot: &mut Xot,
    node: XotNode,
) -> Result<Option<XotNode>, xot::Error> {
    let segments = extract_chain(xot, node);

    if !is_useful_chain(&segments) {
        return Ok(None);
    }

    // Capture the original node's END coordinates BEFORE detaching
    // children — the chain spans from the leftmost source token
    // (receiver) to the rightmost (last arg / close paren). The
    // receiver's start is already threaded onto `<object>` by
    // `emit_chain`; we extend the end here to match the original
    // expression's full range.
    let end_line = get_attr(xot, node, "end_line");
    let end_column = get_attr(xot, node, "end_column");

    detach_segment_refs(xot, &segments)?;

    let new_chain = emit_chain(xot, segments)?;
    if let Some(v) = end_line {
        xot.with_attr(new_chain, "end_line", &v);
    }
    if let Some(v) = end_column {
        xot.with_attr(new_chain, "end_column", &v);
    }

    // Replace the original node with the new chain.
    xot.insert_before(node, new_chain)?;
    xot.detach(node)?;

    Ok(Some(new_chain))
}

/// A "useful chain" has at least 2 segments AND the second segment
/// (the first step) is structurally informative — a Member, a
/// Subscript, or a Call WITH a method name. A bare `f(args)`
/// (Receiver + nameless Call) isn't a chain in the sense the
/// inversion targets, so leave it alone.
fn is_useful_chain(segments: &[ChainSegment]) -> bool {
    if segments.len() < 2 {
        return false;
    }
    if segments.len() == 2 {
        // Only one step. If it's a nameless Call (top-level
        // invocation), leave it alone.
        if let ChainSegment::Call { name_node: None, .. } = &segments[1] {
            return false;
        }
    }
    true
}

/// Detach every node referenced in `segments` from its current
/// parent. After this runs, all referenced nodes are free-floating
/// and ready for `emit_chain` to re-attach them.
fn detach_segment_refs(
    xot: &mut Xot,
    segments: &[ChainSegment],
) -> Result<(), xot::Error> {
    for seg in segments {
        match seg {
            ChainSegment::Receiver(n) => detach_if_attached(xot, *n)?,
            ChainSegment::Member { name_node, markers } => {
                detach_if_attached(xot, *name_node)?;
                for m in markers {
                    detach_if_attached(xot, *m)?;
                }
            }
            ChainSegment::Call { name_node, args, markers } => {
                if let Some(n) = name_node {
                    detach_if_attached(xot, *n)?;
                }
                for a in args {
                    detach_if_attached(xot, *a)?;
                }
                for m in markers {
                    detach_if_attached(xot, *m)?;
                }
            }
            ChainSegment::Subscript { index_node, markers } => {
                detach_if_attached(xot, *index_node)?;
                for m in markers {
                    detach_if_attached(xot, *m)?;
                }
            }
        }
    }
    Ok(())
}

fn detach_if_attached(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    if xot.parent(node).is_some() {
        xot.detach(node)?;
    }
    Ok(())
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_xot() -> (Xot, XotNode) {
        let mut xot = Xot::new();
        let root_name = xot.add_name("doc");
        let root = xot.new_element(root_name);
        let _doc = xot.new_document_with_element(root).unwrap();
        (xot, root)
    }

    fn new_named_element(xot: &mut Xot, name: &str) -> XotNode {
        let id = xot.add_name(name);
        xot.new_element(id)
    }

    fn new_text_element(xot: &mut Xot, name: &str, text: &str) -> XotNode {
        let elem = new_named_element(xot, name);
        let txt = xot.new_text(text);
        xot.append(elem, txt).unwrap();
        elem
    }

    /// Render an element as a simple S-expression for easy assertion:
    ///   `(member (name b) (call (name c)))`
    fn render(xot: &Xot, node: XotNode) -> String {
        let name = xot.local_name_str(xot.element(node).unwrap().name());
        let mut out = format!("({}", name);
        let direct_text: Option<String> = xot.children(node)
            .find_map(|c| xot.text_str(c).map(|s| s.to_string()));
        let elem_children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if let Some(t) = &direct_text {
            if elem_children.is_empty() {
                out.push(' ');
                out.push_str(t);
                out.push(')');
                return out;
            }
        }
        for child in elem_children {
            out.push(' ');
            out.push_str(&render(xot, child));
        }
        out.push(')');
        out
    }

    // --- Receiver-only chains -------------------------------------------

    #[test]
    fn emit_simple_member_access() {
        // a.b → <object><name>a</name><member><name>b</name></member></chain>
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let access = new_text_element(&mut xot, "name", "b");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Member { name_node: access, markers: vec![] },
        ]).unwrap();
        assert_eq!(render(&xot, chain), "(object (access) (name a) (member (name b)))");
    }

    #[test]
    fn emit_terminal_call() {
        // a.b() → <object><name>a</name><call><name>b</name></call></chain>
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let method = new_text_element(&mut xot, "name", "b");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Call { name_node: Some(method), args: vec![], markers: vec![] },
        ]).unwrap();
        assert_eq!(render(&xot, chain), "(object (access) (name a) (call (name b)))");
    }

    // --- Multi-link chains ---------------------------------------------

    #[test]
    fn emit_multi_link_member_chain() {
        // a.b.c.d (pure access)
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let b = new_text_element(&mut xot, "name", "b");
        let c = new_text_element(&mut xot, "name", "c");
        let d = new_text_element(&mut xot, "name", "d");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Member { name_node: b, markers: vec![] },
            ChainSegment::Member { name_node: c, markers: vec![] },
            ChainSegment::Member { name_node: d, markers: vec![] },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name a) (member (name b) (member (name c) (member (name d)))))",
        );
    }

    #[test]
    fn emit_multi_link_call_chain_terminal_call() {
        // a.b.c.d() — three accesses + terminal call
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let b = new_text_element(&mut xot, "name", "b");
        let c = new_text_element(&mut xot, "name", "c");
        let d = new_text_element(&mut xot, "name", "d");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Member { name_node: b, markers: vec![] },
            ChainSegment::Member { name_node: c, markers: vec![] },
            ChainSegment::Call { name_node: Some(d), args: vec![], markers: vec![] },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name a) (member (name b) (member (name c) (call (name d)))))",
        );
    }

    #[test]
    fn emit_mixed_member_and_call_steps() {
        // a.b().c.d() — call mid-chain, call terminal
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let b = new_text_element(&mut xot, "name", "b");
        let c = new_text_element(&mut xot, "name", "c");
        let d = new_text_element(&mut xot, "name", "d");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Call { name_node: Some(b), args: vec![], markers: vec![] },
            ChainSegment::Member { name_node: c, markers: vec![] },
            ChainSegment::Call { name_node: Some(d), args: vec![], markers: vec![] },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name a) (call (name b) (member (name c) (call (name d)))))",
        );
    }

    // --- Args -----------------------------------------------------------

    #[test]
    fn emit_call_with_args() {
        // a.b(x, y)
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let method = new_text_element(&mut xot, "name", "b");
        let arg1 = new_text_element(&mut xot, "argument", "x");
        let arg2 = new_text_element(&mut xot, "argument", "y");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Call {
                name_node: Some(method),
                args: vec![arg1, arg2],
                markers: vec![],
            },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name a) (call (name b) (argument x) (argument y)))",
        );
    }

    #[test]
    fn emit_args_at_each_chain_link() {
        // a.b(1).c(2).d(3) — args at every call step
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let b = new_text_element(&mut xot, "name", "b");
        let arg1 = new_text_element(&mut xot, "argument", "1");
        let c = new_text_element(&mut xot, "name", "c");
        let arg2 = new_text_element(&mut xot, "argument", "2");
        let d = new_text_element(&mut xot, "name", "d");
        let arg3 = new_text_element(&mut xot, "argument", "3");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Call {
                name_node: Some(b),
                args: vec![arg1],
                markers: vec![],
            },
            ChainSegment::Call {
                name_node: Some(c),
                args: vec![arg2],
                markers: vec![],
            },
            ChainSegment::Call {
                name_node: Some(d),
                args: vec![arg3],
                markers: vec![],
            },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name a) \
             (call (name b) (argument 1) \
              (call (name c) (argument 2) \
               (call (name d) (argument 3)))))",
        );
    }

    // --- Markers --------------------------------------------------------

    #[test]
    fn emit_optional_chaining_markers() {
        // a?.b?.c() — optional markers on each step
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let b = new_text_element(&mut xot, "name", "b");
        let opt_b = new_named_element(&mut xot, "optional");
        let c = new_text_element(&mut xot, "name", "c");
        let opt_c = new_named_element(&mut xot, "optional");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Member { name_node: b, markers: vec![opt_b] },
            ChainSegment::Call {
                name_node: Some(c),
                args: vec![],
                markers: vec![opt_c],
            },
        ]).unwrap();
        // markers come BEFORE name (matches the typical [marker]name layout)
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name a) (member (optional) (name b) (call (optional) (name c))))",
        );
    }

    // --- Subscript ------------------------------------------------------

    #[test]
    fn emit_subscript_in_chain() {
        // a[0].b
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let index = new_text_element(&mut xot, "int", "0");
        let b = new_text_element(&mut xot, "name", "b");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Subscript { index_node: index, markers: vec![] },
            ChainSegment::Member { name_node: b, markers: vec![] },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name a) (subscript (int 0) (member (name b))))",
        );
    }

    // --- Complex receivers ---------------------------------------------

    #[test]
    fn emit_with_complex_receiver_cast() {
        // (x as Foo).b → receiver is a <cast>, not a bare <name>
        let (mut xot, _root) = fresh_xot();
        let cast = new_named_element(&mut xot, "cast");
        let xn = new_text_element(&mut xot, "name", "x");
        let typ = new_text_element(&mut xot, "type", "Foo");
        xot.append(cast, xn).unwrap();
        xot.append(cast, typ).unwrap();
        let b = new_text_element(&mut xot, "name", "b");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(cast),
            ChainSegment::Member { name_node: b, markers: vec![] },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (cast (name x) (type Foo)) (member (name b)))",
        );
    }

    #[test]
    fn emit_result_invocation_no_method_name() {
        // f()(args) — second call has no name, just args
        // First step is a terminal call f(); receiver of the "outer"
        // chain is just <name>f</name>, then a Call with no name and
        // an arg.
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "f");
        let arg = new_text_element(&mut xot, "argument", "x");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Call {
                name_node: None,
                args: vec![],
                markers: vec![],
            },
            ChainSegment::Call {
                name_node: None,
                args: vec![arg],
                markers: vec![],
            },
        ]).unwrap();
        assert_eq!(
            render(&xot, chain),
            "(object (access) (name f) (call (call (argument x))))",
        );
    }

    // --- Source-location threading -------------------------------------

    #[test]
    fn emit_threads_source_location_from_receiver_to_chain() {
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        set_attr(&mut xot, recv, "line", "5");
        set_attr(&mut xot, recv, "column", "10");
        let b = new_text_element(&mut xot, "name", "b");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Member { name_node: b, markers: vec![] },
        ]).unwrap();
        assert_eq!(get_attr(&xot, chain, "line"), Some("5".to_string()));
        assert_eq!(get_attr(&xot, chain, "column"), Some("10".to_string()));
    }

    #[test]
    fn emit_threads_source_location_from_name_to_step() {
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let b = new_text_element(&mut xot, "name", "b");
        set_attr(&mut xot, b, "line", "7");
        set_attr(&mut xot, b, "column", "3");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Member { name_node: b, markers: vec![] },
        ]).unwrap();
        let member = xot.children(chain)
            .find(|&c| get_element_name(&xot, c).as_deref() == Some("member"))
            .expect("member step");
        assert_eq!(get_attr(&xot, member, "line"), Some("7".to_string()));
        assert_eq!(get_attr(&xot, member, "column"), Some("3".to_string()));
    }

    // --- Pre-condition guards ------------------------------------------

    #[test]
    #[should_panic(expected = "≥1 step")]
    fn emit_panics_on_receiver_only() {
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let _ = emit_chain(&mut xot, vec![ChainSegment::Receiver(recv)]);
    }

    // ===================================================================
    // EXTRACT — synthetic right-deep input → segment list IR
    // ===================================================================

    /// Build `<member><object>OBJ</object><property><name>P</name></property></member>`
    fn build_member(xot: &mut Xot, object: XotNode, prop_name: &str) -> XotNode {
        let member = new_named_element(xot, "member");
        let obj_slot = new_named_element(xot, "object");
        xot.append(obj_slot, object).unwrap();
        let prop_slot = new_named_element(xot, "property");
        let name = new_text_element(xot, "name", prop_name);
        xot.append(prop_slot, name).unwrap();
        xot.append(member, obj_slot).unwrap();
        xot.append(member, prop_slot).unwrap();
        member
    }

    /// Build a call wrapping a callee + zero or more args.
    fn build_call(xot: &mut Xot, callee: XotNode, args: Vec<XotNode>) -> XotNode {
        let call = new_named_element(xot, "call");
        xot.append(call, callee).unwrap();
        for arg in args {
            xot.append(call, arg).unwrap();
        }
        call
    }

    /// Inspect helper: get the text content of a segment's primary
    /// node. Returns None for receivers without a single text leaf,
    /// or for segments whose primary node is missing.
    fn segment_text(xot: &Xot, seg: &ChainSegment) -> Option<String> {
        let node = match seg {
            ChainSegment::Receiver(n) => Some(*n),
            ChainSegment::Member { name_node, .. } => Some(*name_node),
            ChainSegment::Call { name_node, .. } => *name_node,
            ChainSegment::Subscript { index_node, .. } => Some(*index_node),
        }?;
        xot.children(node).find_map(|c| xot.text_str(c).map(|s| s.to_string()))
    }

    fn segment_kind(seg: &ChainSegment) -> &'static str {
        match seg {
            ChainSegment::Receiver(_) => "Receiver",
            ChainSegment::Member { .. } => "Member",
            ChainSegment::Call { .. } => "Call",
            ChainSegment::Subscript { .. } => "Subscript",
        }
    }

    fn render_segments(xot: &Xot, segments: &[ChainSegment]) -> String {
        segments.iter()
            .map(|s| match (segment_kind(s), segment_text(xot, s)) {
                (k, Some(t)) => format!("{}:{}", k, t),
                (k, None) => k.to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[test]
    fn extract_simple_member_access() {
        // a.b
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let m = build_member(&mut xot, a, "b");
        let segs = extract_chain(&xot, m);
        assert_eq!(render_segments(&xot, &segs), "Receiver:a, Member:b");
    }

    #[test]
    fn extract_multi_link_member_chain() {
        // a.b.c.d
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let m1 = build_member(&mut xot, a, "b");
        let m2 = build_member(&mut xot, m1, "c");
        let m3 = build_member(&mut xot, m2, "d");
        let segs = extract_chain(&xot, m3);
        assert_eq!(
            render_segments(&xot, &segs),
            "Receiver:a, Member:b, Member:c, Member:d"
        );
    }

    #[test]
    fn extract_terminal_method_call() {
        // a.b()  →  call(callee=member(a, b))
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let callee = build_member(&mut xot, a, "b");
        let call = build_call(&mut xot, callee, vec![]);
        let segs = extract_chain(&xot, call);
        assert_eq!(render_segments(&xot, &segs), "Receiver:a, Call:b");
    }

    #[test]
    fn extract_multi_link_call_chain() {
        // a.b.c.d()
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let m1 = build_member(&mut xot, a, "b");
        let m2 = build_member(&mut xot, m1, "c");
        let callee = build_member(&mut xot, m2, "d");
        let call = build_call(&mut xot, callee, vec![]);
        let segs = extract_chain(&xot, call);
        assert_eq!(
            render_segments(&xot, &segs),
            "Receiver:a, Member:b, Member:c, Call:d"
        );
    }

    #[test]
    fn extract_mixed_member_and_call_steps() {
        // a.b().c.d()
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let callee_b = build_member(&mut xot, a, "b");
        let call_b = build_call(&mut xot, callee_b, vec![]);
        let m_c = build_member(&mut xot, call_b, "c");
        let callee_d = build_member(&mut xot, m_c, "d");
        let call_d = build_call(&mut xot, callee_d, vec![]);
        let segs = extract_chain(&xot, call_d);
        assert_eq!(
            render_segments(&xot, &segs),
            "Receiver:a, Call:b, Member:c, Call:d"
        );
    }

    #[test]
    fn extract_call_with_args() {
        // a.b(x, y)
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let callee = build_member(&mut xot, a, "b");
        let arg1 = new_text_element(&mut xot, "argument", "x");
        let arg2 = new_text_element(&mut xot, "argument", "y");
        let call = build_call(&mut xot, callee, vec![arg1, arg2]);
        let segs = extract_chain(&xot, call);
        assert_eq!(segs.len(), 2);
        let ChainSegment::Call { args, .. } = &segs[1] else {
            panic!("expected Call segment");
        };
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn extract_top_level_call_no_chain() {
        // f(x) — bare callee, no chain at all. The whole <call>
        // becomes a single opaque Receiver — preserves the call
        // shape verbatim (would be left untouched by the inverter).
        let (mut xot, _root) = fresh_xot();
        let f = new_text_element(&mut xot, "name", "f");
        let arg = new_text_element(&mut xot, "argument", "x");
        let call = build_call(&mut xot, f, vec![arg]);
        let segs = extract_chain(&xot, call);
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0], ChainSegment::Receiver(_)));
    }

    #[test]
    fn extract_bare_identifier_no_chain() {
        // Just a name, not a chain.
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let segs = extract_chain(&xot, a);
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0], ChainSegment::Receiver(_)));
    }

    #[test]
    fn extract_complex_receiver_passes_through() {
        // (cast).b — receiver is a <cast>, not a bare <name>
        let (mut xot, _root) = fresh_xot();
        let cast = new_named_element(&mut xot, "cast");
        let xn = new_text_element(&mut xot, "name", "x");
        xot.append(cast, xn).unwrap();
        let m = build_member(&mut xot, cast, "b");
        let segs = extract_chain(&xot, m);
        assert_eq!(segs.len(), 2);
        // Receiver should be the <cast> element.
        let ChainSegment::Receiver(r) = segs[0] else {
            panic!("expected Receiver");
        };
        assert_eq!(get_element_name(&xot, r).as_deref(), Some("cast"));
        assert!(matches!(segs[1], ChainSegment::Member { .. }));
    }

    #[test]
    fn extract_result_invocation_double_call() {
        // f()(args) — call where callee is itself a call. The
        // inner `f()` (bare-name callee, no chain) is preserved
        // as an opaque Receiver. The outer call adds a result-
        // invocation step (no method name, just args).
        let (mut xot, _root) = fresh_xot();
        let f = new_text_element(&mut xot, "name", "f");
        let inner_call = build_call(&mut xot, f, vec![]);
        let outer_arg = new_text_element(&mut xot, "argument", "x");
        let outer_call = build_call(&mut xot, inner_call, vec![outer_arg]);
        let segs = extract_chain(&xot, outer_call);
        // Expected: Receiver(inner_call), Call(no name, outer with arg)
        assert_eq!(segs.len(), 2);
        let ChainSegment::Receiver(rcv) = segs[0] else {
            panic!("expected Receiver");
        };
        // The receiver IS the inner <call> element (opaque).
        assert_eq!(get_element_name(&xot, rcv).as_deref(), Some("call"));
        let ChainSegment::Call { name_node: None, args, .. } = &segs[1] else {
            panic!("expected outer Call");
        };
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn extract_marker_collected_on_member_step() {
        // a?.b — optional marker on the <member>
        let (mut xot, _root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let m = build_member(&mut xot, a, "b");
        let opt = new_named_element(&mut xot, "optional");
        xot.append(m, opt).unwrap();
        let segs = extract_chain(&xot, m);
        assert_eq!(segs.len(), 2);
        let ChainSegment::Member { markers, .. } = &segs[1] else {
            panic!("expected Member");
        };
        assert_eq!(markers.len(), 1);
    }

    // ===================================================================
    // ROUND-TRIP — invert_chain_nesting
    // ===================================================================

    /// Append a right-deep chain root as a child of `parent`, then
    /// run `invert_chain_nesting` and render the result.
    fn invert_under_parent(xot: &mut Xot, parent: XotNode, root: XotNode) -> String {
        xot.append(parent, root).unwrap();
        let _ = invert_chain_nesting(xot, root).unwrap();
        // Find the surviving child of parent (the new chain or the
        // original if untouched) and render it.
        let surviving = xot.children(parent)
            .find(|&c| xot.element(c).is_some())
            .expect("parent should have an element child after invert");
        render(xot, surviving)
    }

    #[test]
    fn invert_simple_member_access() {
        // a.b → <object><name>a</name><member><name>b</name></member></chain>
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let m = build_member(&mut xot, a, "b");
        let result = invert_under_parent(&mut xot, doc_root, m);
        assert_eq!(result, "(object (access) (name a) (member (name b)))");
    }

    #[test]
    fn invert_multi_link_member_chain() {
        // a.b.c.d
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let m1 = build_member(&mut xot, a, "b");
        let m2 = build_member(&mut xot, m1, "c");
        let m3 = build_member(&mut xot, m2, "d");
        let result = invert_under_parent(&mut xot, doc_root, m3);
        assert_eq!(
            result,
            "(object (access) (name a) (member (name b) (member (name c) (member (name d)))))",
        );
    }

    #[test]
    fn invert_terminal_method_call() {
        // a.b()
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let callee = build_member(&mut xot, a, "b");
        let call = build_call(&mut xot, callee, vec![]);
        let result = invert_under_parent(&mut xot, doc_root, call);
        assert_eq!(result, "(object (access) (name a) (call (name b)))");
    }

    #[test]
    fn invert_multi_link_call_chain() {
        // a.b.c.d()
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let m1 = build_member(&mut xot, a, "b");
        let m2 = build_member(&mut xot, m1, "c");
        let callee = build_member(&mut xot, m2, "d");
        let call = build_call(&mut xot, callee, vec![]);
        let result = invert_under_parent(&mut xot, doc_root, call);
        assert_eq!(
            result,
            "(object (access) (name a) (member (name b) (member (name c) (call (name d)))))",
        );
    }

    #[test]
    fn invert_call_with_args() {
        // a.b(x, y)
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let callee = build_member(&mut xot, a, "b");
        let arg1 = new_text_element(&mut xot, "argument", "x");
        let arg2 = new_text_element(&mut xot, "argument", "y");
        let call = build_call(&mut xot, callee, vec![arg1, arg2]);
        let result = invert_under_parent(&mut xot, doc_root, call);
        assert_eq!(
            result,
            "(object (access) (name a) (call (name b) (argument x) (argument y)))",
        );
    }

    #[test]
    fn invert_mixed_calls_and_accesses() {
        // a.b().c.d()
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let cb = build_member(&mut xot, a, "b");
        let call_b = build_call(&mut xot, cb, vec![]);
        let mc = build_member(&mut xot, call_b, "c");
        let cd = build_member(&mut xot, mc, "d");
        let call_d = build_call(&mut xot, cd, vec![]);
        let result = invert_under_parent(&mut xot, doc_root, call_d);
        assert_eq!(
            result,
            "(object (access) (name a) (call (name b) (member (name c) (call (name d)))))",
        );
    }

    #[test]
    fn invert_top_level_call_left_untouched() {
        // f(x) — bare function call, no chain. Should NOT be wrapped
        // in <object>; the original <call> stays in place.
        let (mut xot, doc_root) = fresh_xot();
        let f = new_text_element(&mut xot, "name", "f");
        let arg = new_text_element(&mut xot, "argument", "x");
        let call = build_call(&mut xot, f, vec![arg]);
        let result = invert_under_parent(&mut xot, doc_root, call);
        // Original <call> unchanged.
        assert_eq!(result, "(call (name f) (argument x))");
    }

    #[test]
    fn invert_bare_identifier_left_untouched() {
        // Just a name, no chain. Should NOT be wrapped.
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        let result = invert_under_parent(&mut xot, doc_root, a);
        assert_eq!(result, "(name a)");
    }

    #[test]
    fn invert_idempotent_on_already_inverted_chain() {
        // Build an already-inverted <object> directly (NOT a
        // <member>/<call> root) and confirm invert_chain_nesting
        // leaves it alone — since the root isn't <member>/<call>,
        // extract returns just a Receiver (single-segment, not a
        // useful chain) and the helper returns Ok(None).
        let (mut xot, doc_root) = fresh_xot();
        let access = new_named_element(&mut xot, "access");
        let recv = new_text_element(&mut xot, "name", "a");
        let bn = new_text_element(&mut xot, "name", "b");
        let member = new_named_element(&mut xot, "member");
        xot.append(member, bn).unwrap();
        let object = new_named_element(&mut xot, "object");
        xot.append(object, access).unwrap();
        xot.append(object, recv).unwrap();
        xot.append(object, member).unwrap();
        let result = invert_under_parent(&mut xot, doc_root, object);
        assert_eq!(result, "(object (access) (name a) (member (name b)))");
    }

    #[test]
    fn invert_threads_source_location_to_chain() {
        // Verify <object> inherits the receiver's line/column.
        let (mut xot, doc_root) = fresh_xot();
        let a = new_text_element(&mut xot, "name", "a");
        set_attr(&mut xot, a, "line", "12");
        set_attr(&mut xot, a, "column", "4");
        let m = build_member(&mut xot, a, "b");
        xot.append(doc_root, m).unwrap();
        let chain = invert_chain_nesting(&mut xot, m).unwrap()
            .expect("inversion should produce a new chain");
        assert_eq!(get_attr(&xot, chain, "line"), Some("12".to_string()));
        assert_eq!(get_attr(&xot, chain, "column"), Some("4".to_string()));
    }

    #[test]
    fn invert_complex_receiver_passes_through() {
        // (cast).b → cast preserved as the receiver
        let (mut xot, doc_root) = fresh_xot();
        let cast = new_named_element(&mut xot, "cast");
        let xn = new_text_element(&mut xot, "name", "x");
        xot.append(cast, xn).unwrap();
        let m = build_member(&mut xot, cast, "b");
        let result = invert_under_parent(&mut xot, doc_root, m);
        assert_eq!(result, "(object (access) (cast (name x)) (member (name b)))");
    }

    // ===================================================================
    // TREE-WIDE — invert_chains_in_tree + chain-root identification
    // ===================================================================

    #[test]
    fn tree_walker_inverts_single_chain() {
        // `body { stmt(a.b) }` — one chain inside a statement.
        let (mut xot, doc_root) = fresh_xot();
        let body = new_named_element(&mut xot, "body");
        let stmt = new_named_element(&mut xot, "stmt");
        let a = new_text_element(&mut xot, "name", "a");
        let m = build_member(&mut xot, a, "b");
        xot.append(stmt, m).unwrap();
        xot.append(body, stmt).unwrap();
        xot.append(doc_root, body).unwrap();
        invert_chains_in_tree(&mut xot, doc_root).unwrap();
        assert_eq!(
            render(&xot, body),
            "(body (stmt (object (access) (name a) (member (name b)))))",
        );
    }

    #[test]
    fn tree_walker_inverts_chain_inside_argument() {
        // f(obj.method()) — outer is a top-level call (not a chain
        // root), the argument contains a chain that IS a root.
        let (mut xot, doc_root) = fresh_xot();
        let outer_callee = new_text_element(&mut xot, "name", "f");
        let inner_recv = new_text_element(&mut xot, "name", "obj");
        let inner_callee = build_member(&mut xot, inner_recv, "method");
        let inner_call = build_call(&mut xot, inner_callee, vec![]);
        // Wrap the inner call in <argument> so it's a sibling of
        // the outer callee.
        let arg = new_named_element(&mut xot, "argument");
        xot.append(arg, inner_call).unwrap();
        let outer_call = new_named_element(&mut xot, "call");
        xot.append(outer_call, outer_callee).unwrap();
        xot.append(outer_call, arg).unwrap();
        xot.append(doc_root, outer_call).unwrap();
        invert_chains_in_tree(&mut xot, doc_root).unwrap();
        // Outer call left untouched (top-level, no chain), inner
        // call became a <object>.
        assert_eq!(
            render(&xot, outer_call),
            "(call (name f) (argument (object (access) (name obj) (call (name method)))))",
        );
    }

    #[test]
    fn tree_walker_inverts_chains_in_multiple_locations() {
        // `body { stmt1(a.b)  stmt2(c.d.e) }` — two independent
        // chains. Both should invert.
        let (mut xot, doc_root) = fresh_xot();
        let body = new_named_element(&mut xot, "body");

        let s1 = new_named_element(&mut xot, "stmt1");
        let a = new_text_element(&mut xot, "name", "a");
        let m1 = build_member(&mut xot, a, "b");
        xot.append(s1, m1).unwrap();
        xot.append(body, s1).unwrap();

        let s2 = new_named_element(&mut xot, "stmt2");
        let c = new_text_element(&mut xot, "name", "c");
        let m2a = build_member(&mut xot, c, "d");
        let m2b = build_member(&mut xot, m2a, "e");
        xot.append(s2, m2b).unwrap();
        xot.append(body, s2).unwrap();

        xot.append(doc_root, body).unwrap();
        invert_chains_in_tree(&mut xot, doc_root).unwrap();
        assert_eq!(
            render(&xot, body),
            "(body (stmt1 (object (access) (name a) (member (name b)))) \
             (stmt2 (object (access) (name c) (member (name d) (member (name e))))))",
        );
    }

    #[test]
    fn tree_walker_does_not_double_process_nested_chains() {
        // `obj.method(x).other.thing()` — outermost call is the
        // chain root; the inner call (obj.method(x)) is INSIDE the
        // outer chain's receiver and gets consumed as part of the
        // same extract_chain walk. The walker shouldn't try to
        // process it again.
        let (mut xot, doc_root) = fresh_xot();
        let obj = new_text_element(&mut xot, "name", "obj");
        let inner_callee = build_member(&mut xot, obj, "method");
        let arg = new_text_element(&mut xot, "argument", "x");
        let inner_call = build_call(&mut xot, inner_callee, vec![arg]);
        let m_other = build_member(&mut xot, inner_call, "other");
        let outer_callee = build_member(&mut xot, m_other, "thing");
        let outer_call = build_call(&mut xot, outer_callee, vec![]);
        xot.append(doc_root, outer_call).unwrap();
        invert_chains_in_tree(&mut xot, doc_root).unwrap();
        // Single <object> with all 4 segments.
        let surviving = xot.children(doc_root)
            .find(|&c| {
                xot.element(c).is_some()
                    && get_element_name(&xot, c).as_deref() == Some("object")
            })
            .or_else(|| xot.children(doc_root).find(|&c| xot.element(c).is_some()))
            .expect("a child");
        assert_eq!(
            render(&xot, surviving),
            "(object (access) (name obj) (call (name method) (argument x) (member (name other) (call (name thing)))))",
        );
    }

    #[test]
    fn chain_root_predicate() {
        // Confirm the is_chain_root predicate.
        let (mut xot, doc_root) = fresh_xot();
        // Build: <doc><call>OUTER</call> with an inner member
        // (callee) and a top-level <member>.
        let inner_recv = new_text_element(&mut xot, "name", "x");
        let inner_member = build_member(&mut xot, inner_recv, "y");
        let outer_call = build_call(&mut xot, inner_member, vec![]);
        xot.append(doc_root, outer_call).unwrap();
        // outer_call: parent is <doc> → chain root.
        assert!(is_chain_root(&xot, outer_call));
        // inner_member: parent is <call> → NOT a chain root (it's
        // the callee of the outer call).
        let callee = first_element_child(&xot, outer_call).unwrap();
        assert!(!is_chain_root(&xot, callee));
        // The deepest <name>x</name>: not a member/call → not a
        // chain root.
        // (Find it via the inner member's <object> slot.)
        let object_slot = find_named_child(&xot, callee, "object").unwrap();
        let recv = first_element_child(&xot, object_slot).unwrap();
        assert!(!is_chain_root(&xot, recv));
    }
}
