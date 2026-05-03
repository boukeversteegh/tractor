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
        post_transform: Some(typescript_post_transform),
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
        post_transform: Some(python_post_transform),
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
        post_transform: Some(rust_post_transform),
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

/// C# combines two post-transforms: `attach_where_clause_constraints`
/// moves `where T : …` constraints into the matching `<generic>`, then
/// the shared conditional collapse runs.
fn csharp_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    attach_where_clause_constraints(xot, root)?;
    unify_file_scoped_namespace(xot, root)?;
    // Conditional access (`obj?.Method`) emits as
    // `<member[optional]><condition><expression>RECV</expression></condition><name>X</name></member>`.
    // Pre-pass converts the `<condition>` slot to canonical `<object>`
    // and wraps the bare `<name>` in `<property>` so the chain
    // inverter can process it.
    csharp_normalize_conditional_access(xot, root)?;
    // Invert right-deep <member>/<call> chains.
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // C# try/catch: `<try>` parent with `<body>` + multiple `<catch>`
    // siblings. Tag catches with `list="catch"`; body stays singleton.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("try", "catch"),
            // Multi-declarator (`int x = 1, y = 2`) keeps
            // `<declarator>` wrappers (per iter 263). Tag with
            // `list="declarators"` so JSON renders them as an
            // array; single-declarator is flattened earlier and
            // doesn't reach this pass.
            ("variable", "declarator"),
            ("field", "declarator"),
        ],
    )?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        // C# slot wrappers that contain a single expression operand.
        // `then`/`else` are block bodies (statement sequences), not
        // single-expression slots.
        &["value", "condition", "left", "right", "return"],
    )?;
    // Strip braces from C# block/body containers. `<block>` is the
    // statement-block variant; `<section>` is the switch-section
    // wrapper. `<call>` here catches the `(`/`)` parens that the
    // argument_list flatten promotes into the renamed
    // constructor_initializer (`<call[this]>`/`<call[base]>`) after
    // its handler ran.
    crate::transform::strip_body_braces(xot, root, &["body", "block", "section", "call"])?;
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    // Flatten single-declarator wrappers in fields and locals so
    // `int x = 1;` becomes `field/{type, name, value}` instead of
    // `field/{type, declarator/{name, value}}`. Multi-declarator
    // (`int a, b = 5`) keeps the wrapper — each declarator is a
    // role-mixed name+value group whose pairing depends on the
    // wrapper. See cold-read backlog iter 233.
    crate::transform::flatten_single_declarator_children(xot, root, &["field", "variable"])?;
    crate::transform::distribute_member_list_attrs(
        xot, root,
        &["body", "block", "unit", "namespace", "section", "import", "tuple", "list", "dict", "array", "hash", "switch", "literal", "macro", "template", "string", "repetition"],
    )?;
    Ok(())
}

/// C# namespace shape unification (closes todo/34).
///
/// Per Principle #5, both `namespace Foo { ... }` (block-scoped)
/// and `namespace Foo;` (C# 10+ file-scoped) should share the same
/// shape: declarations are direct children of `<namespace>`. The
/// file-scoped form additionally carries a `<file/>` marker so
/// `//namespace[file]` distinguishes the two when needed.
///
/// Two transforms here:
///
/// 1. **Drop the `<body>` wrapper from namespaces.** The C# field-
///    distribution pass adds `<body>` for any element with a `body`
///    field on the source. For namespaces the wrapper is misleading
///    — a namespace doesn't have a "body" the way methods do; its
///    children are first-class declarations. Walks every
///    `<namespace>/<body>` and unwraps the body in place.
///
/// 2. **Fold file-scoped trailing siblings into the namespace.**
///    Tree-sitter exposes file-scoped namespace as `<namespace>`
///    followed by flat sibling declarations under `<unit>`. After
///    step 1, both block-scoped and file-scoped forms have flat
///    declarations; for file-scoped we additionally need to move
///    the trailing siblings INTO the namespace.
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

