//! Declarative set: parse a path expression with embedded values and apply
//! multiple upserts to ensure the described structure exists.
//!
//! Syntax examples:
//!   `database/host`                              — bare path (requires --value)
//!   `database[host='localhost']`                  — set host leaf
//!   `database[host='localhost'][port=1234]`       — set two leaves
//!   `database[user[name='admin'][password='pw']]` — nested structure
//!
//! The expression is parsed into a tree, then flattened into (xpath, value)
//! pairs which are applied as sequential upserts.

use crate::xpath_upsert::{upsert, UpsertError};

/// A single set operation: an XPath targeting a leaf node and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct SetOp {
    pub xpath: String,
    pub value: String,
}

/// Result of applying a declarative set expression.
#[derive(Debug)]
pub struct DeclSetResult {
    /// The final source after all operations.
    pub source: String,
    /// How many upserts were applied.
    pub ops_applied: usize,
    /// Human-readable descriptions of each operation.
    pub descriptions: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a declarative expression into a list of (xpath, value) set operations.
///
/// The expression uses a subset of XPath-like syntax:
///   `name[key='value']`  — predicates specify child values
///   `a/b/c`              — slash-separated path segments
///   `a[b[c='d']]`        — nested predicates for sub-structures
pub fn parse_set_expr(expr: &str) -> Result<Vec<SetOp>, UpsertError> {
    let mut parser = Parser::new(expr);
    let node = parser.parse_path()?;
    parser.expect_end()?;
    let mut ops = Vec::new();
    flatten(&node, "/", &mut ops);
    Ok(ops)
}

/// Apply a declarative set expression to source, returning the modified source.
///
/// Parses the expression, extracts (xpath, value) pairs, and applies them
/// as sequential upserts.
pub fn declarative_set(
    source: &str,
    lang: &str,
    expr: &str,
    explicit_value: Option<&str>,
) -> Result<DeclSetResult, UpsertError> {
    let ops = parse_set_expr(expr)?;

    if ops.is_empty() {
        // No predicates with values — need explicit --value on a bare path
        if let Some(value) = explicit_value {
            // Treat the whole expression as a simple path
            let xpath = format!("//{}", expr);
            let result = upsert(source, lang, &xpath, value, None)?;
            return Ok(DeclSetResult {
                source: result.source,
                ops_applied: 1,
                descriptions: vec![result.description],
            });
        } else {
            return Err(UpsertError::NoInsertionPoint(
                "expression has no value predicates and no --value was given".into(),
            ));
        }
    }

    // If explicit_value is given, it overrides the last leaf's value
    // (e.g., `database/host --value newval` targets //database/host)
    if let Some(value) = explicit_value {
        // With --value, treat the expression as a path, ignore predicates
        let xpath = format!("//{}", strip_predicates(expr));
        let result = upsert(source, lang, &xpath, value, None)?;
        return Ok(DeclSetResult {
            source: result.source,
            ops_applied: 1,
            descriptions: vec![result.description],
        });
    }

    // Apply each set operation sequentially
    let mut current_source = source.to_string();
    let mut descriptions = Vec::new();
    let mut ops_applied = 0;

    for op in &ops {
        let result = upsert(&current_source, lang, &op.xpath, &op.value, None)?;
        if result.source != current_source {
            ops_applied += 1;
            descriptions.push(result.description);
        }
        current_source = result.source;
    }

    Ok(DeclSetResult {
        source: current_source,
        ops_applied,
        descriptions,
    })
}

/// Strip all predicates from a path expression, leaving bare names.
fn strip_predicates(expr: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    for ch in expr.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum PathNode {
    /// A path segment with optional predicates: `name[pred1][pred2]`
    Segment {
        name: String,
        predicates: Vec<Predicate>,
    },
    /// A multi-segment path: `a/b/c`
    Path(Vec<PathNode>),
}

#[derive(Debug, Clone)]
enum Predicate {
    /// `[key='value']` or `[path/to/key='value']` — assign a value
    Assign { path: PathNode, value: String },
    /// `[subexpr]` — ensure a sub-structure exists (has its own predicates)
    Structure(PathNode),
}

// ---------------------------------------------------------------------------
// Flattening: AST → Vec<SetOp>
// ---------------------------------------------------------------------------

fn flatten(node: &PathNode, prefix: &str, ops: &mut Vec<SetOp>) {
    match node {
        PathNode::Segment { name, predicates } => {
            let current_path = if prefix == "/" {
                format!("//{}", name)
            } else {
                format!("{}/{}", prefix, name)
            };

            if predicates.is_empty() {
                // Bare leaf node — ensure it exists with empty value (marker)
                ops.push(SetOp {
                    xpath: current_path.clone(),
                    value: String::new(),
                });
            }

            for pred in predicates {
                match pred {
                    Predicate::Assign { path, value } => {
                        let leaf_xpath = append_path(&current_path, path);
                        ops.push(SetOp {
                            xpath: leaf_xpath,
                            value: value.clone(),
                        });
                    }
                    Predicate::Structure(sub) => {
                        flatten(sub, &current_path, ops);
                    }
                }
            }
        }
        PathNode::Path(segments) => {
            // Build the full prefix from all segments, flattening predicates at each level
            let mut current = prefix.to_string();
            let last_idx = segments.len().saturating_sub(1);
            for (i, seg) in segments.iter().enumerate() {
                match seg {
                    PathNode::Segment { name, predicates } => {
                        current = if current == "/" {
                            format!("//{}", name)
                        } else {
                            format!("{}/{}", current, name)
                        };

                        // Only the terminal segment in a path is a leaf marker;
                        // intermediate segments are structural containers.
                        if predicates.is_empty() && i == last_idx {
                            ops.push(SetOp {
                                xpath: current.clone(),
                                value: String::new(),
                            });
                        }

                        for pred in predicates {
                            match pred {
                                Predicate::Assign { path, value } => {
                                    let leaf_xpath = append_path(&current, path);
                                    ops.push(SetOp {
                                        xpath: leaf_xpath,
                                        value: value.clone(),
                                    });
                                }
                                Predicate::Structure(sub) => {
                                    flatten(sub, &current, ops);
                                }
                            }
                        }
                    }
                    PathNode::Path(_) => {
                        // Nested paths in a path sequence — shouldn't happen from parser,
                        // but handle gracefully
                        flatten(seg, &current, ops);
                    }
                }
            }
        }
    }
}

/// Append a PathNode as path segments to a base xpath.
fn append_path(base: &str, node: &PathNode) -> String {
    match node {
        PathNode::Segment { name, predicates: _ } => {
            format!("{}/{}", base, name)
        }
        PathNode::Path(segments) => {
            let mut result = base.to_string();
            for seg in segments {
                match seg {
                    PathNode::Segment { name, predicates: _ } => {
                        result = format!("{}/{}", result, name);
                    }
                    _ => {}
                }
            }
            result
        }
    }
}

// ---------------------------------------------------------------------------
// Recursive descent parser
// ---------------------------------------------------------------------------

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        Some(ch)
    }

