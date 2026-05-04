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
        transform: csharp::transform,
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
        transform: python::transform,
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
        post_transform: Some(go_post_transform),
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
        post_transform: Some(java_post_transform),
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
        post_transform: Some(ruby_post_transform),
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
        post_transform: Some(php_post_transform),
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
        post_transform: Some(tsql_post_transform),
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

/// Java post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
fn java_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Normalise Java's flat call shape to the canonical input.
    // Java emits `<call><object/>NAME...args</call>` where the
    // method name is a bare `<name>` sibling of `<object>`. The
    // chain inverter expects `<call><member><object/><property/></member>...args</call>`,
    // so pre-wrap the `<object>`+`<name>` pair into a synthetic
    // `<member>` first.
    crate::transform::chain_inversion::wrap_flat_call_member(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // Java method reference: `String::valueOf` produces `<reference>`
    // with two `<name>` siblings (class + method). Tag both with
    // `list="name"` so the JSON name array is uniform; cardinality
    // discriminator (>=2) keeps singleton uses untouched.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("reference", "name"),
            // Multi-declarator (`int x = 1, y = 2`) keeps
            // `<declarator>` wrappers (per iter 263). Tag with
            // `list="declarators"` so JSON renders them as an
            // array; single-declarator is flattened by the
            // post-pass below and doesn't reach this tag.
            ("variable", "declarator"),
            ("field", "declarator"),
        ],
    )?;
    java_unwrap_type_in_path(xot, root)?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(
        xot, root,
        &["body", "block", "then", "else", "call"],
    )?;
    // Single-declarator fields and locals lose their <declarator>
    // wrapper (`int x = 1;` → `field/{type, name, value}`).
    // Multi-declarator (`int a, b = 5`) keeps wrappers — each is a
    // role-mixed name+value group whose pairing depends on the
    // wrapper. See cold-read backlog iter 233.
    crate::transform::flatten_single_declarator_children(xot, root, &["field", "variable"])?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "block", "program", "tuple", "list", "dict", "array", "hash", "repetition"],
    )?;
    Ok(())
}

/// Inside `<path>`, tree-sitter Java's `scoped_type_identifier` produces
/// `<type><name>X</name></type>` segments. The path is a namespace
/// identifier path; the segments are *names*, not types (Principle
/// #14). Walk every `<path>` and collapse `<type>` segment wrappers to
/// bare `<name>` children.
fn java_unwrap_type_in_path(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let mut paths: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "path", &mut paths);
    for path in paths {
        for child in xot.children(path).collect::<Vec<_>>() {
            if get_element_name(xot, child).as_deref() != Some("type") {
                continue;
            }
            // Replace each <type><name>X</name></type> with <name>X</name>.
            let inner_names: Vec<XotNode> = xot.children(child)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .collect();
            if inner_names.len() != 1 {
                continue;
            }
            let inner_name = inner_names[0];
            xot.detach(inner_name)?;
            xot.insert_before(child, inner_name)?;
            xot.detach(child)?;
        }
    }
    Ok(())
}