/// Pre-pass for chain inversion: convert C# conditional-access
/// (`obj?.Method`) shape to canonical input.
///
/// C# emits:
///   `<member[instance and optional]><condition><expression>RECV</expression></condition><name>X</name></member>`
///
/// The chain inverter wants:
///   `<member[instance and optional]><object>RECV</object><property><name>X</name></property></member>`
///
/// Walks every `<member>` with a `<condition>` child and:
///   1. Renames `<condition>` → `<object>`.
///   2. Unwraps the inner `<expression>` host so the receiver is a
///      direct child of `<object>`.
///   3. Wraps the trailing bare `<name>` in `<property>`.
///
/// Idempotent: a `<member>` already in canonical shape (with
/// `<object>` slot, no `<condition>`) is skipped.
fn csharp_normalize_conditional_access(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{copy_source_location, get_element_name, rename};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut members: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("member")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut members);
    for member in members {
        let condition_slot = xot.children(member).find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("condition")
        });
        let condition_slot = match condition_slot {
            Some(c) => c,
            None => continue,
        };
        // Rename <condition> → <object>.
        rename(xot, condition_slot, "object");
        // Unwrap the inner <expression> host.
        let expr_inner = xot.children(condition_slot).find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("expression")
        });
        if let Some(expr) = expr_inner {
            let children: Vec<XotNode> = xot.children(expr).collect();
            for child in children {
                xot.detach(child)?;
                xot.insert_before(expr, child)?;
            }
            xot.detach(expr)?;
        }
        // Wrap bare <name> in <property>.
        let name_node = xot.children(member).find(|&c| {
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
    Ok(())
}

fn unify_file_scoped_namespace(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;

    // Step 1: unwrap `<namespace>/<body>` everywhere.
    let mut bodies_to_unwrap: Vec<XotNode> = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if xot.element(n).is_some()
            && get_element_name(xot, n).as_deref() == Some("namespace")
        {
            for child in xot.children(n) {
                if get_element_name(xot, child).as_deref() == Some("body") {
                    bodies_to_unwrap.push(child);
                }
            }
        }
        for child in xot.children(n) {
            stack.push(child);
        }
    }
    for body in bodies_to_unwrap {
        let inner: Vec<XotNode> = xot.children(body).collect();
        for c in inner {
            xot.detach(c)?;
            xot.insert_before(body, c)?;
        }
        xot.detach(body)?;
    }

    // Step 2: file-scoped namespaces — fold following siblings.
    let mut targets: Vec<XotNode> = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if xot.element(n).is_some()
            && get_element_name(xot, n).as_deref() == Some("namespace")
            && xot.children(n).any(|c| {
                get_element_name(xot, c).as_deref() == Some("file")
            })
        {
            targets.push(n);
        }
        for child in xot.children(n) {
            stack.push(child);
        }
    }
    for ns in targets {
        let parent = match xot.parent(ns) {
            Some(p) => p,
            None => continue,
        };
        let mut following: Vec<XotNode> = Vec::new();
        let mut after = false;
        for sibling in xot.children(parent).collect::<Vec<_>>() {
            if sibling == ns {
                after = true;
                continue;
            }
            if after && xot.element(sibling).is_some() {
                following.push(sibling);
            }
        }
        for sibling in following {
            xot.detach(sibling)?;
            xot.append(ns, sibling)?;
        }
    }
    Ok(())
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

/// Rust post-transform: collapse conditionals + wrap expression positions
/// in `<expression>` hosts (Principle #15).
///
/// The expression-position pass runs after `collapse_conditionals` so the
/// `then`/`else` slots produced by the conditional collapse get hosts too.
fn rust_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Rust's `field_expression` (`obj.foo`) renames to `<field>`
    // alongside FieldDeclaration / FieldInitializer / etc. The
    // chain inverter expects the canonical `<member>` shape, so
    // pre-pass converts the field-expression flavor to canonical
    // first.
    rust_normalize_field_expression(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        // Slot wrappers that contain a single expression operand.
        // `then`/`else` are block bodies (statement sequences) and
        // must not be wrapped — their children carry their own
        // statement-level hosts via `expression_statement`.
        // `return` holds the optional return value as its first
        // element child; wrap so `<return>/<expression>/...` is the
        // uniform shape (no value -> no host, the wrap pass is a
        // no-op for empty returns).
        &["value", "condition", "left", "right", "return"],
    )?;
    rust_restructure_use(xot, root)?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // Rust `if let ... && let ...` chains produce `<condition>` with
    // multiple `<expression>` siblings (one per let-clause).
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("condition", "expression"),
            // Rust `use std::fmt::{Display, Write as IoWrite}` —
            // `<use[group]>` parent with multiple inner `<use>`
            // siblings (one per imported entity). Tag with
            // `list="uses"` so JSON renders as `uses: [...]` array
            // rather than colliding on the singleton `use` key
            // and overflowing into `children`.
            ("use", "use"),
        ],
    )?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "block"])?;
    rust_normalize_lifetime_names(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "block", "file", "tuple", "list", "dict", "array", "switch", "literal", "macro", "template", "string", "repetition"],
    )?;
    Ok(())
}

/// Within-Rust Principle #5: every `<lifetime>` exposes its identifier
/// as `<name>X</name>` without the leading apostrophe. Tree-sitter
/// declaration-position lifetimes (`<'a>` in generics) keep the `'`
/// inside the inner name's text via field-wrapping, while use-position
/// lifetimes (`&'a str`) emit the `'` as a separate text leaf and
/// rename the identifier to `<name>a</name>`. Normalize the
/// declaration-position form to match: strip a leading `'` from any
/// `<name>` text whose parent is a `<lifetime>`. Idempotent.
/// Pre-pass for chain inversion: convert Rust `field_expression`-derived
/// `<field>` elements to canonical `<member>`/`<object>`/`<property>`.
///
/// Rust emits:
///   `<field><value><expression>RECEIVER</expression></value><name>X</name></field>`
///
/// The chain inverter wants:
///   `<member><object>RECEIVER</object><property><name>X</name></property></member>`
///
/// Identifies the field-expression flavor by the presence of a
/// `<value>` child slot (Rust's other `<field>` uses — declarations,
/// initializers — don't have this shape). Skips non-matching `<field>`
/// elements. Idempotent.
fn rust_normalize_field_expression(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{copy_source_location, get_attr, get_element_name, rename};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut fields: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("field")
            // Discriminate by tree-sitter kind. `<field>` is shared
            // by FieldDeclaration / FieldExpression /
            // FieldInitializer / ShorthandFieldInitializer (all
            // renamed to Field in rules.rs). Only the expression
            // flavour participates in member-access chains.
            && get_attr(xot, node, "kind").as_deref() == Some("field_expression")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut fields);
    for field in fields {
        // Field-expression always has a <value> slot (the receiver).
        let value_slot = xot.children(field).find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("value")
        });
        let value_slot = match value_slot {
            Some(v) => v,
            None => continue,
        };
        // Rename element: <field> → <member>.
        rename(xot, field, "member");
        // Rename slot: <value> → <object>.
        rename(xot, value_slot, "object");
        // Unwrap the inner <expression> host so <object>RECV</object>
        // is direct (matches canonical shape — Python/Go don't have an
        // <expression> host inside <object>).
        let expr_inner = xot.children(value_slot).find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("expression")
        });
        if let Some(expr) = expr_inner {
            // Lift expression's children up into <object>.
            let children: Vec<XotNode> = xot.children(expr).collect();
            for c in children {
                xot.detach(c)?;
                xot.insert_before(expr, c)?;
            }
            xot.detach(expr)?;
        }
        // Wrap bare <name> in <property>.
        let name_node = xot.children(field).find(|&c| {
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
    Ok(())
}

fn rust_normalize_lifetime_names(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::*;
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("lifetime")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut targets);
    for lifetime in targets {
        let name_children: Vec<XotNode> = xot.children(lifetime)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("name")
            })
            .collect();
        for name in name_children {
            if let Some(text) = get_text_content(xot, name) {
                let trimmed = text.trim_start_matches('\'');
                if trimmed.len() != text.len() {
                    xot.with_only_text(name, trimmed)?;
                }
            }
        }
    }
    Ok(())
}

