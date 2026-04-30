// DO NOT EDIT — regenerate via `task gen:kinds`.
// Source: this grammar's node-types.json (named, non-supertype kinds only).

use strum_macros::{EnumString, IntoStaticStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum TomlKind {
    Array,
    BareKey,
    Boolean,
    Comment,
    Document,
    DottedKey,
    EscapeSequence,
    Float,
    InlineTable,
    Integer,
    LocalDate,
    LocalDateTime,
    LocalTime,
    OffsetDateTime,
    Pair,
    QuotedKey,
    String,
    Table,
    TableArrayElement,
}

impl TomlKind {
    pub fn from_str(s: &str) -> Option<Self> {
        <Self as std::str::FromStr>::from_str(s).ok()
    }

    pub fn as_str(&self) -> &'static str {
        (*self).into()
    }
}
