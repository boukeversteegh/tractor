//! Data-language tree — a format-agnostic typed shape for JSON / YAML /
//! TOML / INI.
//!
//! ## Why a separate type from [`crate::tree::SyntaxTree`]
//!
//! The programming-language tree ([`crate::tree::SyntaxTree`]) is built around
//! constructs like `Class`, `Function`, `If` — it carries semantic
//! type information specific to programming-language ASTs.
//!
//! Data languages have a much smaller, simpler universe — mappings,
//! sequences, scalars. Reusing the programming-language tree would
//! introduce noise; a focused [`DataTree`] type lets each variant
//! carry only what data languages need.
//!
//! ## Format-agnostic
//!
//! A single [`DataTree`] tree can be rendered to any of:
//! - `<object>/<array>/<property>` XML (JSON syntax branch shape)
//! - `<mapping>/<sequence>/<pair>` XML (YAML syntax branch shape)
//! - data-branch XML where keys become element names (`{a: 1}` →
//!   `<a>1</a>`)
//! - canonical JSON / YAML / TOML text
//!
//! The renderer chooses element names + serialization conventions;
//! the tree is purely structural. Mirrors how Xot serves as a shared
//! container today, but with type-checked variants instead of
//! string-keyed elements.
//!
//! ## Invariants (parsed mode)
//!
//! 1. **Round-trip identity** — `to_source(data_tree, source) ==
//!    source`. The renderer that targets the *original* format
//!    preserves bytes verbatim via `range`-anchored gap text.
//!    Cross-format render (e.g. JSON → YAML) breaks round-trip by
//!    construction; the invariant only holds for same-format
//!    renders.
//! 2. **Source attributes** — every variant carries `range:
//!    ByteRange` and `span: Span` for line/column reporting.
//! 3. **No silent drops** — un-handled CST kinds fall through to
//!    [`DataTree::Unknown`] (visible `<unknown kind="…"/>`).


use crate::tree::types::{ByteRange, QuoteStyle, Span, TreeNode};

/// Format-agnostic data-language tree.
#[derive(Debug, Clone)]
pub enum DataTree {
    /// Top-level document. The CST root.
    Document {
        children: Vec<DataTree>,
        range: ByteRange,
        span: Span,
    },

    /// Object / mapping / dict / table — collection of pairs.
    /// Renders as `<object>` (JSON) / `<mapping>` (YAML) / `<table>`
    /// (TOML) / `<section>` (INI) per format choice.
    Mapping {
        pairs: Vec<DataTree>, // each is DataTree::Pair (or Comment)
        range: ByteRange,
        span: Span,
    },

    /// Array / list / sequence — ordered values.
    /// Renders as `<array>` (JSON) / `<sequence>` (YAML).
    Sequence {
        items: Vec<DataTree>,
        range: ByteRange,
        span: Span,
    },

    /// Key-value pair. Key is typically [`DataTree::Scalar`] with a
    /// string value (object keys in JSON are quoted strings; YAML
    /// keys can be other scalars). Value is any [`DataTree`].
    /// Renders as `<property><key>...</key><value>...</value></property>`
    /// in syntax mode; data mode lifts the key string to the
    /// element name.
    Pair {
        key: Box<DataTree>,
        value: Box<DataTree>,
        range: ByteRange,
        span: Span,
    },

    /// Section header (INI / TOML). Distinct from a Pair containing
    /// a Mapping because the source bytes for `[name]` need to be
    /// preserved.
    Section {
        name: Box<DataTree>, // typically Scalar(String)
        children: Vec<DataTree>,
        range: ByteRange,
        span: Span,
    },

