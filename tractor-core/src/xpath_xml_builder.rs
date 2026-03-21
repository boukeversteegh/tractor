//! Build the smallest XML structure matching a given XPath expression.
//!
//! This module parses a subset of XPath and constructs a minimal XML document
//! that would be matched by the expression. It is intended as a proof-of-concept
//! utility for testing and development.
//!
//! # Supported XPath features
//!
//! - Absolute paths: `/root/child`
//! - Descendant-or-self (`//`): `//foo`, `//foo//bar`
//! - Attribute predicates: `//foo[@id='val']`
//! - Attribute existence predicates: `//foo[@id]`
//! - Text predicates: `//foo[.='text']`, `//foo[text()='text']`
//! - Child element predicates: `//foo[bar='val']`
//! - Nested child predicates: `//foo[bar/baz='val']`
//! - Wildcard steps: `//*`, `//foo/*`
//! - Multiple predicates: `//foo[@a='1'][@b='2']`
//! - Chained paths: `//foo/bar[@id='x']/baz`
//!
//! # Example
//!
//! ```
//! use tractor_core::xpath_xml_builder::build_xml_from_xpath;
//!
//! let xml = build_xml_from_xpath("//book[@lang='en']/title").unwrap();
//! assert_eq!(xml, r#"<book lang="en"><title/></book>"#);
//! ```

use xot::Xot;

/// Errors that can occur during XPath parsing or XML construction.
#[derive(Debug, thiserror::Error)]
pub enum XPathBuildError {
    #[error("empty XPath expression")]
    Empty,
    #[error("unexpected token at position {pos}: '{ch}'")]
    UnexpectedToken { pos: usize, ch: char },
    #[error("unterminated string literal starting at position {pos}")]
    UnterminatedString { pos: usize },
    #[error("unterminated predicate starting at position {pos}")]
    UnterminatedPredicate { pos: usize },
    #[error("empty step in path")]
    EmptyStep,
    #[error("xot error: {0}")]
    Xot(#[from] xot::Error),
}

// ---------------------------------------------------------------------------
// XPath AST
// ---------------------------------------------------------------------------

/// A single step in a parsed XPath location path.
#[derive(Debug, Clone, PartialEq)]
struct Step {
    /// Element name, or `*` for wildcard.
    name: String,
    /// Whether this step was preceded by `//` (descendant-or-self axis).
    is_descendant: bool,
    /// Predicates attached to this step.
    predicates: Vec<Predicate>,
}

/// A predicate expression inside `[…]`.
#[derive(Debug, Clone, PartialEq)]
enum Predicate {
    /// `[@attr='value']` — attribute with a specific value.
    AttributeEquals { name: String, value: String },
    /// `[@attr]` — attribute existence (no value check).
    AttributeExists { name: String },
    /// `[.='value']` or `[text()='value']` — text content of the context node.
    TextEquals(String),
    /// `[child='value']` or `[child/grandchild='value']` — a child path whose
    /// string-value equals the given literal.
    ChildPathEquals { steps: Vec<String>, value: String },
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Parser {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Parse a complete XPath expression into a list of steps.
    fn parse(mut self) -> Result<Vec<Step>, XPathBuildError> {
        let mut steps = Vec::new();
        self.skip_whitespace();

        if self.pos >= self.chars.len() {
            return Err(XPathBuildError::Empty);
        }

        // Consume leading `/` or `//`
        let mut first_is_descendant = false;
        if self.peek() == Some('/') {
            self.advance();
            if self.peek() == Some('/') {
                self.advance();
                first_is_descendant = true;
            }
        }

        // Parse first step
        let mut step = self.parse_step()?;
        step.is_descendant = first_is_descendant;
        steps.push(step);

        // Parse remaining steps separated by `/` or `//`
        loop {
            self.skip_whitespace();
            if self.peek() != Some('/') {
                break;
            }
            self.advance(); // consume '/'
            let mut is_desc = false;
            if self.peek() == Some('/') {
                self.advance();
                is_desc = true;
            }
            let mut step = self.parse_step()?;
            step.is_descendant = is_desc;
            steps.push(step);
        }

        Ok(steps)
    }

    /// Parse a single step: name-test followed by zero or more predicates.
    fn parse_step(&mut self) -> Result<Step, XPathBuildError> {
        self.skip_whitespace();

        let name = self.parse_name_test()?;
        let mut predicates = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some('[') {
                predicates.push(self.parse_predicate()?);
            } else {
                break;
            }
        }

        Ok(Step {
            name,
            is_descendant: false,
            predicates,
        })
    }

