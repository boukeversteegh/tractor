//! Per-kind transformations for Ruby.
//!
//! Each function is a `Rule::Custom` target — `rule(RubyKind) -> Rule`
//! references these by name. Simple flattens / pure renames live as
//! data in `rule()` (see `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::input::RubyKind;
use super::output::TractorNode::{self, Comment as CommentName, Leading, Name, Parameter, Trailing};

/// `method_parameters` / `block_parameters` / `lambda_parameters` —
/// wrap bare `identifier` children in `<parameter>` so cross-language
/// `//parameter` finds positional bare params (Principle #5). Other
/// param kinds (`keyword_parameter`, `optional_parameter`,
/// `splat_parameter`, `hash_splat_parameter`, `block_parameter`)
/// already get their own custom rule handling and are left alone.
pub fn parameters(
    xot: &mut Xot,
    node: XotNode,
    distribute_field: Option<&str>,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<XotNode> = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if get_kind(xot, child).as_deref() == Some("identifier") {
            let param_name_id = xot.add_name(Parameter.as_str());
            let param = xot.new_element(param_name_id);
            xot.with_source_location_from(param, child);
            xot.insert_before(child, param)?;
            xot.detach(child)?;
            xot.append(param, child)?;
        }
    }
    if let Some(field) = distribute_field {
        distribute_field_to_children(xot, node, field);
    }
    Ok(TransformAction::Flatten)
}

/// `method_parameters` adapter — distributes `parameters` field after
/// wrapping bare identifiers (matches the previous Flatten rule).
pub fn method_parameters(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    parameters(xot, node, Some("parameters"))
}

/// `block_parameters` / `lambda_parameters` adapter — no field
/// distribution (matches the previous bare-Flatten rule).
pub fn block_parameters(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    parameters(xot, node, None)
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
        let child_kind = get_element_name(xot, child)
            .and_then(|name| name.parse::<RubyKind>().ok());
        let child_name = get_element_name(xot, child)
            .and_then(|name| name.parse::<TractorNode>().ok());
        if matches!(
            child_kind,
            Some(RubyKind::Identifier | RubyKind::Constant | RubyKind::Operator)
        ) || child_name == Some(Name) {
            if let Some(text) = get_text_content(xot, child) {
                xot.with_only_text(node, &text)?;
                return Ok(TransformAction::Done);
            }
        }
    }
    Ok(TransformAction::Continue)
}

/// `comment` — Ruby uses `#` for line comments. Rename and run the
/// shared classifier.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["#"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}
