//! Per-kind transformations for Ruby.
//!
//! Each function is a `Rule::Custom` target — `rule(RubyKind) -> Rule`
//! references these by name. Simple flattens / pure renames live as
//! data in `rule()` (see `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::input::RubyKind;
use super::output::TractorNode::{
    self, Block as RubyBlock, Call, Comment as CommentName, Do, Leading, Name, Optional, Parameter,
    Trailing,
};

/// `block` (`{ |x| ... }`) and `do_block` (`do |x| ... end`) — both
/// are call-attached closures in Ruby. Re-tag the `<body>` wrapper
/// to `<value>` for single-statement bodies so cross-language
/// `//block/value/expression/...` works (Principle #15; mirrors iter
/// 161/162/167/168 closure archetype).
///
/// Multi-statement bodies keep `<body>` so per-statement `list=`
/// distribution remains visible (multiple peer children with
/// `list="expression"` etc.). The discriminator: a single element
/// child under `<body>` whose kind isn't itself a `block` →
/// re-tag; otherwise keep.
fn retag_single_stmt_body_as_value(xot: &mut Xot, node: XotNode) {
    let body_child = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
    let body = match body_child { Some(b) => b, None => return };
    let elem_children: Vec<XotNode> = xot.children(body)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    if elem_children.len() != 1 { return; }
    let inner_kind = get_kind(xot, elem_children[0]);
    // Skip if the inner element is a nested closure (the lambda case
    // with a doubled body wrapper). `block_body` / `body_statement`
    // are just statement-list intermediates that flatten away later
    // in the walk; treating them as "single-statement" is correct.
    if matches!(inner_kind.as_deref(), Some("block" | "do_block")) {
        return;
    }
    let value_id = xot.add_name("value");
    if let Some(elem) = xot.element_mut(body) {
        elem.set_name(value_id);
    }
}

pub fn block(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    retag_single_stmt_body_as_value(xot, node);
    Ok(TransformAction::Continue)
}

pub fn do_block(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    retag_single_stmt_body_as_value(xot, node);
    xot.with_renamed(node, RubyBlock)
        .with_prepended_marker(node, Do)?;
    Ok(TransformAction::Continue)
}

/// `call` — `obj.method(...)` or `obj&.method(...)` (safe-navigation).
/// Tree-sitter Ruby uses ONE kind for both; the only difference is
/// the operator text (`.` vs `&.`). Detect `&.` and add an
/// `<optional/>` marker so cross-language `//call[optional]` finds
/// Ruby safe-navigation (matches C# `?.` shape from iter 57).
pub fn call_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let texts = get_text_children(xot, node);
    let is_safe_nav = texts.iter().any(|t| t.contains("&."));
    if is_safe_nav {
        xot.with_prepended_marker(node, Optional)?;
    }
    xot.with_renamed(node, Call);
    Ok(TransformAction::Continue)
}

/// `method_parameters` / `block_parameters` / `lambda_parameters` —
/// wrap bare `identifier` children in `<parameter>` so cross-language
/// `//parameter` finds positional bare params (Principle #5). Other
/// param kinds (`keyword_parameter`, `optional_parameter`,
/// `splat_parameter`, `hash_splat_parameter`, `block_parameter`)
/// already get their own custom rule handling and are left alone.
pub fn parameters(
    xot: &mut Xot,
    node: XotNode,
    distribute_list: Option<&str>,
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
    if let Some(field) = distribute_list {
        distribute_list_to_children(xot, node, field);
    }
    Ok(TransformAction::Flatten)
}

/// `method_parameters` adapter — distributes `parameters` field after
/// wrapping bare identifiers (matches the previous Flatten rule).
pub fn method_parameters(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    parameters(xot, node, Some("parameters"))
}

/// `block_parameters` / `lambda_parameters` adapter — distributes
/// the same `parameters` field as method_parameters so within-Ruby
/// (Principle #5) block params and method params share one shape.
/// Each parameter ends up with `list="parameters"` so
/// JSON output is uniformly an array regardless of count.
pub fn block_parameters(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    parameters(xot, node, Some("parameters"))
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

/// `superclass` — `class Foo < Base`. Renames to `<extends>` with
/// `list="extends"` so JSON serializers produce a uniform
/// `extends: [...]` array (Principle #12 + #18).
pub fn superclass(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::Extends;
    xot.with_renamed(node, Extends)
        .with_attr(node, "list", "extends");
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