/// Restructure every Rust `<use>` element into the unified shape
/// (per `imports-grouping.md`):
///
///   use std::collections::HashMap                  → <use><path><name>std</name><name>collections</name></path><name>HashMap</name></use>
///   use std::collections::HashSet as Set           → <use[alias]>...<name>HashSet</name><alias><name>Set</name></alias></use>
///   use std::collections::{HashMap, HashSet}       → <use[group]>...<use><name>HashMap</name></use><use><name>HashSet</name></use></use>
///   use std::fmt::self                             → <use[self]>...</use>
///   use std::fmt::*                                → <use[wildcard]>...</use>
///   pub use foo::bar                               → <use[reexport][pub]>...</use>
fn rust_restructure_use(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name};
    use rust_lang::output::TractorNode::{Alias, Group, Reexport, Self_, Wildcard};

    let mut targets: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "use", &mut targets);

    for use_node in targets {
        // Skip inner <use> elements that are children of a grouped <use>
        // (we may be re-walking after restructuring an outer one).
        if xot.parent(use_node)
            .and_then(|p| get_element_name(xot, p))
            .as_deref() == Some("use")
        {
            continue;
        }

        // 1. Inspect text leaves for keywords / sigils.
        let mut has_as = false;
        let mut has_wildcard = false;
        let mut has_reexport_keyword = false;
        for child in xot.children(use_node).collect::<Vec<_>>() {
            let Some(text) = xot.text_str(child) else { continue };
            for tok in text.split(|c: char| {
                c.is_whitespace() || matches!(c, ':' | '{' | '}' | ';' | ',')
            }) {
                match tok {
                    "as" => has_as = true,
                    "*" => has_wildcard = true,
                    _ => {}
                }
            }
            if text.contains("pub use") {
                has_reexport_keyword = true;
            }
        }
        // The `[pub]` marker on a use element implies a re-export.
        let has_pub_marker = xot.children(use_node)
            .any(|c| get_element_name(xot, c).as_deref() == Some("pub"));
        if has_pub_marker {
            has_reexport_keyword = true;
        }

        // 2. Note `<self>` element (e.g. `use std::fmt::self`).
        let self_child = xot.children(use_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("self"));
        let has_self = self_child.is_some();
        if let Some(s) = self_child {
            xot.detach(s)?;
        }

        // 2b. BEFORE stripping noise text, capture which `<name>` pairs
        //     are joined by an `as` text node. This is the only signal
        //     we have that `Foo as Bar` belongs together inside a group
        //     `{X, Foo as Bar, Y}` — `use_as_clause` flattens its
        //     children, so the only remaining trace of pairing is the
        //     `as` text leaf between two adjacent name elements.
        let mut alias_pairs: Vec<(XotNode, XotNode)> = Vec::new();
        let children_seq: Vec<XotNode> = xot.children(use_node).collect();
        for window in children_seq.windows(3) {
            let (a, mid, b) = (window[0], window[1], window[2]);
            if get_element_name(xot, a).as_deref() == Some("name")
                && get_element_name(xot, b).as_deref() == Some("name")
            {
                if let Some(text) = xot.text_str(mid) {
                    if text.split_whitespace().any(|t| t == "as") {
                        alias_pairs.push((a, b));
                    }
                }
            }
        }

        // 3. Strip ALL noise text leaves on use_node.
        for child in xot.children(use_node).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }

        // 4. Lift the trailing `<name>` out of the `<path>` IF this is
        //    a simple-leaf case (`use std::collections::HashMap`) or an
        //    alias case (`use std::collections::HashSet as Set` —
        //    which has the leaf inside path and `as Set` as a sibling
        //    name; we need both as siblings to wrap one as `<alias>`).
        //    DON'T lift for group / wildcard / self-only cases — the
        //    path-trailing segment IS a path segment there.
        let path_child = xot.children(use_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("path"));
        if let Some(path) = path_child {
            // Flatten any nested `<path>` once.
            let inner_paths: Vec<XotNode> = xot.children(path)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("path"))
                .collect();
            for inner in inner_paths {
                let inner_children: Vec<_> = xot.children(inner).collect();
                for c in inner_children {
                    xot.detach(c)?;
                    xot.insert_before(inner, c)?;
                }
                xot.detach(inner)?;
            }
            // Strip path-internal noise.
            for child in xot.children(path).collect::<Vec<_>>() {
                if xot.text_str(child).is_some() {
                    xot.detach(child)?;
                }
            }
            // Count sibling names of path BEFORE the lift to classify the
            // variant.
            let pre_sibling_names = xot.children(use_node)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .count();
            // Lift only when this is a simple-leaf (0 siblings + no
            // wildcard/self) OR an alias (has_as case where the alias
            // occupies one sibling slot but the leaf still lives inside
            // path).
            let should_lift = (!has_wildcard && !has_self && pre_sibling_names == 0)
                || (has_as && pre_sibling_names == 1);
            if should_lift {
                let path_names: Vec<XotNode> = xot.children(path)
                    .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                    .collect();
                if path_names.len() >= 2 {
                    let leaf = *path_names.last().unwrap();
                    xot.detach(leaf)?;
                    if let Some(next) = xot.next_sibling(path) {
                        xot.insert_before(next, leaf)?;
                    } else if let Some(parent) = xot.parent(path) {
                        xot.append(parent, leaf)?;
                    }
                }
            }
        }

        // 5. Now the use_node has: optional <path>, then 0+ <name>
        //    siblings. The number of name siblings + has_as / has_self
        //    determines the variant.
        let leaf_names: Vec<XotNode> = xot.children(use_node)
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .collect();

        // Set of names that are the *alias* (second of an `as` pair).
        let alias_targets: std::collections::HashSet<XotNode> =
            alias_pairs.iter().map(|&(_, b)| b).collect();
        // Set of names that are the *original* (first of an `as` pair).
        let alias_originals: std::collections::HashSet<XotNode> =
            alias_pairs.iter().map(|&(a, _)| a).collect();

        let is_flat_alias_with_pair = has_as && alias_pairs.len() == 1
            && leaf_names.len() == 2
            && alias_pairs[0].0 == leaf_names[0]
            && alias_pairs[0].1 == leaf_names[1];
        // Flat alias when `use std::Foo as Bar` — original was inside
        // `<path>` so no name-name `as` pair was captured (the captured
        // adjacency was path-as-name). After step 4 lifted the path
        // leaf, we now have two name siblings. has_as + 2 names with
        // no captured pair = flat path-leaf alias.
        let is_flat_alias_path_form = has_as && alias_pairs.is_empty()
            && leaf_names.len() == 2;

        if is_flat_alias_with_pair || is_flat_alias_path_form {
            let alias_name = leaf_names[1];
            let alias_elt = xot.add_name("aliased");
            let alias_node = xot.new_element(alias_elt);
            xot.insert_before(alias_name, alias_node)?;
            xot.detach(alias_name)?;
            xot.append(alias_node, alias_name)?;
            xot.with_prepended_marker(use_node, Alias)?;
        } else if leaf_names.len() >= 2 || (leaf_names.len() >= 1 && has_self) {
            // Group form. For each leaf name that's NOT the second of an
            // alias pair, create an inner `<use>`. Pair-original names
            // get inner `<use[alias]>` wrappers that ALSO consume the
            // following alias-target name.
            let mut i = 0;
            while i < leaf_names.len() {
                let name = leaf_names[i];
                if alias_targets.contains(&name) {
                    // Already consumed by previous alias pair.
                    i += 1;
                    continue;
                }
                let inner_use_elt = xot.add_name("use");
                let inner_use = xot.new_element(inner_use_elt);
                xot.insert_before(name, inner_use)?;
                xot.detach(name)?;
                xot.append(inner_use, name)?;
                if alias_originals.contains(&name) {
                    // Find paired alias target and wrap in <alias>.
                    let paired = alias_pairs.iter()
                        .find(|&&(orig, _)| orig == name)
                        .map(|&(_, alias)| alias);
                    if let Some(alias_name) = paired {
                        let alias_elt = xot.add_name("aliased");
                        let alias_node = xot.new_element(alias_elt);
                        xot.append(inner_use, alias_node)?;
                        xot.detach(alias_name)?;
                        xot.append(alias_node, alias_name)?;
                        xot.with_prepended_marker(inner_use, Alias)?;
                    }
                }
                i += 1;
            }
            // If there was a `<self>` entry, add inner `<use[self]/>`.
            if has_self {
                let inner_use_elt = xot.add_name("use");
                let inner_use = xot.new_element(inner_use_elt);
                xot.append(use_node, inner_use)?;
                xot.with_prepended_marker(inner_use, Self_)?;
            }
            xot.with_prepended_marker(use_node, Group)?;
        } else if has_self && leaf_names.is_empty() {
            // Single self-import: `use std::fmt::self`.
            xot.with_prepended_marker(use_node, Self_)?;
        }

        if has_wildcard {
            xot.with_prepended_marker(use_node, Wildcard)?;
        }
        if has_reexport_keyword {
            xot.with_prepended_marker(use_node, Reexport)?;
        }
    }

    Ok(())
}

