//! Per-kind rule table for INI.
//!
//! The compiler enforces exhaustive coverage of every `IniKind`
//! variant.

use crate::languages::rule::Rule;

use super::input::IniKind;
use super::transformations::*;

pub fn rule(kind: IniKind) -> Rule<&'static str> {
    match kind {
        IniKind::Document     => Rule::Custom(document),
        IniKind::Section      => Rule::Custom(section),
        IniKind::SectionName  => Rule::Custom(section_name),
        IniKind::Setting      => Rule::Custom(setting),
        IniKind::SettingName  => Rule::Flatten { distribute_list: None },
        IniKind::SettingValue => Rule::Flatten { distribute_list: None },
        IniKind::Text         => Rule::Flatten { distribute_list: None },
        IniKind::Comment      => Rule::Custom(comment),
    }
}
