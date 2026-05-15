//! Generic XML walker — variant-blind rendering driven by three
//! per-variant accessors.
//!
//! The walker is the long-term target for `tree/to_xot.rs`: a single
//! function that emits XML for any `SyntaxTree` without knowing the
//! variants. It reads each node via three accessors:
//!
//! - [`element_name_of`] — the XML element name to emit
//! - [`flags_of`] — named flags emitted as empty marker children with
//!   carried source positions
//! - [`children_of`] — direct subtree children, walked recursively
//!
//! Variants whose XML projection still requires renderer-side
//! synthesis (manufacturing slot wrappers / hosts that aren't in the
//! tree) return `WalkerEligibility::NeedsLegacy` from
//! [`walker_eligibility`] and the dispatching caller falls back to
//! the per-variant `render_tree_*` path in `tree/to_xot.rs`.
//!
//! As variants migrate to keep all shape decisions in the tree, their
//! eligibility flips and the walker handles them uniformly. When every
//! variant is `Eligible`, the per-variant renderer functions retire
//! and the walker becomes the sole projection path.
//!
//! ## Codegen plan
//!
//! `docs/design-walker-codegen.md` lays out the migration to
//! build-time codegen. The three accessor functions below are
//! hand-written today; they will be replaced by output of `task
//! gen:walker`, which reads `#[shape(...)]` attributes on each
//! `SyntaxTree` variant in `tree/types.rs` and emits one match arm
//! per variant per accessor. The generated file is committed; the
//! diff is reviewable; the file is a real `.rs` file so debuggers
//! step into it directly (the key advantage over a derive macro).

#![cfg(feature = "native")]

use xot::{Node as XotNode, Xot};

use super::types::{Marker, SyntaxTree, Span};

/// Variant-specific walker eligibility. `Eligible` means the walker
/// emits the correct XML using only the accessor trio. `NeedsLegacy`
/// means the variant still depends on renderer-side wrapper
/// synthesis; the dispatching caller should defer to `render_to_xot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkerEligibility {
    Eligible,
    NeedsLegacy,
}

/// Whether the generic walker can render `tree`.
pub fn walker_eligibility(tree: &SyntaxTree) -> WalkerEligibility {
    match tree {
        // Variants whose XML projection still requires renderer-side
        // synthesis live here until their shape is fully lifted into
        // the tree (Z-steps 2 and 3 of the unified-renderer plan).
        SyntaxTree::Assign { .. }
            | SyntaxTree::Class { .. }
            | SyntaxTree::Access { .. }
            | SyntaxTree::Foreach { .. }
            | SyntaxTree::ExceptHandler { .. }
            | SyntaxTree::Lambda { .. }
            | SyntaxTree::Parameter { .. }
            | SyntaxTree::Variable { .. }
            | SyntaxTree::ObjectCreation { .. }
            | SyntaxTree::Property { .. }
            | SyntaxTree::Accessor { .. }
            | SyntaxTree::Constructor { .. }
            | SyntaxTree::Using { .. }
            | SyntaxTree::Namespace { .. }
            | SyntaxTree::Is { .. }
            | SyntaxTree::Cast { .. }
            | SyntaxTree::Comparison { .. }
            | SyntaxTree::Enum { .. }
            | SyntaxTree::EnumMember { .. }
            | SyntaxTree::Binary { .. }
            | SyntaxTree::Unary { .. }
            | SyntaxTree::Ternary { .. }
            | SyntaxTree::Decorator { .. }
            | SyntaxTree::From { .. }
            | SyntaxTree::FromImport { .. }
            | SyntaxTree::Aliased { .. }
            | SyntaxTree::Import { .. }
            | SyntaxTree::Body { .. }
            | SyntaxTree::Return { .. }
            | SyntaxTree::Returns { .. }
            | SyntaxTree::Comment { .. }
            | SyntaxTree::FieldWrap { .. }
            | SyntaxTree::TypeAlias { .. }
            | SyntaxTree::KeywordArgument { .. }
            | SyntaxTree::ListSplat { .. }
            | SyntaxTree::DictSplat { .. }
            | SyntaxTree::Try { .. }
            | SyntaxTree::GenericType { .. }
            | SyntaxTree::Generic { .. }
            | SyntaxTree::TypeParameter { .. }
            | SyntaxTree::ElseIf { .. }
            | SyntaxTree::If { .. }
            | SyntaxTree::Else { .. }
            | SyntaxTree::While { .. }
            | SyntaxTree::CFor { .. }
            | SyntaxTree::DoWhile { .. }
            | SyntaxTree::For { .. }
            | SyntaxTree::Pair { .. }
            | SyntaxTree::Tuple { .. }
            | SyntaxTree::List { .. }
            | SyntaxTree::Set { .. }
            | SyntaxTree::Dictionary { .. }
            | SyntaxTree::Path { .. }
            | SyntaxTree::Call { .. }
            | SyntaxTree::Function { .. }
            | SyntaxTree::Expression { .. }
            | SyntaxTree::Module { .. }
            | SyntaxTree::Inline { .. }
            | SyntaxTree::Unknown { .. }
            | SyntaxTree::Raw { .. } => WalkerEligibility::NeedsLegacy,

        // Pure leaves — element_name + text content. The walker
        // handles these via `leaf_text_of`; no children, no flags.
        SyntaxTree::Name { .. }
            | SyntaxTree::Atom { .. }
            | SyntaxTree::Int { .. }
            | SyntaxTree::Float { .. }
            | SyntaxTree::String { .. }
            | SyntaxTree::True { .. }
            | SyntaxTree::False { .. }
            | SyntaxTree::None { .. }
            | SyntaxTree::Null { .. } => WalkerEligibility::Eligible,

        // Skip / separator atoms — emit nothing or a synthetic element.
        // The walker handles these via element_name_of (None → skip).
        SyntaxTree::Skip { .. }
            | SyntaxTree::PositionalSeparator { .. }
            | SyntaxTree::KeywordSeparator { .. } => WalkerEligibility::Eligible,

        // SimpleStatement: structural carrier that the walker handles
        // uniformly (element_name + extra_markers + children).
        SyntaxTree::SimpleStatement { .. }
            | SyntaxTree::Break { .. }
            | SyntaxTree::Continue { .. } => WalkerEligibility::Eligible,
    }
}

