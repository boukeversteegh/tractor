// DO NOT EDIT — regenerate via `task gen:kinds`.
// Source: this grammar's node-types.json (named, non-supertype kinds only).

use strum_macros::{EnumString, IntoStaticStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum YamlKind {
    Alias,
    AliasName,
    Anchor,
    AnchorName,
    BlockMapping,
    BlockMappingPair,
    BlockNode,
    BlockScalar,
    BlockSequence,
    BlockSequenceItem,
    BooleanScalar,
    Comment,
    DirectiveName,
    DirectiveParameter,
    Document,
    DoubleQuoteScalar,
    EscapeSequence,
    FloatScalar,
    FlowMapping,
    FlowNode,
    FlowPair,
    FlowSequence,
    IntegerScalar,
    NullScalar,
    PlainScalar,
    ReservedDirective,
    SingleQuoteScalar,
    Stream,
    StringScalar,
    Tag,
    TagDirective,
    TagHandle,
    TagPrefix,
    TimestampScalar,
    YamlDirective,
    YamlVersion,
}

impl YamlKind {
    pub fn from_str(s: &str) -> Option<Self> {
        <Self as std::str::FromStr>::from_str(s).ok()
    }

    pub fn as_str(&self) -> &'static str {
        (*self).into()
    }
}
