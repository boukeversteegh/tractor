//! Per-kind rule table for TOML.
//!
//! The compiler enforces exhaustive coverage of every `TomlKind`
//! variant.

use crate::languages::rule::Rule;

use super::input::TomlKind;
use super::transformations::*;

pub fn rule(kind: TomlKind) -> Rule<&'static str> {
    match kind {
        // Top-level
        TomlKind::Document          => Rule::Custom(document),

        // Pair / table / table-array → custom dotted-key wrapping
        TomlKind::Pair              => Rule::Custom(pair),
        TomlKind::Table             => Rule::Custom(table),
        TomlKind::TableArrayElement => Rule::Custom(table_array_element),

        // Arrays / inline tables
        TomlKind::Array             => Rule::Custom(array),
        TomlKind::InlineTable       => Rule::Custom(inline_table),

        // Strings / scalars — promote text to parent
        TomlKind::String            => Rule::Custom(string),
        TomlKind::Integer           => Rule::Flatten { distribute_field: None },
        TomlKind::Float             => Rule::Flatten { distribute_field: None },
        TomlKind::Boolean           => Rule::Flatten { distribute_field: None },
        TomlKind::LocalDate         => Rule::Flatten { distribute_field: None },
        TomlKind::LocalDateTime     => Rule::Flatten { distribute_field: None },
        TomlKind::LocalTime         => Rule::Flatten { distribute_field: None },
        TomlKind::OffsetDateTime    => Rule::Flatten { distribute_field: None },
        TomlKind::EscapeSequence    => Rule::Flatten { distribute_field: None },

        // Keys — promote text (consumed by pair/table extractors)
        TomlKind::BareKey           => Rule::Flatten { distribute_field: None },
        TomlKind::QuotedKey         => Rule::Flatten { distribute_field: None },
        TomlKind::DottedKey         => Rule::Flatten { distribute_field: None },

        // Comments — drop the `#` and text
        TomlKind::Comment           => Rule::Custom(comment),
    }
}
