// DO NOT EDIT — regenerate via `task gen:kinds`.
// Source: this grammar's node-types.json (named, non-supertype kinds only).

use strum_macros::{EnumIter, EnumString, IntoStaticStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum MdKind {
    AtxH1Marker,
    AtxH2Marker,
    AtxH3Marker,
    AtxH4Marker,
    AtxH5Marker,
    AtxH6Marker,
    AtxHeading,
    BackslashEscape,
    BlockContinuation,
    BlockQuote,
    BlockQuoteMarker,
    CodeFenceContent,
    CodeSpan,
    CodeSpanDelimiter,
    CollapsedReferenceLink,
    Document,
    EmailAutolink,
    Emphasis,
    EmphasisDelimiter,
    EntityReference,
    FencedCodeBlock,
    FencedCodeBlockDelimiter,
    FullReferenceLink,
    HardLineBreak,
    HtmlBlock,
    HtmlTag,
    Image,
    ImageDescription,
    IndentedCodeBlock,
    InfoString,
    Inline,
    InlineLink,
    Language,
    LatexBlock,
    LatexSpanDelimiter,
    LinkDestination,
    LinkLabel,
    LinkReferenceDefinition,
    LinkText,
    LinkTitle,
    List,
    ListItem,
    ListMarkerDot,
    ListMarkerMinus,
    ListMarkerParenthesis,
    ListMarkerPlus,
    ListMarkerStar,
    MinusMetadata,
    NumericCharacterReference,
    Paragraph,
    PipeTable,
    PipeTableAlignLeft,
    PipeTableAlignRight,
    PipeTableCell,
    PipeTableDelimiterCell,
    PipeTableDelimiterRow,
    PipeTableHeader,
    PipeTableRow,
    PlusMetadata,
    Section,
    SetextH1Underline,
    SetextH2Underline,
    SetextHeading,
    ShortcutLink,
    Strikethrough,
    StrongEmphasis,
    TaskListMarkerChecked,
    TaskListMarkerUnchecked,
    ThematicBreak,
    UriAutolink,
}

impl MdKind {
    pub fn from_str(s: &str) -> Option<Self> {
        <Self as std::str::FromStr>::from_str(s).ok()
    }

    pub fn as_str(&self) -> &'static str {
        (*self).into()
    }
}