    /// Parse a name test (NCName or `*`).
    fn parse_name_test(&mut self) -> Result<String, XPathBuildError> {
        self.skip_whitespace();

        if self.peek() == Some('*') {
            self.advance();
            return Ok("*".to_string());
        }

        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                self.advance();
            } else {
                break;
            }
        }

        if self.pos == start {
            return Err(XPathBuildError::EmptyStep);
        }

        Ok(self.chars[start..self.pos].iter().collect())
    }

    /// Parse a predicate: `[…]`.
    fn parse_predicate(&mut self) -> Result<Predicate, XPathBuildError> {
        let pred_start = self.pos;
        self.advance(); // consume '['
        self.skip_whitespace();

        let pred = if self.peek() == Some('@') {
            // Attribute predicate: [@attr='val'] or [@attr]
            self.advance(); // consume '@'
            let attr_name = self.parse_name_test()?;
            self.skip_whitespace();
            if self.peek() == Some('=') {
                self.advance();
                self.skip_whitespace();
                let value = self.parse_string_literal()?;
                Predicate::AttributeEquals {
                    name: attr_name,
                    value,
                }
            } else {
                Predicate::AttributeExists { name: attr_name }
            }
        } else if self.peek() == Some('.') {
            // [.='value']
            self.advance();
            self.skip_whitespace();
            self.expect_char('=')?;
            self.skip_whitespace();
            let value = self.parse_string_literal()?;
            Predicate::TextEquals(value)
        } else if self.lookahead_text_fn() {
            // [text()='value']
            self.consume_str("text()");
            self.skip_whitespace();
            self.expect_char('=')?;
            self.skip_whitespace();
            let value = self.parse_string_literal()?;
            Predicate::TextEquals(value)
        } else {
            // [child='value'] or [child/grandchild='value']
            let mut path_steps = vec![self.parse_name_test()?];
            while self.peek() == Some('/') {
                self.advance();
                path_steps.push(self.parse_name_test()?);
            }
            self.skip_whitespace();
            self.expect_char('=')?;
            self.skip_whitespace();
            let value = self.parse_string_literal()?;
            Predicate::ChildPathEquals {
                steps: path_steps,
                value,
            }
        };

        self.skip_whitespace();
        if self.peek() != Some(']') {
            return Err(XPathBuildError::UnterminatedPredicate { pos: pred_start });
        }
        self.advance(); // consume ']'

        Ok(pred)
    }

    /// Check if the upcoming characters are `text()`.
    fn lookahead_text_fn(&self) -> bool {
        let remaining: String = self.chars[self.pos..].iter().collect();
        remaining.starts_with("text()")
    }

    /// Consume a known string (caller must have verified via lookahead).
    fn consume_str(&mut self, s: &str) {
        for _ in s.chars() {
            self.advance();
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), XPathBuildError> {
        match self.peek() {
            Some(ch) if ch == expected => {
                self.advance();
                Ok(())
            }
            Some(ch) => Err(XPathBuildError::UnexpectedToken {
                pos: self.pos,
                ch,
            }),
            None => Err(XPathBuildError::UnexpectedToken {
                pos: self.pos,
                ch: '\0',
            }),
        }
    }

    /// Parse a string literal delimited by `'` or `"`.
    fn parse_string_literal(&mut self) -> Result<String, XPathBuildError> {
        let quote = match self.peek() {
            Some(q @ '\'' | q @ '"') => {
                self.advance();
                q
            }
            Some(ch) => {
                return Err(XPathBuildError::UnexpectedToken {
                    pos: self.pos,
                    ch,
                })
            }
            None => {
                return Err(XPathBuildError::UnterminatedString { pos: self.pos })
            }
        };

        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == quote {
                let value: String = self.chars[start..self.pos].iter().collect();
                self.advance(); // consume closing quote
                return Ok(value);
            }
            self.advance();
        }

        Err(XPathBuildError::UnterminatedString { pos: start })
    }
}

