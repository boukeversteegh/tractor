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
pub struct NodeSpec {
    pub name: &'static str,
    pub marker: bool,
    pub container: bool,
    pub syntax: SyntaxCategory,
}

/// How a tree-sitter `kind` is handled by the language's transform.
///
/// A language's `KINDS` catalogue (in `<lang>/semantic.rs`) lists every
/// tree-sitter kind the transform knows about, tagged with one of
/// these handling variants. The `kind_catalogue` lint test parses the
/// per-language blueprint fixture and asserts every distinct kind in
/// the raw parse tree appears in `KINDS` — so when tree-sitter ships a
/// new kind we don't yet handle, the test fails at a known site rather
/// than silently producing `<some_unknown_kind kind="…">` output.
#[derive(Debug, Clone, Copy)]
pub enum KindHandling {
    /// Pure rename: `kind` → `semantic` (no marker, no structural change).
    Rename(&'static str),
    /// Rename + marker: `kind` → `semantic` with `marker` empty element prepended.
    RenameWithMarker(&'static str, &'static str),
    /// Imperative dispatch arm in transform.rs with no rename hand-off
    /// — the arm fully owns the renaming of the node (or leaves the
    /// kind name in place).
    Custom,
    /// Imperative dispatch arm in transform.rs that ends with
    /// `apply_rename(…, kind)` — i.e. the arm does structural work,
    /// then defers the rename to `map_element_name`. Distinct from
    /// `Custom` so the catalogue still drives the rename.
    CustomThenRename(&'static str),
    /// Same as `CustomThenRename` but with a marker prepended.
    CustomThenRenameWithMarker(&'static str, &'static str),
    /// Wrapper dropped, children promoted to siblings (Principle #12).
    Flatten,
    /// Kind passes through unchanged (no rename, no transform).
    PassThrough,
}

/// Single entry in a language's tree-sitter kind catalogue. See
/// [`KindHandling`] for the meaning of each variant.
#[derive(Debug, Clone, Copy)]
pub struct KindEntry {
    pub kind: &'static str,
    pub handling: KindHandling,
}

/// Type alias for language transform functions
pub type TransformFn = fn(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>;

/// Type alias for language post-transform functions.
///
/// Runs after `walk_transform`, receiving the full document root. Used
/// for structural rewrites that need their descendants already
/// renamed — e.g. collapsing a nested `else`/`if` chain into the flat
/// `<if><else_if/><else/>` shape (see
/// `specs/tractor-parse/semantic-tree/transformations.md`).
pub type PostTransformFn = fn(&mut Xot, XotNode) -> Result<(), xot::Error>;

/// Type alias for syntax category mapping functions
/// Maps a transformed element name to a syntax category for highlighting
pub type SyntaxCategoryFn = fn(&str) -> SyntaxCategory;

/// Type alias for per-language NodeSpec lookup.
pub type NodeSpecLookupFn = fn(&str) -> Option<&'static NodeSpec>;

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
    pub transform: TransformFn,
    pub post_transform: Option<PostTransformFn>,
    pub syntax_category: SyntaxCategoryFn,
    pub field_wrappings: &'static [(&'static str, &'static str)],
    pub node_spec: Option<NodeSpecLookupFn>,
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
    LanguageOps {
        ids: &["typescript", "ts", "tsx", "javascript", "js", "jsx"],
        transform: typescript::transform,
        post_transform: Some(collapse_conditionals),
        syntax_category: typescript::syntax_category,
        field_wrappings: TS_FIELD_WRAPPINGS,
        node_spec: Some(typescript::semantic::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["csharp", "cs"],
        transform: csharp::transform,
        post_transform: Some(csharp_post_transform),
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
        transform: python::transform,
        post_transform: None,
        syntax_category: python::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: Some(python::semantic::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["go"],
        transform: go::transform,
        post_transform: Some(collapse_conditionals),
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
        transform: rust_lang::transform,
        post_transform: Some(collapse_conditionals),
        syntax_category: rust_lang::syntax_category,
        field_wrappings: RUST_FIELD_WRAPPINGS,
        node_spec: Some(rust_lang::semantic::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["java"],
        transform: java::transform,
        post_transform: Some(collapse_conditionals),
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
        transform: ruby::transform,
        post_transform: Some(collapse_conditionals),
        syntax_category: ruby::syntax_category,
        field_wrappings: RUBY_FIELD_WRAPPINGS,
        node_spec: Some(ruby::semantic::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["php"],
        transform: php::transform,
        post_transform: Some(collapse_conditionals),
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
        transform: tsql::transform,
        post_transform: None,
        syntax_category: tsql::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: Some(tsql::semantic::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["json"],
        transform: json::data_transform,
        post_transform: None,
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
        transform: yaml::data_transform,
        post_transform: None,
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
        transform: toml::transform,
        post_transform: None,
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
        transform: ini::transform,
        post_transform: None,
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
        transform: env::transform,
        post_transform: None,
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
        transform: markdown::transform,
        post_transform: None,
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

/// Get the post-transform function for a language, if any.
pub fn get_post_transform(lang: &str) -> Option<PostTransformFn> {
    get_language(lang).and_then(|l| l.post_transform)
}

/// C# combines two post-transforms: `attach_where_clause_constraints`
/// moves `where T : …` constraints into the matching `<generic>`, then
/// the shared conditional collapse runs.
fn csharp_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    attach_where_clause_constraints(xot, root)?;
    collapse_conditionals(xot, root)
}

/// For each `<type_parameter_constraints_clause>` in the tree, move its
/// constraints into the matching `<generic>` sibling and drop the
/// clause. Empty markers (`<class/>`, `<struct/>`, `<notnull/>`,
/// `<unmanaged/>`, `<new/>`) for shape constraints; `<extends>` wrapping
/// a `<type>` for type bounds. See
/// `specs/tractor-parse/semantic-tree/transformations/csharp.md`.
fn attach_where_clause_constraints(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::*;

    // Collect all clause nodes (mutate later).
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        use crate::transform::helpers::*;
        if xot.element(node).is_some()
            && get_kind(xot, node).as_deref() == Some("type_parameter_constraints_clause")
        {
            out.push(node);
        }
        for child in xot.children(node) {
            collect(xot, child, out);
        }
    }

    let mut clauses: Vec<XotNode> = Vec::new();
    collect(xot, root, &mut clauses);

    for clause in clauses {
        if xot.parent(clause).is_none() && !xot.is_document(clause) {
            continue;
        }

        // Target name: the first <name> child text of the clause.
        let target_name: Option<String> = xot.children(clause)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .and_then(|n| get_text_content(xot, n));
        let target_name = match target_name {
            Some(n) => n,
            None => continue,
        };

        // Find the matching <generic> sibling (under the same parent).
        let parent = match xot.parent(clause) {
            Some(p) => p,
            None => continue,
        };
        let target_generic: Option<XotNode> = xot.children(parent)
            .filter(|&c| xot.element(c).is_some())
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("generic"))
            .find(|&c| {
                xot.children(c)
                    .filter(|&gc| xot.element(gc).is_some())
                    .find(|&gc| get_element_name(xot, gc).as_deref() == Some("name"))
                    .and_then(|n| get_text_content(xot, n))
                    .as_deref()
                    == Some(target_name.as_str())
            });
        let generic = match target_generic {
            Some(g) => g,
            None => continue,
        };

        // Walk the clause's `type_parameter_constraint` children and
        // transplant each as a marker or `<extends>` onto the generic.
        let constraint_children: Vec<XotNode> = xot.children(clause)
            .filter(|&c| xot.element(c).is_some())
            .filter(|&c| get_kind(xot, c).as_deref() == Some("type_parameter_constraint"))
            .collect();

        for constraint in constraint_children {
            append_constraint_to_generic(xot, constraint, generic)?;
        }

        // Drop the now-empty clause wrapper.
        xot.detach(clause)?;
    }
    Ok(())
}

/// Move one `<type_parameter_constraint>`'s meaning onto a `<generic>`:
/// add an empty marker (`<class/>` / `<struct/>` / `<new/>` / …) for
/// shape constraints; wrap type references in `<extends>`.
fn append_constraint_to_generic(
    xot: &mut Xot,
    constraint: XotNode,
    generic: XotNode,
) -> Result<(), xot::Error> {
    use crate::transform::helpers::*;

    // `constructor_constraint` → <new/> (the literal text is "new()")
    let has_ctor_ctor = xot.children(constraint)
        .any(|c| get_kind(xot, c).as_deref() == Some("constructor_constraint"));
    if has_ctor_ctor {
        let marker_name = xot.add_name("new");
        let marker = xot.new_element(marker_name);
        xot.append(generic, marker)?;
        return Ok(());
    }

    // A `<type>` child means this is a specific type bound → <extends>
    let type_child = xot.children(constraint)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("type"));
    if let Some(type_child) = type_child {
        let extends_name = xot.add_name("extends");
        let extends = xot.new_element(extends_name);
        xot.detach(type_child)?;
        xot.append(extends, type_child)?;
        xot.append(generic, extends)?;
        return Ok(());
    }

    // Otherwise the constraint is a bare keyword like "class" / "struct"
    // / "notnull" / "unmanaged" — add as empty marker with that name.
    if let Some(text) = get_text_content(xot, constraint) {
        let trimmed = text.trim();
        if matches!(trimmed, "class" | "struct" | "notnull" | "unmanaged") {
            let marker_name = xot.add_name(trimmed);
            let marker = xot.new_element(marker_name);
            xot.append(generic, marker)?;
        }
    }
    Ok(())
}

/// Post-transform pass that collapses every `<if>` in the tree into
/// the flat conditional shape (see the cross-cutting convention in
/// `specs/tractor-parse/semantic-tree/transformations.md`).
fn collapse_conditionals(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::conditionals::collapse_else_if_chain;
    // Collect all <if> nodes first (we mutate the tree as we go).
    let mut if_nodes: Vec<XotNode> = Vec::new();
    collect_if_nodes(xot, root, &mut if_nodes);
    // Process outer-most `<if>` first. `collect_if_nodes` returns
    // document order, which is parent-before-child; handling the outer
    // one first lifts its `<else_if>` siblings correctly before we
    // recurse into any nested ifs.
    for node in if_nodes {
        // Skip nodes that were detached by an earlier pass (happens when
        // we lift an inner `<if>`'s children into an `<else_if>` — the
        // inner `<if>` is left empty and its own recursion becomes a
        // no-op, but we still call it to be safe).
        if xot.parent(node).is_none() && !xot.is_document(node) {
            continue;
        }
        collapse_else_if_chain(xot, node)?;
    }
    Ok(())
}

fn collect_if_nodes(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
    use crate::transform::helpers::*;
    if xot.element(node).is_some() && get_element_name(xot, node).as_deref() == Some("if") {
        out.push(node);
    }
    for child in xot.children(node) {
        collect_if_nodes(xot, child, out);
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
/// `alternative` is intentionally not in this list. For `if_statement`,
/// tree-sitter's `else_clause` child already renames to `<else>` via each
/// language's `map_element_name`, so a global wrap would double-nest.
/// For ternary expressions, the `<else>` wrap is done surgically in the
/// per-language ternary handler via `wrap_field_child`.
const COMMON_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "then"),
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
/// builder pass, before the per-language transform. Programming
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
/// Used by the `markers_stay_empty` invariant to assert that names
/// declared as marker-only never carry text or element children.
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