/// Go post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
fn go_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Invert right-deep `<member>`/`<call>` chains. Go's tree
    // matches the canonical input shape exactly (same as Python),
    // so no normalization step is needed. Run early so subsequent
    // passes see the post-inversion shape.
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    go_retag_singleton_closure_body(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // Go struct fields and shared-type parameters can declare
    // multiple names with one type:
    //   `x, y int` (struct field) → `<field>` with two `<name>` + `<type>`.
    //   `func f(x, y int)` (param) → `<parameter>` with two `<name>` + `<type>`.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("field", "name"),
            ("parameter", "name"),
            // Go multi-return functions: `func f() (int, error)`
            // produce `<returns>` with multiple `<type>` siblings.
            ("returns", "type"),
            // Go multi-target var: `var x, y = 1, 2` produces
            // `<var>` with multiple `<name>` siblings.
            ("var", "name"),
            // Go struct fields: `type T struct { A int; B string }`
            // produces `<struct>` with multiple `<field>` siblings
            // (each a member declaration, role-uniform per
            // Principle #19). Tag so JSON renders `fields: [...]`.
            ("struct", "field"),
            // Go generics with shared constraint — `func F[T, U any]`
            // produces `<generic>` with multiple `<name>` siblings
            // (one per type-parameter name) plus a singleton `<type>`
            // constraint. Tag the names; the type stays singleton.
            ("generic", "name"),
            // Go type switch with multiple types per case —
            // `case int, int32, int64:` produces `<case>` with
            // multiple `<type>` siblings (one per alternative).
            // Role-uniform alternatives per Principle #19.
            ("case", "type"),
            // Go interfaces with multiple methods + type-set elements.
            ("interface", "method"),
            ("interface", "type"),
            // Go multi-value return: `return x, err` produces
            // `<return>` with multiple `<expression>` siblings
            // (after `wrap_expression_positions`). Tag so JSON
            // renders `expressions: [...]` instead of overflowing
            // to `children`. Mirrors Python iter 265.
            ("return", "expression"),
            // Go multi-value var declaration `name, age = "alice", 30`
            // produces `<value>` with multiple `<expression>`
            // siblings. Same archetype as multi-return, scoped to
            // var declarations.
            ("value", "expression"),
            // Go select with multiple cases `select { case ... }`.
            // Multiple `<case>` siblings under `<select>` —
            // role-uniform per Principle #19.
            ("select", "case"),
            // Go switch with multiple cases `switch x { case ... }`.
            // Targeted role tag replaces the bulk-distribute entry on
            // `"switch"` (removed iter 304 — that entry was wrapping
            // the singleton subject `<value>` in a 1-elem array).
            ("switch", "case"),
            // Go if-then with multi-statement body `if cond {
            // stmt1; stmt2 }` produces `<then>` with multiple
            // `<assign>` (or other statement) siblings. Role-
            // uniform.
            ("then", "assign"),
            ("else", "assign"),
        ],
    )?;
    // Go's `if x { ... }` has `<then>` body; strip braces there too.
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::distribute_member_list_attrs(
        // `"array"` removed iter 311 — Go's `<array>` is the array
        // TYPE spec `[5]int` (singleton size + singleton element
        // type). Bulk distribute was creating 1-elem JSON arrays
        // on both. Go has no multi-cardinality `<array>` cases in
        // the blueprint (literals go inside `<literal>/<array>+<body>`
        // — the body holds elements). No targeted tags needed.
        xot, root, &["body", "file", "tuple", "list", "dict", "repetition"],
    )?;
    Ok(())
}

/// Re-tag a `<closure>`'s `<body>` wrapper as `<value>` for
/// single-statement bodies so Go closures match the closure
/// archetype unification (Rust closure / TS arrow / C# lambda /
/// PHP arrow / Python lambda / Ruby Block / Ruby Lambda from iters
/// 161-174). Multi-statement bodies keep `<body>`.
///
/// Runs as a post-pass (not a per-kind Custom handler) because
/// Go's `block` rule is Pure Flatten, which runs DURING the walk —
/// at FuncLiteral-handler time, body still wraps the unflattened
/// block. By post-transform time, body's element children are
/// the actual statements.
///
/// Run BEFORE `wrap_expression_positions` so the new `<value>`
/// slot's first child gets wrapped in `<expression>` automatically
/// (closing iter 174's "all 8 PLs" claim — Go was missed).
fn go_retag_singleton_closure_body(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut closures: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("closure")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut closures);

    let value_id = xot.add_name("value");
    for closure in closures {
        let body = xot.children(closure)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
        let body = match body { Some(b) => b, None => continue };
        let elem_count = xot.children(body)
            .filter(|&c| xot.element(c).is_some())
            .count();
        if elem_count != 1 { continue; }
        if let Some(elem) = xot.element_mut(body) {
            elem.set_name(value_id);
        }
        // Strip stray `{`/`}` text leaves: `strip_body_braces` later
        // in the pipeline only fires on `<body>`-named containers;
        // we just renamed body→value, so handle it here.
        let text_targets: Vec<XotNode> = xot.children(body)
            .filter(|&c| {
                xot.text_str(c)
                    .map(|s| matches!(s.trim(), "{" | "}"))
                    .unwrap_or(false)
            })
            .collect();
        for t in text_targets {
            xot.detach(t)?;
        }
    }
    Ok(())
}

/// TSQL post-transform: pre-iter-182 had `post_transform: None`,
/// which left every container with multiple uniform-role children
/// overflowing into the anonymous `children: [...]` JSON array.
/// Adds `distribute_member_list_attrs` for the role-uniform
/// containers (every direct element child shares a role): file
/// scripts, transaction blocks, union arms, explicit value lists,
/// columns lists, statement bodies (DDL/DML body containers).
///
/// Role-MIXED containers (`<select>`, `<insert>`, `<from>`,
/// `<call>`, `<case>`, `<compare>`, `<between>`, `<assign>`) need
/// targeted handlers that tag only the multi-instance child role —
/// out of scope for this iter.
fn tsql_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    tsql_wrap_binary_operands(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        xot,
        root,
        &[
            "file", "transaction", "union", "columns", "list",
            "select", "insert", "from", "call", "case", "constraint",
        ],
    )?;
    tsql_tag_select_columns(xot, root)?;
    Ok(())
}

