//! Conditional collapser transformer.
//!
//! Collapses nested `else { if ... }` chains (and Ruby's nested
//! `<elsif>` shape) into the flat `<if>[condition][then][else_if*][else?]`
//! shape called for by the semantic-tree spec. Languages call
//! [`collapse_else_if_chain`] from their post-transform passes.
//!
//! ## DO NOT RENAME `<else_if>`
//!
//! The element name `else_if` is the canonical *allowed exception*
//! to Principle #17 (Avoid Compound Node Names) in
//! `specs/tractor-parse/semantic-tree/design.md` § 17:
//!
//! > The bar is high — the obvious test case is `else_if`, where
//! > the concept is genuinely the *combination* of two keywords
//! > and neither half alone names it. Rare; expect to justify
//! > each one individually.
//!
//! Iter 252 renamed it to `<elseif>` based on a misread of the
//! Principle #2 underscore rule and was reverted (commit
//! 85e58e29). If you're considering renaming this name to
//! `<elseif>` / `<elif>` / nesting `<else>/<if>/...` etc., DO NOT
//! — go re-read design.md § 17 first.

use xot::{Xot, Node as XotNode};

use super::helpers::{copy_source_location, get_element_children, get_element_name, rename};

// /specs/tractor-parse/semantic-tree/transformations.md: Conditional shape
/// Collapse a nested `else`/`if` chain under `if_node` into the flat
/// conditional shape `<if>[condition][then][else_if*][else?]`.
///
/// Applies after names have been rewritten, so children are already
/// `<if>` / `<else>` / `<else_if>` / `<elsif>` (the raw Ruby kind).
/// Handles two input shapes:
///
/// - **C-like** (JS/TS, Java, C#, Go, Rust) — the `<else>` field
///   wrapper holds a renamed `else_clause` (itself `<else>`). If the
///   inner `<else>` contains a single `<if>`, that `<if>`'s
///   condition and then become a new `<else_if>` sibling; the
///   nested `<if>`'s own `<else>` chain continues. Final `<else>`
///   with a plain block stays.
/// - **Ruby** — the grammar already emits `<elsif>` (nested) and
///   a final `<else>`. The `<elsif>` is renamed to `<else_if>` and
///   lifted out so it becomes a sibling of the outer `<if>`'s
///   condition/then; the same for any nested `<else>`.
pub fn collapse_else_if_chain(xot: &mut Xot, if_node: XotNode) -> Result<(), xot::Error> {
    // Walk the `<else>` / `<elsif>` chain, lifting each level out
    // of the previous one so they become flat children of
    // `if_node`. `current` is the node we scan for a trailing
    // alternative (initially `if_node`; later each lifted
    // `<else_if>`). `anchor` is the child of `if_node` that the
    // next lifted alternative should be inserted *after* — None
    // means "append at the end" (modulo trailing text). Before
    // each step we normalize the C-like `<else>` wrapper around a
    // renamed `else_clause` (also `<else>`) into a single `<else>`
    // child.
    let mut current = if_node;
    let mut anchor: Option<XotNode> = None;
    loop {
        // Find the trailing alternative child (else / elsif / else_if)
        // on the current node.
        let alt = match find_trailing_alternative(xot, current) {
            Some(a) => a,
            None => break,
        };
        let alt_name = get_element_name(xot, alt).unwrap_or_default();

        match alt_name.as_str() {
            "else" => {
                // Before finishing, check whether this `<else>` holds
                // only a single `<if>` (else if in C-like shape).
                // The `<else>` wraps an `<if>` directly (C-like else-if chain).
                let inner_if = single_if_child(xot, alt);
                if let Some(inner_if) = inner_if {
                    // Grab the "else" keyword text so it survives
                    // inside the new <else_if>. Tree-sitter grammars
                    // put it in one of two places:
                    //   Rust-style: text child of the <else> wrapper
                    //     itself (tree-sitter emits else_clause →
                    //     <else> containing "else" + inner if).
                    //   Java/C#/Go-style: preceding text sibling of
                    //     the <else> wrapper we added via
                    //     wrap_field_child (the original source has
                    //     no else_clause node; the keyword is a text
                    //     child of the outer if).
                    let else_text = collect_else_keyword_text(xot, alt)?;

                    let else_if = lift_if_as_else_if(xot, if_node, anchor, inner_if)?;

                    if let Some(content) = else_text {
                        let new_text = xot.new_text(&content);
                        xot.prepend(else_if, new_text)?;
                    }

                    xot.detach(alt)?; // drop the now-empty <else>
                    current = else_if;
                    anchor = Some(else_if);
                    continue;
                }
                // Terminal <else>: if the "else" keyword lives as a
                // preceding text sibling on the outer if (Java/C#/Go
                // shape), move it inside the wrapper before
                // reparenting — Rust's shape already has the text
                // inside the wrapper.
                if let Some(content) = take_preceding_text_sibling(xot, alt)? {
                    let new_text = xot.new_text(&content);
                    xot.prepend(alt, new_text)?;
                }
                reparent_in(xot, alt, if_node, anchor)?;
                break;
            }
            "elsif" | "else_if" => {
                // Ruby's <elsif> (or any previously-renamed
                // <else_if>). Same preceding-text fold as above.
                if let Some(content) = take_preceding_text_sibling(xot, alt)? {
                    let new_text = xot.new_text(&content);
                    xot.prepend(alt, new_text)?;
                }
                rename(xot, alt, "else_if");
                reparent_in(xot, alt, if_node, anchor)?;
                current = alt;
                anchor = Some(alt);
            }
            _ => break,
        }
    }

    Ok(())
}

