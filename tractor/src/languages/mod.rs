//! Language-specific transform modules and metadata.
//!
//! Each language owns its complete transform logic.
//! The shared infrastructure (crate::transform) provides only the walker and helpers.

pub mod info;
pub mod comments;
pub mod rule;
pub mod typescript;
pub mod csharp;
pub mod python;
pub mod go;
pub mod rust_lang;
pub mod java;
pub mod ruby;
pub mod php;
pub mod json;
pub mod yaml;
pub mod toml;
pub mod ini;
pub mod env;
pub mod markdown;
pub mod tsql;

use xot::{Xot, Node as XotNode};
use crate::transform::TransformAction;
use crate::output::syntax_highlight::SyntaxCategory;

/// Per-name metadata for a language's semantic vocabulary.
///
/// Single source of truth for every element name the transform can
/// emit, tagged with whether it's ever used as an empty marker, ever
/// used as a structural container, and its syntax-highlighting
/// category.
///
/// `marker` and `container` are NOT mutually exclusive — a dual-use
/// name (e.g. Python's `static`, `list`, `dict`, `set`; C#'s `class`)
/// sets both true. The typed booleans replace the "marker_only"
/// vs "ALL_NAMES" duplication and the comment-documented dual-use
/// notes.
#[derive(Debug, Clone, Copy)]
pub struct TractorNodeSpec {
    pub name: &'static str,
    pub marker: bool,
    pub container: bool,
    pub syntax: SyntaxCategory,
}

/// Per-name role classification — phase 2 of the transform-validation
/// architecture (`docs/transform-validation-architecture.md` § 4).
///
/// Derived from `(marker, container)` for now so per-language enums
/// don't need to declare it explicitly. As phase 2 progresses,
/// stronger roles like `TextLeaf` (a `ContainerOnly` whose content is
/// text-only, no element children — e.g. `<name>`) and
/// `SlotWrapper { parents }` (a singleton role-slot under a specific
/// parent — e.g. `<condition>` under `<if>`) will need explicit
/// declaration; for now, `role()` returns `ContainerOnly` for those
/// and they're handled by hand-coded invariants in
/// `tree_invariants.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Empty element only (no text, no element children). E.g. `<async/>`.
    MarkerOnly,
    /// Container with content (text or element children). E.g. `<call>`.
    ContainerOnly,
    /// Both marker AND wrapper forms valid. E.g. `<new/>` AND
    /// `<new>...</new>`. Implies the marker+wrapper collision invariant
    /// is suppressed for this name.
    DualUse,
    /// Neither declared as marker nor container — the spec is
    /// underspecified. Treat as `ContainerOnly` for now; tighten via
    /// explicit declaration in phase 2.
    Unspecified,
}

impl TractorNodeSpec {
    /// Derive role from the legacy `(marker, container)` booleans.
    pub fn role(&self) -> NodeRole {
        match (self.marker, self.container) {
            (true, false)  => NodeRole::MarkerOnly,
            (false, true)  => NodeRole::ContainerOnly,
            (true, true)   => NodeRole::DualUse,
            (false, false) => NodeRole::Unspecified,
        }
    }
}

