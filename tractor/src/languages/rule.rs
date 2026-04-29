//! Per-kind transformation rule: declarative table that drives a
//! language's dispatcher.
//!
//! Each language declares a `rule(<Lang>Kind) -> Rule` exhaustive
//! match. The compiler enforces coverage of every grammar kind; the
//! shared [`dispatch`] helper executes whichever variant the rule
//! produced.
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

use xot::{Xot, Node as XotNode};

use crate::transform::TransformAction;
use crate::transform::helpers::{
    distribute_field_to_children, prepend_empty_element, rename,
};
use crate::transform::operators::extract_operator;

/// What to do with a tree-sitter node identified by its `Kind` enum.
#[derive(Clone, Copy)]
pub enum Rule {
    /// Rename the node to `to`. No marker, no structural change.
    Rename(&'static str),
    /// Rename to `to` and prepend an empty `marker` element as the
    /// first child.
    RenameWithMarker(&'static str, &'static str),
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
    ExtractOpThenRename(&'static str),
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
        to: &'static str,
        default_access: fn(&Xot, XotNode) -> Option<&'static str>,
    },
    /// Run the given handler. The function owns the renaming, child
    /// reshaping, and `TransformAction` choice.
    Custom(fn(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>),
}

/// Execute a [`Rule`] on a node. Shared dispatcher used by every
/// language that opts into the rule-driven shape.
pub fn dispatch(
    xot: &mut Xot,
    node: XotNode,
    rule: Rule,
) -> Result<TransformAction, xot::Error> {
    match rule {
        Rule::Rename(to) => {
            rename(xot, node, to);
            Ok(TransformAction::Continue)
        }
        Rule::RenameWithMarker(to, marker) => {
            rename(xot, node, to);
            prepend_empty_element(xot, node, marker)?;
            Ok(TransformAction::Continue)
        }
        Rule::Flatten { distribute_field } => {
            if let Some(field) = distribute_field {
                distribute_field_to_children(xot, node, field);
            }
            Ok(TransformAction::Flatten)
        }
        Rule::ExtractOpThenRename(to) => {
            extract_operator(xot, node)?;
            rename(xot, node, to);
            Ok(TransformAction::Continue)
        }
        Rule::DefaultAccessThenRename { to, default_access } => {
            if let Some(marker) = default_access(xot, node) {
                prepend_empty_element(xot, node, marker)?;
            }
            rename(xot, node, to);
            Ok(TransformAction::Continue)
        }
        Rule::Custom(f) => f(xot, node),
    }
}