    fn expect_end(&self) -> Result<(), UpsertError> {
        if self.pos < self.chars.len() {
            Err(UpsertError::Parse(format!(
                "unexpected character '{}' at position {}",
                self.chars[self.pos], self.pos
            )))
        } else {
            Ok(())
        }
    }

    /// Parse a path: `segment ('/' segment)*`
    fn parse_path(&mut self) -> Result<PathNode, UpsertError> {
        let first = self.parse_segment()?;
        let mut segments = vec![first];

        while self.peek() == Some('/') {
            self.advance(); // consume '/'
            segments.push(self.parse_segment()?);
        }

        if segments.len() == 1 {
            Ok(segments.pop().unwrap())
        } else {
            Ok(PathNode::Path(segments))
        }
    }

    /// Parse a segment: `NAME predicate*`
    fn parse_segment(&mut self) -> Result<PathNode, UpsertError> {
        let name = self.parse_name()?;
        let mut predicates = Vec::new();

        while self.peek() == Some('[') {
            predicates.push(self.parse_predicate()?);
        }

        Ok(PathNode::Segment { name, predicates })
    }

    /// Parse a name: `[a-zA-Z_][a-zA-Z0-9_.-]*`
    fn parse_name(&mut self) -> Result<String, UpsertError> {
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                name.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        if name.is_empty() {
            Err(UpsertError::Parse(format!(
                "expected name at position {}",
                self.pos
            )))
        } else {
            Ok(name)
        }
    }