/// Type alias for language transform functions
pub type TransformFn = fn(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>;

/// Type alias for syntax category mapping functions
/// Maps a transformed element name to a syntax category for highlighting
pub type SyntaxCategoryFn = fn(&str) -> SyntaxCategory;

/// Type alias for per-language TractorNodeSpec lookup.
pub type TractorNodeSpecLookupFn = fn(&str) -> Option<&'static TractorNodeSpec>;

// --- Tree-sitter and tree metadata (native only) -------------------------------

/// Function pointer that produces a tree-sitter [`Language`](tree_sitter::Language)
/// for a row in [`LANGUAGES`]. One thin shim per language wraps the
/// corresponding `tree_sitter_<lang>::LANGUAGE` constant — the
/// constant itself isn't stable enough to embed in a `const` literal
/// (it carries an `Arc`-like state), so we go through a `fn()` indirection.
#[cfg(feature = "native")]
pub type GrammarFn = fn() -> tree_sitter::Language;

/// CST → [`SyntaxTree`](crate::tree::SyntaxTree) lowering function pointer (programming languages).
#[cfg(feature = "native")]
pub type LowerToSyntaxTree = for<'a> fn(tree_sitter::Node<'a>, &'a str) -> crate::tree::SyntaxTree;

/// CST → [`DataTree`](crate::tree::DataTree) lowering function pointer (data languages).
#[cfg(feature = "native")]
pub type LowerToDataTree = for<'a> fn(tree_sitter::Node<'a>, &'a str) -> crate::tree::DataTree;

/// CST → [`SqlTree`](crate::tree::sql::SqlTree) lowering function pointer (SQL family).
#[cfg(feature = "native")]
pub type LowerToSqlTree = for<'a> fn(tree_sitter::Node<'a>, &'a str) -> crate::tree::sql::SqlTree;

/// Renderer for a `DataTree` tree. Each [`DataParser`] pairs a lower
/// fn with one of these so the pipeline never needs a per-language
/// match to choose a renderer.
#[cfg(feature = "native")]
pub type DataRenderFn = fn(
    &mut xot::Xot,
    xot::Node,
    &crate::tree::DataTree,
    &str,
) -> Result<xot::Node, xot::Error>;

/// One parsing+rendering path a data language supports. Data
/// languages register two of these (one per tree mode); the pipeline
/// picks based on `--tree=structure` vs `--tree=data`.
#[cfg(feature = "native")]
#[derive(Copy, Clone)]
pub struct DataParser {
    pub lower: LowerToDataTree,
    pub render: DataRenderFn,
}

/// Which language family a language belongs to. The variant carries
/// the per-language CST→tree lower fn(s) so dispatch never needs a
/// separate match by language name.
///
/// `Syntax` and `Sql` are single-lowering: only one tree mode
/// (Structure) makes sense, and the lower fn is unique.
///
/// `Data` is two-lowering: data languages support both
/// `--tree=structure` (syntax-tree view) and `--tree=data` (content
/// view). Each tree mode is its own parser+renderer pair, hence the
/// `structure` and `content` fields. Today, the `content` parser is
/// declared but not yet wired — Data-mode parses still fall back to
/// the legacy imperative path until that work lands.
///
/// `None` means the language stays on the legacy imperative path
/// entirely.
#[cfg(feature = "native")]
#[derive(Copy, Clone)]
pub enum TreeKind {
    None,
    Syntax(LowerToSyntaxTree),
    Sql(LowerToSqlTree),
    Data {
        structure: DataParser,
        content: DataParser,
    },
}

#[cfg(feature = "native")]
impl LanguageOps {
    /// True iff this language should run through the typed-tree pipeline
    /// at the given tree mode. Reads `tree_kind` from the registry —
    /// no per-language match needed at the call site.
    ///
    /// - `TreeKind::None` → never; the legacy imperative path handles it.
    /// - `TreeKind::Syntax` / `TreeKind::Sql` → any non-Raw mode.
    /// - `TreeKind::Data` → both `Structure` and `Data` modes (the
    ///   variant carries one parser per mode).
    pub fn uses_tree(&self, tree_mode: crate::tree_mode::TreeMode) -> bool {
        use crate::tree_mode::TreeMode;
        match self.tree_kind {
            TreeKind::None => false,
            TreeKind::Syntax(_) | TreeKind::Sql(_) => tree_mode != TreeMode::Raw,
            TreeKind::Data { .. } => matches!(tree_mode, TreeMode::Structure | TreeMode::Data),
        }
    }
}

// Per-language tree-sitter grammar shims. Each fn coerces a language
// crate's `LANGUAGE` constant into a `tree_sitter::Language` so it can
// sit in [`LanguageOps::grammar`] as a `fn()` pointer.
#[cfg(feature = "native")] fn ts_typescript() -> tree_sitter::Language { tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into() }
#[cfg(feature = "native")] fn ts_tsx()        -> tree_sitter::Language { tree_sitter_typescript::LANGUAGE_TSX.into() }
#[cfg(feature = "native")] fn ts_javascript() -> tree_sitter::Language { tree_sitter_javascript::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_csharp()     -> tree_sitter::Language { tree_sitter_c_sharp::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_python()     -> tree_sitter::Language { tree_sitter_python::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_go()         -> tree_sitter::Language { tree_sitter_go::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_rust()       -> tree_sitter::Language { tree_sitter_rust::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_java()       -> tree_sitter::Language { tree_sitter_java::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_ruby()       -> tree_sitter::Language { tree_sitter_ruby::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_php()        -> tree_sitter::Language { tree_sitter_php::LANGUAGE_PHP.into() }
#[cfg(feature = "native")] fn ts_tsql()       -> tree_sitter::Language { tree_sitter_sequel_tsql::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_json()       -> tree_sitter::Language { tree_sitter_json::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_yaml()       -> tree_sitter::Language { tree_sitter_yaml::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_toml()       -> tree_sitter::Language { tree_sitter_toml_ng::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_ini()        -> tree_sitter::Language { tree_sitter_ini::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_env()        -> tree_sitter::Language { tree_sitter_bash::LANGUAGE.into() }
#[cfg(feature = "native")] fn ts_markdown()   -> tree_sitter::Language { tree_sitter_md::LANGUAGE.into() }

/// Declarative per-language operations table.
///
/// One entry per language registers all of its dispatch targets so
/// adding a new language is a single `LanguageOps { … }` entry in
/// [`LANGUAGES`] — no hunting through seven `match` statements. Every
/// `get_*` helper below is a 2-liner against this table.
///
/// `ids` is the full alias list (e.g. `&["csharp", "cs"]` or
/// `&["rust", "rs"]`). A language ID is matched iff it appears in this
/// slice. `node_spec` is `None` for data/config languages that have
/// not (yet) declared a semantic vocabulary.
pub struct LanguageOps {
    pub ids: &'static [&'static str],
    /// File extensions (no leading dot) this language claims. Each
    /// entry must be unique across `LANGUAGES`; ambiguous extensions
    /// are surfaced by `parser::check_ambiguous_extension`.
    #[cfg(feature = "native")]
    pub extensions: &'static [&'static str],
    /// Tree-sitter grammar producer. See [`GrammarFn`].
    #[cfg(feature = "native")]
    pub grammar: GrammarFn,
    /// Which tree family this language lowers to (and the lower fn). See
    /// [`TreeKind`]. `None` means the language stays on the legacy
    /// imperative path.
    #[cfg(feature = "native")]
    pub tree_kind: TreeKind,
    pub transform: TransformFn,
    pub syntax_category: SyntaxCategoryFn,
    pub field_wrappings: &'static [(&'static str, &'static str)],
    pub node_spec: Option<TractorNodeSpecLookupFn>,
    /// Structured/"programming" language (as opposed to data/config).
    pub is_programming: bool,
    /// Has a `/data` branch projection (JSON/YAML).
    pub supports_data_tree: bool,
    /// Dual-branch transforms for data-aware languages
    /// (Some((ast_transform, data_transform))).
    pub data_transforms: Option<(TransformFn, TransformFn)>,
    /// Singleton wrapper list used by the builder's `apply_singleton_wrappers`.
    pub singleton_wrappers: &'static [&'static str],
}

