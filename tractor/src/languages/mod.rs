//! Language-specific transform modules and metadata.
//!
//! Each language owns its complete transform logic.
//! The shared infrastructure (xot_transform) provides only the walker and helpers.

pub mod info;
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
use crate::xot_transform::TransformAction;
use crate::output::syntax_highlight::SyntaxCategory;

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

/// Get the transform function for a language (single-branch transform)
///
/// For data-aware languages (JSON, YAML), prefer `get_data_transforms()` which
/// returns separate AST and data transforms for dual-branch output.
pub fn get_transform(lang: &str) -> TransformFn {
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => typescript::transform,
        "csharp" | "cs" => csharp::transform,
        "python" | "py" => python::transform,
        "go" => go::transform,
        "rust" | "rs" => rust_lang::transform,
        "java" => java::transform,
        "ruby" | "rb" => ruby::transform,
        "php" => php::transform,
        "json" => json::data_transform,
        "yaml" | "yml" => yaml::data_transform,
        "toml" => toml::transform,
        "ini" => ini::transform,
        "env" => env::transform,
        "markdown" | "md" | "mdx" => markdown::transform,
        "tsql" | "mssql" => tsql::transform,
        // Default: passthrough (no transforms)
        _ => passthrough_transform,
    }
}

// /specs/tractor-parse/dual-view/supported-languages.md: Supported Languages
/// Get dual-branch transform functions for data-aware languages.
///
/// Returns `Some((syntax_transform, data_transform))` for languages
/// that produce both a `/syntax` and `/data` branch, or `None` for other languages.
pub fn get_data_transforms(lang: &str) -> Option<(TransformFn, TransformFn)> {
    match lang {
        "json" => Some((json::ast_transform, json::data_transform)),
        "yaml" | "yml" => Some((yaml::ast_transform, yaml::data_transform)),
        _ => None,
    }
}

/// Get the post-transform function for a language, if any.
///
/// The post-transform runs after `walk_transform` has completed. It
/// receives the document root so it can walk the already-renamed tree
/// and perform structural rewrites that need final names in place.
///
/// Currently this is where the conditional-shape collapse lives for
/// languages whose grammars produce nested `else`/`if` chains (all
/// seven programming languages; Python's elif is flat but the pass is
/// a no-op for it).
pub fn get_post_transform(lang: &str) -> Option<PostTransformFn> {
    match lang {
        "csharp" | "cs" => Some(csharp_post_transform),
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx"
        | "go"
        | "rust" | "rs"
        | "java"
        | "ruby" | "rb"
        | "php" => Some(collapse_conditionals),
        _ => None,
    }
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
    use crate::xot_transform::helpers::*;

    // Collect all clause nodes (mutate later).
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        use crate::xot_transform::helpers::*;
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
    use crate::xot_transform::helpers::*;

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
    use crate::xot_transform::helpers::*;
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
    use crate::xot_transform::helpers::*;
    if xot.element(node).is_some() && get_element_name(xot, node).as_deref() == Some("if") {
        out.push(node);
    }
    for child in xot.children(node) {
        collect_if_nodes(xot, child, out);
    }
}

/// Check whether a language supports the data tree projection.
pub fn supports_data_tree(lang: &str) -> bool {
    matches!(lang, "json" | "yaml" | "yml")
}

/// True for programming languages (as opposed to data/config languages).
/// Used to gate post-transforms like identifier-role marking that only
/// make sense when the tree has declaration/reference semantics.
pub fn is_programming_language(lang: &str) -> bool {
    matches!(
        lang,
        "typescript" | "ts" | "tsx"
            | "javascript" | "js" | "jsx"
            | "csharp" | "cs"
            | "python" | "py"
            | "go"
            | "rust" | "rs"
            | "java"
            | "ruby" | "rb"
            | "php"
            | "tsql" | "mssql"
    )
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
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => TS_FIELD_WRAPPINGS,
        "rust" | "rs" => RUST_FIELD_WRAPPINGS,
        "go" => GO_FIELD_WRAPPINGS,
        "csharp" | "cs" => CSHARP_FIELD_WRAPPINGS,
        "ruby" | "rb" => RUBY_FIELD_WRAPPINGS,
        _ => COMMON_FIELD_WRAPPINGS,
    }
}

