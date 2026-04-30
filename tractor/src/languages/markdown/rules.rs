//! Per-kind rule table for Markdown.
//!
//! The compiler enforces exhaustive coverage of every `MdKind`
//! variant — the union of the block + inline grammars (70 kinds).

use crate::languages::rule::Rule;

use super::input::MdKind;
use super::output::*;
use super::transformations::*;

pub fn rule(kind: MdKind) -> Rule<&'static str> {
    use Rule::*;
    match kind {
        // ---- Headings ---------------------------------------------------
        MdKind::AtxHeading        => Custom(atx_heading),
        MdKind::SetextHeading     => Custom(setext_heading),
        // Heading markers / underlines — drop entirely; level captured on parent.
        MdKind::AtxH1Marker        => Detach,
        MdKind::AtxH2Marker        => Detach,
        MdKind::AtxH3Marker        => Detach,
        MdKind::AtxH4Marker        => Detach,
        MdKind::AtxH5Marker        => Detach,
        MdKind::AtxH6Marker        => Detach,
        MdKind::SetextH1Underline  => Detach,
        MdKind::SetextH2Underline  => Detach,

        // ---- Code blocks / spans ---------------------------------------
        MdKind::FencedCodeBlock          => Custom(fenced_code_block),
        MdKind::IndentedCodeBlock        => Rename(CODE_BLOCK),
        MdKind::CodeFenceContent         => Rename(CODE),
        MdKind::CodeSpan                 => Rename(CODE),
        MdKind::InfoString               => Detach,
        MdKind::FencedCodeBlockDelimiter => Detach,
        MdKind::CodeSpanDelimiter        => Detach,
        // `language` is consumed by the fenced_code_block handler — info_string
        // is detached before we'd recurse into language.
        MdKind::Language                 => Custom(passthrough),

        // ---- Emphasis ---------------------------------------------------
        MdKind::Emphasis            => Rename(EMPHASIS),
        MdKind::StrongEmphasis      => Rename(STRONG),
        MdKind::Strikethrough       => Rename(STRIKETHROUGH),
        MdKind::EmphasisDelimiter   => Detach,

        // ---- Links / images --------------------------------------------
        MdKind::InlineLink             => Rename(LINK),
        MdKind::FullReferenceLink      => Rename(LINK),
        MdKind::CollapsedReferenceLink => Rename(LINK),
        MdKind::ShortcutLink           => Rename(LINK),
        MdKind::UriAutolink            => Rename(LINK),
        MdKind::EmailAutolink          => Rename(LINK),
        MdKind::Image                  => Rename(IMAGE),
        MdKind::LinkText               => Rename(TEXT),
        MdKind::ImageDescription       => Rename(TEXT),
        MdKind::LinkDestination        => Rename(DESTINATION),
        MdKind::LinkTitle              => Rename(TITLE),
        MdKind::LinkLabel              => Rename(LABEL),
        MdKind::LinkReferenceDefinition => Rename(REFERENCE),

        // ---- Lists ------------------------------------------------------
        MdKind::List     => Custom(list),
        MdKind::ListItem => Rename(ITEM),
        MdKind::ListMarkerPlus        => Detach,
        MdKind::ListMarkerMinus       => Detach,
        MdKind::ListMarkerStar        => Detach,
        MdKind::ListMarkerDot         => Detach,
        MdKind::ListMarkerParenthesis => Detach,
        MdKind::TaskListMarkerChecked   => Rename(CHECKED),
        MdKind::TaskListMarkerUnchecked => Rename(UNCHECKED),

        // ---- Block quotes ----------------------------------------------
        MdKind::BlockQuote       => Rename(BLOCKQUOTE),
        MdKind::BlockQuoteMarker => Detach,

        // ---- Tables (GFM pipe tables) ----------------------------------
        MdKind::PipeTable             => Rename(TABLE),
        MdKind::PipeTableHeader       => Rename(THEAD),
        MdKind::PipeTableRow          => Rename(ROW),
        MdKind::PipeTableCell         => Rename(CELL),
        MdKind::PipeTableDelimiterRow  => Detach,
        MdKind::PipeTableDelimiterCell => Detach,
        MdKind::PipeTableAlignLeft     => Detach,
        MdKind::PipeTableAlignRight    => Detach,

        // ---- HTML / latex / metadata -----------------------------------
        MdKind::HtmlBlock     => Rename(HTML),
        MdKind::HtmlTag       => Rename(HTML),
        MdKind::LatexBlock    => Rename(LATEX),
        MdKind::LatexSpanDelimiter => Detach,
        MdKind::MinusMetadata => Rename(FRONTMATTER),
        MdKind::PlusMetadata  => Rename(FRONTMATTER),

        // ---- Breaks / leaves -------------------------------------------
        MdKind::ThematicBreak  => Custom(thematic_break),
        MdKind::HardLineBreak  => Rename(BR),
        MdKind::BackslashEscape => Rename(ESCAPE),
        MdKind::EntityReference            => Rename(ENTITY),
        MdKind::NumericCharacterReference  => Rename(ENTITY),

        // ---- Block continuation — drop -------------------------------
        MdKind::BlockContinuation => Detach,

        // ---- Passthrough — structural wrappers (current behaviour) ---
        MdKind::Document  => Custom(passthrough),
        MdKind::Section   => Custom(passthrough),
        MdKind::Paragraph => Custom(passthrough),
        MdKind::Inline    => Custom(passthrough),
    }
}
