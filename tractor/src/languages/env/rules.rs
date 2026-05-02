//! Per-kind rule table for env (.env files via tree-sitter-bash).
//!
//! The compiler enforces exhaustive coverage of every `EnvKind`
//! variant. Note that `EnvKind` is a hand-curated subset of bash's
//! grammar — bash kinds outside this set are no-ops at the
//! orchestrator (they don't reach the dispatcher).

use crate::languages::rule::Rule;

use super::input::EnvKind;
use super::transformations::*;

pub fn rule(kind: EnvKind) -> Rule<&'static str> {
    match kind {
        // Top-level + structural
        EnvKind::Program             => Rule::Custom(program),
        EnvKind::DeclarationCommand  => Rule::Custom(declaration_command),
        EnvKind::Concatenation       => Rule::Custom(concatenation),

        // Variable assignments + comments — full reshape
        EnvKind::VariableAssignment  => Rule::Custom(variable_assignment),
        EnvKind::Comment             => Rule::Custom(comment),

        // Value wrappers — promote text to the parent assignment
        EnvKind::VariableName        => Rule::Flatten { distribute_list: None },
        EnvKind::Word                => Rule::Flatten { distribute_list: None },
        EnvKind::Number              => Rule::Flatten { distribute_list: None },
        EnvKind::RawString           => Rule::Flatten { distribute_list: None },
        EnvKind::AnsiiCString        => Rule::Flatten { distribute_list: None },
        EnvKind::String              => Rule::Flatten { distribute_list: None },
        EnvKind::SimpleExpansion     => Rule::Flatten { distribute_list: None },
        EnvKind::Expansion           => Rule::Flatten { distribute_list: None },
        EnvKind::StringContent       => Rule::Flatten { distribute_list: None },
    }
}
