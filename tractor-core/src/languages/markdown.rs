//! Markdown transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a Markdown AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Extract heading level from the marker child and set as attribute
        "atx_heading" => {
            // Determine heading level from the marker child (atx_h1_marker..atx_h6_marker)
            let level = detect_heading_level(xot, node);
            rename(xot, node, "heading");
            if let Some(lvl) = level {
                set_attr(xot, node, "level", &lvl.to_string());
            }
            Ok(TransformAction::Continue)
        }
        "setext_heading" => {
            // Determine heading level from the underline child
            let level = detect_setext_level(xot, node);
            rename(xot, node, "heading");
            if let Some(lvl) = level {
                set_attr(xot, node, "level", &lvl.to_string());
            }
            Ok(TransformAction::Continue)
        }

        // Heading markers and underlines - remove entirely (level is captured on parent)
        "atx_h1_marker" | "atx_h2_marker" | "atx_h3_marker"
        | "atx_h4_marker" | "atx_h5_marker" | "atx_h6_marker"
        | "setext_h1_underline" | "setext_h2_underline" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Code blocks
        "fenced_code_block" => {
            // Extract language from info_string child if present
            let lang = detect_code_language(xot, node);
            rename(xot, node, "code_block");
            if let Some(lang) = lang {
                set_attr(xot, node, "language", &lang);
            }
            Ok(TransformAction::Continue)
        }
        "indented_code_block" => {
            rename(xot, node, "code_block");
            Ok(TransformAction::Continue)
        }
        "code_fence_content" => {
            rename(xot, node, "code");
            Ok(TransformAction::Continue)
        }
        "info_string" | "language" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }
        "fenced_code_block_delimiter" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Inline code
        "code_span" => {
            rename(xot, node, "code");
            Ok(TransformAction::Continue)
        }
        "code_span_delimiter" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Links
        "inline_link" | "full_reference_link" | "collapsed_reference_link"
        | "shortcut_link" => {
            rename(xot, node, "link");
            Ok(TransformAction::Continue)
        }

        // Images
        "image" => {
            rename(xot, node, "image");
            Ok(TransformAction::Continue)
        }

        // Emphasis
        "emphasis" => {
            rename(xot, node, "emphasis");
            Ok(TransformAction::Continue)
        }
        "strong_emphasis" => {
            rename(xot, node, "strong");
            Ok(TransformAction::Continue)
        }
        "strikethrough" => {
            rename(xot, node, "strikethrough");
            Ok(TransformAction::Continue)
        }
        "emphasis_delimiter" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Lists
        "list" => {
            // Detect ordered vs unordered from marker type
            let list_type = detect_list_type(xot, node);
            rename(xot, node, "list");
            if let Some(t) = list_type {
                set_attr(xot, node, "type", t);
            }
            Ok(TransformAction::Continue)
        }
        "list_item" => {
            rename(xot, node, "item");
            Ok(TransformAction::Continue)
        }
        // List markers - remove entirely (type is captured on parent list)
        "list_marker_plus" | "list_marker_minus" | "list_marker_star"
        | "list_marker_dot" | "list_marker_parenthesis" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Task list markers
        "task_list_marker_checked" => {
            rename(xot, node, "checked");
            Ok(TransformAction::Done)
        }
        "task_list_marker_unchecked" => {
            rename(xot, node, "unchecked");
            Ok(TransformAction::Done)
        }

        // Block quotes
        "block_quote" => {
            rename(xot, node, "blockquote");
            Ok(TransformAction::Continue)
        }
        "block_quote_marker" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Tables (GFM pipe tables)
        "pipe_table" => {
            rename(xot, node, "table");
            Ok(TransformAction::Continue)
        }
        "pipe_table_header" => {
            rename(xot, node, "thead");
            Ok(TransformAction::Continue)
        }
        "pipe_table_row" => {
            rename(xot, node, "row");
            Ok(TransformAction::Continue)
        }
        "pipe_table_cell" => {
            rename(xot, node, "cell");
            Ok(TransformAction::Continue)
        }
        // Delimiter row and alignment markers - remove entirely
        "pipe_table_delimiter_row" | "pipe_table_delimiter_cell"
        | "pipe_table_align_left" | "pipe_table_align_right" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // HTML blocks
        "html_block" => {
            rename(xot, node, "html");
            Ok(TransformAction::Done)
        }
        "html_tag" => {
            rename(xot, node, "html");
            Ok(TransformAction::Done)
        }

        // Thematic breaks (horizontal rules)
        "thematic_break" => {
            rename(xot, node, "hr");
            remove_text_children(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Hard line breaks
        "hard_line_break" => {
            rename(xot, node, "br");
            Ok(TransformAction::Done)
        }

        // Link components
        "link_text" | "image_description" => {
            rename(xot, node, "text");
            Ok(TransformAction::Continue)
        }
        "link_destination" => {
            rename(xot, node, "destination");
            Ok(TransformAction::Continue)
        }
        "link_title" => {
            rename(xot, node, "title");
            Ok(TransformAction::Continue)
        }
        "link_label" => {
            rename(xot, node, "label");
            Ok(TransformAction::Continue)
        }
        "link_reference_definition" => {
            rename(xot, node, "reference");
            Ok(TransformAction::Continue)
        }

        // Autolinks
        "uri_autolink" | "email_autolink" => {
            rename(xot, node, "link");
            Ok(TransformAction::Continue)
        }

        // Block continuation markers - remove entirely
        "block_continuation" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Backslash escapes
        "backslash_escape" => {
            rename(xot, node, "escape");
            Ok(TransformAction::Done)
        }

        // Entity/character references
        "entity_reference" | "numeric_character_reference" => {
            rename(xot, node, "entity");
            Ok(TransformAction::Done)
        }

        // Metadata blocks (YAML frontmatter)
        "minus_metadata" | "plus_metadata" => {
            rename(xot, node, "frontmatter");
            Ok(TransformAction::Done)
        }

        // Latex
        "latex_block" => {
            rename(xot, node, "latex");
            Ok(TransformAction::Done)
        }
        "latex_span_delimiter" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        _ => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

/// Detect heading level from atx_heading marker children
fn detect_heading_level(xot: &Xot, node: XotNode) -> Option<u8> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.as_str() {
                "atx_h1_marker" => return Some(1),
                "atx_h2_marker" => return Some(2),
                "atx_h3_marker" => return Some(3),
                "atx_h4_marker" => return Some(4),
                "atx_h5_marker" => return Some(5),
                "atx_h6_marker" => return Some(6),
                _ => {}
            }
        }
    }
    None
}