/// Element name to emit when projecting `tree` to XML. Returns
/// `None` for tree nodes that emit no element at all (e.g.
/// `SyntaxTree::Skip`, separator atoms, `SyntaxTree::Inline`).
pub fn element_name_of(tree: &SyntaxTree) -> Option<&'static str> {
    match tree {
        SyntaxTree::Module { element_name, .. } => Some(element_name),
        SyntaxTree::SimpleStatement { element_name, .. } => Some(element_name),
        SyntaxTree::Name { .. } => Some("name"),
        SyntaxTree::Atom { element_name, .. } => Some(element_name),
        SyntaxTree::Int { .. } => Some("int"),
        SyntaxTree::Float { .. } => Some("float"),
        SyntaxTree::String { .. } => Some("string"),
        SyntaxTree::True { .. } => Some("true"),
        SyntaxTree::False { .. } => Some("false"),
        SyntaxTree::None { .. } => Some("none"),
        SyntaxTree::Null { .. } => Some("null"),
        SyntaxTree::Break { .. } => Some("break"),
        SyntaxTree::Continue { .. } => Some("continue"),
        SyntaxTree::PositionalSeparator { .. } => Some("positional"),
        SyntaxTree::KeywordSeparator { .. } => Some("keyword"),
        SyntaxTree::Skip { .. } => None,
        // Walker-ineligible variants — the per-variant renderer is
        // still authoritative; the walker doesn't query their name.
        _ => None,
    }
}

/// Named flags this tree node should project as empty XML markers.
/// Each entry carries the marker's name and source position. The
/// renderer emits `<{name} line=… end_column=…/>`.
///
/// For walker-eligible variants this is the union of `Modifiers`
/// markers + per-variant `extra_markers` + (eventually) operator-
/// derived markers. Walker-ineligible variants return an empty Vec —
/// the legacy renderer emits their flags directly.
pub fn flags_of(tree: &SyntaxTree) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    match tree {
        SyntaxTree::SimpleStatement { modifiers, extra_markers, span, .. } => {
            for (name, marker_span) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name,
                    range: crate::tree::types::ByteRange::synthetic_empty(),
                    span: marker_span.unwrap_or(*span),
                });
            }
            for m in extra_markers {
                out.push(*m);
            }
        }
        _ => {}
    }
    out
}

/// Direct children of `tree`, in source order if anchored. The walker
/// recursively renders each child via `render_generic`.
pub fn children_of(tree: &SyntaxTree) -> Vec<&SyntaxTree> {
    match tree {
        SyntaxTree::SimpleStatement { children, .. } => children.iter().collect(),
        SyntaxTree::Module { children, .. } => children.iter().collect(),
        // Leaves have no children. Walker emits source-slice text via
        // the existing `leaf` helper.
        _ => Vec::new(),
    }
}

