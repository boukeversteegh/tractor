//! C# post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply C#-specific structural
//! rewrites (`where T : …` constraint attachment, file-scoped
//! namespace unification, conditional-access pre-pass) and the
//! shared cross-language passes (chain inversion, conditional
//! collapse, expression-position wrap, list distribution).
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 330 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::collapse_conditionals;

/// C# combines two post-transforms: `attach_where_clause_constraints`
/// moves `where T : …` constraints into the matching `<generic>`, then
/// the shared conditional collapse runs.
pub fn csharp_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
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
            // Tuple deconstruction pattern `var (cnt, tg) = pair;`
            // produces `<pattern[tuple]>` with multiple `<name>`
            // siblings (one per binding slot). Mirrors Ruby/Python
            // iter 273.
            ("pattern", "name"),
            // Alternative patterns (`x is 1 or 2 or 3`) — same
            // archetype as Python/Ruby. Mirrors iter 273.
            ("pattern", "int"),
            ("pattern", "string"),
            // Multi-argument indexer `arr[1, 2, 3]` — `<index>`
            // parent with multiple `<argument>` siblings.
            ("index", "argument"),
            // C# tuple type `(int count, string tag)` — `<type[tuple]>`
            // parent with multiple `<element>` siblings (each is one
            // tuple position). Role-uniform per Principle #19.
            ("type", "element"),
            // Switch-expression `pat switch { a => b, c => d }` —
            // `<switch>` parent with multiple `<arm>` siblings AND a
            // singleton subject (`<name>` or `<value>`). Role-mixed:
            // tag arms only via this targeted entry rather than via
            // bulk distribute on `"switch"` (which would also wrap
            // the singleton subject in a 1-elem JSON array — iter-213
            // archetype, surfaced iter 303). Switch-statement arms
            // are inside `<body>` and still get list-tagged via the
            // bulk distribute on `"body"`.
            ("switch", "arm"),
            // C# `<literal>` parent role-uniform multi cases: array
            // literals `new int[] { 1, 2, 3 }` (multi `<int>`) and
            // collection initializers `new() { ["a"] = 1, ["b"] = 2 }`
            // (multi `<assign>`). Targeted role tags replace the
            // bulk-distribute entry on `"literal"` (removed iter 306
            // — that entry was wrapping with-expression singleton
            // string+name fields in 1-elem JSON arrays).
            ("literal", "assign"),
            ("literal", "int"),
            // C# `<namespace>` parent role-uniform multi member-type
            // cases: classes/records/enums/etc. can appear multiple
            // times under one namespace (multi enums/records/classes
            // are common in the blueprint; rest are rarer but
            // possible). Targeted role tags replace the
            // bulk-distribute entry on `"namespace"` (removed iter
            // 307 — that entry was wrapping the singleton namespace
            // name and any singleton member-type wrappers in 1-elem
            // JSON arrays). The ratchet (iter 298) catches future
            // C# member-types missed here.
            ("namespace", "class"),
            ("namespace", "struct"),
            ("namespace", "interface"),
            ("namespace", "enum"),
            ("namespace", "delegate"),
            ("namespace", "record"),
            ("namespace", "import"),
            ("namespace", "namespace"),
            // Comments between namespace members (e.g.
            // `// section header` between two classes).
            ("namespace", "comment"),
            // C# `<string>` parent: interpolated strings `$"hi {x}"`
            // have one or more `<interpolation>` chunks.
            ("string", "interpolation"),
            // IR-pipeline coverage — the imperative pipeline tagged
            // these via per-list `Flatten { distribute_list }` rules
            // before this post-pass runs. The IR pipeline carries
            // parameters / accessors / arguments etc. directly on
            // typed variants so they don't go through that step;
            // tag them here instead.
            ("method", "parameter"),
            ("constructor", "parameter"),
            ("delegate", "parameter"),
            ("indexer", "parameter"),
            ("operator", "parameter"),
            ("lambda", "parameter"),
            ("destructor", "parameter"),
            ("property", "name"),         // property has accessor names
            ("call", "name"),             // call args (atoms render as <name>)
            ("call", "int"),
            ("call", "string"),
            ("call", "argument"),
            ("new", "name"),
            ("new", "int"),
            ("new", "binary"),
            ("new", "assign"),
            ("new", "argument"),
            ("enum", "constant"),
            ("from", "name"),
            ("index", "int"),
            ("index", "name"),
            ("foreach", "name"),
            ("if", "name"),
            ("class", "method"),
            ("class", "field"),
            ("class", "property"),
            ("class", "constructor"),
            ("class", "comment"),
            ("class", "operator"),
            ("class", "event"),
            ("class", "name"),
            ("interface", "method"),
            ("interface", "property"),
            ("interface", "event"),
            ("struct", "method"),
            ("struct", "field"),
            ("struct", "property"),
            ("struct", "constructor"),
            ("struct", "operator"),
            ("record", "name"),
            ("body", "variable"),
            ("body", "expression"),
            ("body", "if"),
            ("body", "for"),
            ("body", "foreach"),
            ("body", "while"),
            ("body", "do"),
            ("body", "switch"),
            ("body", "try"),
            ("body", "return"),
            ("body", "comment"),
            ("body", "method"),           // local function
            ("body", "name"),
            ("body", "yield"),
            ("body", "lock"),
            ("body", "checked"),
            ("body", "label"),
            ("body", "goto"),
            ("body", "block"),
            ("body", "throw"),
            ("body", "using"),
            ("body", "fixed"),
            ("body", "unsafe"),
            ("unit", "comment"),
            ("unit", "import"),
            ("unit", "class"),
            ("unit", "namespace"),
            ("unit", "name"),
            // More IR-pipeline tag pairs surfaced by the contract scan.
            ("attribute", "name"),
            ("spread", "dict"),
            ("object", "call"),
            ("join", "name"),
            ("when", "list"),
            ("variable", "name"),
            ("statement", "when"),
            ("statement", "ref"),
            ("statement", "alias"),
            ("set", "int"),
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
    crate::transform::strip_body_braces(xot, root, &["body", "block", "call"])?;
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
        // `"import"` removed iter 305 — was creating 1-elem JSON arrays
        // on singleton `<name>`/`<path>` children of `<import>` (each
        // C# import has exactly 1 name OR 1 path, never multiple
        // direct children needing list-tagging). Path's inner names
        // are still tagged via `flatten_nested_paths`.
        // `"namespace"` removed iter 307 — same archetype: was
        // wrapping singleton `<name>` (namespace name) and singleton
        // member-type wrappers (`<delegate>`, `<interface>`, `<struct>`)
        // in 1-elem arrays. Targeted role tags below cover the
        // role-uniform multi-cardinality member types.
        &["body", "block", "unit", "tuple", "list", "dict", "array", "hash", "repetition"],
    )?;
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
