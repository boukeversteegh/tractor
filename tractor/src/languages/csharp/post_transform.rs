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

/// C# post-transforms run after IR rendering. The IR emits canonical
/// shapes directly (file-scoped namespace marker, `<object>`-rooted
/// access chains), so the imperative-pipeline pre-passes
/// (`attach_where_clause_constraints`, `unify_file_scoped_namespace`,
/// `csharp_normalize_conditional_access`) are gone — only
/// `attach_ir_where_clauses` and the shared cross-language passes
/// remain.
pub fn csharp_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    attach_ir_where_clauses(xot, root)?;
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
            // IR pipeline: multi-attribute on declarations.
            ("class", "attribute"),
            ("struct", "attribute"),
            ("interface", "attribute"),
            ("record", "attribute"),
            ("method", "attribute"),
            ("property", "attribute"),
            ("field", "attribute"),
            ("parameter", "attribute"),
            // Multi-argument attribute call (e.g. `[Obsolete("x", false)]`).
            ("attribute", "argument"),
            // Multi-base inheritance: `class Dog : Animal, IBarker`.
            ("class", "extends"),
            ("struct", "extends"),
            ("interface", "extends"),
            ("record", "extends"),
            // Method-level multi-generic (e.g. `void M<T,U>()`).
            ("method", "generic"),
            // Recursive patterns with multiple subpatterns.
            ("pattern", "subpattern"),
            // LINQ `group by … into name` produces `<group>` with two
            // `<name>` children (the value to group, the binding).
            ("group", "name"),
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


/// IR-aware where-clause attachment. The IR pipeline emits each
/// `type_parameter_constraints_clause` as a `<where>` element with a
/// `<name>` (target generic) and `<constraint>` children. Merge each
/// constraint onto the matching `<generic>` sibling (same vocabulary
/// as the imperative `attach_where_clause_constraints`):
/// `class`/`struct`/`notnull`/`unmanaged` → empty marker;
/// `new()` → `<new/>`; type bounds → `<extends><type>...</type></extends>`.
fn attach_ir_where_clauses(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::*;

    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        use crate::transform::helpers::*;
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("where")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }

    let mut clauses: Vec<XotNode> = Vec::new();
    collect(xot, root, &mut clauses);

    for clause in clauses {
        if xot.parent(clause).is_none() && !xot.is_document(clause) {
            continue;
        }
        // The clause must sit under a class-like declaration whose
        // generic siblings we can patch. If the parent is a query
        // expression's `<where>` filter (different shape — only an
        // `<expression>` child), skip.
        let target_name: Option<String> = xot.children(clause)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .and_then(|n| get_text_content(xot, n));
        let target_name = match target_name {
            Some(n) => n,
            None => continue,
        };

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
                    .as_deref() == Some(target_name.as_str())
            });
        let generic = match target_generic {
            Some(g) => g,
            None => continue,
        };

        let constraints: Vec<XotNode> = xot.children(clause)
            .filter(|&c| xot.element(c).is_some())
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("constraint"))
            .collect();

        for constraint in constraints {
            attach_ir_constraint_to_generic(xot, constraint, generic)?;
        }

        xot.detach(clause)?;
    }
    Ok(())
}

fn attach_ir_constraint_to_generic(
    xot: &mut Xot,
    constraint: XotNode,
    generic: XotNode,
) -> Result<(), xot::Error> {
    use crate::transform::helpers::*;

    // `new()` constructor constraint: contains a `<new>` child element.
    let has_new = xot.children(constraint)
        .any(|c| get_element_name(xot, c).as_deref() == Some("new"));
    if has_new {
        let n = xot.add_name("new");
        let m = xot.new_element(n);
        xot.append(generic, m)?;
        return Ok(());
    }

    // Type bound: a `<type>` child → wrap in `<extends>`.
    let type_child = xot.children(constraint)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("type"));
    if let Some(t) = type_child {
        let ex_name = xot.add_name("extends");
        let ex = xot.new_element(ex_name);
        xot.detach(t)?;
        xot.append(ex, t)?;
        xot.append(generic, ex)?;
        return Ok(());
    }

    // Bare keyword: `class` / `struct` / `notnull` / `unmanaged` —
    // text content of the constraint.
    if let Some(text) = get_text_content(xot, constraint) {
        let trimmed = text.trim();
        if matches!(trimmed, "class" | "struct" | "notnull" | "unmanaged") {
            let n = xot.add_name(trimmed);
            let m = xot.new_element(n);
            xot.append(generic, m)?;
        }
    }
    Ok(())
}