/// Detach the immediately preceding text sibling of `node` (if
/// any) and return its content. Used to fold the "else" keyword
/// that tree-sitter emits as a sibling of the alternative branch
/// into the branch element itself, so the chain's text reads as
/// a continuous source token rather than splitting across the
/// sibling boundary.
fn take_preceding_text_sibling(
    xot: &mut Xot,
    node: XotNode,
) -> Result<Option<String>, xot::Error> {
    let Some(prev) = xot.previous_sibling(node) else {
        return Ok(None);
    };
    let Some(content) = xot.text_str(prev).map(|s| s.to_string()) else {
        return Ok(None);
    };
    xot.detach(prev)?;
    Ok(Some(content))
}

/// Collect the "else" keyword text that belongs to an `<else>`
/// wrapper, handling both tree-sitter shapes:
///
/// - **Rust-style** (else_clause → <else>): the keyword is a
///   text child of the wrapper itself. Detach and return it.
/// - **Java/C#/Go-style** (our wrap_field_child wrapped the
///   alternative): the keyword sits as a text sibling BEFORE
///   the wrapper on the outer if.
///
/// Returns the concatenated text content so the caller can
/// prepend it into the new <else_if> / <else>, which then
/// consolidates with adjacent "if (" text for a single
/// "else if (" source token.
fn collect_else_keyword_text(
    xot: &mut Xot,
    else_wrapper: XotNode,
) -> Result<Option<String>, xot::Error> {
    // Case 1: text children of the wrapper itself (Rust shape).
    let inner_texts: Vec<XotNode> = xot
        .children(else_wrapper)
        .filter(|&c| xot.text_str(c).is_some())
        .collect();
    if !inner_texts.is_empty() {
        let mut parts: Vec<String> = Vec::new();
        for t in inner_texts {
            if let Some(s) = xot.text_str(t) {
                parts.push(s.to_string());
            }
            xot.detach(t)?;
        }
        return Ok(Some(parts.join("")));
    }
    // Case 2: preceding text sibling of the wrapper (Java / C# / Go).
    take_preceding_text_sibling(xot, else_wrapper)
}