/// Declarative registry of every language tractor knows about.
///
/// Adding a new language is one entry here. The old seven-way `match`
/// fan-out collapses to simple `iter().find()` calls below.
pub const LANGUAGES: &[LanguageOps] = &[
    // ----- TypeScript / JSX / JavaScript family -----------------------------
    // TS / JS / TSX / JSX all flow through `crate::tree::typescript` —
    // tree-sitter's TS / JS / TSX grammars share most node kinds and
    // TS is a superset; the tree's lower_node arms handle the JSX-only
    // kinds (jsx_element, jsx_attribute, jsx_text, …) too. They share
    // every transform/post-transform/vocabulary; only the grammar and
    // canonical name differ, which is why the family is three rows
    // rather than one.
    LanguageOps {
        ids: &["typescript", "ts"],
        #[cfg(feature = "native")]
        extensions: &["ts"],
        #[cfg(feature = "native")]
        grammar: ts_typescript,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::typescript::lower_typescript_root),
        transform: passthrough_transform,
        syntax_category: typescript::syntax_category,
        field_wrappings: TS_FIELD_WRAPPINGS,
        node_spec: Some(typescript::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["tsx"],
        #[cfg(feature = "native")]
        extensions: &["tsx"],
        #[cfg(feature = "native")]
        grammar: ts_tsx,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::typescript::lower_typescript_root),
        transform: passthrough_transform,
        syntax_category: typescript::syntax_category,
        field_wrappings: TS_FIELD_WRAPPINGS,
        node_spec: Some(typescript::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["javascript", "js", "jsx"],
        #[cfg(feature = "native")]
        extensions: &["js", "mjs", "cjs", "jsx"],
        #[cfg(feature = "native")]
        grammar: ts_javascript,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::typescript::lower_typescript_root),
        transform: passthrough_transform,
        syntax_category: typescript::syntax_category,
        field_wrappings: TS_FIELD_WRAPPINGS,
        node_spec: Some(typescript::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    // ----- Other programming languages --------------------------------------
    LanguageOps {
        ids: &["csharp", "cs"],
        #[cfg(feature = "native")]
        extensions: &["cs"],
        #[cfg(feature = "native")]
        grammar: ts_csharp,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::csharp::lower_csharp_root),
        // C# flows entirely through `crate::languages::csharp::lower`. The imperative
        // walker is no longer reachable for C#; `passthrough_transform`
        // satisfies the field's contract for any code path that still
        // looks up `transform` by language id.
        transform: passthrough_transform,
        syntax_category: csharp::syntax_category,
        field_wrappings: CSHARP_FIELD_WRAPPINGS,
        node_spec: Some(csharp::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["python", "py"],
        #[cfg(feature = "native")]
        extensions: &["py", "pyw", "pyi"],
        #[cfg(feature = "native")]
        grammar: ts_python,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::python::lower_python_root),
        transform: passthrough_transform,
        syntax_category: python::syntax_category,
        field_wrappings: PYTHON_FIELD_WRAPPINGS,
        node_spec: Some(python::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["go"],
        #[cfg(feature = "native")]
        extensions: &["go"],
        #[cfg(feature = "native")]
        grammar: ts_go,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::go::lower_go_root),
        transform: passthrough_transform,
        syntax_category: go::syntax_category,
        field_wrappings: GO_FIELD_WRAPPINGS,
        node_spec: Some(go::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["rust", "rs"],
        #[cfg(feature = "native")]
        extensions: &["rs"],
        #[cfg(feature = "native")]
        grammar: ts_rust,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::rust_lang::lower_rust_root),
        transform: passthrough_transform,
        syntax_category: rust_lang::syntax_category,
        field_wrappings: RUST_FIELD_WRAPPINGS,
        node_spec: Some(rust_lang::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["java"],
        #[cfg(feature = "native")]
        extensions: &["java"],
        #[cfg(feature = "native")]
        grammar: ts_java,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::java::lower_java_root),
        transform: passthrough_transform,
        syntax_category: java::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: Some(java::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["ruby", "rb"],
        #[cfg(feature = "native")]
        extensions: &["rb", "rake", "gemspec"],
        #[cfg(feature = "native")]
        grammar: ts_ruby,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::ruby::lower_ruby_root),
        transform: passthrough_transform,
        syntax_category: ruby::syntax_category,
        field_wrappings: RUBY_FIELD_WRAPPINGS,
        node_spec: Some(ruby::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["php"],
        #[cfg(feature = "native")]
        extensions: &["php"],
        #[cfg(feature = "native")]
        grammar: ts_php,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Syntax(crate::languages::php::lower_php_root),
        // PHP flows entirely through `crate::tree::php`. The imperative
        // walker is no longer reachable; passthrough satisfies the
        // registry contract.
        transform: passthrough_transform,
        syntax_category: php::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: Some(php::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["tsql", "mssql", "sql"],
        #[cfg(feature = "native")]
        extensions: &["sql"],
        #[cfg(feature = "native")]
        grammar: ts_tsql,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Sql(crate::tree::sql_lower::lower_sql_root),
        transform: tsql::transform,
        syntax_category: tsql::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: Some(tsql::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    // ----- Data / config languages ------------------------------------------
    LanguageOps {
        ids: &["json"],
        #[cfg(feature = "native")]
        extensions: &["json"],
        #[cfg(feature = "native")]
        grammar: ts_json,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Data {
            structure: DataParser {
                lower: crate::tree::lower_json_data_root,
                render: crate::tree::render_data_to_xot_json,
            },
            content: DataParser {
                lower: crate::tree::lower_json_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
        },
        transform: json::data_transform,
        syntax_category: json::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: None,
        is_programming: false,
        supports_data_tree: true,
        data_transforms: Some((json::ast_transform, json::data_transform)),
        singleton_wrappers: &[],
    },
    LanguageOps {
        ids: &["yaml", "yml"],
        #[cfg(feature = "native")]
        extensions: &["yml", "yaml"],
        #[cfg(feature = "native")]
        grammar: ts_yaml,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Data {
            structure: DataParser {
                lower: crate::tree::lower_yaml_data_root,
                render: crate::tree::render_data_to_xot_json,
            },
            content: DataParser {
                lower: crate::tree::lower_yaml_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
        },
        transform: yaml::data_transform,
        syntax_category: yaml::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: None,
        is_programming: false,
        supports_data_tree: true,
        data_transforms: Some((yaml::ast_transform, yaml::data_transform)),
        singleton_wrappers: &[],
    },
    LanguageOps {
        ids: &["toml"],
        #[cfg(feature = "native")]
        extensions: &["toml"],
        #[cfg(feature = "native")]
        grammar: ts_toml,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Data {
            structure: DataParser {
                lower: crate::tree::lower_toml_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
            content: DataParser {
                lower: crate::tree::lower_toml_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
        },
        // TOML flows entirely through `crate::tree::toml_data` (parser
        // dispatches to `parse_with_ir_pipeline`). The tree's data
        // lowering already collapses array-of-tables; no post-pass
        // needed.
        transform: passthrough_transform,
        syntax_category: toml::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: None,
        is_programming: false,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: &[],
    },
    LanguageOps {
        ids: &["ini"],
        #[cfg(feature = "native")]
        extensions: &["ini", "cfg", "inf"],
        #[cfg(feature = "native")]
        grammar: ts_ini,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Data {
            structure: DataParser {
                lower: crate::tree::lower_ini_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
            content: DataParser {
                lower: crate::tree::lower_ini_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
        },
        // INI flows entirely through `crate::tree::ini_data`.
        transform: passthrough_transform,
        syntax_category: ini::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: None,
        is_programming: false,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: &[],
    },
    LanguageOps {
        ids: &["env"],
        #[cfg(feature = "native")]
        extensions: &["env"],
        #[cfg(feature = "native")]
        grammar: ts_env,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Data {
            structure: DataParser {
                lower: crate::tree::lower_ini_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
            content: DataParser {
                lower: crate::tree::lower_ini_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
        },
        // .env flows entirely through `crate::tree::ini_data` (shares
        // INI's data lowering — same shape). Grammar is bash because
        // the .env shell-style syntax overlaps closely.
        transform: passthrough_transform,
        syntax_category: env::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: None,
        is_programming: false,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: &[],
    },
    LanguageOps {
        ids: &["markdown", "md", "mdx"],
        #[cfg(feature = "native")]
        extensions: &["md", "markdown", "mdx"],
        #[cfg(feature = "native")]
        grammar: ts_markdown,
        #[cfg(feature = "native")]
        tree_kind: TreeKind::Data {
            structure: DataParser {
                lower: crate::tree::lower_markdown_data_root,
                render: crate::tree::render_data_to_xot_json,
            },
            content: DataParser {
                lower: crate::tree::lower_markdown_data_root,
                render: crate::tree::render_data_to_xot_keyed,
            },
        },
        // Markdown flows entirely through `crate::tree::markdown_data`.
        transform: passthrough_transform,
        syntax_category: markdown::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: None,
        is_programming: false,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: &[],
    },
];

/// Look up the `LanguageOps` entry for a language ID / alias. `None`
/// if the ID is not registered.
pub fn get_language(lang: &str) -> Option<&'static LanguageOps> {
    LANGUAGES.iter().find(|l| l.ids.iter().any(|id| *id == lang))
}

/// Get the transform function for a language (single-branch transform).
///
/// For data-aware languages (JSON, YAML), prefer `get_data_transforms()`
/// which returns separate AST and data transforms for dual-branch output.
pub fn get_transform(lang: &str) -> TransformFn {
    get_language(lang).map(|l| l.transform).unwrap_or(passthrough_transform)
}

// /specs/tractor-parse/dual-view/supported-languages.md: Supported Languages
/// Get dual-branch transform functions for data-aware languages.
///
/// Returns `Some((syntax_transform, data_transform))` for languages
/// that produce both a `/syntax` and `/data` branch, or `None` otherwise.
pub fn get_data_transforms(lang: &str) -> Option<(TransformFn, TransformFn)> {
    get_language(lang).and_then(|l| l.data_transforms)
}


/// Recursively collect every element with the given name into `out`,
/// in document order.
pub(crate) fn collect_named_elements(xot: &Xot, node: XotNode, name: &str, out: &mut Vec<XotNode>) {
    use crate::transform::helpers::*;
    if xot.element(node).is_some() && get_element_name(xot, node).as_deref() == Some(name) {
        out.push(node);
    }
    for child in xot.children(node) {
        collect_named_elements(xot, child, name, out);
    }
}

/// Check whether a language supports the data tree projection.
pub fn supports_data_tree(lang: &str) -> bool {
    get_language(lang).map(|l| l.supports_data_tree).unwrap_or(false)
}

/// True for programming languages (as opposed to data/config languages).
/// Used to gate post-transforms like identifier-role marking that only
/// make sense when the tree has declaration/reference semantics.
pub fn is_programming_language(lang: &str) -> bool {
    get_language(lang).map(|l| l.is_programming).unwrap_or(false)
}

/// Default field wrappings shared by most programming-language grammars.
/// Each language opts in (and can add language-specific entries) via
/// `get_field_wrappings`.
///
/// ## ⚠ Scope: GLOBAL per-language
///
/// Every entry here applies to EVERY tree-sitter kind that uses the
/// field name. There is no per-kind scoping. Before adding a new
/// `(field, wrapper)` pair, verify the field name doesn't appear
/// on kinds where the wrap is unwanted:
///
/// 1. Search the language's `input.rs` (or tree-sitter grammar) for
///    other kinds that emit `field=X` children.
/// 2. Check whether wrapping is appropriate for ALL of them.
/// 3. If any kind needs a different shape, use a Custom handler
///    with [`crate::transform::helpers::wrap_field_child`] instead
///    of adding a global entry here.
///
/// Examples of what NOT to do:
///
/// - `("alternative", "else")` would wrap `if_statement`'s
///   `else_clause` (which already renames to `<else>`) → double-nest.
///   Surgical `wrap_field_child` in the ternary Custom handler
///   instead — see iter 179 for the full bug story.
/// - `("pattern", "pattern")` on Rust would wrap `let_condition`'s
///   pattern AND `parameter`'s pattern → broke `<parameter>/<name>`
///   shape across all of Rust. See iter 347 for the failed attempt.
///
/// Re-read lesson "Field-wrap is global per-language" in
/// `todo/39-post-cycle-review-backlog.md` before extending these.
const COMMON_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "then"),
    ("return_type", "returns"),
];

const PYTHON_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "then"),
    ("return_type", "returns"),
];

const TS_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "then"),
    ("return_type", "returns"),
    // The callee of a call expression. Renamed from the tree-sitter
    // field `function` to avoid colliding with `<function>` used for
    // function declarations.
    ("function", "callee"),
    ("object", "object"),
    ("property", "property"),
];

const RUST_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "then"),
    ("return_type", "returns"),
];

const GO_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "then"),
    ("result", "returns"),
];

const CSHARP_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "then"),
    ("returns", "returns"),
];

