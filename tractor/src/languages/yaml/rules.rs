//! Per-kind rule tables for YAML, one per output branch.
//!
//! The compiler enforces exhaustive coverage of every `YamlKind`
//! variant — adding a kind to the grammar surfaces here as a compile
//! error until both branches assign a `Rule`.

use crate::languages::rule::Rule;

use super::input::YamlKind;
use super::transformations::*;

/// Syntax-branch rule. Produces the JSON-shared
/// `<object>/<array>/<property>/<string>/<number>/<bool>/<null>`
/// vocabulary.
pub fn syntax_rule(kind: YamlKind) -> Rule<&'static str> {
    match kind {
        // Top-level wrappers
        YamlKind::Stream             => Rule::Custom(strip_punct_flatten),
        YamlKind::Document           => Rule::Custom(strip_punct_continue),
        YamlKind::BlockNode          => Rule::Custom(strip_punct_flatten),
        YamlKind::FlowNode           => Rule::Custom(strip_punct_flatten),
        YamlKind::PlainScalar        => Rule::Custom(strip_punct_flatten),

        // Mappings → <object> + <property>
        YamlKind::BlockMapping       => Rule::Custom(syntax_mapping),
        YamlKind::FlowMapping        => Rule::Custom(syntax_mapping),
        YamlKind::BlockMappingPair   => Rule::Custom(syntax_pair),
        YamlKind::FlowPair           => Rule::Custom(syntax_pair),

        // Sequences → <array>
        YamlKind::BlockSequence      => Rule::Custom(syntax_sequence),
        YamlKind::FlowSequence       => Rule::Custom(syntax_sequence),
        YamlKind::BlockSequenceItem  => Rule::Custom(strip_punct_flatten),

        // Scalars
        YamlKind::DoubleQuoteScalar  => Rule::Custom(syntax_quoted_string),
        YamlKind::SingleQuoteScalar  => Rule::Custom(syntax_quoted_string),
        YamlKind::BlockScalar        => Rule::Custom(syntax_block_scalar),
        YamlKind::StringScalar       => Rule::Custom(syntax_string_scalar),
        YamlKind::IntegerScalar      => Rule::Custom(syntax_number),
        YamlKind::FloatScalar        => Rule::Custom(syntax_number),
        YamlKind::BooleanScalar      => Rule::Custom(syntax_bool),
        YamlKind::NullScalar         => Rule::Custom(syntax_null),

        // Anchors / aliases / tags / comments → flatten
        YamlKind::Anchor             => Rule::Custom(strip_punct_flatten),
        YamlKind::Tag                => Rule::Custom(strip_punct_flatten),
        YamlKind::Alias              => Rule::Custom(strip_punct_flatten),
        YamlKind::AliasName          => Rule::Flatten { distribute_field: None },
        YamlKind::AnchorName         => Rule::Flatten { distribute_field: None },
        YamlKind::Comment            => Rule::Custom(strip_punct_flatten),

        // Directive family — `%YAML 1.2` and `%TAG !! tag:…`. Both
        // surface under `<directive>` with a marker for the family
        // (yaml/tag/reserved); inner pieces use single-word names
        // (handle/prefix/version/parameter) so `//directive//handle`
        // is the broad-to-narrow path.
        YamlKind::YamlDirective      => Rule::RenameWithMarker("directive", "yaml"),
        YamlKind::TagDirective       => Rule::RenameWithMarker("directive", "tag"),
        YamlKind::ReservedDirective  => Rule::RenameWithMarker("directive", "reserved"),
        YamlKind::DirectiveName      => Rule::Rename("name"),
        YamlKind::DirectiveParameter => Rule::Rename("parameter"),
        YamlKind::TagHandle          => Rule::Rename("handle"),
        YamlKind::TagPrefix          => Rule::Rename("prefix"),
        YamlKind::YamlVersion        => Rule::Rename("version"),
        // Escape sequence inside a quoted scalar — `\n`, `\t`, etc.
        YamlKind::EscapeSequence     => Rule::Rename("escape"),
        // ISO 8601 timestamps (YAML 1.1 type). Joins int/float/bool/null
        // as a top-level scalar element.
        YamlKind::TimestampScalar    => Rule::Rename("timestamp"),
    }
}

/// Data-branch rule. Projects mapping keys into element names; scalars
/// flatten so their text bubbles up to the renamed parent.
pub fn data_rule(kind: YamlKind) -> Rule<&'static str> {
    match kind {
        // Pairs / items
        YamlKind::BlockMappingPair   => Rule::Custom(data_pair),
        YamlKind::FlowPair           => Rule::Custom(data_pair),
        YamlKind::BlockSequenceItem  => Rule::Custom(data_sequence_item),
        YamlKind::FlowSequence       => Rule::Custom(data_flow_sequence),

        // Scalars
        YamlKind::DoubleQuoteScalar  => Rule::Custom(data_double_quote),
        YamlKind::SingleQuoteScalar  => Rule::Custom(data_single_quote),
        YamlKind::BlockScalar        => Rule::Custom(data_block_scalar),
        YamlKind::StringScalar       => Rule::Flatten { distribute_field: None },
        YamlKind::IntegerScalar      => Rule::Flatten { distribute_field: None },
        YamlKind::FloatScalar        => Rule::Flatten { distribute_field: None },
        YamlKind::BooleanScalar      => Rule::Flatten { distribute_field: None },
        YamlKind::NullScalar         => Rule::Flatten { distribute_field: None },

        // Anchors / aliases / tags
        YamlKind::Anchor             => Rule::Custom(strip_punct_flatten),
        YamlKind::Tag                => Rule::Custom(strip_punct_flatten),
        YamlKind::Alias              => Rule::Custom(strip_punct_flatten),
        YamlKind::AliasName          => Rule::Flatten { distribute_field: None },
        YamlKind::AnchorName         => Rule::Flatten { distribute_field: None },

        // Document is kept (multi-doc YAML).
        YamlKind::Document           => Rule::Custom(strip_punct_continue),

        // Wrappers to flatten
        YamlKind::Stream             => Rule::Custom(strip_punct_flatten),
        YamlKind::BlockNode          => Rule::Custom(strip_punct_flatten),
        YamlKind::FlowNode           => Rule::Custom(strip_punct_flatten),
        YamlKind::BlockMapping       => Rule::Custom(strip_punct_flatten),
        YamlKind::FlowMapping        => Rule::Custom(strip_punct_flatten),
        YamlKind::PlainScalar        => Rule::Custom(strip_punct_flatten),
        YamlKind::BlockSequence      => Rule::Custom(strip_punct_flatten),

        // Comments removed
        YamlKind::Comment            => Rule::Custom(strip_punct_flatten),

        // Directive family — same shape as the syntax branch.
        YamlKind::YamlDirective      => Rule::RenameWithMarker("directive", "yaml"),
        YamlKind::TagDirective       => Rule::RenameWithMarker("directive", "tag"),
        YamlKind::ReservedDirective  => Rule::RenameWithMarker("directive", "reserved"),
        YamlKind::DirectiveName      => Rule::Rename("name"),
        YamlKind::DirectiveParameter => Rule::Rename("parameter"),
        YamlKind::TagHandle          => Rule::Rename("handle"),
        YamlKind::TagPrefix          => Rule::Rename("prefix"),
        YamlKind::YamlVersion        => Rule::Rename("version"),
        YamlKind::EscapeSequence     => Rule::Rename("escape"),
        YamlKind::TimestampScalar    => Rule::Rename("timestamp"),
    }
}
