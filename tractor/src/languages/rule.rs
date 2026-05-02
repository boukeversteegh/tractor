//! Per-kind transformation rule: declarative table that drives a
//! language's dispatcher.
//!
//! Each language declares a `rule(<Lang>Kind) -> Rule<<Lang>Name>`
//! exhaustive match. The compiler enforces coverage of every grammar
//! kind; the shared [`dispatch`] helper executes whichever variant the
//! rule produced.
//!
//! Variants split into two roles:
//!   - **Pure data** (`Rename`, `RenameWithMarker`, `Flatten`): the
//!     dispatcher executes them directly. No language code needed.
//!   - **Composed pattern** (`ExtractOpThenRename`): a multi-step
//!     transform that several languages share (binary / unary
//!     expressions). Promoted to a variant so rule tables read as
//!     data; the dispatcher knows the steps.
//!   - **Escape hatch** (`Custom`): a function pointer for
//!     language-specific logic that doesn't fit a shared shape. The
//!     handler module owns the body.
//!
//! New shared variants are added when a *second* language wants the
//! same composition — no speculative variants.
//!
//! `N` is the language's output-name enum (e.g. `TractorNode`) — typically
//! a `Copy` enum that converts to `&'static str` via strum's
//! `IntoStaticStr`. Carrying `N` instead of `&'static str` makes typos
//! a compile error and lets call sites write `Type` (with
//! `use TractorNode::*`) instead of stringy constants.

use xot::{Xot, Node as XotNode};

use crate::transform::TransformAction;
use crate::transform::helpers::{
    distribute_list_to_children, XotWithExt,
};
use crate::transform::operators::extract_operator;