/// Ruby — grammar already uses a literal `<then>` kind for the
/// consequence branch, so wrapping `consequence` in `<then>` would
/// double-nest. The rest comes from the common defaults.
const RUBY_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
];

/// Field wrappings for the given language — applied after the raw
/// builder pass, before the per-language transform. Syntax-tree
/// languages with language-specific mappings override; everything else
/// (including data/config formats) gets the common defaults, since
/// JSON/YAML/TOML data transforms still rely on the `<value>` wrapper
/// for pair values.
pub fn get_field_wrappings(lang: &str) -> &'static [(&'static str, &'static str)] {
    get_language(lang).map(|l| l.field_wrappings).unwrap_or(COMMON_FIELD_WRAPPINGS)
}

/// Return true if `name` is a field wrapper element emitted by the
/// builder's `apply_field_wrappings` pass for the given language —
/// i.e. the right-hand side of some entry in that language's
/// `*_FIELD_WRAPPINGS` table.
///
/// Derived from the existing per-language wrapping table so there's
/// exactly one source of truth: adding a wrapper to `TS_FIELD_WRAPPINGS`
/// (say) automatically extends the allowlist for TS — and crucially,
/// does NOT extend it for Python. The
/// `all_names_declared_in_semantic_module` invariant uses this to
/// treat field wrappers as universally allowed within their declaring
/// language.
pub fn is_field_wrapper_name(lang: &str, name: &str) -> bool {
    get_field_wrappings(lang)
        .iter()
        .any(|(_ts_field, wrapper)| *wrapper == name)
}