// ---------------------------------------------------------------------------
// XML builder
// ---------------------------------------------------------------------------

/// Build the smallest XML string that matches the given XPath expression.
///
/// For descendant-or-self steps (`//`), the intermediate wrapper is omitted —
/// the returned XML is a flat chain of the named elements. For absolute paths
/// (`/a/b/c`) every step produces an element.
///
/// Returns the XML as a string (no XML declaration, no pretty-printing).
pub fn build_xml_from_xpath(xpath: &str) -> Result<String, XPathBuildError> {
    let steps = Parser::new(xpath).parse()?;
    let mut xot = Xot::new();
    build_xml_tree(&mut xot, &steps)
}

/// Build the XML tree in a `Xot` and serialize the root element to a string.
fn build_xml_tree(xot: &mut Xot, steps: &[Step]) -> Result<String, XPathBuildError> {
    if steps.is_empty() {
        return Err(XPathBuildError::Empty);
    }

    // Build from outermost to innermost.
    let root_node = build_step(xot, steps, 0)?;

    // Serialize — xot always wraps in a document, so we get the element
    // serialization via `to_string`.
    Ok(xot.to_string(root_node)?)
}

/// Recursively build elements for step[index..].
///
/// Returns the xot node for the outermost element created.
fn build_step(
    xot: &mut Xot,
    steps: &[Step],
    index: usize,
) -> Result<xot::Node, XPathBuildError> {
    let step = &steps[index];

    // Choose element name — wildcards become `_` (a legal XML name).
    let elem_name_str = if step.name == "*" { "_" } else { &step.name };
    let name_id = xot.add_name(elem_name_str);
    let element = xot.new_element(name_id);

    // Apply predicates.
    let mut has_text = false;
    for pred in &step.predicates {
        match pred {
            Predicate::AttributeEquals { name, value } => {
                let attr_id = xot.add_name(name);
                xot.attributes_mut(element).insert(attr_id, value.clone());
            }
            Predicate::AttributeExists { name } => {
                let attr_id = xot.add_name(name);
                xot.attributes_mut(element).insert(attr_id, String::new());
            }
            Predicate::TextEquals(value) => {
                if !has_text {
                    let text = xot.new_text(value);
                    xot.append(element, text)?;
                    has_text = true;
                }
            }
            Predicate::ChildPathEquals { steps: child_steps, value } => {
                // Build nested child elements: child_steps[0] > child_steps[1] > … > text
                let mut current = element;
                for child_name in child_steps {
                    let child_name_str = if child_name == "*" { "_" } else { child_name.as_str() };
                    let cid = xot.add_name(child_name_str);
                    let child_el = xot.new_element(cid);
                    xot.append(current, child_el)?;
                    current = child_el;
                }
                let text = xot.new_text(value);
                xot.append(current, text)?;
            }
        }
    }

    // If there are more steps, build the child and append.
    if index + 1 < steps.len() {
        let child_node = build_step(xot, steps, index + 1)?;
        xot.append(element, child_node)?;
    }

    Ok(element)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Basic path tests --------------------------------------------------

    #[test]
    fn single_element() {
        let xml = build_xml_from_xpath("//foo").unwrap();
        assert_eq!(xml, "<foo/>");
    }

    #[test]
    fn absolute_path() {
        let xml = build_xml_from_xpath("/root/child").unwrap();
        assert_eq!(xml, "<root><child/></root>");
    }

    #[test]
    fn descendant_chain() {
        let xml = build_xml_from_xpath("//foo/bar/baz").unwrap();
        assert_eq!(xml, "<foo><bar><baz/></bar></foo>");
    }

    #[test]
    fn double_descendant() {
        let xml = build_xml_from_xpath("//foo//bar").unwrap();
        assert_eq!(xml, "<foo><bar/></foo>");
    }

    // -- Attribute predicates ----------------------------------------------

    #[test]
    fn attribute_equals() {
        let xml = build_xml_from_xpath("//div[@class='main']").unwrap();
        assert_eq!(xml, r#"<div class="main"/>"#);
    }

    #[test]
    fn attribute_exists() {
        let xml = build_xml_from_xpath("//input[@required]").unwrap();
        assert_eq!(xml, r#"<input required=""/>"#);
    }

    #[test]
    fn multiple_attributes() {
        let xml = build_xml_from_xpath("//a[@href='#'][@class='link']").unwrap();
        assert_eq!(xml, "<a href=\"#\" class=\"link\"/>");
    }

    // -- Text predicates ---------------------------------------------------

    #[test]
    fn text_dot_equals() {
        let xml = build_xml_from_xpath("//name[.='hello']").unwrap();
        assert_eq!(xml, "<name>hello</name>");
    }

    #[test]
    fn text_fn_equals() {
        let xml = build_xml_from_xpath("//name[text()='hello']").unwrap();
        assert_eq!(xml, "<name>hello</name>");
    }

    // -- Child path predicates ---------------------------------------------

    #[test]
    fn child_equals() {
        let xml = build_xml_from_xpath("//book[title='Rust']").unwrap();
        assert_eq!(xml, "<book><title>Rust</title></book>");
    }

    #[test]
    fn nested_child_equals() {
        let xml = build_xml_from_xpath("//book[meta/author='Alice']").unwrap();
        assert_eq!(xml, "<book><meta><author>Alice</author></meta></book>");
    }

    // -- Wildcards ---------------------------------------------------------

    #[test]
    fn wildcard_step() {
        let xml = build_xml_from_xpath("//*").unwrap();
        assert_eq!(xml, "<_/>");
    }

    #[test]
    fn wildcard_child() {
        let xml = build_xml_from_xpath("//foo/*").unwrap();
        assert_eq!(xml, "<foo><_/></foo>");
    }

    // -- Combined ----------------------------------------------------------

    #[test]
    fn attribute_with_child() {
        let xml = build_xml_from_xpath("//book[@lang='en']/title").unwrap();
        assert_eq!(xml, r#"<book lang="en"><title/></book>"#);
    }

    #[test]
    fn predicate_and_deeper_path() {
        let xml = build_xml_from_xpath("//library[name='City']/section/book").unwrap();
        assert_eq!(
            xml,
            "<library><name>City</name><section><book/></section></library>"
        );
    }

    #[test]
    fn double_quoted_strings() {
        let xml = build_xml_from_xpath(r#"//foo[@bar="baz"]"#).unwrap();
        assert_eq!(xml, r#"<foo bar="baz"/>"#);
    }

    // -- Error cases -------------------------------------------------------

    #[test]
    fn empty_xpath_is_error() {
        assert!(build_xml_from_xpath("").is_err());
    }

    #[test]
    fn unterminated_string_is_error() {
        assert!(build_xml_from_xpath("//foo[@a='unterminated").is_err());
    }

    #[test]
    fn unterminated_predicate_is_error() {
        assert!(build_xml_from_xpath("//foo[@a='val'").is_err());
    }

    // -- Roundtrip: the generated XML should match the XPath ---------------

    #[test]
    fn roundtrip_simple() {
        assert_roundtrip("//function");
    }

    #[test]
    fn roundtrip_attribute() {
        assert_roundtrip("//class[@name='Foo']");
    }

    #[test]
    fn roundtrip_child_text() {
        assert_roundtrip("//method[name='doStuff']");
    }

    #[test]
    fn roundtrip_nested_path() {
        assert_roundtrip("//module/class/method");
    }

    /// Helper: build XML from an XPath, load it, query with that XPath,
    /// and assert at least one match is found.
    fn assert_roundtrip(xpath: &str) {
        use crate::parser::load_xml_string_to_documents;
        use crate::xpath::XPathEngine;
        use std::sync::Arc;

        let xml = build_xml_from_xpath(xpath).unwrap();

        // Wrap in root so load_xml_string_to_documents is happy
        let doc_xml = format!("<root>{}</root>", xml);

        let mut result =
            load_xml_string_to_documents(&doc_xml, "test.xml".to_string()).unwrap();
        let engine = XPathEngine::new();

        let matches = engine
            .query_documents(
                &mut result.documents,
                result.doc_handle,
                xpath,
                Arc::new(vec![]),
                "test.xml",
            )
            .unwrap();

        assert!(
            !matches.is_empty(),
            "XPath `{}` should match the generated XML:\n{}",
            xpath,
            xml
        );
    }
}
