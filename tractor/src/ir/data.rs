//! Data-language IR — a format-agnostic typed shape for JSON / YAML /
//! TOML / INI.
//!
//! ## Why a separate type from [`crate::ir::Ir`]
//!
//! The programming-language IR ([`crate::ir::Ir`]) is built around
//! constructs like `Class`, `Function`, `If` — it carries semantic
//! type information specific to programming-language ASTs.
//!
//! Data languages have a much smaller, simpler universe — mappings,
//! sequences, scalars. Reusing the programming-language IR would
//! introduce noise; a focused [`DataIr`] type lets each variant
//! carry only what data languages need.
//!
//! ## Format-agnostic
//!
//! A single [`DataIr`] tree can be rendered to any of:
//! - `<object>/<array>/<property>` XML (JSON syntax branch shape)
//! - `<mapping>/<sequence>/<pair>` XML (YAML syntax branch shape)
//! - data-branch XML where keys become element names (`{a: 1}` →
//!   `<a>1</a>`)
//! - canonical JSON / YAML / TOML text
//!
//! The renderer chooses element names + serialization conventions;
//! the IR is purely structural. Mirrors how Xot serves as a shared
//! container today, but with type-checked variants instead of
//! string-keyed elements.
//!
//! ## Invariants (parsed mode)
//!
//! 1. **Round-trip identity** — `to_source(data_ir, source) ==
//!    source`. The renderer that targets the *original* format
//!    preserves bytes verbatim via `range`-anchored gap text.
//!    Cross-format render (e.g. JSON → YAML) breaks round-trip by
//!    construction; the invariant only holds for same-format
//!    renders.
//! 2. **Source attributes** — every variant carries `range:
//!    ByteRange` and `span: Span` for line/column reporting.
//! 3. **No silent drops** — un-handled CST kinds fall through to
//!    [`DataIr::Unknown`] (visible `<unknown kind="…"/>`).

#![cfg(feature = "native")]

use super::types::{ByteRange, Span};