    /// Parse a predicate: `'[' expr ']'`
    ///
    /// Inside brackets, we have either:
    ///   - `path = value` → Assign
    ///   - `path` (with its own predicates) → Structure
    fn parse_predicate(&mut self) -> Result<Predicate, UpsertError> {
        self.expect_char('[')?;
        let path = self.parse_path()?;

        if self.peek() == Some('=') {
            self.advance(); // consume '='
            let value = self.parse_value()?;
            self.expect_char(']')?;
            Ok(Predicate::Assign { path, value })
        } else {
            self.expect_char(']')?;
            Ok(Predicate::Structure(path))
        }
    }

    /// Parse a value: quoted string or bare value.
    fn parse_value(&mut self) -> Result<String, UpsertError> {
        match self.peek() {
            Some('\'') => self.parse_quoted('\''),
            Some('"') => self.parse_quoted('"'),
            _ => self.parse_bare_value(),
        }
    }

    fn parse_quoted(&mut self, quote: char) -> Result<String, UpsertError> {
        self.advance(); // consume opening quote
        let mut value = String::new();
        loop {
            match self.advance() {
                Some(ch) if ch == quote => return Ok(value),
                Some(ch) => value.push(ch),
                None => {
                    return Err(UpsertError::Parse(
                        "unterminated quoted string".into(),
                    ))
                }
            }
        }
    }

