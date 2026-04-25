//! Shared comment classification + grouping for every language with
//! line / block comments.
//!
//! Single source of truth for the `<trailing/>` / `<leading/>` markers
//! and the "merge consecutive line comments" rule. Each language plugs
//! in its own line-comment prefix(es) (e.g. `//` for C-family, `#` for
//! Python/Ruby, both for PHP) and the marker element names (typically
//! the language's `TRAILING` / `LEADING` constants).
//!
//! Classification rules:
//!   `<trailing/>` — comment starts on the same line as the previous
//!                   sibling's end (i.e. an inline trailing comment).
//!   `<leading/>`  — comment ends on the line immediately before the
//!                   next non-comment sibling (no blank-line gap).
//!   (no marker)   — floating / standalone comment.
//!
//! Grouping: consecutive line comments (matching one of the configured
//! prefixes) on adjacent lines are merged into a single `<comment>`
//! with multiline text content. Block comments are never grouped.
//!
//! Note: this module owns ONLY classification + grouping. Trailing-comment
//! ADOPTION (moving the comment INTO the predecessor element as a child)
//! is intentionally NOT done here — the comment stays a sibling.

use xot::{Xot, Node as XotNode};

use crate::xot_transform::TransformAction;
use crate::xot_transform::helpers::*;

/// Per-language configuration: which token(s) introduce a line comment.
///
/// Use `&["//"]` for C-family, `&["#"]` for Python/Ruby, `&["//", "#"]`
/// for PHP (which accepts both styles). The list is consulted by the
/// grouping pass: only sibling comments whose text starts with one of
/// these prefixes participate in line-comment merging.
pub struct CommentClassifier {
    pub line_prefixes: &'static [&'static str],
}

impl CommentClassifier {
    /// Classify `node` (a `<comment>`-renamed element) and group adjacent
    /// line-comment siblings into it.
    ///
    /// Returns `TransformAction::Done` — the caller should NOT recurse
    /// into a comment, and the dispatch arm has already done the rename.
    /// Consumed siblings are detached internally.
    pub fn classify_and_group(
        &self,
        xot: &mut Xot,
        node: XotNode,
        trailing_name: &'static str,
        leading_name: &'static str,
    ) -> Result<TransformAction, xot::Error> {
        // Skip if already consumed by a preceding comment's grouping
        if xot.parent(node).is_none() {
            return Ok(TransformAction::Done);
        }

        // Trailing comments are attached to the previous sibling — no grouping
        if is_inline_node(xot, node) {
            prepend_empty_element(xot, node, trailing_name)?;
            return Ok(TransformAction::Done);
        }

        // Group consecutive line comments into this node
        let consumed = self.group_line_comments(xot, node)?;

        // Classify the (possibly merged) comment
        if is_leading_comment(xot, node) {
            prepend_empty_element(xot, node, leading_name)?;
        }

        // Detach consumed siblings (they've been merged into this node)
        for sibling in consumed {
            xot.detach(sibling)?;
        }

        Ok(TransformAction::Done)
    }

    /// True iff `text` begins (after trim) with one of the configured
    /// line-comment prefixes.
    fn is_line_comment_text(&self, text: &str) -> bool {
        let trimmed = text.trim_start();
        self.line_prefixes.iter().any(|p| trimmed.starts_with(p))
    }

    /// Group consecutive line comments on adjacent lines into a single
    /// comment node. Merges the text content of following comment
    /// siblings into `node` and returns the consumed sibling nodes
    /// (caller should detach them after classification).
    ///
    /// Only groups line comments (matching one of `line_prefixes`); block
    /// comments are never merged.
    fn group_line_comments(
        &self,
        xot: &mut Xot,
        node: XotNode,
    ) -> Result<Vec<XotNode>, xot::Error> {
        let text = match get_text_content(xot, node) {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };

        // Only group line comments (matching configured prefixes)
        if !self.is_line_comment_text(&text) {
            return Ok(Vec::new());
        }

        let mut end_line = match get_line(xot, node, "end_line") {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };
        let mut end_column = get_attr(xot, node, "end_column")
            .unwrap_or_else(|| "1".to_string());

        let mut consumed: Vec<XotNode> = Vec::new();
        let mut merged_text = text.clone();

        // Walk following siblings looking for adjacent line comments
        let following: Vec<XotNode> = xot.following_siblings(node)
            .filter(|&s| s != node && xot.element(s).is_some())
            .collect();

        for sibling in following {
            let sibling_kind = match get_kind(xot, sibling) {
                Some(k) => k,
                None => break,
            };
            if !is_comment_kind(&sibling_kind) {
                break;
            }

            let sibling_text = match get_text_content(xot, sibling) {
                Some(t) => t,
                None => break,
            };

            // Must also be a line comment of the same family
            if !self.is_line_comment_text(&sibling_text) {
                break;
            }

            let sibling_start_line = match get_line(xot, sibling, "line") {
                Some(l) => l,
                None => break,
            };

            // Must be on the very next line (adjacent)
            if sibling_start_line != end_line + 1 {
                break;
            }

            // Merge: append text with newline
            merged_text.push('\n');
            merged_text.push_str(&sibling_text);

            // Update end line to the consumed sibling's end
            end_line = get_line(xot, sibling, "end_line").unwrap_or(end_line + 1);
            end_column = get_attr(xot, sibling, "end_column")
                .unwrap_or_else(|| end_column.clone());

            consumed.push(sibling);
        }

        if !consumed.is_empty() {
            // Replace text content of node with merged text
            let text_children: Vec<XotNode> = xot.children(node)
                .filter(|&c| xot.text_str(c).is_some())
                .collect();
            for child in text_children {
                xot.detach(child)?;
            }
            let new_text = xot.new_text(&merged_text);
            xot.append(node, new_text)?;

            // Update end attribute to reflect the last consumed comment
            set_attr(xot, node, "end_line", &end_line.to_string());
            set_attr(xot, node, "end_column", &end_column);
        }

        Ok(consumed)
    }
}

/// True iff a comment (or merged comment block) immediately precedes a
/// non-comment sibling. "Immediately" means the next non-comment element
/// sibling starts on the line right after this comment ends, with no
/// blank-line gap.
///
/// Shared across languages — comment kind detection covers tree-sitter's
/// `comment`, `line_comment`, `block_comment`, and Rust's `doc_comment`.
fn is_leading_comment(xot: &Xot, node: XotNode) -> bool {
    let comment_end_line = match get_line(xot, node, "end_line") {
        Some(l) => l,
        None => return false,
    };

    // Find next element sibling that is NOT a comment
    let next = xot.following_siblings(node)
        .filter(|&s| s != node)
        .find(|&s| {
            xot.element(s).is_some()
                && !get_kind(xot, s).as_deref().map(is_comment_kind).unwrap_or(false)
        });

    match next {
        Some(next) => {
            let next_start_line = get_line(xot, next, "line").unwrap_or(0);
            // Next declaration starts on the very next line (no blank-line gap)
            next_start_line == comment_end_line + 1
        }
        None => false,
    }
}

/// Tree-sitter comment kinds across all supported languages.
fn is_comment_kind(kind: &str) -> bool {
    matches!(kind, "comment" | "line_comment" | "block_comment" | "doc_comment")
}