/// What to do with a tree-sitter node identified by its `Kind` enum.
#[derive(Clone, Copy)]
pub enum Rule<N> {
    /// Rename the node to `to`. No marker, no structural change.
    Rename(N),
    /// Rename to `to` and prepend an empty `marker` element as the
    /// first child.
    RenameWithMarker(N, N),
    /// Strip text children whose content equals `keyword` (with
    /// optional trailing `;`), then rename the node to `to`. Used for
    /// bare keyword statements where tree-sitter emits the keyword as
    /// an anonymous text leaf inside the statement element (e.g.
    /// `<break>break;</break>` → `<break/>`). Element children are
    /// untouched, so labelled `break LABEL;` and `return value;` are
    /// preserved.
    RenameStripKeyword(N, &'static str),
    /// Drop the wrapper, promote children to siblings. If
    /// `distribute_field` is `Some`, set `field=<name>` on every child
    /// before flattening (so the children are still grouped under a
    /// uniform field).
    Flatten {
        distribute_field: Option<&'static str>,
    },
    /// Find the operator text inside the node, prepend it as an `<op>`
    /// child via [`extract_operator`], then rename the node to `to`.
    /// Used by binary / unary / assignment expressions across several
    /// languages.
    ExtractOpThenRename(N),
    /// If the node lacks an explicit access modifier (per the language-
    /// specific `default_access` resolver), prepend a default-access
    /// marker. Then rename to `to`. Used for declaration kinds (class,
    /// interface, method, field, …) where languages have implicit
    /// access defaults.
    ///
    /// `default_access(xot, node)` returns:
    ///   - `Some(marker_name)` if the node lacks an access modifier
    ///     and should get the given default marker
    ///   - `None` if the node already has an access modifier
    DefaultAccessThenRename {
        to: N,
        default_access: fn(&Xot, XotNode) -> Option<N>,
    },
    /// Detach the node entirely (children gone with it) and stop
    /// recursion. Distinct from `Flatten` (children promoted) and
    /// `Skip` (children promoted before recursion). Used for
    /// purely-syntactic leaves the source text already carries
    /// — e.g. tsql's hundreds of `keyword_*` reserved words.
    Detach,
    /// Leave the node untouched: keep its raw grammar kind as the
    /// element name and continue walking children. Used for kinds the
    /// transform has not yet rewritten — usually grammar supertypes
    /// (`expression`, `pattern`) or kinds whose name already matches
    /// our vocabulary (`pair`, `tuple`, `interpolation`).
    ///
    /// A passthrough kind whose name contains an underscore is a
    /// drift signal: the raw grammar string leaks through into the
    /// transformed tree. The
    /// `no_underscore_in_node_names_except_whitelist` invariant in
    /// `tractor/tests/tree_invariants.rs` enumerates every language's
    /// rule table and gates on this.
    Passthrough,
    /// Run the given handler. The function owns the renaming, child
    /// reshaping, and `TransformAction` choice.
    Custom(fn(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>),
}

impl<N> Rule<N> {
    /// True iff this rule leaves the element name as-is. Used by
    /// invariants that need to know which kinds bleed through their
    /// raw grammar name.
    pub fn is_passthrough(&self) -> bool {
        matches!(self, Rule::Passthrough)
    }
}

/// Enumerate every kind whose `rule()` is `Passthrough` — i.e. the
/// raw grammar kind name survives into the transformed tree. Used by
/// the `no_underscore_in_node_names_except_whitelist` invariant in
/// `tractor/tests/tree_invariants.rs` to gate underscored grammar
/// kinds at the table layer (rather than waiting for them to surface
/// in fixture output).
///
/// `K` is a language's `Kind` enum (derives `EnumIter` + `IntoStaticStr`
/// with snake_case). `N` is irrelevant here — we only inspect the
/// rule's variant.
pub fn passthrough_kinds<K, N>(rule_fn: fn(K) -> Rule<N>) -> Vec<&'static str>
where
    K: strum::IntoEnumIterator + Into<&'static str> + Copy,
{
    K::iter()
        .filter_map(|k| rule_fn(k).is_passthrough().then(|| k.into()))
        .collect()
}

/// Execute a [`Rule`] on a node. Shared dispatcher used by every
/// language that opts into the rule-driven shape.
pub fn dispatch<N>(
    xot: &mut Xot,
    node: XotNode,
    rule: Rule<N>,
) -> Result<TransformAction, xot::Error>
where
    N: Copy + AsRef<str>,
{
    match rule {
        Rule::Rename(to) => {
            xot.with_renamed(node, to);
            Ok(TransformAction::Continue)
        }
        Rule::RenameWithMarker(to, marker) => {
            // The marker is tied to the renamed element's source range
            // (e.g. `<member[optional]>` for `?.` carries the `?.`
            // operator's location). Copy `node`'s line/column onto the
            // marker (Principle #10).
            xot.with_renamed(node, to)
                .with_prepended_marker_from(node, marker, node)?;
            Ok(TransformAction::Continue)
        }
        Rule::RenameStripKeyword(to, keyword) => {
            crate::transform::helpers::strip_keyword_text(xot, node, keyword)?;
            xot.with_renamed(node, to);
            Ok(TransformAction::Continue)
        }
        Rule::Flatten { distribute_field } => {
            if let Some(field) = distribute_field {
                distribute_list_to_children(xot, node, field);
            }
            Ok(TransformAction::Flatten)
        }
        Rule::ExtractOpThenRename(to) => {
            extract_operator(xot, node)?;
            xot.with_renamed(node, to);
            Ok(TransformAction::Continue)
        }
        Rule::DefaultAccessThenRename { to, default_access } => {
            if let Some(marker) = default_access(xot, node) {
                xot.with_prepended_empty_element(node, marker)?;
            }
            xot.with_renamed(node, to);
            Ok(TransformAction::Continue)
        }
        Rule::Detach => {
            // If the parent is a field-distribution wrapper element
            // that becomes empty after we remove `node`, detach the
            // wrapper too. Catches the TSQL `cast/<name/>` case where
            // a `keyword_cast` was wrapped in `<name>` by the
            // field-distribution pass and Detach leaves `<name/>`
            // behind.
            let parent = xot.parent(node).filter(|&p| xot.element(p).is_some());
            xot.detach(node)?;
            if let Some(parent) = parent {
                let has_field = crate::transform::helpers::get_attr(xot, parent, "field").is_some();
                let has_any_child = xot.children(parent).next().is_some();
                if has_field && !has_any_child {
                    xot.detach(parent)?;
                }
            }
            Ok(TransformAction::Done)
        }
        Rule::Passthrough => Ok(TransformAction::Continue),
        Rule::Custom(f) => f(xot, node),
    }
}