/// Return the last element child of `node` whose name is `else`,
/// `elsif`, or `else_if` — the tail of the conditional chain.
fn find_trailing_alternative(xot: &Xot, node: XotNode) -> Option<XotNode> {
    let children = get_element_children(xot, node);
    let last = *children.last()?;
    match get_element_name(xot, last).as_deref() {
        Some("else") | Some("elsif") | Some("else_if") => Some(last),
        _ => None,
    }
}

/// If `else_node` has exactly one element child and that child is
/// `<if>`, return it. Used to detect the "else if" C-like shape.
fn single_if_child(xot: &Xot, else_node: XotNode) -> Option<XotNode> {
    let children = get_element_children(xot, else_node);
    if children.len() != 1 {
        return None;
    }
    let only = children[0];
    if get_element_name(xot, only).as_deref() == Some("if") {
        Some(only)
    } else {
        None
    }
}

/// Build an `<else_if>` from `inner_if`'s condition/then and place
/// it as a child of `outer_if`, positioned after `after` (or at
/// the end of `outer_if` when `after` is `None`). The inner
/// `<if>`'s own `<else>` / `<elsif>` chain is moved into the new
/// `<else_if>` so the caller can continue iterating. Returns the
/// new `<else_if>` node.
fn lift_if_as_else_if(
    xot: &mut Xot,
    outer_if: XotNode,
    after: Option<XotNode>,
    inner_if: XotNode,
) -> Result<XotNode, xot::Error> {
    let else_if_name = xot.add_name("else_if");
    let else_if = xot.new_element(else_if_name);
    copy_source_location(xot, inner_if, else_if);

    // Insert the new `<else_if>` as a child of `outer_if`, placed
    // right after `after` so the chain reads in source order.
    match after {
        Some(a) => {
            let next = xot.next_sibling(a);
            match next {
                Some(n) => xot.insert_before(n, else_if)?,
                None => xot.append(outer_if, else_if)?,
            }
        }
        None => xot.append(outer_if, else_if)?,
    }

    // Move condition / then element children AND all text children
    // (source keywords like "if") from the inner <if> to the new
    // <else_if> so its XPath string-value stays source-accurate
    // (`"if (n==0) { ... }"` rather than just the condition + body).
    let inner_children: Vec<_> = xot.children(inner_if).collect();
    for child in inner_children {
        if xot.text_str(child).is_some() {
            xot.detach(child)?;
            xot.append(else_if, child)?;
            continue;
        }
        let name = get_element_name(xot, child).unwrap_or_default();
        match name.as_str() {
            "condition" | "then" => {
                xot.detach(child)?;
                xot.append(else_if, child)?;
            }
            _ => {}
        }
    }

    // The inner <if>'s remaining alternative children (its own
    // <else> / <elsif> chain) now belong semantically to the new
    // <else_if>'s tail. Move them under `else_if` so the caller
    // can continue iterating via `find_trailing_alternative`.
    let remaining = get_element_children(xot, inner_if);
    for child in remaining {
        let name = get_element_name(xot, child).unwrap_or_default();
        if matches!(name.as_str(), "else" | "elsif" | "else_if") {
            xot.detach(child)?;
            xot.append(else_if, child)?;
        }
    }

    Ok(else_if)
}

/// Detach `node` from its current parent and place it as a child
/// of `parent`, positioned immediately after `after` when set, or
/// at the end of `parent` when `after` is `None`. No-op if `node`
/// is already positioned correctly.
fn reparent_in(
    xot: &mut Xot,
    node: XotNode,
    parent: XotNode,
    after: Option<XotNode>,
) -> Result<(), xot::Error> {
    // Already in the right position?
    if xot.parent(node) == Some(parent) {
        match after {
            Some(a) if xot.next_sibling(a) == Some(node) => return Ok(()),
            None if xot.next_sibling(node).is_none() => return Ok(()),
            _ => {}
        }
    }
    xot.detach(node)?;
    match after {
        Some(a) => {
            let next = xot.next_sibling(a);
            match next {
                Some(n) => xot.insert_before(n, node)?,
                None => xot.append(parent, node)?,
            }
        }
        None => xot.append(parent, node)?,
    }
    Ok(())
}