    /// String scalar. `value` is the *parsed* string (escape sequences
    /// resolved); `quote_style` preserves the surface form so the
    /// renderer can reproduce JSON `"x"` vs YAML `'x'` vs YAML plain
    /// `x` vs TOML multiline `"""x"""` byte-identically (S13/DataTree).
    /// Source bytes (with quotes / escapes) are also recoverable via
    /// `range.slice(source)` when anchored.
    String {
        value: String,
        quote_style: QuoteStyle,
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

    /// Boolean scalar. `value` is the decoded boolean; `text` preserves
    /// the surface form so YAML `yes`/`no`/`True`/`true`/`Y` round-
    /// trip distinctly. The renderer emits the stored `text` when
    /// anchored and the value's canonical spelling when synthetic.
    Bool {
        value: bool,
        text: String,
        range: ByteRange,
        span: Span,
    },

    /// Null / nil literal. JSON `null`, YAML `null` / `~` / empty,
    /// TOML omits but YAML+JSON5 have it. `text` preserves the surface
    /// form so YAML `~` vs `null` vs `Null` vs empty round-trip
    /// distinctly.
    Null {
        text: String,
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
        children: Vec<DataTree>,
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
    ///
    /// @element_name = element_name_for_data_element
    Element {
        name: &'static str,
        markers: Vec<&'static str>,
        children: Vec<DataTree>,
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

impl DataTree {
    /// Source byte range covered by this node.
    pub fn range(&self) -> ByteRange {
        match self {
            DataTree::Document { range, .. }
            | DataTree::Mapping { range, .. }
            | DataTree::Sequence { range, .. }
            | DataTree::Pair { range, .. }
            | DataTree::Section { range, .. }
            | DataTree::String { range, .. }
            | DataTree::Number { range, .. }
            | DataTree::Bool { range, .. }
            | DataTree::Null { range, .. }
            | DataTree::Comment { range, .. }
            | DataTree::Directive { range, .. }
            | DataTree::Element { range, .. }
            | DataTree::Unknown { range, .. } => *range,
        }
    }

    /// Source-location span (line / column).
    pub fn span(&self) -> Span {
        match self {
            DataTree::Document { span, .. }
            | DataTree::Mapping { span, .. }
            | DataTree::Sequence { span, .. }
            | DataTree::Pair { span, .. }
            | DataTree::Section { span, .. }
            | DataTree::String { span, .. }
            | DataTree::Number { span, .. }
            | DataTree::Bool { span, .. }
            | DataTree::Null { span, .. }
            | DataTree::Comment { span, .. }
            | DataTree::Directive { span, .. }
            | DataTree::Element { span, .. }
            | DataTree::Unknown { span, .. } => *span,
        }
    }

    /// Round-trip helper: the original source slice covered by this
    /// node. Equivalent to `self.range().slice(source)`.
    pub fn to_source<'a>(&self, source: &'a str) -> &'a str {
        self.range().slice(source)
    }

    /// True iff this node's range refers to a real source position.
    /// False for synthetic nodes built without source (programmatic
    /// mutation, tests). Mirrors `SyntaxTree::is_anchored()`.
    pub fn is_anchored(&self) -> bool {
        self.range().is_anchored()
    }

    /// Stored text for scalar-leaf variants. `String` returns the
    /// *decoded* value (no surrounding quotes), `Number` the verbatim
    /// numeric text, `Bool`/`Null` their surface text. Returns `None`
    /// for compound variants. The renderer uses this to emit leaf
    /// content without consulting the source string.
    pub fn scalar_text(&self) -> Option<&str> {
        match self {
            DataTree::String { value, .. } => Some(value.as_str()),
            DataTree::Number { text, .. }
            | DataTree::Bool { text, .. }
            | DataTree::Null { text, .. }
            | DataTree::Comment { text, .. } => Some(text.as_str()),
            _ => None,
        }
    }
}

/// Override for [`DataTree::Element`]: open-set `name` string used
/// by Markdown / YAML directive shapes (`<heading>`, `<list>`,
/// `<codeblock>`, ...). The variant carries the name as a
/// `&'static str` field, so this just forwards it — no leak.
pub fn element_name_for_data_element(t: &DataTree) -> &'static str {
    if let DataTree::Element { name, .. } = t {
        *name
    } else {
        "element"
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

impl DataTree {
    /// Build a synthetic scalar `DataTree` with no source coverage.
    /// `range` is `synthetic_empty()`; `span` is point (0, 0). Used by
    /// mutation primitives to introduce values that don't exist in the
    /// original source.
    pub fn synthetic_scalar(text: &str, kind: ScalarKind) -> DataTree {
        let range = ByteRange::synthetic_empty();
        let span = Span::point(0, 0);
        Self::scalar_with(text, kind, range, span)
    }

    fn scalar_with(text: &str, kind: ScalarKind, range: ByteRange, span: Span) -> DataTree {
        match kind {
            ScalarKind::Auto => match text {
                "null" => DataTree::Null { text: text.to_string(), range, span },
                "true" => DataTree::Bool { value: true, text: text.to_string(), range, span },
                "false" => DataTree::Bool { value: false, text: text.to_string(), range, span },
                _ if !text.is_empty() && text.parse::<f64>().is_ok() => {
                    DataTree::Number { text: text.to_string(), range, span }
                }
                _ => DataTree::String {
                    value: text.to_string(),
                    quote_style: QuoteStyle::default_double(),
                    range,
                    span,
                },
            },
            ScalarKind::String => DataTree::String {
                value: text.to_string(),
                quote_style: QuoteStyle::default_double(),
                range,
                span,
            },
            ScalarKind::Number => DataTree::Number { text: text.to_string(), range, span },
            ScalarKind::Bool => DataTree::Bool {
                value: text == "true",
                text: text.to_string(),
                range,
                span,
            },
            ScalarKind::Null => DataTree::Null { text: text.to_string(), range, span },
        }
    }

    /// Walk this tree and return the deepest descendant whose range
    /// starts at exactly `byte_offset`. Used by mutation paths to map
    /// an XPath-derived position back to the typed tree.
    pub fn find_at_offset(&self, byte_offset: u32) -> Option<&DataTree> {
        // Prefer the deepest match: try children first, fall back to self.
        for child in self.children() {
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
    pub fn find_at_offset_mut(&mut self, byte_offset: u32) -> Option<&mut DataTree> {
        // First do an immutable walk to decide whether any descendant
        // matches. Use that decision to commit the mutable borrow to
        // the right path (Rust can't reborrow `self` after a partial
        // borrow inside a match arm, so we have to know the path
        // up-front).
        let self_starts_here = self.range().start == byte_offset;
        let child_match = self
            .children()
            .into_iter()
            .any(|c| c.find_at_offset(byte_offset).is_some());
        if !self_starts_here && !child_match {
            return None;
        }
        if !child_match {
            return Some(self);
        }
        // Some descendant matches; drill in.
        match self {
            DataTree::Document { children, .. }
            | DataTree::Sequence { items: children, .. }
            | DataTree::Section { children, .. }
            | DataTree::Directive { children, .. }
            | DataTree::Element { children, .. } => {
                for child in children.iter_mut() {
                    if child.find_at_offset(byte_offset).is_some() {
                        return child.find_at_offset_mut(byte_offset);
                    }
                }
                None
            }
            DataTree::Mapping { pairs, .. } => {
                for child in pairs.iter_mut() {
                    if child.find_at_offset(byte_offset).is_some() {
                        return child.find_at_offset_mut(byte_offset);
                    }
                }
                None
            }
            DataTree::Pair { key, value, .. } => {
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

}

impl TreeNode for DataTree {
    fn span(&self) -> Span { self.span() }
    fn range(&self) -> ByteRange { self.range() }

    fn span_mut(&mut self) -> &mut Span {
        match self {
            DataTree::Document { span, .. }
            | DataTree::Mapping { span, .. }
            | DataTree::Sequence { span, .. }
            | DataTree::Pair { span, .. }
            | DataTree::Section { span, .. }
            | DataTree::String { span, .. }
            | DataTree::Number { span, .. }
            | DataTree::Bool { span, .. }
            | DataTree::Null { span, .. }
            | DataTree::Comment { span, .. }
            | DataTree::Directive { span, .. }
            | DataTree::Element { span, .. }
            | DataTree::Unknown { span, .. } => span,
        }
    }

    fn children(&self) -> Vec<&Self> {
        let mut v: Vec<&DataTree> = Vec::new();
        match self {
            DataTree::Document { children, .. }
            | DataTree::Section { children, .. }
            | DataTree::Directive { children, .. }
            | DataTree::Element { children, .. } => v.extend(children.iter()),
            DataTree::Sequence { items, .. } => v.extend(items.iter()),
            DataTree::Mapping { pairs, .. } => v.extend(pairs.iter()),
            DataTree::Pair { key, value, .. } => {
                v.push(key);
                v.push(value);
            }
            DataTree::String { .. }
            | DataTree::Number { .. }
            | DataTree::Bool { .. }
            | DataTree::Null { .. }
            | DataTree::Comment { .. }
            | DataTree::Unknown { .. } => {}
        }
        v.sort_by_key(|c| c.range().start);
        v
    }

    fn children_mut(&mut self) -> Vec<&mut Self> {
        let mut v: Vec<&mut DataTree> = Vec::new();
        match self {
            DataTree::Document { children, .. }
            | DataTree::Section { children, .. }
            | DataTree::Directive { children, .. }
            | DataTree::Element { children, .. } => v.extend(children.iter_mut()),
            DataTree::Sequence { items, .. } => v.extend(items.iter_mut()),
            DataTree::Mapping { pairs, .. } => v.extend(pairs.iter_mut()),
            DataTree::Pair { key, value, .. } => {
                v.push(key.as_mut());
                v.push(value.as_mut());
            }
            DataTree::String { .. }
            | DataTree::Number { .. }
            | DataTree::Bool { .. }
            | DataTree::Null { .. }
            | DataTree::Comment { .. }
            | DataTree::Unknown { .. } => {}
        }
        v.sort_by_key(|c| c.range().start);
        v
    }
}

impl DataTree {
    /// Replace this scalar node in place with a new scalar of the
    /// given kind. Preserves `range` / `span` so callers using the
    /// position for splice operations still work. Returns `Err` if
    /// called on a non-scalar variant.
    pub fn set_scalar(&mut self, text: &str, kind: ScalarKind) -> Result<(), String> {
        if !matches!(
            self,
            DataTree::String { .. }
                | DataTree::Number { .. }
                | DataTree::Bool { .. }
                | DataTree::Null { .. }
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
            DataTree::Pair { value, .. } => {
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
            DataTree::Document { children, .. } => {
                children.push(leaf_pair);
                Ok(())
            }
            DataTree::Mapping { pairs, .. } => {
                pairs.push(leaf_pair);
                Ok(())
            }
            DataTree::Section { children, .. } => {
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

fn build_nested_pair(keys: &[&str], value: &str, kind: ScalarKind) -> DataTree {
    let range = ByteRange::empty_at(0);
    let span = Span::point(0, 0);

    let (head, tail) = keys.split_first().expect("keys is non-empty");
    let key_node = DataTree::String {
        value: head.to_string(),
        quote_style: QuoteStyle::default_double(),
        range,
        span,
    };
    let value_node = if tail.is_empty() {
        DataTree::scalar_with(value, kind, range, span)
    } else {
        DataTree::Mapping {
            pairs: vec![build_nested_pair(tail, value, kind)],
            range,
            span,
        }
    };
    DataTree::Pair {
        key: Box::new(key_node),
        value: Box::new(value_node),
        range,
        span,
    }
}

#[cfg(all(test, feature = "native"))]
mod mutation_tests {
    use super::*;
    use crate::tree::lower_json_data_root;

    fn lower(src: &str) -> DataTree {
        let language = tree_sitter_json::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(src, None).unwrap();
        lower_json_data_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), src), src)
    }

    #[test]
    fn synthetic_scalar_auto_detects() {
        match DataTree::synthetic_scalar("null", ScalarKind::Auto) {
            DataTree::Null { .. } => {}
            other => panic!("expected Null, got {:?}", other),
        }
        match DataTree::synthetic_scalar("42", ScalarKind::Auto) {
            DataTree::Number { text, .. } => assert_eq!(text, "42"),
            other => panic!("expected Number, got {:?}", other),
        }
        match DataTree::synthetic_scalar("true", ScalarKind::Auto) {
            DataTree::Bool { value: true, .. } => {}
            other => panic!("expected Bool(true), got {:?}", other),
        }
        match DataTree::synthetic_scalar("hello", ScalarKind::Auto) {
            DataTree::String { value, .. } => assert_eq!(value, "hello"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn synthetic_scalar_kind_overrides_auto_detection() {
        // Forcing String keeps "true" as a string.
        match DataTree::synthetic_scalar("true", ScalarKind::String) {
            DataTree::String { value, .. } => assert_eq!(value, "true"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn find_at_offset_drills_to_deepest_match() {
        let src = r#"{"name": "Alice"}"#;
        let tree = lower(src);
        // The String "Alice" starts at byte 9 (the opening quote).
        let at_value = tree.find_at_offset(9).expect("value found");
        match at_value {
            DataTree::String { value, .. } => assert_eq!(value, "Alice"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn set_scalar_replaces_in_place_preserving_range() {
        let src = r#"{"name": "Alice"}"#;
        let mut tree = lower(src);
        let value_offset = 9;
        let original_range = tree.find_at_offset(value_offset).unwrap().range();
        let target = tree.find_at_offset_mut(value_offset).unwrap();
        target.set_scalar("Bob", ScalarKind::String).unwrap();
        let after = tree.find_at_offset(value_offset).unwrap();
        assert_eq!(after.range(), original_range);
        match after {
            DataTree::String { value, .. } => assert_eq!(value, "Bob"),
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn set_scalar_errors_on_non_scalar() {
        let src = r#"{"name": "Alice"}"#;
        let mut tree = lower(src);
        let err = tree.set_scalar("oops", ScalarKind::String).unwrap_err();
        assert!(err.contains("not a scalar"));
    }

    #[test]
    fn insert_nested_pair_extends_top_level_mapping() {
        let src = r#"{"name": "Alice"}"#;
        let mut tree = lower(src);
        // Find the top-level mapping (it's a child of Document).
        let mapping = match &mut tree {
            DataTree::Document { children, .. } => &mut children[0],
            _ => panic!("expected Document"),
        };
        mapping
            .insert_nested_pair(&["age"], "30", ScalarKind::Auto)
            .unwrap();
        match mapping {
            DataTree::Mapping { pairs, .. } => {
                assert_eq!(pairs.len(), 2);
                let last = &pairs[1];
                match last {
                    DataTree::Pair { key, value, .. } => {
                        match key.as_ref() {
                            DataTree::String { value: k, .. } => assert_eq!(k, "age"),
                            other => panic!("expected key String, got {:?}", other),
                        }
                        match value.as_ref() {
                            DataTree::Number { text, .. } => assert_eq!(text, "30"),
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
        let mut tree = lower(src);
        let mapping = match &mut tree {
            DataTree::Document { children, .. } => &mut children[0],
            _ => panic!("expected Document"),
        };
        mapping
            .insert_nested_pair(&["db", "host"], "localhost", ScalarKind::String)
            .unwrap();
        match mapping {
            DataTree::Mapping { pairs, .. } => {
                let added = &pairs[1];
                match added {
                    DataTree::Pair { value, .. } => match value.as_ref() {
                        DataTree::Mapping { pairs: inner, .. } => {
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
