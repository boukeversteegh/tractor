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

/// Type alias for per-language TractorNodeSpec lookup.
pub type TractorNodeSpecLookupFn = fn(&str) -> Option<&'static TractorNodeSpec>;

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
    LanguageOps {
        ids: &["typescript", "ts", "tsx", "javascript", "js", "jsx"],
        transform: typescript::transform,
        post_transform: Some(typescript::typescript_post_transform),
        syntax_category: typescript::syntax_category,
        field_wrappings: TS_FIELD_WRAPPINGS,
        node_spec: Some(typescript::output::spec),
        is_programming: true,
        supports_data_tree: false,
        data_transforms: None,
        singleton_wrappers: crate::transform::singletons::DEFAULT_SINGLETON_WRAPPERS,
    },
    LanguageOps {
        ids: &["csharp", "cs"],
        // C# now flows entirely through `crate::ir::csharp` (parser
        // dispatches to `parse_with_ir_pipeline`). The imperative
        // walker is no longer reachable for C#; `passthrough_transform`
        // satisfies the field's contract for any code path that still
        // looks up `transform` by language id.
        transform: passthrough_transform,
        post_transform: Some(csharp::csharp_post_transform),
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
        // Python flows entirely through `crate::ir::python` (parser
        // dispatches to `parse_with_ir_pipeline`). The imperative
        // walker is no longer reachable for Python; passthrough
        // satisfies the registry contract.
        transform: passthrough_transform,
        post_transform: Some(python::python_post_transform),
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
        transform: go::transform,
        post_transform: Some(go::go_post_transform),
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
        post_transform: Some(rust_lang::rust_post_transform),
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
        transform: java::transform,
        post_transform: Some(java::java_post_transform),
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
        post_transform: Some(ruby::ruby_post_transform),
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
        transform: php::transform,
        post_transform: Some(php::php_post_transform),
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
        post_transform: Some(tsql::tsql_post_transform),
        syntax_category: tsql::syntax_category,
        field_wrappings: COMMON_FIELD_WRAPPINGS,
        node_spec: Some(tsql::output::spec),
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
        post_transform: Some(toml_post_transform),
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

// C# post_transform moved iter 330 to
// tractor/src/languages/csharp/post_transform.rs per user direction.
// The LanguageOps::post_transform registration above references
// csharp::csharp_post_transform.

/// TOML `[[arrays-of-tables]]` collapse (closes todo/35).
///
/// Each `[[servers]]` entry produces a separate `<servers><item>...`
/// from the per-element transform. Multiple `[[servers]]` blocks
/// thus emit sibling `<servers>` elements, each with one `<item>`.
/// This contradicts every other array shape in tractor (one parent
/// `<key>` with many `<item>` children) and breaks intuitive
/// queries like `count(//servers) = 1`.
///
/// Walk every container; for each pair of adjacent same-named
/// element children, merge the second into the first by moving its
/// children over and detaching the now-empty second wrapper.
fn toml_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Skip xot's document wrapper if present.
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    // Only merge same-named adjacent SIBLINGS at the top-level
    // `<document>` (the AOT collapse case). Don't recurse — that
    // would merge legitimate `<item>` siblings that represent
    // distinct array elements. If multi-level AOT becomes a real
    // case (`[[a.b]]` repeating where the inner `<b>` siblings
    // should also merge), revisit and apply this recursively only
    // for non-`<item>` names.
    merge_adjacent_same_named(xot, root)?;
    Ok(())
}

fn merge_adjacent_same_named(xot: &mut Xot, parent: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    if xot.element(parent).is_none() {
        return Ok(());
    }
    loop {
        let children: Vec<XotNode> = xot.children(parent).collect();
        let mut merged = false;
        for i in 0..children.len().saturating_sub(1) {
            let first = children[i];
            let second = children[i + 1];
            if xot.element(first).is_none() || xot.element(second).is_none() {
                continue;
            }
            // Don't merge `<item>` siblings — they're distinct
            // array elements.
            if get_element_name(xot, first).as_deref() == Some("item") {
                continue;
            }
            let n1 = get_element_name(xot, first);
            let n2 = get_element_name(xot, second);
            if n1.is_some() && n1 == n2 {
                let to_move: Vec<XotNode> = xot.children(second).collect();
                for c in to_move {
                    xot.detach(c)?;
                    xot.append(first, c)?;
                }
                xot.detach(second)?;
                merged = true;
                break;
            }
        }
        if !merged {
            break;
        }
    }
    Ok(())
}
// C# helpers (csharp_normalize_conditional_access,
// unify_file_scoped_namespace, attach_where_clause_constraints,
// append_constraint_to_generic) moved iter 330 to
// tractor/src/languages/csharp/post_transform.rs alongside
// csharp_post_transform itself.

// Rust post_transform + helpers (rust_normalize_field_expression,
// rust_normalize_lifetime_names, rust_restructure_use) moved iter
// 329 to `tractor/src/languages/rust_lang/post_transform.rs`
// per user direction. The LanguageOps::post_transform registration
// above references `rust_lang::rust_post_transform`.

// TypeScript post_transform + helpers (typescript_unwrap_callee,
// typescript_restructure_import) moved iter 330 to
// tractor/src/languages/typescript/post_transform.rs per user
// direction. The LanguageOps::post_transform registration above
// references typescript::typescript_post_transform.

// Python post_transform + helpers (python_tag_from_imports_uniform,
// python_restructure_imports, python_alias_pairs,
// python_flatten_dotted_name) moved iter 330 to
// tractor/src/languages/python/post_transform.rs per user direction.
// The LanguageOps::post_transform registration above references
// python::python_post_transform.

// Java post_transform + helpers (java_unwrap_type_in_path) moved iter
// 331 to tractor/src/languages/java/post_transform.rs per user direction.
// The LanguageOps::post_transform registration above references
// java::java_post_transform.

// Go post_transform + helpers (go_retag_singleton_closure_body) moved
// iter 331 to tractor/src/languages/go/post_transform.rs per user
// direction. The LanguageOps::post_transform registration above
// references go::go_post_transform.

// TSQL post_transform + helpers (tsql_wrap_binary_operands,
// tsql_tag_select_columns) moved iter 333 to
// tractor/src/languages/tsql/post_transform.rs per user direction.
// The LanguageOps::post_transform registration above references
// tsql::tsql_post_transform.

// PHP post_transform + helpers (php_wrap_member_call_slots,
// php_restructure_use) moved iter 333 to
// tractor/src/languages/php/post_transform.rs per user direction.
// The LanguageOps::post_transform registration above references
// php::php_post_transform.

// Ruby post_transform + helpers (ruby_tag_case_when_lists,
// ruby_retag_singleton_block_body, ruby_collapse_lambda_body,
// ruby_extract_pair_keys, RUBY_VALUE_KINDS) moved iter 333 to
// tractor/src/languages/ruby/post_transform.rs per user direction.
// The LanguageOps::post_transform registration above references
// ruby::ruby_post_transform.

/// Post-transform pass that collapses every `<if>` in the tree into
/// the flat conditional shape (see the cross-cutting convention in
/// `specs/tractor-parse/semantic-tree/transformations.md`).
pub(crate) fn collapse_conditionals(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
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
