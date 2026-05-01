// DO NOT EDIT — regenerate via `task gen:kinds`.
// Source: this grammar's node-types.json (named, non-supertype kinds only).

use strum_macros::{EnumIter, EnumString, IntoStaticStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum JsonKind {
    Array,
    Comment,
    Document,
    EscapeSequence,
    False,
    Null,
    Number,
    Object,
    Pair,
    String,
    StringContent,
    True,
}

impl JsonKind {
    pub fn from_str(s: &str) -> Option<Self> {
        <Self as std::str::FromStr>::from_str(s).ok()
    }

    pub fn as_str(&self) -> &'static str {
        (*self).into()
    }
}