/// TypeScript post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
fn typescript_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Normalise the call shape so it matches the canonical right-deep
    // input expected by chain_inversion::extract_chain. TS wraps the
    // call's callee in `<callee>` (via FIELD_WRAPPINGS); unwrap it so
    // `<call>` directly contains the callee element (a `<member>` or
    // bare `<name>`/`<call>`/etc.). Same shape as Python/Go.
    typescript_unwrap_callee(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // TS function-type / object-type signatures: `<type>` parent with
    // multiple `<parameter>` (function type) or `<property>` (object
    // type) siblings — uniform-role children inside a role-MIXED
    // parent (since `<type>` is also used as a singleton type wrapper).
    // Targeted via tag_multi_role_children rather than bulk distribute.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("type", "parameter"),
            ("type", "property"),
            // TS object literals: `{a: 1, b: 2}` → `<object>` parent
            // with multiple `<pair>` siblings.
            ("object", "pair"),
            // Multi-declarator (`let i = 0, j = 100`) keeps
            // `<declarator>` wrappers (per iter 264) so JSON
            // can render them as `declarators: [...]` array.
            // Single-declarator is flattened by
            // `flatten_single_declarator_children` and never
            // reaches this tag pass.
            ("variable", "declarator"),
            ("field", "declarator"),
        ],
    )?;
    typescript_restructure_import(xot, root)?;
    // Run AFTER restructure_import so the `<import>` group-form
    // element has its final inner `<import>` siblings.
    crate::transform::tag_multi_same_name_children(xot, root, &["import"])?;
    crate::transform::strip_body_braces(xot, root, &["body", "block", "then", "else"])?;
    // Single-declarator variable declarations flatten the
    // <declarator> wrapper. Multi-declarator (`let i = 0, j = 100`)
    // keeps wrappers so name↔value pairing is preserved per
    // declarator. Mirrors Java/C# iter 263.
    crate::transform::flatten_single_declarator_children(xot, root, &["variable", "field"])?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "block", "program", "tuple", "list", "dict", "array", "hash", "switch", "literal", "macro", "template", "string", "repetition"],
    )?;
    Ok(())
}