/// Wrap `<compare>` / `<assign>` / `<between>` operand children in
/// role-named `<left>` / `<right>` slots based on their `field=`
/// attribute. TSQL's transform dispatcher (`tsql/transform.rs:29`)
/// intentionally Skip-routes builder-inserted `<left>` / `<right>`
/// wrappers, so this post-pass re-wraps the operands that retained
/// their `field="left"` / `field="right"` attributes from the raw
/// tree-sitter input.
///
/// Closes the iter-185 deferred mystery (root cause SOLVED iter-197
/// review).
fn tsql_wrap_binary_operands(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_attr, get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut parents: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some() {
            if let Some(name) = get_element_name(xot, node) {
                if matches!(name.as_str(), "compare" | "assign" | "between") {
                    out.push(node);
                }
            }
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut parents);

    for parent in parents {
        let elem_children: Vec<XotNode> = xot.children(parent)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in elem_children {
            let field = get_attr(xot, child, "field");
            // BETWEEN's low/high → <low>/<high>; binary operands → <left>/<right>.
            let wrapper = match field.as_deref() {
                Some("left") => "left",
                Some("right") => "right",
                Some("low") => "low",
                Some("high") => "high",
                _ => continue,
            };
            let wrapper_id = xot.add_name(wrapper);
            let wrapper_node = xot.new_element(wrapper_id);
            xot.with_source_location_from(wrapper_node, child)
                .with_wrap_child(child, wrapper_node)?;
        }
    }
    Ok(())
}

/// Tag `<column>` children of `<select>` / `<insert>` with
/// `list="column"` so JSON `select.column: [...]` becomes a uniform
/// array (was: first column lifted as singleton, rest in
/// `children` overflow). Targeted (not bulk via
/// `distribute_member_list_attrs`) because select/insert have
/// role-MIXED children: column lists + singleton clauses
/// (`<from>`, `<where>`, `<order>`, `<alias>`).
fn tsql_tag_select_columns(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_attr, get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    fn collect(xot: &Xot, node: XotNode, names: &[&str], out: &mut Vec<XotNode>) {
        if xot.element(node).is_some() {
            if let Some(name) = get_element_name(xot, node) {
                if names.contains(&name.as_str()) {
                    out.push(node);
                }
            }
        }
        for c in xot.children(node) {
            collect(xot, c, names, out);
        }
    }
    let mut parents: Vec<XotNode> = Vec::new();
    collect(xot, root, &["select", "insert"], &mut parents);
    for parent in parents {
        let columns: Vec<XotNode> = xot.children(parent)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("column")
            })
            .collect();
        for col in columns {
            if get_attr(xot, col, "list").is_none() {
                xot.with_attr(col, "list", "columns");
            }
        }
    }
    Ok(())
}

/// PHP post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15) + restructure
/// `<use>` elements into the unified path/alias/marker shape.
fn php_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // PHP's `<member>` and `<call>` use the `->` operator and emit
    // unwrapped slots: receiver as a child with `field="object"`,
    // access name as a bare `<name>` sibling (no `<property>`).
    // Pre-pass wraps these into the canonical input shape, then
    // chain inversion runs.
    php_wrap_member_call_slots(xot, root)?;
    crate::transform::chain_inversion::wrap_flat_call_member(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    php_restructure_use(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            // PHP `use Foo\{First, Second};` — `<use[group]>` parent
            // with multiple inner `<use>` siblings (one per imported
            // entity). Tag with `list="uses"` so JSON renders as
            // `uses: [...]` array. Mirrors Rust iter 267.
            ("use", "use"),
            // C-style for header `for ($i=0, $j=10; ...; $i++, $j--)`
            // produces `<for>` with multiple `<assign>` siblings (init
            // sequence) AND multiple `<unary>` siblings (post-update
            // sequence). Both role-uniform per Principle #19. The
            // unary tagging mirrors TypeScript iter 269.
            ("for", "assign"),
            ("for", "unary"),
            // PHP `<string>` parent: interpolated strings have one or
            // more `<interpolation>` chunks; heredoc strings have one
            // or more `<value>` chunks. Bulk-distribute on `"string"`
            // (removed below iter 308) was wrapping single-interp /
            // single-value cases in 1-elem JSON arrays. Targeted role
            // tags here cover both single (lifts as singleton) and
            // multi (proper array) cases.
            ("string", "interpolation"),
            ("string", "value"),
            // PHP namespace path `App\Blueprint` produces multiple
            // `<name>` children of `<namespace>`. Iter 328 dropped
            // `"namespace"` from PHP's bulk distribute (the
            // architectural ROLE_MIXED_PARENTS guard requires it);
            // this targeted tag covers the multi-name path case.
            // Single-name namespaces lift as `name: "App"` singleton.
            ("namespace", "name"),
        ],
    )?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        // `"namespace"` removed iter 328 — see targeted role tag above.
        xot, root, &["body", "program", "tuple", "list", "dict", "array", "repetition"],
    )?;
    Ok(())
}

