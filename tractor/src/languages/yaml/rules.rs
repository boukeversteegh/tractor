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
pub fn syntax_rule(kind: YamlKind) -> Rule {
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

        // Currently-unhandled kinds — preserve passthrough (matches
        // the old `_ => Continue` default). TODO: directive and tag
        // kinds may want explicit treatments.
        YamlKind::DirectiveName      => Rule::Custom(passthrough),
        YamlKind::DirectiveParameter => Rule::Custom(passthrough),
        YamlKind::EscapeSequence     => Rule::Custom(passthrough),
        YamlKind::ReservedDirective  => Rule::Custom(passthrough),
        YamlKind::TagDirective       => Rule::Custom(passthrough),
        YamlKind::TagHandle          => Rule::Custom(passthrough),
        YamlKind::TagPrefix          => Rule::Custom(passthrough),
        YamlKind::TimestampScalar    => Rule::Custom(passthrough),
        YamlKind::YamlDirective      => Rule::Custom(passthrough),
        YamlKind::YamlVersion        => Rule::Custom(passthrough),
    }
}

/// Data-branch rule. Projects mapping keys into element names; scalars
/// flatten so their text bubbles up to the renamed parent.
pub fn data_rule(kind: YamlKind) -> Rule {
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

        // Currently-unhandled kinds — preserve passthrough.
        YamlKind::DirectiveName      => Rule::Custom(passthrough),
        YamlKind::DirectiveParameter => Rule::Custom(passthrough),
        YamlKind::EscapeSequence     => Rule::Custom(passthrough),
        YamlKind::ReservedDirective  => Rule::Custom(passthrough),
        YamlKind::TagDirective       => Rule::Custom(passthrough),
        YamlKind::TagHandle          => Rule::Custom(passthrough),
        YamlKind::TagPrefix          => Rule::Custom(passthrough),
        YamlKind::TimestampScalar    => Rule::Custom(passthrough),
        YamlKind::YamlDirective      => Rule::Custom(passthrough),
        YamlKind::YamlVersion        => Rule::Custom(passthrough),
    }
}