/// Every element name introduced by the builder's `apply_field_wrappings`
/// pass for the given language (i.e. the right-hand side of each
/// `(tree_sitter_field, wrapper_element_name)` pair). These names are
/// wrapper elements created outside of the per-language transform, so
/// they don't (and shouldn't) live in each language's `semantic::ALL_NAMES`.
/// The `all_names_declared_in_semantic_module` invariant consults this
/// helper to treat them as universally allowed.
pub fn field_wrapper_names(_lang: &str) -> &'static [&'static str] {
    // Union of every wrapper name used anywhere. Keeping a single static
    // list (rather than per-language) is safe because the invariant only
    // asks "is this name a recognised field wrapper?" — not "was this
    // wrapper name configured for *this* language?". Any language-
    // specific drift is caught by the transform's own tests.
    &[
        "name", "value", "left", "right", "body", "condition", "then",
        "returns", "callee", "object", "property",
    ]
}

/// Get the syntax category function for a language
/// This maps transformed element names to syntax categories for highlighting
pub fn get_syntax_category(lang: &str) -> SyntaxCategoryFn {
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => typescript::syntax_category,
        "csharp" | "cs" => csharp::syntax_category,
        "python" | "py" => python::syntax_category,
        "go" => go::syntax_category,
        "rust" | "rs" => rust_lang::syntax_category,
        "java" => java::syntax_category,
        "ruby" | "rb" => ruby::syntax_category,
        "php" => php::syntax_category,
        "json" => json::syntax_category,
        "yaml" | "yml" => yaml::syntax_category,
        "toml" => toml::syntax_category,
        "ini" => ini::syntax_category,
        "env" => env::syntax_category,
        "markdown" | "md" | "mdx" => markdown::syntax_category,
        "tsql" | "mssql" => tsql::syntax_category,
        // Default: generic fallback
        _ => default_syntax_category,
    }
}

/// Get the singleton wrapper list for a language.
///
/// Returns the list of wrapper element names that should have their single
/// child annotated with `field` for JSON property lifting.
/// Data-aware languages (JSON, YAML) return an empty list.
pub fn get_singleton_wrappers(lang: &str) -> &'static [&'static str] {
    use crate::xot_transform::helpers::DEFAULT_SINGLETON_WRAPPERS;
    match lang {
        // Data languages don't have singleton wrappers
        "json" | "yaml" | "yml" | "toml" | "ini" | "env" | "markdown" | "md" | "mdx" => &[],
        // All programming languages use the default list
        _ => DEFAULT_SINGLETON_WRAPPERS,
    }
}

/// Return the MARKER_ONLY slice for a language ID, if any.
/// Used by the `markers_stay_empty` invariant to assert that names
/// declared as marker-only never carry text or element children.
pub fn marker_only_names(lang: &str) -> Option<&'static [&'static str]> {
    match lang {
        "csharp" | "cs" => Some(csharp::semantic::MARKER_ONLY),
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => Some(typescript::semantic::MARKER_ONLY),
        "python" | "py" => Some(python::semantic::MARKER_ONLY),
        "rust" | "rs" => Some(rust_lang::semantic::MARKER_ONLY),
        "go" => Some(go::semantic::MARKER_ONLY),
        "java" => Some(java::semantic::MARKER_ONLY),
        "php" => Some(php::semantic::MARKER_ONLY),
        "ruby" | "rb" => Some(ruby::semantic::MARKER_ONLY),
        "tsql" | "mssql" | "sql" => Some(tsql::semantic::MARKER_ONLY),
        _ => None,
    }
}

/// Return the ALL_NAMES slice for a language ID, if any.
/// Covers every semantic element name a language's transform can emit
/// — structural containers AND marker-only names.
pub fn all_semantic_names(lang: &str) -> Option<&'static [&'static str]> {
    match lang {
        "csharp" | "cs" => Some(csharp::semantic::ALL_NAMES),
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => Some(typescript::semantic::ALL_NAMES),
        "python" | "py" => Some(python::semantic::ALL_NAMES),
        "rust" | "rs" => Some(rust_lang::semantic::ALL_NAMES),
        "go" => Some(go::semantic::ALL_NAMES),
        "java" => Some(java::semantic::ALL_NAMES),
        "php" => Some(php::semantic::ALL_NAMES),
        "ruby" | "rb" => Some(ruby::semantic::ALL_NAMES),
        "tsql" | "mssql" | "sql" => Some(tsql::semantic::ALL_NAMES),
        _ => None,
    }
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
