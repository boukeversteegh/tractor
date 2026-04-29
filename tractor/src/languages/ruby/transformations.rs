//! Per-kind transformations for Ruby.
//!
//! Each function is a `Rule::Custom` target — `rule(RubyKind) -> Rule`
//! references these by name. Simple flattens / pure renames live as
//! data in `rule()` (see `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::output::*;

/// Kinds whose name happens to match our semantic vocabulary already
/// (`block`, `break`, `conditional`, `constant`, `do`, `false`, `in`,
/// `interpolation`, `lambda`, `next`, `nil`, `operator`, `pair`,
/// `pattern`, `range`, `redo`, `regex`, `retry`, `self`, `superclass`,
/// `then`, `true`, `unary`, `when`, `yield`, `exceptions`).
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder. Inline single
/// identifier / constant / operator child as text. Operators apply
/// for `def ==(other)` and friends (Ruby's tree-sitter tags the
/// operator token as an element inside `<name>`).
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    let element_children: Vec<_> = children
        .iter()
        .copied()
        .filter(|&c| xot.element(c).is_some())
        .collect();
    if element_children.len() == 1 {
        let child = element_children[0];
        let child_name = get_element_name(xot, child).unwrap_or_default();
        if matches!(child_name.as_str(), "identifier" | "constant" | "name" | "operator") {
            if let Some(text) = get_text_content(xot, child) {
                for c in children {
                    xot.detach(c)?;
                }
                let text_node = xot.new_text(&text);
                xot.append(node, text_node)?;
                return Ok(TransformAction::Done);
            }
        }
    }
    Ok(TransformAction::Continue)
}

/// `comment` — Ruby uses `#` for line comments. Rename and run the
/// shared classifier.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, COMMENT);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["#"] };
    CLASSIFIER.classify_and_group(xot, node, TRAILING, LEADING)
}