/// Get the syntax category function for a language
/// This maps transformed element names to syntax categories for highlighting.
pub fn get_syntax_category(lang: &str) -> SyntaxCategoryFn {
    get_language(lang).map(|l| l.syntax_category).unwrap_or(default_syntax_category)
}

/// Get the singleton wrapper list for a language.
///
/// Returns the list of wrapper element names that should have their single
/// child annotated with `field` for JSON property lifting.
/// Data-aware languages (JSON, YAML) return an empty list.
pub fn get_singleton_wrappers(lang: &str) -> &'static [&'static str] {
    get_language(lang).map(|l| l.singleton_wrappers).unwrap_or(&[])
}

/// True iff `name` is a pure marker (never a container) in the given
/// language's semantic vocabulary. Returns `false` for unknown
/// languages, unknown names, and dual-use names (which set both
/// `marker: true` and `container: true` in the NODES table).
///
/// Used by the `marker-stays-empty` shape-contract rule (in
/// `tractor/src/transform/shape_contracts.rs`) to assert that names
/// declared `NodeRole::MarkerOnly` never carry text or element
/// children.
pub fn is_marker_only_name(lang: &str, name: &str) -> bool {
    match get_language(lang).and_then(|l| l.node_spec).and_then(|f| f(name)) {
        Some(spec) => spec.marker && !spec.container,
        None => false,
    }
}

/// True iff the given language has a declared semantic vocabulary
/// (i.e. a populated NODES table). Used to gate the per-language
/// ALL_NAMES invariant — languages that haven't yet defined a spec
/// (data / config formats) are simply skipped.
pub fn has_semantic_vocabulary(lang: &str) -> bool {
    get_language(lang).map(|l| l.node_spec.is_some()).unwrap_or(false)
}

/// True iff `name` is declared in the given language's NODES table —
/// i.e. it's a semantic element the language's transform can emit.
///
/// Returns `false` for unknown languages AND for languages without a
/// declared vocabulary; use `has_semantic_vocabulary` to distinguish
/// "undeclared name" from "language doesn't declare anything yet".
pub fn is_declared_name(lang: &str, name: &str) -> bool {
    get_language(lang)
        .and_then(|l| l.node_spec)
        .and_then(|f| f(name))
        .is_some()
}

/// Default passthrough transform - just continues without changes
fn passthrough_transform(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// Default syntax category - generic fallback for unknown languages
fn default_syntax_category(element: &str) -> SyntaxCategory {
    // Fallback to the generic mapping in syntax_highlight.rs
    SyntaxCategory::from_element_name(element)
}