/// Render a tree node to XML via the variant-blind walker. Caller
/// guarantees `walker_eligibility(tree) == Eligible`; the walker
/// panics otherwise. The dispatching caller in `to_xot.rs` falls back
/// to `render_to_xot` for `NeedsLegacy` variants.
pub fn render_generic(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    debug_assert_eq!(walker_eligibility(tree), WalkerEligibility::Eligible);

    // Leaf path — emit element with source-slice text content.
    let name = match element_name_of(tree) {
        Some(n) => n,
        None => {
            // No-element variants (e.g. Skip): defer the parent's gap
            // tracking; emit nothing here. Range bookkeeping is the
            // caller's responsibility.
            return Ok(parent);
        }
    };

    let span = tree.span();
    let range = tree.range();
    let node = element_with_span(xot, name, span);
    xot.append(parent, node)?;

    // Flags → empty marker children, each at the marker's recorded span.
    for marker in flags_of(tree) {
        let m = element_with_span(xot, marker.name, marker.span);
        xot.append(node, m)?;
    }

    // Children → recurse. If a child is walker-eligible, recurse via
    // render_generic; otherwise defer to the legacy renderer.
    let kids = children_of(tree);
    if kids.is_empty() {
        // Leaf-style: emit the source-slice text content.
        let text = range.slice(source);
        if !text.is_empty() {
            let t = xot.new_text(text);
            xot.append(node, t)?;
        }
    } else {
        // Interleave gap text between children. The gap-rendering
        // logic lives in `to_xot::render_with_gaps`; we replicate the
        // contract here so the walker is self-contained.
        let mut cursor = range.start;
        for child in &kids {
            let child_range = child.range();
            emit_gap_text(xot, node, source, cursor, child_range.start)?;
            cursor = child_range.end;
            if walker_eligibility(child) == WalkerEligibility::Eligible {
                render_generic(xot, node, child, source)?;
            } else {
                super::to_xot::render_to_xot(xot, node, child, source)?;
            }
        }
        emit_gap_text(xot, node, source, cursor, range.end)?;
    }

    Ok(node)
}

fn element_with_span(xot: &mut Xot, name: &str, span: Span) -> XotNode {
    let xname = xot.add_name(name);
    let node = xot.new_element(xname);
    let line = xot.add_name("line");
    let column = xot.add_name("column");
    let end_line = xot.add_name("end_line");
    let end_column = xot.add_name("end_column");
    let id = xot.add_name("id");
    let _ = xot.set_attribute(node, line, &span.line.to_string());
    let _ = xot.set_attribute(node, column, &span.column.to_string());
    let _ = xot.set_attribute(node, end_line, &span.end_line.to_string());
    let _ = xot.set_attribute(node, end_column, &span.end_column.to_string());
    if span.id != 0 {
        let _ = xot.set_attribute(node, id, &span.id.to_string());
    }
    node
}

fn emit_gap_text(
    xot: &mut Xot,
    container: XotNode,
    source: &str,
    from: u32,
    to: u32,
) -> Result<(), xot::Error> {
    if from < to {
        let slice = &source[from as usize..to as usize];
        if !slice.is_empty() {
            let t = xot.new_text(slice);
            xot.append(container, t)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::types::{ByteRange, Modifiers, Span as S};

    /// Walker eligibility is a closed set — every variant must be
    /// either explicitly `Eligible` or explicitly `NeedsLegacy`.
    /// This test forces a compile error when a new `SyntaxTree`
    /// variant is added without an eligibility classification.
    #[test]
    fn walker_eligibility_classifies_a_representative_set() {
        let span = S::point(1, 1);
        let r = ByteRange::synthetic_empty();
        // Eligible variants — leaves + SimpleStatement.
        assert_eq!(
            walker_eligibility(&SyntaxTree::Name { text: "x".into(), range: r, span }),
            WalkerEligibility::Eligible
        );
        assert_eq!(
            walker_eligibility(&SyntaxTree::SimpleStatement {
                element_name: "x",
                modifiers: Modifiers::default(),
                extra_markers: Vec::new(),
                children: Vec::new(),
                range: r,
                span,
            }),
            WalkerEligibility::Eligible
        );
        // NeedsLegacy — variants whose renderer still synthesises
        // wrappers (Z-steps 2 and 3 will lift these into the tree).
        assert_eq!(
            walker_eligibility(&SyntaxTree::Module {
                element_name: "m",
                children: Vec::new(),
                range: r,
                span,
            }),
            WalkerEligibility::NeedsLegacy
        );
    }

    /// Smoke test: a tiny SimpleStatement renders to XML via the walker.
    #[test]
    fn walker_renders_simple_statement_leaf() {
        let span = S::point(1, 1);
        let range = ByteRange::synthetic(0, 3);
        let tree = SyntaxTree::SimpleStatement {
            element_name: "thing",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![SyntaxTree::Name {
                text: "abc".into(),
                range: ByteRange::synthetic(0, 3),
                span,
            }],
            range,
            span,
        };
        let source = "abc";
        let mut xot = Xot::new();
        let doc_name = xot.add_name("doc");
        let doc = xot.new_element(doc_name);
        render_generic(&mut xot, doc, &tree, source).expect("render");
        let xml = xot.to_string(doc).unwrap();
        assert!(xml.contains("<thing"), "missing <thing>: {xml}");
        assert!(xml.contains("<name"), "missing <name>: {xml}");
    }
}
