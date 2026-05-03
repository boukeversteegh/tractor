//! Chain inversion — convert right-deep operator-precedence trees
//! into left-deep nested-`<chain>` shape that mirrors the developer
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
//! NESTED inverted shape — `<chain>` wrapper around the receiver
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
//! <chain>
//!   <name>console</name>          <!-- receiver -->
//!   <member>                       <!-- step 1: .stdout -->
//!     <name>stdout</name>
//!     <call>                        <!-- step 2 (terminal): .write() -->
//!       <name>write</name>
//!     </call>
//!   </member>
//! </chain>
//! ```
//!
//! Receiver (first child of `<chain>`) is any expression element
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

/// Build the inverted `<chain>` tree from a segment list.
///
/// Returns the new `<chain>` element. The caller is responsible for
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
/// Source-location: copied from the receiver node onto `<chain>`,
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

    let chain_id = xot.add_name("chain");
    let chain = xot.new_element(chain_id);
    copy_source_location(xot, receiver, chain);

    xot.append(chain, receiver)?;

    // Each step is appended as the LAST child of the previous
    // step; the first step is appended directly to `<chain>`.
    let mut anchor = chain;
    for segment in iter {
        let step = build_step(xot, segment)?;
        xot.append(anchor, step)?;
        anchor = step;
    }

    Ok(chain)
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
// EXTRACT (stub for iter 236)
// =============================================================================

/// Walk a right-deep chain rooted at `node` and produce a segment
/// list in source order. NOT YET IMPLEMENTED — input shapes vary
/// per language and need per-language adaptors. Iter 236 will
/// land the canonical-shape extractor + Java-pilot adaptor.
#[allow(dead_code)]
pub fn extract_chain(_xot: &Xot, _node: XotNode) -> Vec<ChainSegment> {
    unimplemented!("iter 236 — extract from right-deep input")
}

/// In-place wrapper: extract the chain at `node`, emit the
/// inverted form, and replace `node` in the tree. Stub for iter
/// 237.
#[allow(dead_code)]
pub fn invert_chain_nesting(
    _xot: &mut Xot,
    _node: XotNode,
) -> Result<(), xot::Error> {
    unimplemented!("iter 237 — emit + replace pipeline")
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
        // a.b → <chain><name>a</name><member><name>b</name></member></chain>
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let access = new_text_element(&mut xot, "name", "b");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Member { name_node: access, markers: vec![] },
        ]).unwrap();
        assert_eq!(render(&xot, chain), "(chain (name a) (member (name b)))");
    }

    #[test]
    fn emit_terminal_call() {
        // a.b() → <chain><name>a</name><call><name>b</name></call></chain>
        let (mut xot, _root) = fresh_xot();
        let recv = new_text_element(&mut xot, "name", "a");
        let method = new_text_element(&mut xot, "name", "b");
        let chain = emit_chain(&mut xot, vec![
            ChainSegment::Receiver(recv),
            ChainSegment::Call { name_node: Some(method), args: vec![], markers: vec![] },
        ]).unwrap();
        assert_eq!(render(&xot, chain), "(chain (name a) (call (name b)))");
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
            "(chain (name a) (member (name b) (member (name c) (member (name d)))))",
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
            "(chain (name a) (member (name b) (member (name c) (call (name d)))))",
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
            "(chain (name a) (call (name b) (member (name c) (call (name d)))))",
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
            "(chain (name a) (call (name b) (argument x) (argument y)))",
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
            "(chain (name a) \
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
            "(chain (name a) (member (optional) (name b) (call (optional) (name c))))",
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
            "(chain (name a) (subscript (int 0) (member (name b))))",
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
            "(chain (cast (name x) (type Foo)) (member (name b)))",
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
            "(chain (name f) (call (call (argument x))))",
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
}