/// Unwrap `<callee>` field-wrapper inside `<call>` so the call's
/// first element child is the actual callee (matching the canonical
/// right-deep input that `chain_inversion::extract_chain` expects).
///
/// FIELD_WRAPPINGS routes tree-sitter `field="function"` to
/// `<callee>X</callee>`, exposing the call target as a named slot.
/// For chain inversion this wrapper is in the way: the extractor
/// looks for the callee as the first non-marker child of `<call>`,
/// not nested under `<callee>`. Unwrapping post-build (and pre-
/// inversion) preserves the FIELD_WRAPPINGS contract for languages
/// that don't run chain inversion while letting TS adopt the
/// canonical shape.
fn typescript_unwrap_callee(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut callees: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("callee")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut callees);
    for callee in callees {
        // Lift each child of <callee> up to the parent <call>, then
        // detach the now-empty <callee>.
        let children: Vec<XotNode> = xot.children(callee).collect();
        for child in children {
            xot.with_detach(child)?
                .with_insert_before(callee, child)?;
        }
        xot.with_detach(callee)?;
    }
    Ok(())
}

/// Restructure every TypeScript `<import>` element into the unified
/// shape (per `imports-grouping.md`).
fn typescript_restructure_import(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name, get_text_content};
    use typescript::output::TractorNode::{
        Alias, Group, Namespace, Path, Sideeffect,
    };

    let mut targets: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "import", &mut targets);

    for import_node in targets {
        // Skip inner <import> children of an already-grouped import.
        if xot.parent(import_node)
            .and_then(|p| get_element_name(xot, p))
            .as_deref() == Some("import")
        {
            continue;
        }

        // 1. Identify structural children. Tree-sitter TS produces:
        //    - `<clause>` (import_clause: bindings)
        //    - `<string>` (path module specifier)
        //    plus noise text (`import`, `from`, `;`).
        let clause = xot.children(import_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("clause"));
        let path_string = xot.children(import_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("string"));

        // 2. Extract path text (strip surrounding quotes).
        let path_text = path_string
            .and_then(|s| get_text_content(xot, s))
            .map(|raw| raw.trim()
                .trim_start_matches('"').trim_end_matches('"')
                .trim_start_matches('\'').trim_end_matches('\'')
                .trim_start_matches('`').trim_end_matches('`')
                .to_string())
            .unwrap_or_default();

        // 3. Strip ALL direct text leaves and the path string element.
        for child in xot.children(import_node).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }
        if let Some(s) = path_string {
            xot.detach(s)?;
        }

        // 4. Build new structure.
        if clause.is_none() {
            // No `<clause>` — could be:
            //  - side-effect: `import './x'` (only string)
            //  - TS legacy: `import x = require('y')` (has `<name>` directly)
            // Side-effect = no name child either; legacy keeps its <name>.
            let has_direct_name = xot.children(import_node)
                .any(|c| get_element_name(xot, c).as_deref() == Some("name"));
            if !path_text.is_empty() {
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.append(import_node, path_node)?;
                let path_text_node = xot.new_text(&path_text);
                xot.append(path_node, path_text_node)?;
            }
            if !has_direct_name {
                xot.with_prepended_marker(import_node, Sideeffect)?;
            }
            continue;
        }
        let clause = clause.unwrap();

        // Append <path> (always, when clause is present).
        if !path_text.is_empty() {
            let path_elt = xot.add_name(Path.as_str());
            let path_node = xot.new_element(path_elt);
            xot.append(import_node, path_node)?;
            let path_text_node = xot.new_text(&path_text);
            xot.append(path_node, path_text_node)?;
        }

        // Inspect the clause's children to determine variant.
        let namespace_child = xot.children(clause)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("namespace"));
        let imports_child = xot.children(clause)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("imports"));

        if let Some(ns) = namespace_child {
            // `import * as ns from 'mod'`. Find the name inside <namespace>.
            let ns_name = xot.children(ns)
                .find(|&c| get_element_name(xot, c).as_deref() == Some("name"));
            if let Some(name) = ns_name {
                let alias_elt = xot.add_name("aliased");
                let alias_node = xot.new_element(alias_elt);
                xot.append(import_node, alias_node)?;
                xot.detach(name)?;
                xot.append(alias_node, name)?;
            }
            xot.detach(clause)?;
            xot.with_prepended_marker(import_node, Namespace)?;
            continue;
        }

        if let Some(imports) = imports_child {
            // Default name + group OR group only.
            // Default name: clause has a direct <name> child.
            let default_name = xot.children(clause)
                .find(|&c| get_element_name(xot, c).as_deref() == Some("name"));
            if let Some(d) = default_name {
                xot.detach(d)?;
                xot.append(import_node, d)?;
            }
            // Group: each <spec> child becomes inner <import>.
            for spec in xot.children(imports).filter(|&c|
                get_element_name(xot, c).as_deref() == Some("spec")
            ).collect::<Vec<_>>() {
                // Capture name-`as`-name pair if present.
                let names: Vec<XotNode> = xot.children(spec)
                    .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                    .collect();
                let has_inner_as = xot.children(spec).any(|c|
                    xot.text_str(c).map(|t| t.split_whitespace().any(|tok| tok == "as"))
                        .unwrap_or(false)
                );
                // Build inner <import>.
                let inner_elt = xot.add_name("import");
                let inner = xot.new_element(inner_elt);
                xot.append(import_node, inner)?;
                if has_inner_as && names.len() == 2 {
                    let original = names[0];
                    let alias_name = names[1];
                    xot.detach(original)?;
                    xot.append(inner, original)?;
                    let alias_elt = xot.add_name("aliased");
                    let alias_node = xot.new_element(alias_elt);
                    xot.append(inner, alias_node)?;
                    xot.detach(alias_name)?;
                    xot.append(alias_node, alias_name)?;
                    xot.with_prepended_marker(inner, Alias)?;
                } else if let Some(&name) = names.first() {
                    xot.detach(name)?;
                    xot.append(inner, name)?;
                }
                xot.detach(spec)?;
            }
            xot.detach(clause)?;
            xot.with_prepended_marker(import_node, Group)?;
            continue;
        }

        // Default-only: `import def from 'mod'`. clause/<name>def</name>.
        let default_name = xot.children(clause)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("name"));
        if let Some(d) = default_name {
            xot.detach(d)?;
            xot.append(import_node, d)?;
        }
        xot.detach(clause)?;
    }

    Ok(())
}

