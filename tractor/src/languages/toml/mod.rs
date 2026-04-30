//! TOML transform logic.
//!
//! Per-language pipeline ownership:
//!
//! ```text
//! input → rules → output
//!         ↑
//!         transformations (Custom handlers)
//! ```
//!
//! - [`input`]    — generated `TomlKind` enum (the input vocabulary).
//!                  Regenerate via `task gen:kinds`.
//! - [`output`]   — output element-name constants. Table / pair keys
//!                  are user-driven (open vocabulary), so only
//!                  `<item>` is named here.
//! - [`rules`]    — `rule(TomlKind) -> Rule` exhaustive match.
//! - [`transformations`] — `Rule::Custom` handlers + dotted-key
//!                          segment helpers.
//! - [`transform`] — thin orchestrator.
//!
//! Maps the TOML data structure to XML elements:
//! ```toml
//! [database]
//! host = "localhost"
//! ```
//! becomes:
//! ```xml
//! <database>
//!   <host>localhost</host>
//! </database>
//! ```
//! Queryable as: `//database/host[.='localhost']`.

pub mod input;
pub mod output;
pub mod rules;
pub mod transformations;
pub mod transform;

pub use transform::transform;

use xot::{Xot, Node as XotNode};
use crate::output::syntax_highlight::SyntaxCategory;

/// Strip surrounding quotes from a TOML string.
///
/// Handles multi-line variants (`"""…"""` / `'''…'''`) before the
/// single-line case. Multi-line strings drop the leading newline
/// after the opening delimiter (TOML spec).
pub(crate) fn strip_quotes(s: &str) -> String {
    if (s.starts_with("\"\"\"") && s.ends_with("\"\"\"")) ||
       (s.starts_with("'''") && s.ends_with("'''")) {
        return s[3..s.len() - 3].trim_start_matches('\n').to_string();
    }
    if (s.starts_with('"') && s.ends_with('"')) ||
       (s.starts_with('\'') && s.ends_with('\'')) {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

/// Strip quotes from a string node's text content.
pub(crate) fn strip_quotes_from_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let mut raw_text = String::new();
    for child in xot.children(node) {
        if let Some(t) = xot.text_str(child) {
            raw_text.push_str(t);
        }
    }

    if raw_text.is_empty() {
        return Ok(());
    }

    let stripped = strip_quotes(&raw_text);

    let all_children: Vec<XotNode> = xot.children(node).collect();
    for c in all_children {
        xot.detach(c)?;
    }
    let text_node = xot.new_text(&stripped);
    xot.append(node, text_node)?;
    Ok(())
}

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        "item" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'world'"), "world");
        assert_eq!(strip_quotes("plain"), "plain");
        assert_eq!(strip_quotes("\"\"\"multi\nline\"\"\""), "multi\nline");
        assert_eq!(strip_quotes("'''literal'''"), "literal");
    }
}
