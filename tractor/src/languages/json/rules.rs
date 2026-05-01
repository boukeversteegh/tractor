//! Per-kind rule tables for JSON, one per output branch.
//!
//! The compiler enforces exhaustive coverage of every `JsonKind`
//! variant, so adding a kind to the grammar surfaces here as a
//! compile error until both branches assign a `Rule` to it.

use crate::languages::rule::Rule;

use super::input::JsonKind;
use super::transformations::*;

/// Syntax-branch rule. Produces the unified
/// `<object>/<array>/<property>/<string>/<number>/<bool>/<null>`
/// vocabulary.
pub fn syntax_rule(kind: JsonKind) -> Rule<&'static str> {
    match kind {
        JsonKind::Document      => Rule::Custom(strip_punct_flatten),
        JsonKind::Object        => Rule::Custom(strip_punct_continue),
        JsonKind::Array         => Rule::Custom(strip_punct_continue),
        JsonKind::Pair          => Rule::Custom(syntax_pair),
        JsonKind::String        => Rule::Custom(syntax_string),
        JsonKind::StringContent => Rule::Flatten { distribute_field: None },
        JsonKind::Number        => Rule::Custom(syntax_number),
        JsonKind::True          => Rule::Custom(syntax_bool),
        JsonKind::False         => Rule::Custom(syntax_bool),
        JsonKind::Null          => Rule::Custom(syntax_null),

        // Leaves the syntax transform leaves alone.
        JsonKind::Comment        => Rule::Passthrough,
        JsonKind::EscapeSequence => Rule::Passthrough,
    }
}

/// Data-branch rule. Projects keys into element names; scalars
/// flatten so their text bubbles up to the renamed parent.
pub fn data_rule(kind: JsonKind) -> Rule<&'static str> {
    match kind {
        JsonKind::Document => Rule::Custom(strip_punct_flatten),
        JsonKind::Object   => Rule::Custom(strip_punct_flatten),
        JsonKind::Pair     => Rule::Custom(data_pair),
        JsonKind::Array    => Rule::Custom(data_array),
        JsonKind::String   => Rule::Custom(data_string),

        JsonKind::StringContent => Rule::Flatten { distribute_field: None },
        JsonKind::Number        => Rule::Flatten { distribute_field: None },
        JsonKind::True          => Rule::Flatten { distribute_field: None },
        JsonKind::False         => Rule::Flatten { distribute_field: None },
        JsonKind::Null          => Rule::Flatten { distribute_field: None },

        JsonKind::Comment        => Rule::Passthrough,
        JsonKind::EscapeSequence => Rule::Passthrough,
    }
}