/// Python post-transform: wrap expression positions in `<expression>`
/// hosts (Principle #15). Python doesn't run `collapse_conditionals`
/// because tree-sitter-python emits an explicit `elif_clause`.
fn python_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Invert right-deep `<member>`/`<call>` chains into nested
    // `<chain>` form (per `docs/design-chain-inversion.md`).
    // Python's tree already matches the canonical input shape:
    // `<call><member><object/><property/></member>...args</call>`
    // and `<member><object/><property/></member>`. Run early so
    // subsequent passes see the post-inversion shape.
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("string", "interpolation"),
            // Python's `comparison_operator` doesn't tag operands
            // with field=left/right (unlike binary_operator), so
            // multi-name compare chains overflow without this.
            ("compare", "name"),
            // Same situation when both operands of a comparison
            // are member-access chains (`self.name == other.name`)
            // — both become <object[access]> siblings under
            // <compare> and collide on the singleton `object`
            // JSON key without this tag.
            ("compare", "object"),
            // Multi-value returns (`return a, b, c`) — after
            // `wrap_expression_positions` each becomes a sibling
            // <expression> direct child of <return>; tag them so
            // JSON renders as `expressions: [...]` instead of
            // colliding on the singleton `expression` key and
            // overflowing into `children`.
            ("return", "expression"),
        ],
    )?;
    python_restructure_imports(xot, root)?;
    // Run AFTER restructure_imports so the `<from>` element has its
    // final `<import>` siblings (the restructure pass rewires them).
    // `<from>`/`<import>` is tagged unconditionally — single-name
    // and multi-name imports both render as `imports: [...]` in
    // JSON. Per Principle #12, the `<import>` role is always a
    // list inside `<from>`; the cardinality discriminator used
    // elsewhere would split the JSON shape (`"import": {...}` vs
    // `"imports": [...]`) and force consumers to branch on count.
    python_tag_from_imports_uniform(xot, root)?;
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            // Python `with X as a, Y as b: ...` — `<with>` parent
            // with multiple `<value>` (as-clause) siblings.
            ("with", "value"),
            // Python `try: ... except A: ... except B: ...` — `<try>`
            // parent with multiple `<except>` siblings.
            ("try", "except"),
        ],
    )?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body"])?;
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "module", "tuple", "list", "dict", "switch", "literal", "macro", "template", "string", "repetition"],
    )?;
    Ok(())
}