/// Pre-pass for chain inversion: rewrite PHP's `<member>` and
/// `<call>` shapes into the canonical right-deep input.
///
/// PHP emits:
///   `<member><instance/>RECEIVER<name>X</name></member>` — receiver
///   has `field="object"`, name is a bare sibling.
///   `<call><instance/>CALLEE...args</call>` where CALLEE may be
///   `<member>` for method calls or any other expression for direct
///   function calls.
///
/// The chain inverter wants:
///   `<member><object>RECEIVER</object><property><name>X</name></property></member>`
///
/// This pass walks every `<member>` element and:
///   1. Wraps the `field="object"` child in an `<object>` slot.
///   2. Wraps the trailing bare `<name>` in a `<property>` slot.
///
/// `<call>` elements need no rewriting: PHP's tree-sitter places
/// the `<member>` callee as the first non-marker child, matching
/// the canonical `<call><member>...</member>...args</call>` shape
/// once the `<member>` itself is normalised.
fn php_wrap_member_call_slots(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{copy_source_location, get_attr, get_element_name};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some() {
            let name = get_element_name(xot, node);
            if matches!(name.as_deref(), Some("member") | Some("call")) {
                out.push(node);
            }
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut targets);
    for node in targets {
        let elem_name = get_element_name(xot, node);
        let is_member = elem_name.as_deref() == Some("member");
        // Find the field=object child (the receiver).
        let receiver = xot.children(node).find(|&c| {
            xot.element(c).is_some()
                && get_attr(xot, c, "field").as_deref() == Some("object")
        });
        // Skip if already canonical (has <object> slot child).
        let has_object_slot = xot.children(node).any(|c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("object")
        });
        if has_object_slot {
            continue;
        }
        let receiver = match receiver {
            Some(r) => r,
            None => continue,
        };

        // Wrap receiver in <object>.
        let object_id = xot.add_name("object");
        let object_slot = xot.new_element(object_id);
        copy_source_location(xot, receiver, object_slot);
        xot.insert_before(receiver, object_slot)?;
        xot.detach(receiver)?;
        xot.append(object_slot, receiver)?;

        // For <member>: also wrap the trailing bare <name> in
        // <property>. For <call>: leave the name bare —
        // `wrap_flat_call_member` will package it under a
        // synthetic <member> callee.
        if is_member {
            let name_node = xot.children(node).find(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("name")
            });
            if let Some(name_node) = name_node {
                let property_id = xot.add_name("property");
                let property_slot = xot.new_element(property_id);
                copy_source_location(xot, name_node, property_slot);
                xot.insert_before(name_node, property_slot)?;
                xot.detach(name_node)?;
                xot.append(property_slot, name_node)?;
            }
        }
    }
    Ok(())
}

