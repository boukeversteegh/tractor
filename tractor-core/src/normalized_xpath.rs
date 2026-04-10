//! A typed wrapper for XPath expressions that guarantees normalization.
//!
//! Bare element names (e.g. `"function"`) are automatically prefixed with
//! `"//"` on construction, so the query engine always receives a valid axis.
//! This makes it impossible to forget the prefix — the type system enforces it.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserialize, Deserializer};
use serde::{Serialize, Serializer};

/// An XPath expression that has been normalized (bare names → `//name`).
///
/// Constructed via [`FromStr`], [`From<&str>`], or serde [`Deserialize`].
/// All paths go through [`normalize`] so the invariant holds by construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedXpath(String);

impl NormalizedXpath {
    /// Normalize a raw XPath string.
    ///
    /// - Bare element names like `"function"` become `"//function"`.
    /// - Absolute paths, expressions, literals, etc. are preserved as-is.
    pub fn new(raw: &str) -> Self {
        Self(normalize(raw))
    }

    /// Return the normalized XPath as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---- Display / conversions ----

impl fmt::Display for NormalizedXpath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<&str> for NormalizedXpath {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl AsRef<str> for NormalizedXpath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NormalizedXpath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for NormalizedXpath {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

// ---- FromStr (for clap) ----

impl FromStr for NormalizedXpath {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

// ---- Serde Serialize / Deserialize ----

impl Serialize for NormalizedXpath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NormalizedXpath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer).map_err(de::Error::custom)?;
        Ok(Self::new(&raw))
    }
}

// ---------------------------------------------------------------------------
// Normalization logic
// ---------------------------------------------------------------------------

fn is_msys_environment() -> bool {
    std::env::var("MSYSTEM").is_ok()
}

fn fix_msys_xpath_mangling(xpath: &str) -> String {
    if !is_msys_environment() {
        return xpath.to_string();
    }

    if xpath.starts_with('/') && !xpath.starts_with("//") {
        let rest = &xpath[1..];
        if !rest.is_empty()
            && (rest.chars().next().unwrap().is_alphabetic() || rest.starts_with('*'))
        {
            return format!("/{}", xpath);
        }
    }

    xpath.to_string()
}

fn looks_like_xpath_expression(xpath: &str) -> bool {
    let keywords = [
        "let ", "let$", "for ", "for$", "if ", "if(", "some ", "some$", "every ", "every$",
    ];
    keywords.iter().any(|kw| xpath.starts_with(kw))
        || xpath.starts_with("not(")
        || xpath.starts_with("count(")
        || xpath.starts_with("string(")
        || xpath.starts_with("contains(")
        || xpath.starts_with("starts-with(")
        || xpath.chars().next().map_or(false, |c| c.is_ascii_digit())
}

/// Normalize a raw XPath string: bare element names become `//name`.
fn normalize(xpath: &str) -> String {
    let xpath = fix_msys_xpath_mangling(xpath);

    if xpath.starts_with('/')
        || xpath.starts_with('(')
        || xpath.starts_with('$')
        || xpath.starts_with('"')
        || xpath.starts_with('\'')
        || xpath == "."
        || looks_like_xpath_expression(&xpath)
    {
        xpath
    } else {
        format!("//{}", xpath)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Bare element names get auto-prefixed with // --

    #[test]
    fn auto_prefixes_bare_element_names() {
        assert_eq!(NormalizedXpath::new("function"), "//function");
        assert_eq!(NormalizedXpath::new("variable"), "//variable");
        assert_eq!(NormalizedXpath::new("class"), "//class");
        assert_eq!(NormalizedXpath::new("name"), "//name");
        assert_eq!(NormalizedXpath::new("debug"), "//debug");
    }

    // -- Absolute paths preserved --

    #[test]
    fn preserves_absolute_paths() {
        assert_eq!(NormalizedXpath::new("//function"), "//function");
        assert_eq!(NormalizedXpath::new("//class[name='Foo']"), "//class[name='Foo']");
        if !is_msys_environment() {
            assert_eq!(NormalizedXpath::new("/root"), "/root");
        }
    }

    // -- Parenthesized expressions --

    #[test]
    fn preserves_parenthesized_expressions() {
        assert_eq!(NormalizedXpath::new("(//a | //b)"), "(//a | //b)");
    }

    // -- Dot --

    #[test]
    fn preserves_dot() {
        assert_eq!(NormalizedXpath::new("."), ".");
    }

    // -- let / for / if / some / every expressions --

    #[test]
    fn preserves_let_expressions() {
        assert_eq!(
            NormalizedXpath::new("let $v := //function return $v/name"),
            "let $v := //function return $v/name"
        );
        assert_eq!(
            NormalizedXpath::new("let$v := //x return $v"),
            "let$v := //x return $v"
        );
    }

    #[test]
    fn preserves_for_expressions() {
        assert_eq!(
            NormalizedXpath::new("for $v in //name return string($v)"),
            "for $v in //name return string($v)"
        );
        assert_eq!(
            NormalizedXpath::new("for$v in //name return $v"),
            "for$v in //name return $v"
        );
    }

    #[test]
    fn preserves_if_expressions() {
        assert_eq!(
            NormalizedXpath::new("if (//x) then 1 else 0"),
            "if (//x) then 1 else 0"
        );
        assert_eq!(
            NormalizedXpath::new("if(//x) then 1 else 0"),
            "if(//x) then 1 else 0"
        );
    }

    #[test]
    fn preserves_quantified_expressions() {
        assert_eq!(
            NormalizedXpath::new("some $v in //x satisfies $v/name"),
            "some $v in //x satisfies $v/name"
        );
        assert_eq!(
            NormalizedXpath::new("every $v in //x satisfies $v/name"),
            "every $v in //x satisfies $v/name"
        );
    }

    // -- Variable references --

    #[test]
    fn preserves_variable_references() {
        assert_eq!(NormalizedXpath::new("$var"), "$var");
    }

    // -- String literals --

    #[test]
    fn preserves_string_literals() {
        assert_eq!(NormalizedXpath::new("\"hello\""), "\"hello\"");
        assert_eq!(NormalizedXpath::new("'hello'"), "'hello'");
    }

    // -- Numeric literals --

    #[test]
    fn preserves_numeric_literals() {
        assert_eq!(NormalizedXpath::new("42"), "42");
        assert_eq!(NormalizedXpath::new("3.14"), "3.14");
    }

    // -- Function calls --

    #[test]
    fn preserves_function_calls() {
        assert_eq!(NormalizedXpath::new("count(//item)"), "count(//item)");
        assert_eq!(NormalizedXpath::new("not(//x)"), "not(//x)");
        assert_eq!(NormalizedXpath::new("string(//x)"), "string(//x)");
        assert_eq!(NormalizedXpath::new("contains(//x, 'y')"), "contains(//x, 'y')");
        assert_eq!(NormalizedXpath::new("starts-with(//x, 'y')"), "starts-with(//x, 'y')");
    }

    // -- FromStr / Display / Serde --

    #[test]
    fn from_str_normalizes() {
        let xpath: NormalizedXpath = "debug".parse().unwrap();
        assert_eq!(xpath, "//debug");
    }

    #[test]
    fn display_shows_normalized() {
        let xpath = NormalizedXpath::new("function");
        assert_eq!(format!("{}", xpath), "//function");
    }

    #[test]
    fn serde_deserialize_normalizes() {
        let xpath: NormalizedXpath = serde_json::from_str("\"debug\"").unwrap();
        assert_eq!(xpath, "//debug");

        let xpath: NormalizedXpath = serde_json::from_str("\"//already\"").unwrap();
        assert_eq!(xpath, "//already");
    }
}