/// Format-agnostic data-language IR.
#[derive(Debug, Clone)]
pub enum DataIr {
    /// Top-level document. The CST root.
    Document {
        children: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Object / mapping / dict / table — collection of pairs.
    /// Renders as `<object>` (JSON) / `<mapping>` (YAML) / `<table>`
    /// (TOML) / `<section>` (INI) per format choice.
    Mapping {
        pairs: Vec<DataIr>, // each is DataIr::Pair (or Comment)
        range: ByteRange,
        span: Span,
    },

    /// Array / list / sequence — ordered values.
    /// Renders as `<array>` (JSON) / `<sequence>` (YAML).
    Sequence {
        items: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Key-value pair. Key is typically [`DataIr::Scalar`] with a
    /// string value (object keys in JSON are quoted strings; YAML
    /// keys can be other scalars). Value is any [`DataIr`].
    /// Renders as `<property><key>...</key><value>...</value></property>`
    /// in syntax mode; data mode lifts the key string to the
    /// element name.
    Pair {
        key: Box<DataIr>,
        value: Box<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Section header (INI / TOML). Distinct from a Pair containing
    /// a Mapping because the source bytes for `[name]` need to be
    /// preserved.
    Section {
        name: Box<DataIr>, // typically Scalar(String)
        children: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// String scalar. `value` is the *parsed* string (escape
    /// sequences resolved). Source bytes (with quotes / escapes)
    /// recoverable via `range.slice(source)`.
    String {
        value: String,
        range: ByteRange,
        span: Span,
    },

    /// Numeric scalar. We keep the *raw text* rather than parsing
    /// to f64/i64 to avoid lossy round-trip (`1.0` ≠ `1` in YAML
    /// even though both parse to the same number).
    Number {
        text: String,
        range: ByteRange,
        span: Span,
    },

    /// Boolean scalar. The raw text (`"true"` / `"false"` / YAML's
    /// `yes`/`no`/...) is recoverable via the range.
    Bool {
        value: bool,
        range: ByteRange,
        span: Span,
    },

    /// Null / nil literal. JSON `null`, YAML `null` / `~` / empty,
    /// TOML omits but YAML+JSON5 have it.
    Null {
        range: ByteRange,
        span: Span,
    },

    /// Comment (YAML / TOML / INI / JSON5). Text excludes the
    /// leading delimiter (`#` / `//`) — recover it via the range
    /// when the original delimiter matters. `leading` / `trailing`
    /// are inferred during lowering: a comment immediately after a
    /// value on the same line is `trailing: true`.
    Comment {
        text: String,
        leading: bool,
        trailing: bool,
        range: ByteRange,
        span: Span,
    },

    /// YAML / TOML processing directive (e.g. `%YAML 1.2`,
    /// `%TAG !! tag:…`). `flavor` becomes a marker child of the
    /// rendered `<directive>` element so XPath queries can
    /// distinguish: `<directive[yaml]><version>1.2</version></directive>`,
    /// `<directive[tag]><handle>!!</handle><prefix>…</prefix></directive>`.
    Directive {
        flavor: &'static str, // "yaml", "tag", "reserved"
        children: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Generic element with a fixed name plus optional empty
    /// `<marker/>` children and content children. Used for shapes
    /// that don't fit the structural variants above — Markdown's
    /// `<heading[h1]>` / `<list[ordered]>` / `<codeblock>` etc.,
    /// for example.
    ///
    /// Renders as `<{name}><{m1}/><{m2}/>…children…</{name}>`.
    /// Element names are static (declared by the lower fn for the
    /// language) — *not* user-keyed; the keyed renderer's
    /// sanitization rules don't apply.
    Element {
        name: &'static str,
        markers: Vec<&'static str>,
        children: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Last-resort hatch for unhandled kinds. Renders as
    /// `<unknown kind="…">…</unknown>` so XPath queries can still
    /// traverse and source-text recovery still holds.
    Unknown {
        kind: String,
        range: ByteRange,
        span: Span,
    },
}

impl DataIr {
    /// Source byte range covered by this node.
    pub fn range(&self) -> ByteRange {
        match self {
            DataIr::Document { range, .. }
            | DataIr::Mapping { range, .. }
            | DataIr::Sequence { range, .. }
            | DataIr::Pair { range, .. }
            | DataIr::Section { range, .. }
            | DataIr::String { range, .. }
            | DataIr::Number { range, .. }
            | DataIr::Bool { range, .. }
            | DataIr::Null { range, .. }
            | DataIr::Comment { range, .. }
            | DataIr::Directive { range, .. }
            | DataIr::Element { range, .. }
            | DataIr::Unknown { range, .. } => *range,
        }
    }

    /// Source-location span (line / column).
    pub fn span(&self) -> Span {
        match self {
            DataIr::Document { span, .. }
            | DataIr::Mapping { span, .. }
            | DataIr::Sequence { span, .. }
            | DataIr::Pair { span, .. }
            | DataIr::Section { span, .. }
            | DataIr::String { span, .. }
            | DataIr::Number { span, .. }
            | DataIr::Bool { span, .. }
            | DataIr::Null { span, .. }
            | DataIr::Comment { span, .. }
            | DataIr::Directive { span, .. }
            | DataIr::Element { span, .. }
            | DataIr::Unknown { span, .. } => *span,
        }
    }

    /// Round-trip helper: the original source slice covered by this
    /// node. Equivalent to `self.range().slice(source)`.
    pub fn to_source<'a>(&self, source: &'a str) -> &'a str {
        self.range().slice(source)
    }
}

/// How to interpret a scalar value's text when inserting / replacing
/// it via the mutation primitives below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    /// Auto-detect from text: `null` → Null, `true`/`false` → Bool,
    /// parseable-as-f64 → Number, otherwise String.
    Auto,
    String,
    Number,
    Bool,
    Null,
}

impl DataIr {
    /// Build a synthetic scalar `DataIr` with no source coverage.
    /// `range` is zero-width at byte 0; `span` is point (0, 0). Used
    /// by mutation primitives to introduce values that don't exist in
    /// the original source.
    pub fn synthetic_scalar(text: &str, kind: ScalarKind) -> DataIr {
        let range = ByteRange::empty_at(0);
        let span = Span::point(0, 0);
        Self::scalar_with(text, kind, range, span)
    }

    fn scalar_with(text: &str, kind: ScalarKind, range: ByteRange, span: Span) -> DataIr {
        match kind {
            ScalarKind::Auto => match text {
                "null" => DataIr::Null { range, span },
                "true" => DataIr::Bool { value: true, range, span },
                "false" => DataIr::Bool { value: false, range, span },
                _ if !text.is_empty() && text.parse::<f64>().is_ok() => {
                    DataIr::Number { text: text.to_string(), range, span }
                }
                _ => DataIr::String { value: text.to_string(), range, span },
            },
            ScalarKind::String => DataIr::String { value: text.to_string(), range, span },
            ScalarKind::Number => DataIr::Number { text: text.to_string(), range, span },
            ScalarKind::Bool => DataIr::Bool { value: text == "true", range, span },
            ScalarKind::Null => DataIr::Null { range, span },
        }
    }

    /// Walk this tree and return the deepest descendant whose range
    /// starts at exactly `byte_offset`. Used by mutation paths to map
    /// an XPath-derived position back to the typed IR.
    pub fn find_at_offset(&self, byte_offset: u32) -> Option<&DataIr> {
        // Prefer the deepest match: try children first, fall back to self.
        for child in self.children_iter() {
            if let Some(found) = child.find_at_offset(byte_offset) {
                return Some(found);
            }
        }
        if self.range().start == byte_offset {
            Some(self)
        } else {
            None
        }
    }

    /// Mutable counterpart to [`find_at_offset`].
    pub fn find_at_offset_mut(&mut self, byte_offset: u32) -> Option<&mut DataIr> {
        // First do an immutable walk to decide whether any descendant
        // matches. Use that decision to commit the mutable borrow to
        // the right path (Rust can't reborrow `self` after a partial
        // borrow inside a match arm, so we have to know the path
        // up-front).
        let self_starts_here = self.range().start == byte_offset;
        let child_match = self
            .children_iter()
            .any(|c| c.find_at_offset(byte_offset).is_some());
        if !self_starts_here && !child_match {
            return None;
        }
        if !child_match {
            return Some(self);
        }
        // Some descendant matches; drill in.
        match self {
            DataIr::Document { children, .. }
            | DataIr::Sequence { items: children, .. }
            | DataIr::Section { children, .. }
            | DataIr::Directive { children, .. }
            | DataIr::Element { children, .. } => {
                for child in children.iter_mut() {
                    if child.find_at_offset(byte_offset).is_some() {
                        return child.find_at_offset_mut(byte_offset);
                    }
                }
                None
            }
            DataIr::Mapping { pairs, .. } => {
                for child in pairs.iter_mut() {
                    if child.find_at_offset(byte_offset).is_some() {
                        return child.find_at_offset_mut(byte_offset);
                    }
                }
                None
            }
            DataIr::Pair { key, value, .. } => {
                if key.find_at_offset(byte_offset).is_some() {
                    return key.find_at_offset_mut(byte_offset);
                }
                if value.find_at_offset(byte_offset).is_some() {
                    return value.find_at_offset_mut(byte_offset);
                }
                None
            }
            _ => None,
        }
    }

    /// Iterate this node's direct children. Yields nothing for leaf
    /// scalars (`String`, `Number`, `Bool`, `Null`, `Comment`,
    /// `Unknown`).
    pub fn children_iter(&self) -> Box<dyn Iterator<Item = &DataIr> + '_> {
        match self {
            DataIr::Document { children, .. }
            | DataIr::Sequence { items: children, .. }
            | DataIr::Section { children, .. }
            | DataIr::Directive { children, .. }
            | DataIr::Element { children, .. } => Box::new(children.iter()),
            DataIr::Mapping { pairs, .. } => Box::new(pairs.iter()),
            DataIr::Pair { key, value, .. } => {
                Box::new([key.as_ref(), value.as_ref()].into_iter())
            }
            _ => Box::new(std::iter::empty()),
        }
    }

    /// Replace this scalar node in place with a new scalar of the
    /// given kind. Preserves `range` / `span` so callers using the
    /// position for splice operations still work. Returns `Err` if
    /// called on a non-scalar variant.
    pub fn set_scalar(&mut self, text: &str, kind: ScalarKind) -> Result<(), String> {
        if !matches!(
            self,
            DataIr::String { .. }
                | DataIr::Number { .. }
                | DataIr::Bool { .. }
                | DataIr::Null { .. }
        ) {
            return Err(format!(
                "set_scalar: target is not a scalar variant (got {:?})",
                std::mem::discriminant(self)
            ));
        }
        let range = self.range();
        let span = self.span();
        *self = Self::scalar_with(text, kind, range, span);
        Ok(())
    }

    /// Replace a Pair's value in place with a new scalar. Returns
    /// `Err` if called on a non-Pair variant.
    pub fn set_pair_value(&mut self, text: &str, kind: ScalarKind) -> Result<(), String> {
        match self {
            DataIr::Pair { value, .. } => {
                let range = value.range();
                let span = value.span();
                *value = Box::new(Self::scalar_with(text, kind, range, span));
                Ok(())
            }
            _ => Err("set_pair_value: not a Pair node".to_string()),
        }
    }

    /// Append a nested key path ending in a scalar leaf. Operates on
    /// `Document`, `Mapping`, or `Section` (each conceptually a key/
    /// value collection). For `Document`, the new pair is added as a
    /// direct child (the natural place for top-level JSON objects);
    /// for `Mapping` / `Section`, it's appended to the pair list.
    ///
    /// Each intermediate key creates a `Pair { key: String, value:
    /// Mapping }`; the leaf creates a `Pair { key, value: scalar }`.
    /// All new nodes have synthetic ranges / spans (zero-width at 0).
    pub fn insert_nested_pair(
        &mut self,
        keys: &[&str],
        value: &str,
        kind: ScalarKind,
    ) -> Result<(), String> {
        if keys.is_empty() {
            return Err("insert_nested_pair: keys is empty".to_string());
        }
        let leaf_pair = build_nested_pair(keys, value, kind);
        match self {
            DataIr::Document { children, .. } => {
                children.push(leaf_pair);
                Ok(())
            }
            DataIr::Mapping { pairs, .. } => {
                pairs.push(leaf_pair);
                Ok(())
            }
            DataIr::Section { children, .. } => {
                children.push(leaf_pair);
                Ok(())
            }
            other => Err(format!(
                "insert_nested_pair: target is not a key/value container (got {:?})",
                std::mem::discriminant(other)
            )),
        }
    }
}

fn build_nested_pair(keys: &[&str], value: &str, kind: ScalarKind) -> DataIr {
    let range = ByteRange::empty_at(0);
    let span = Span::point(0, 0);

    let (head, tail) = keys.split_first().expect("keys is non-empty");
    let key_node = DataIr::String {
        value: head.to_string(),
        range,
        span,
    };
    let value_node = if tail.is_empty() {
        DataIr::scalar_with(value, kind, range, span)
    } else {
        DataIr::Mapping {
            pairs: vec![build_nested_pair(tail, value, kind)],
            range,
            span,
        }
    };
    DataIr::Pair {
        key: Box::new(key_node),
        value: Box::new(value_node),
        range,
        span,
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;
    use crate::ir::lower_json_data_root;

    fn lower(src: &str) -> DataIr {
        let language = tree_sitter_json::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(src, None).unwrap();
        lower_json_data_root(tree.root_node(), src)
    }

    #[test]
    fn synthetic_scalar_auto_detects() {
        match DataIr::synthetic_scalar("null", ScalarKind::Auto) {
            DataIr::Null { .. } => {}
            other => panic!("expected Null, got {:?}", other),
        }
        match DataIr::synthetic_scalar("42", ScalarKind::Auto) {
            DataIr::Number { text, .. } => assert_eq!(text, "42"),
            other => panic!("expected Number, got {:?}", other),
        }
        match DataIr::synthetic_scalar("true", ScalarKind::Auto) {
            DataIr::Bool { value: true, .. } => {}
            other => panic!("expected Bool(true), got {:?}", other),
        }
        match DataIr::synthetic_scalar("hello", ScalarKind::Auto) {
            DataIr::String { value, .. } => assert_eq!(value, "hello"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn synthetic_scalar_kind_overrides_auto_detection() {
        // Forcing String keeps "true" as a string.
        match DataIr::synthetic_scalar("true", ScalarKind::String) {
            DataIr::String { value, .. } => assert_eq!(value, "true"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn find_at_offset_drills_to_deepest_match() {
        let src = r#"{"name": "Alice"}"#;
        let ir = lower(src);
        // The String "Alice" starts at byte 9 (the opening quote).
        let at_value = ir.find_at_offset(9).expect("value found");
        match at_value {
            DataIr::String { value, .. } => assert_eq!(value, "Alice"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn set_scalar_replaces_in_place_preserving_range() {
        let src = r#"{"name": "Alice"}"#;
        let mut ir = lower(src);
        let value_offset = 9;
        let original_range = ir.find_at_offset(value_offset).unwrap().range();
        let target = ir.find_at_offset_mut(value_offset).unwrap();
        target.set_scalar("Bob", ScalarKind::String).unwrap();
        let after = ir.find_at_offset(value_offset).unwrap();
        assert_eq!(after.range(), original_range);
        match after {
            DataIr::String { value, .. } => assert_eq!(value, "Bob"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn set_scalar_errors_on_non_scalar() {
        let src = r#"{"name": "Alice"}"#;
        let mut ir = lower(src);
        let err = ir.set_scalar("oops", ScalarKind::String).unwrap_err();
        assert!(err.contains("not a scalar"));
    }

    #[test]
    fn insert_nested_pair_extends_top_level_mapping() {
        let src = r#"{"name": "Alice"}"#;
        let mut ir = lower(src);
        // Find the top-level mapping (it's a child of Document).
        let mapping = match &mut ir {
            DataIr::Document { children, .. } => &mut children[0],
            _ => panic!("expected Document"),
        };
        mapping
            .insert_nested_pair(&["age"], "30", ScalarKind::Auto)
            .unwrap();
        match mapping {
            DataIr::Mapping { pairs, .. } => {
                assert_eq!(pairs.len(), 2);
                let last = &pairs[1];
                match last {
                    DataIr::Pair { key, value, .. } => {
                        match key.as_ref() {
                            DataIr::String { value: k, .. } => assert_eq!(k, "age"),
                            other => panic!("expected key String, got {:?}", other),
                        }
                        match value.as_ref() {
                            DataIr::Number { text, .. } => assert_eq!(text, "30"),
                            other => panic!("expected value Number, got {:?}", other),
                        }
                    }
                    other => panic!("expected Pair, got {:?}", other),
                }
            }
            other => panic!("expected Mapping, got {:?}", other),
        }
    }

    #[test]
    fn insert_nested_pair_creates_inner_mapping_for_multi_key_path() {
        let src = r#"{"name": "Alice"}"#;
        let mut ir = lower(src);
        let mapping = match &mut ir {
            DataIr::Document { children, .. } => &mut children[0],
            _ => panic!("expected Document"),
        };
        mapping
            .insert_nested_pair(&["db", "host"], "localhost", ScalarKind::String)
            .unwrap();
        match mapping {
            DataIr::Mapping { pairs, .. } => {
                let added = &pairs[1];
                match added {
                    DataIr::Pair { value, .. } => match value.as_ref() {
                        DataIr::Mapping { pairs: inner, .. } => {
                            assert_eq!(inner.len(), 1);
                        }
                        other => panic!("expected inner Mapping, got {:?}", other),
                    },
                    other => panic!("expected Pair, got {:?}", other),
                }
            }
            _ => panic!("expected Mapping"),
        }
    }
}