/// Walk every `<use>` element and restructure to the shape:
///   `use App\Base`           → `<use><path><name>App</name></path><name>Base</name></use>`
///   `use App\Foo as Bar`     → `<use[alias]><path><name>App</name></path><name>Foo</name><alias><name>Bar</name></alias></use>`
///   `use App\{First, Second}` → `<use[group]><path><name>App</name></path><use><name>First</name></use><use><name>Second</name></use></use>`
///   `use function App\foo`   → `<use[function]><path><name>App</name></path><name>foo</name></use>`
///
/// Operates on the post-rule tree where children are already mostly
/// `<name>` siblings (qualified_name / namespace_use_clause flattened).
/// Detects markers from text content (`as`, `function`, `const`, `\`,
/// `;`, `,`) and rebuilds the structural slots.
fn php_restructure_use(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name};
    use php::output::TractorNode::{Alias, Group, Function as PhpFunction, Path, Const};

    // Collect `<use>` nodes first to avoid mutating during walk.
    let mut targets: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "use", &mut targets);

    for use_node in targets {
        // 1. Determine flavor from preceding bare-keyword text.
        //    `use function App\foo` → flavor Function; `use const App\BAR` → flavor Const.
        let mut flavor: Option<php::output::TractorNode> = None;
        let mut has_alias_keyword = false;
        for child in xot.children(use_node).collect::<Vec<_>>() {
            let Some(text) = xot.text_str(child) else { continue };
            for tok in text.split_whitespace() {
                match tok {
                    "function" => flavor = Some(PhpFunction),
                    "const" => flavor = Some(Const),
                    "as" => has_alias_keyword = true,
                    _ => {}
                }
            }
        }

        // 2. Detect group form: child is `<body>` containing names.
        let group_body = xot.children(use_node).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("body")
        });

        // 3. Strip ALL noise text leaves on use_node.
        for child in xot.children(use_node).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }

        // 4. Collect remaining element children in document order.
        let element_children: Vec<XotNode> = xot.children(use_node)
            .filter(|&c| xot.element(c).is_some())
            .collect();

        // 5. Branch on group vs flat.
        if let Some(body) = group_body {
            // Group form. Element children before <body> = path segments.
            let path_segments: Vec<XotNode> = element_children.iter()
                .copied()
                .take_while(|&c| c != body)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .collect();

            // Detach body's noise text leaves; remaining elements are leaf
            // names (the {First, Second} list).
            for child in xot.children(body).collect::<Vec<_>>() {
                if xot.text_str(child).is_some() {
                    xot.detach(child)?;
                }
            }
            let leaf_names: Vec<XotNode> = xot.children(body)
                .filter(|&c| xot.element(c).is_some())
                .collect();

            // Build <path> from path_segments (clone-and-detach each segment
            // into a fresh <path> wrapper).
            let path_node = if !path_segments.is_empty() {
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.append(use_node, path_node)?;
                for seg in path_segments {
                    xot.detach(seg)?;
                    xot.append(path_node, seg)?;
                }
                Some(path_node)
            } else {
                None
            };
            let _ = path_node;

            // For each leaf name, create a child `<use><name>X</name></use>`.
            for name in leaf_names {
                let inner_use_elt = xot.add_name("use");
                let inner_use = xot.new_element(inner_use_elt);
                xot.append(use_node, inner_use)?;
                xot.detach(name)?;
                xot.append(inner_use, name)?;
            }

            // Detach the now-empty body wrapper.
            xot.detach(body)?;

            // Add [group] marker.
            xot.with_prepended_marker(use_node, Group)?;
        } else {
            // Flat form: handle alias if present.
            // Element children all start as <name>X</name>.
            let names: Vec<XotNode> = element_children.iter()
                .copied()
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .collect();

            if has_alias_keyword && names.len() >= 2 {
                // Last <name> is the alias; preceding ones are path + leaf.
                let alias_name = *names.last().unwrap();
                let path_and_leaf = &names[..names.len() - 1];

                // path = all but last; leaf = last of path_and_leaf
                let leaf_idx = path_and_leaf.len() - 1;
                let path_segments = &path_and_leaf[..leaf_idx];
                let _leaf = path_and_leaf[leaf_idx];

                // Build <path> wrapping segments.
                if !path_segments.is_empty() {
                    let path_elt = xot.add_name(Path.as_str());
                    let path_node = xot.new_element(path_elt);
                    xot.insert_before(path_and_leaf[0], path_node)?;
                    for &seg in path_segments {
                        xot.detach(seg)?;
                        xot.append(path_node, seg)?;
                    }
                }
                // Wrap alias name in <alias>.
                let alias_elt = xot.add_name("aliased");
                let alias_node = xot.new_element(alias_elt);
                xot.insert_before(alias_name, alias_node)?;
                xot.detach(alias_name)?;
                xot.append(alias_node, alias_name)?;

                xot.with_prepended_marker(use_node, Alias)?;
            } else if names.len() >= 2 {
                // Plain multi-segment: all but last become <path>; last is leaf.
                let leaf_idx = names.len() - 1;
                let path_segments = &names[..leaf_idx];
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.insert_before(path_segments[0], path_node)?;
                for &seg in path_segments {
                    xot.detach(seg)?;
                    xot.append(path_node, seg)?;
                }
            }
            // names.len() == 1 → bare leaf, leave as-is.
        }

        if let Some(f) = flavor {
            xot.with_prepended_marker(use_node, f)?;
        }
        let _ = has_alias_keyword;

        // Discard the description doc-comment about `Const` only.
        // The const flavor distinguishes from function flavor.
    }

    Ok(())
}