/// Detect heading level from setext underline children
fn detect_setext_level(xot: &Xot, node: XotNode) -> Option<u8> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.as_str() {
                "setext_h1_underline" => return Some(1),
                "setext_h2_underline" => return Some(2),
                _ => {}
            }
        }
    }
    None
}

/// Detect code block language from info_string child
fn detect_code_language(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "info_string" {
                // The info_string may contain a language child or direct text
                if let Some(text) = get_text_content(xot, child) {
                    let lang = text.trim().to_string();
                    if !lang.is_empty() {
                        return Some(lang);
                    }
                }
                // Check for language child element
                for grandchild in xot.children(child) {
                    if let Some(gname) = get_element_name(xot, grandchild) {
                        if gname == "language" {
                            if let Some(text) = get_text_content(xot, grandchild) {
                                let lang = text.trim().to_string();
                                if !lang.is_empty() {
                                    return Some(lang);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Detect list type from marker children
fn detect_list_type(xot: &Xot, node: XotNode) -> Option<&'static str> {
    // Check first list_item's marker
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "list_item" {
                for marker in xot.children(child) {
                    if let Some(mname) = get_element_name(xot, marker) {
                        match mname.as_str() {
                            "list_marker_plus" | "list_marker_minus" | "list_marker_star" => {
                                return Some("unordered");
                            }
                            "list_marker_dot" | "list_marker_parenthesis" => {
                                return Some("ordered");
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    None
}

fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "document" => Some("document"),
        "section" => Some("section"),
        "paragraph" => Some("paragraph"),
        "inline" => Some("inline"),
        _ => None,
    }
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Headings
        "heading" => SyntaxCategory::Keyword,

        // Code
        "code_block" | "code" => SyntaxCategory::String,

        // Emphasis
        "emphasis" | "strong" | "strikethrough" => SyntaxCategory::Identifier,

        // Links and images
        "link" | "image" => SyntaxCategory::Function,
        "destination" => SyntaxCategory::String,
        "label" | "title" => SyntaxCategory::String,

        // Lists
        "list" | "item" => SyntaxCategory::Default,
        "checked" | "unchecked" => SyntaxCategory::Keyword,

        // Block elements
        "blockquote" => SyntaxCategory::Comment,
        "hr" => SyntaxCategory::Operator,

        // Table
        "table" | "thead" | "row" | "cell" => SyntaxCategory::Default,

        // HTML
        "html" => SyntaxCategory::Type,

        // Frontmatter
        "frontmatter" => SyntaxCategory::Comment,

        // LaTeX
        "latex" => SyntaxCategory::String,

        // Escape and entity
        "escape" | "entity" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements
        _ => SyntaxCategory::Default,
    }
}