/// Tag every `<import>` child of `<from>` with `list="imports"`,
/// regardless of cardinality. Mirrors `tag_multi_role_children`'s
/// (`from`, `import`) entry but without the `>= 2` gate. Per
/// Principle #12 the `<import>` role inside `<from>` is always a
/// list; the cardinality-gated tag would split the JSON shape
/// (`"import": {...}` for single, `"imports": [...]` for multi) and
/// force consumers to branch on count.
fn python_tag_from_imports_uniform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_attr, get_element_name};
    let mut froms: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "from", &mut froms);
    for from in froms {
        let kids: Vec<XotNode> = xot.children(from)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("import")
            })
            .collect();
        for k in kids {
            if get_attr(xot, k, "list").is_none() {
                xot.with_attr(k, "list", "imports");
            }
        }
    }
    Ok(())
}

/// Restructure Python `<import>` and `<from>` elements per the
/// imports-grouping shape: `<path>` for the module path, `<alias>` for
/// renamed bindings, inner `<import>` per imported entity inside `<from>`.
fn python_restructure_imports(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name};
    use python::output::TractorNode::{Alias, Path, Relative};

    // Handle `<import>` (plain `import X` and `import X as Y`).
    let mut imports: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "import", &mut imports);
    for imp in imports {
        // Skip if nested inside a <from> (we handle those separately
        // below — the outer pass already restructured them).
        if xot.parent(imp)
            .and_then(|p| get_element_name(xot, p))
            .as_deref() == Some("from")
        {
            continue;
        }
        // Capture name-`as`-name pair from text adjacency.
        let alias_pairs = python_alias_pairs(xot, imp);
        // Strip noise text (`import`, `as`, commas).
        for child in xot.children(imp).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }
        // Now look at children. For dotted name `import a.b.c`, there
        // may be a wrapper `<name>` containing inner `<name>X</name>`
        // segments — flatten that into a `<path>`. For aliased
        // `import a.b as x`, alias_pairs has the (last_segment, alias)
        // pair captured.
        python_flatten_dotted_name(xot, imp)?;
        let names: Vec<XotNode> = xot.children(imp)
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .collect();

        if !alias_pairs.is_empty() && names.len() >= 2 {
            // Treat the last name as alias; rest become `<path>`.
            let alias_name = *names.last().unwrap();
            let path_segs = &names[..names.len() - 1];
            // Single-segment path also wraps in <path> for cross-language
            // consistency.
            if !path_segs.is_empty() {
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.insert_before(path_segs[0], path_node)?;
                for &seg in path_segs {
                    xot.detach(seg)?;
                    xot.append(path_node, seg)?;
                }
            }
            let alias_elt = xot.add_name("aliased");
            let alias_node = xot.new_element(alias_elt);
            xot.insert_before(alias_name, alias_node)?;
            xot.detach(alias_name)?;
            xot.append(alias_node, alias_name)?;
            xot.with_prepended_marker(imp, Alias)?;
        } else if !names.is_empty() {
            // Plain dotted import: wrap all names in <path>.
            let path_elt = xot.add_name(Path.as_str());
            let path_node = xot.new_element(path_elt);
            xot.insert_before(names[0], path_node)?;
            for &seg in &names {
                xot.detach(seg)?;
                xot.append(path_node, seg)?;
            }
        }
    }

    // Handle `<from>`.
    let mut froms: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "from", &mut froms);
    for fnode in froms {
        // Look at text leaves to find the `import` keyword (separates
        // the module path from imported names) and any leading dots
        // (relative import marker).
        let mut import_kw_seen_at: Option<usize> = None;
        let mut has_relative = false;
        let mut has_relative_only = false;
        let children_seq: Vec<XotNode> = xot.children(fnode).collect();
        for (idx, child) in children_seq.iter().enumerate() {
            if let Some(text) = xot.text_str(*child) {
                let trimmed = text.trim();
                if trimmed.starts_with("from .") || trimmed == "from . import" {
                    has_relative = true;
                    if trimmed == "from . import" || trimmed == "from .. import" {
                        has_relative_only = true;
                    }
                }
                if trimmed.contains("import") {
                    import_kw_seen_at = Some(idx);
                }
            }
        }

        let alias_pairs = python_alias_pairs(xot, fnode);
        // Strip text noise.
        for child in xot.children(fnode).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }
        python_flatten_dotted_name(xot, fnode)?;
        let names: Vec<XotNode> = xot.children(fnode)
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .collect();

        // Determine the boundary: how many leading names belong to the
        // module path. import_kw_seen_at tells us roughly where, but
        // since we stripped indices, count via text layout. Heuristic:
        //  - If all names are 0: nothing to do (relative-only).
        //  - Else module_path = first N names where N = total - num_imports.
        // We don't have an easy way to count num_imports without the
        // text layout. Fallback: assume the FIRST name is the module
        // (most common case `from X import a, b, c`); the rest are
        // imports. Aliases are tracked from alias_pairs.

        if names.is_empty() {
            if has_relative_only {
                xot.with_prepended_marker(fnode, Relative)?;
            }
            continue;
        }

        // For relative-only `from . import x`: all names are imports,
        // no module path. For `from .x import y`: first name is the
        // (relative) module, rest are imports.
        let path_count = if has_relative_only { 0 } else { 1 };
        let path_segs: Vec<XotNode> = names.iter().take(path_count).copied().collect();
        let import_names: Vec<XotNode> = names.iter().skip(path_count).copied().collect();

        // Build <path> from path_segs.
        if !path_segs.is_empty() {
            let path_elt = xot.add_name(Path.as_str());
            let path_node = xot.new_element(path_elt);
            xot.insert_before(path_segs[0], path_node)?;
            for &seg in &path_segs {
                xot.detach(seg)?;
                xot.append(path_node, seg)?;
            }
        }

        // Identify alias pair targets within import_names.
        let alias_target_set: std::collections::HashSet<XotNode> =
            alias_pairs.iter().map(|&(_, b)| b).collect();
        let alias_orig_pair: std::collections::HashMap<XotNode, XotNode> =
            alias_pairs.iter().map(|&(a, b)| (a, b)).collect();

        // Wrap each import-name in inner <import>; pair aliases.
        let mut idx = 0;
        while idx < import_names.len() {
            let name = import_names[idx];
            if alias_target_set.contains(&name) {
                idx += 1;
                continue;
            }
            let inner_imp_elt = xot.add_name("import");
            let inner_imp = xot.new_element(inner_imp_elt);
            xot.insert_before(name, inner_imp)?;
            xot.detach(name)?;
            xot.append(inner_imp, name)?;
            if let Some(&alias_name) = alias_orig_pair.get(&name) {
                let alias_elt = xot.add_name("aliased");
                let alias_node = xot.new_element(alias_elt);
                xot.append(inner_imp, alias_node)?;
                xot.detach(alias_name)?;
                xot.append(alias_node, alias_name)?;
                xot.with_prepended_marker(inner_imp, Alias)?;
            }
            idx += 1;
        }

        if has_relative {
            xot.with_prepended_marker(fnode, Relative)?;
        }
        let _ = import_kw_seen_at;
    }

    Ok(())
}