/// Ruby post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
///
/// Ruby's tree-sitter grammar has no `expression_statement` analog —
/// expressions appear directly under `<body>`. So statement-level
/// host migration handles two layers:
/// 1. slot-level hosts (`left`/`right`/`condition`/`value`/`return`)
/// 2. body-level: walk `<body>` / `<then>` / `<else>` children and
///    wrap value-producing kinds in `<expression>`. Ruby has implicit
///    return — every method body's last expression IS the return
///    value — so value-producing children of body containers are
///    real expression positions and should carry the host.
fn ruby_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Ruby uses Java's flat call shape (`<call><object/>NAME...</call>`).
    // Wrap object+name into canonical `<member>` callee, then invert.
    crate::transform::chain_inversion::wrap_flat_call_member(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    // Ruby destructured params `proc { |(x, y)| ... }` produce a
    // `<parameter[destructured]>` with multiple `<name>` siblings.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("parameter", "name"),
            // Alternative patterns `1 | 2 | 3` produce
            // `<pattern[alternative]>` with multiple `<int>` (or
            // `<string>`/`<name>`) siblings. Per Principle #19
            // they're role-uniform — each is one alternative
            // option. Tag so JSON renders e.g. `ints: [...]`
            // instead of overflowing to `children`. Cardinality
            // discriminator (>=2) keeps singleton patterns alone.
            ("pattern", "int"),
            ("pattern", "string"),
            ("pattern", "name"),
            // Ruby interpolated strings: `<string>` parent with one or
            // more `<interpolation>` chunks. Bulk-distribute on
            // `"string"` (removed iter 309) was wrapping single-interp
            // cases in 1-elem JSON arrays.
            ("string", "interpolation"),
            // Ruby concatenated strings `"a" "b" "c"` —
            // `<string[concatenated]>` parent with multiple
            // `<string>` children. (Ruby has no
            // `tag_multi_same_name_children` call; cover this case
            // with the targeted role tag.)
            ("string", "string"),
            // Ruby array literals `[1, 2, 3]` etc. Iter 323 dropped
            // `"array"` from the bulk distribute config (was wrapping
            // singleton spread `[*items]` cases in 1-elem JSON arrays).
            // These targeted role tags cover the multi-cardinality
            // element types that exist in the blueprint.
            ("array", "int"),
            ("array", "name"),
            ("array", "object"),
        ],
    )?;
    crate::transform::wrap_body_value_children(
        xot,
        root,
        &["body", "then", "else"],
        RUBY_VALUE_KINDS,
    )?;
    ruby_retag_singleton_block_body(xot, root)?;
    ruby_collapse_lambda_body(xot, root)?;
    ruby_extract_pair_keys(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    ruby_tag_case_when_lists(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        // `"array"` removed iter 323 — was wrapping singleton `<spread>`
        // children of `[*items]` arrays in 1-elem JSON arrays.
        // Targeted role tags above cover the multi-cardinality cases
        // (int/name/object).
        xot, root, &["body", "program", "tuple", "list", "dict", "hash", "repetition"],
    )?;
    Ok(())
}

/// Tag the multi-instance role children of Ruby's pattern-match
/// constructs (`<case>`, `<when>`, `<match>`) with `list=` so JSON
/// consumers see them as arrays:
/// - `<case>` → `<when>` children → `list="when"` (case branches; multi).
/// - `<when>` → `<pattern>` children → `list="pattern"` (multi-pattern
///   `when X, Y` lifts each as a sibling).
/// - `<match>` → `<in>` children → `list="in"` (Ruby 3.0+
///   pattern-match `case x in ... in ... end`; multi-arm).
///
/// `distribute_member_list_attrs` would over-tag siblings that are
/// role-MIXED (e.g. `<case>`'s `<value>` discriminant and `<else>`,
/// which are singletons). Per Principle #19, we hand-pick the
/// roles that genuinely repeat.
fn ruby_tag_case_when_lists(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_attr, get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    fn collect(xot: &Xot, node: XotNode, name: &str, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some(name)
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, name, out);
        }
    }

    // `<case>` → tag `<when>` children with list="when".
    let mut cases: Vec<XotNode> = Vec::new();
    collect(xot, root, "case", &mut cases);
    for case in cases {
        let whens: Vec<XotNode> = xot.children(case)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("when")
            })
            .collect();
        for w in whens {
            if get_attr(xot, w, "list").is_none() {
                xot.with_attr(w, "list", "whens");
            }
        }
    }

    // `<when>` → tag `<pattern>` children with list="pattern".
    let mut whens: Vec<XotNode> = Vec::new();
    collect(xot, root, "when", &mut whens);
    for w in whens {
        let patterns: Vec<XotNode> = xot.children(w)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("pattern")
            })
            .collect();
        for p in patterns {
            if get_attr(xot, p, "list").is_none() {
                xot.with_attr(p, "list", "patterns");
            }
        }
    }

    // `<match>` → tag `<in>` children with list="in" (Ruby 3.0+
    // `case x in pat1 then ... in pat2 then ... end` pattern-match).
    let mut matches: Vec<XotNode> = Vec::new();
    collect(xot, root, "match", &mut matches);
    for m in matches {
        let ins: Vec<XotNode> = xot.children(m)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("in")
            })
            .collect();
        for i in ins {
            if get_attr(xot, i, "list").is_none() {
                xot.with_attr(i, "list", "ins");
            }
        }
    }

    Ok(())
}