    fn parse_bare_value(&mut self) -> Result<String, UpsertError> {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            // Bare values end at ] or / or [ or whitespace
            if ch == ']' || ch == '[' || ch == '/' || ch.is_whitespace() {
                break;
            }
            value.push(ch);
            self.advance();
        }
        if value.is_empty() {
            Err(UpsertError::Parse(format!(
                "expected value at position {}",
                self.pos
            )))
        } else {
            Ok(value)
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), UpsertError> {
        match self.advance() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(UpsertError::Parse(format!(
                "expected '{}' but found '{}' at position {}",
                expected, ch, self.pos - 1
            ))),
            None => Err(UpsertError::Parse(format!(
                "expected '{}' but reached end of input",
                expected
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_leaf() {
        let ops = parse_set_expr("database[host='localhost']").unwrap();
        assert_eq!(ops, vec![SetOp {
            xpath: "//database/host".into(),
            value: "localhost".into(),
        }]);
    }

    #[test]
    fn parse_multiple_predicates() {
        let ops = parse_set_expr("database[host='localhost'][port=5432]").unwrap();
        assert_eq!(ops, vec![
            SetOp { xpath: "//database/host".into(), value: "localhost".into() },
            SetOp { xpath: "//database/port".into(), value: "5432".into() },
        ]);
    }

    #[test]
    fn parse_nested_predicates() {
        let ops = parse_set_expr(
            "database[host='localhost'][user[name='admin'][password='secret']]"
        ).unwrap();
        assert_eq!(ops, vec![
            SetOp { xpath: "//database/host".into(), value: "localhost".into() },
            SetOp { xpath: "//database/user/name".into(), value: "admin".into() },
            SetOp { xpath: "//database/user/password".into(), value: "secret".into() },
        ]);
    }

    #[test]
    fn parse_bare_path() {
        // The terminal segment is a marker (empty value)
        let ops = parse_set_expr("database/host").unwrap();
        assert_eq!(ops, vec![SetOp {
            xpath: "//database/host".into(),
            value: "".into(),
        }]);
    }

    #[test]
    fn parse_path_with_value() {
        let ops = parse_set_expr("database/host[port=8080]").unwrap();
        assert_eq!(ops, vec![SetOp {
            xpath: "//database/host/port".into(),
            value: "8080".into(),
        }]);
    }

    #[test]
    fn parse_deeply_nested() {
        let ops = parse_set_expr(
            "a[b[c[d='deep']]]"
        ).unwrap();
        assert_eq!(ops, vec![SetOp {
            xpath: "//a/b/c/d".into(),
            value: "deep".into(),
        }]);
    }

    #[test]
    fn parse_marker_node() {
        // [sub[marker]] — marker is a bare leaf, should produce empty-value op
        let ops = parse_set_expr("parent[sub[marker]]").unwrap();
        assert_eq!(ops, vec![SetOp {
            xpath: "//parent/sub/marker".into(),
            value: "".into(),
        }]);
    }

    #[test]
    fn parse_marker_with_siblings() {
        let ops = parse_set_expr("config[enabled][name='test']").unwrap();
        assert_eq!(ops, vec![
            SetOp { xpath: "//config/enabled".into(), value: "".into() },
            SetOp { xpath: "//config/name".into(), value: "test".into() },
        ]);
    }

    #[test]
    fn parse_double_quoted_value() {
        let ops = parse_set_expr(r#"db[host="localhost"]"#).unwrap();
        assert_eq!(ops, vec![SetOp {
            xpath: "//db/host".into(),
            value: "localhost".into(),
        }]);
    }

    #[test]
    fn parse_complex_expression() {
        let ops = parse_set_expr(
            "database[host='localhost'][port=1234][user[name='dbadmin'][password='1234']]"
        ).unwrap();
        assert_eq!(ops, vec![
            SetOp { xpath: "//database/host".into(), value: "localhost".into() },
            SetOp { xpath: "//database/port".into(), value: "1234".into() },
            SetOp { xpath: "//database/user/name".into(), value: "dbadmin".into() },
            SetOp { xpath: "//database/user/password".into(), value: "1234".into() },
        ]);
    }

    #[test]
    fn strip_predicates_works() {
        assert_eq!(strip_predicates("database[host='x']/port"), "database/port");
        assert_eq!(strip_predicates("a[b[c='d']]/e"), "a/e");
        assert_eq!(strip_predicates("simple/path"), "simple/path");
    }

    // -----------------------------------------------------------------------
    // Integration: declarative_set on real source
    // -----------------------------------------------------------------------

    #[test]
    fn declarative_set_json_new_structure() {
        let source = "{}";
        let result = declarative_set(
            source, "json",
            "database[host='localhost'][port=5432]",
            None,
        ).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["database"]["host"], "localhost");
        assert_eq!(parsed["database"]["port"], "5432");
        assert_eq!(result.ops_applied, 2);
    }

    #[test]
    fn declarative_set_json_nested() {
        let source = "{}";
        let result = declarative_set(
            source, "json",
            "database[host='localhost'][user[name='admin'][password='secret']]",
            None,
        ).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["database"]["host"], "localhost");
        assert_eq!(parsed["database"]["user"]["name"], "admin");
        assert_eq!(parsed["database"]["user"]["password"], "secret");
    }

    #[test]
    fn declarative_set_json_with_explicit_value() {
        let source = r#"{"database": {"host": "old"}}"#;
        let result = declarative_set(
            source, "json",
            "database/host",
            Some("new"),
        ).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["database"]["host"], "new");
    }

    #[test]
    fn declarative_set_yaml_creates_structure() {
        let source = "name: test\n";
        let result = declarative_set(
            source, "yaml",
            "database[host='localhost'][port=5432]",
            None,
        ).unwrap();
        assert!(result.source.contains("host: localhost"), "source: {}", result.source);
        assert!(result.source.contains("port: 5432"), "source: {}", result.source);
    }

    #[test]
    fn declarative_set_updates_existing() {
        let source = r#"{"database": {"host": "old", "port": 1234}}"#;
        let result = declarative_set(
            source, "json",
            "database[host='new']",
            None,
        ).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["database"]["host"], "new");
        // port should be preserved
        assert_eq!(parsed["database"]["port"], 1234);
    }

    #[test]
    fn declarative_set_bare_path_creates_marker() {
        let result = declarative_set("{}", "json", "database/host", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        // Bare path creates structure — the node exists (value may be empty object or string)
        assert!(!parsed["database"]["host"].is_null(), "source: {}", result.source);
    }
}