/// Capture (a, b) pairs of `<name>` siblings joined by an `as` text
/// node — used by the Python import restructure to identify alias
/// pairs inside `import x as y` / `from x import y as z`.
fn python_alias_pairs(xot: &Xot, node: XotNode) -> Vec<(XotNode, XotNode)> {
    use crate::transform::helpers::get_element_name;
    let mut out = Vec::new();
    let seq: Vec<XotNode> = xot.children(node).collect();
    for window in seq.windows(3) {
        let (a, mid, b) = (window[0], window[1], window[2]);
        if get_element_name(xot, a).as_deref() == Some("name")
            && get_element_name(xot, b).as_deref() == Some("name")
        {
            if let Some(text) = xot.text_str(mid) {
                if text.split_whitespace().any(|t| t == "as") {
                    out.push((a, b));
                }
            }
        }
    }
    out
}

/// Tree-sitter Python's `dotted_name` (e.g. `a.b.c`) gets wrapped in
/// the field `<name>` wrapper, producing `<name><name>a</name>"."<name>b</name>...</name>`.
/// Flatten any such inner `<name>` wrapper child of `node` so its
/// segments become direct children, ready for `<path>` wrapping.
fn python_flatten_dotted_name(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let wrappers: Vec<XotNode> = xot.children(node)
        .filter(|&c| {
            get_element_name(xot, c).as_deref() == Some("name")
                && xot.children(c).any(|cc| {
                    get_element_name(xot, cc).as_deref() == Some("name")
                })
        })
        .collect();
    for wrapper in wrappers {
        let inner: Vec<XotNode> = xot.children(wrapper).collect();
        for c in inner {
            // Skip text "." separators inside the wrapper.
            if xot.text_str(c).is_some() {
                xot.detach(c)?;
                continue;
            }
            xot.detach(c)?;
            xot.insert_before(wrapper, c)?;
        }
        xot.detach(wrapper)?;
    }
    Ok(())
}

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
        xot, root, &["body", "block", "program", "tuple", "list", "dict", "array", "hash", "switch", "literal", "macro", "template", "string", "repetition"],
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
            // Go interfaces with multiple methods + type-set elements.
            ("interface", "method"),
            ("interface", "type"),
            // Go multi-value return: `return x, err` produces
            // `<return>` with multiple `<expression>` siblings
            // (after `wrap_expression_positions`). Tag so JSON
            // renders `expressions: [...]` instead of overflowing
            // to `children`. Mirrors Python iter 265.
            ("return", "expression"),
        ],
    )?;
    // Go's `if x { ... }` has `<then>` body; strip braces there too.
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "file", "tuple", "list", "dict", "array", "switch", "literal", "macro", "template", "string", "repetition"],
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
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "namespace", "program", "tuple", "list", "dict", "array", "switch", "literal", "macro", "template", "string", "repetition"],
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
        &[("parameter", "name")],
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
        xot, root, &["body", "program", "tuple", "list", "dict", "array", "hash", "switch", "literal", "macro", "template", "string", "repetition"],
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

/// Recursively collect every element with the given name into `out`,
/// in document order.
fn collect_named_elements(xot: &Xot, node: XotNode, name: &str, out: &mut Vec<XotNode>) {
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