/// Re-tag a `<body>` wrapper as `<value>` when its parent is a
/// `<block>` (call-attached closure: `arr.each { |x| ... }`,
/// `proc { ... }`, `arr.each do |x| ... end`) AND the body has
/// exactly one element child. This brings call-attached closures
/// into the iter 161/162/167/168 closure-body archetype:
/// `block/value/expression/...` for single-statement bodies;
/// multi-statement bodies keep `<body>` so per-statement `list=`
/// distribution remains visible.
///
/// Runs as a post-pass (not in a per-kind Custom handler) because
/// the count must be taken AFTER `block_body` / `body_statement`
/// flatten and AFTER `wrap_body_value_children` wraps value-
/// producing kids in `<expression>`. Doing this at walk-time would
/// always see "1 element child" (the unflattened block_body
/// wrapper), retagging multi-statement blocks too — bug fixed in
/// this iter (was iter 169).
///
/// Lambda's outer `<body>` (whose parent is `<lambda>`, not
/// `<block>`) is NOT touched here — see backlog item for Lambda
/// outer-body collapse.
fn ruby_retag_singleton_block_body(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("block")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut targets);

    let value_id = xot.add_name("value");
    let expr_id = xot.add_name("expression");
    for block in targets {
        let body_child = xot.children(block)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
        let body = match body_child { Some(b) => b, None => continue };
        let elem_children: Vec<XotNode> = xot.children(body)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if elem_children.len() != 1 { continue; }
        let only_child = elem_children[0];
        if let Some(elem) = xot.element_mut(body) {
            elem.set_name(value_id);
        }
        // wrap_body_value_children handled value-producing kinds at step 3
        // (they're already inside `<expression>`). Non-value-producing
        // single statements (`<if>`, `<break>`, `<while>`, …) need an
        // `<expression>` host now that they live in a `<value>` slot
        // (Principle #15). Idempotent: skip when already an expression.
        if get_element_name(xot, only_child).as_deref() != Some("expression") {
            let host = xot.new_element(expr_id);
            xot.with_wrap_child(only_child, host)?;
        }
    }
    Ok(())
}

/// Collapse the doubled-body shape produced by `->(x) { ... }` /
/// `->(x) do ... end`-style stabby lambdas into the
/// closure-archetype shape used by other languages
/// (`lambda/value/expression/...` for single-stmt; `lambda/body/...`
/// multi-stmt).
///
/// Tree-sitter's grammar nests two `<body>` levels for stabby
/// lambdas: one from field-wrapping `lambda.body` (outer), one from
/// field-wrapping `block.body` (inner). The inner block element
/// (`<block>` from `RubyKind::Block` Passthrough) sits between them,
/// carrying the literal `{` `}` text leaves. After
/// `ruby_retag_singleton_block_body`, the inner block contains
/// either `<value>` (single-stmt) or `<body>` (multi-stmt).
///
/// This pass lifts that inner element up to replace the outer
/// `body/block` chain, producing:
/// - single-stmt `->(x) { x + 1 }` → `lambda/value/expression/binary/...`
///   (matches Rust closure / TS arrow / C# lambda / PHP arrow / Python
///    lambda from iters 161/162/167/168).
/// - multi-stmt `->(x) { puts x; x + 1 }` → `lambda/body/expression: [..., ...]`
///   (mirrors Ruby Block multi-stmt shape from iter 173).
///
/// Note: `lambda do ... end` is parsed as a `call` to the `lambda`
/// method with an attached `<do_block>`, NOT as a `<lambda>`
/// element — handled by the iter-173 call-attached path.
fn ruby_collapse_lambda_body(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut lambdas: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("lambda")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut lambdas);

    for lambda in lambdas {
        let outer_body = xot.children(lambda)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
        let outer_body = match outer_body { Some(b) => b, None => continue };

        let body_elem_children: Vec<XotNode> = xot.children(outer_body)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if body_elem_children.len() != 1 { continue; }
        let block = body_elem_children[0];
        if get_element_name(xot, block).as_deref() != Some("block") { continue; }

        let block_elem_children: Vec<XotNode> = xot.children(block)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if block_elem_children.len() != 1 { continue; }
        let inner = block_elem_children[0];
        let inner_name = get_element_name(xot, inner);
        if !matches!(inner_name.as_deref(), Some("value") | Some("body")) { continue; }

        // Lift: detach inner from block, insert before outer_body, detach outer_body.
        // The block element (and any text leaves it contained, like `{` / `}`)
        // is dropped — this is structural, source-text fidelity is advisory.
        xot.detach(inner)?;
        xot.insert_before(outer_body, inner)?;
        xot.detach(outer_body)?;
    }
    Ok(())
}

