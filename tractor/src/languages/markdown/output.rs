//! Output element names — Markdown's vocabulary after transform.
//!
//! Closed vocabulary: every name the markdown transform emits is
//! listed here. Unlike the data languages (JSON / YAML / TOML / INI),
//! markdown's element names are not user-data driven — they're
//! structural (`<heading>`, `<list>`, `<link>`, …).
//!
//! No `NodeSpec` table yet (markdown doesn't currently declare a
//! semantic-vocabulary spec); these constants exist so `rules.rs`
//! and `transformations.rs` can reference output names symbolically.

// Headings
pub const HEADING: &str = "heading";

// Code
pub const CODE_BLOCK: &str = "code_block";
pub const CODE: &str = "code";
pub const LANGUAGE: &str = "language";

// Emphasis
pub const EMPHASIS: &str = "emphasis";
pub const STRONG: &str = "strong";
pub const STRIKETHROUGH: &str = "strikethrough";

// Links and images
pub const LINK: &str = "link";
pub const IMAGE: &str = "image";
pub const TEXT: &str = "text";
pub const DESTINATION: &str = "destination";
pub const TITLE: &str = "title";
pub const LABEL: &str = "label";
pub const REFERENCE: &str = "reference";

// Lists
pub const LIST: &str = "list";
pub const ITEM: &str = "item";
pub const ORDERED: &str = "ordered";
pub const UNORDERED: &str = "unordered";
pub const CHECKED: &str = "checked";
pub const UNCHECKED: &str = "unchecked";

// Block-level
pub const BLOCKQUOTE: &str = "blockquote";
pub const HR: &str = "hr";
pub const BR: &str = "br";

// Tables
pub const TABLE: &str = "table";
pub const THEAD: &str = "thead";
pub const ROW: &str = "row";
pub const CELL: &str = "cell";

// HTML / latex / metadata
pub const HTML: &str = "html";
pub const FRONTMATTER: &str = "frontmatter";
pub const LATEX: &str = "latex";

// Escapes / entities
pub const ESCAPE: &str = "escape";
pub const ENTITY: &str = "entity";