/// Within-language Principle #5: every Ruby pair should expose its
/// key as a structured child (not bare text). Three source forms:
///   1. `id: value`     — key is shorthand symbol; tree-sitter emits
///                         `"id:"` as a single text leaf.
///   2. `'k' => value`  — key is a string literal; the `=>` is a
///                         bare text leaf between key and value.
///   3. `:foo => value` — key is an explicit symbol; tree-sitter
///                         emits `":foo =>"` as a single text leaf.
///
/// Extract the key into a proper `<name>` (form 1) or `<symbol>`
/// (form 3) element, and strip the `=>` text (form 2). Source-text
/// preservation is given up for queryability — Ruby pairs become
/// uniformly `<pair><name|symbol|string>K</...><value>V</value></pair>`.
fn ruby_extract_pair_keys(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let mut pairs: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "pair", &mut pairs);

    for pair in pairs {
        // Inspect text leaves and strip arrow-only ones.
        let children: Vec<XotNode> = xot.children(pair).collect();
        for child in &children {
            let trimmed = match xot.text_str(*child) {
                Some(t) => t.trim().to_string(),
                None => continue,
            };
            // Form 2: bare `=>` text — strip.
            if trimmed == "=>" {
                xot.detach(*child)?;
                continue;
            }
            // Form 3: ":foo =>" — extract symbol foo, strip arrow.
            if trimmed.starts_with(':') && trimmed.ends_with("=>") {
                let key_part = trimmed
                    .trim_start_matches(':')
                    .trim_end_matches("=>")
                    .trim()
                    .to_string();
                if !key_part.is_empty() {
                    let symbol_elt = xot.add_name("symbol");
                    let symbol_node = xot.new_element(symbol_elt);
                    let key_text = xot.new_text(&key_part);
                    xot.append(symbol_node, key_text)?;
                    xot.insert_before(*child, symbol_node)?;
                }
                xot.detach(*child)?;
                continue;
            }
            // Form 1: "id:" — extract bare name, strip trailing `:`.
            if trimmed.ends_with(':') && !trimmed.starts_with(':') {
                let key_part = trimmed
                    .trim_end_matches(':')
                    .trim()
                    .to_string();
                if !key_part.is_empty() && !key_part.contains(char::is_whitespace) {
                    let name_elt = xot.add_name("name");
                    let name_node = xot.new_element(name_elt);
                    let key_text = xot.new_text(&key_part);
                    xot.append(name_node, key_text)?;
                    xot.insert_before(*child, name_node)?;
                }
                xot.detach(*child)?;
                continue;
            }
        }
        let _ = get_element_name;
    }
    Ok(())
}

/// Element names that are value-producing in Ruby and should be
/// wrapped in `<expression>` when they appear as direct children of
/// a body-level container (`<body>`, `<then>`, `<else>`). Names NOT in
/// this list are statement-only (declarations, control flow, jump
/// statements, comments) and are left bare.
const RUBY_VALUE_KINDS: &[&str] = &[
    // Calls / member access / indexing — function results are values.
    "call", "member", "index", "lambda", "yield",
    // Operator expressions
    "binary", "unary", "conditional", "range", "match",
    // Literals
    "string", "symbol", "int", "float", "regex",
    "true", "false", "nil", "self",
    "array", "hash", "pair",
    // Identifiers / references
    "name",
];

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
