//! Python transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a Python AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        "expression_statement" => Ok(TransformAction::Skip),
        "parenthesized_expression" => Ok(TransformAction::Flatten),
        "block" => Ok(TransformAction::Flatten),
        // Purely-grouping wrappers (Principle #12):
        //   as_pattern_target — the target of `with x as y` / `except E as y`.
        //   pattern_list — `a, b = ...` unpacking; drop wrapper so the
        //     underlying patterns are direct children of the assignment.
        //   expression_list — tuple-like returns/yields (`return x, y`).
        //     Drop the wrapper so expressions are direct children of the
        //     enclosing statement; matches Go's behavior.
        "as_pattern_target" | "pattern_list" | "expression_list" => Ok(TransformAction::Flatten),

        // `lambda_parameters` is a pure grouping wrapper around
        // the parameter list of a `lambda`; flatten so the
        // parameters become direct siblings of the `<lambda>`.
        "lambda_parameters" => Ok(TransformAction::Flatten),

        // `keyword_separator` / `positional_separator` are grammar
        // markers for `*` and `/` in function signatures. Rename to
        // the short marker-style names.
        "keyword_separator" => {
            rename(xot, node, "keyword");
            Ok(TransformAction::Continue)
        }
        "positional_separator" => {
            rename(xot, node, "positional");
            Ok(TransformAction::Continue)
        }

        // Import paths (`from a.b.c import d`). Flatten so the
        // dotted-path segments become siblings of the enclosing
        // `<import>` — matches how we handle scoped identifiers in
        // C#/Rust (Principle #12).
        "dotted_name" | "relative_import" | "import_prefix" => Ok(TransformAction::Flatten),

        // Pattern kinds in `match` arms — normalise to `<pattern>`.
        "case_pattern" => {
            rename(xot, node, "pattern");
            Ok(TransformAction::Continue)
        }

        // `if_clause` / `for_in_clause` inside a comprehension —
        // grammar wrappers, flatten so the comprehension reads as
        // body + for + if siblings rather than nested clauses.
        "if_clause" | "for_in_clause" | "async_if_clause" => {
            Ok(TransformAction::Flatten)
        }

        // `case_clause` (match pattern clause), `with_item` — grammar
        // wrappers. `case_clause` renames to `<arm>` for uniformity
        // with Rust/C#/Java match vocabulary; `with_item` flattens.
        "case_clause" => {
            rename(xot, node, "arm");
            Ok(TransformAction::Continue)
        }
        "with_item" | "with_clause" => Ok(TransformAction::Flatten),

        // list_splat / dictionary_splat now handled via the rename map
        // with marker — see map_element_name. The marker child
        // distinguishes sequence-style (`*`) from mapping-style (`**`)
        // unpacks so `//spread[dict]` finds every `**kwargs` regardless
        // of argument vs pattern vs literal context.

        "keyword_argument" => {
            rename(xot, node, "argument");
            Ok(TransformAction::Continue)
        }
        "keyword_pattern" => {
            rename(xot, node, "pattern");
            Ok(TransformAction::Continue)
        }
        "aliased_import" => {
            rename(xot, node, "import");
            Ok(TransformAction::Continue)
        }
        "type_conversion" => {
            rename(xot, node, "cast");
            Ok(TransformAction::Continue)
        }
        // union_type / union_pattern / splat_pattern are now handled
        // via map_element_name with marker children so the collapsed
        // element names remain queryable by shape.

        // Tree-sitter python emits `escape_sequence` inside strings
        // — flatten into the string body text.
        "escape_sequence" => Ok(TransformAction::Flatten),

        // Python string internals: `string_start` / `string_content` /
        // `string_end` are grammar tokens around a string body. They
        // carry no semantic beyond their text (the opening quote, the
        // literal text, the closing quote). Flatten them to bare text
        // siblings so a `<string>` reads as text + interpolations, not
        // as a soup of grammar wrappers.
        //
        // Preserves `<interpolation>` as a wrapper for f-string expressions
        // so `//string/interpolation/name='age'` continues to work.
        "string_start" | "string_content" | "string_end" => {
            Ok(TransformAction::Flatten)
        }

        // Flat lists (Principle #12)
        "parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "argument_list" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "type_parameter" => {
            // Python's `type_parameter` is the `[X]` portion of `List[X]` —
            // the list of type arguments, not a single parameter. Flatten
            // so each inner type is a sibling with field="arguments".
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Generic type references: apply the C# pattern.
        "generic_type" => {
            rewrite_generic_type(xot, node, &["identifier", "type_identifier"])?;
            Ok(TransformAction::Continue)
        }

        // Name wrappers created by the builder for field="name".
        // Inline the single identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        //
        // Also:
        //   - Flatten when the single element child is an `aliased_import`
        //     (or post-rename `<import>`). Walking top-down, the outer
        //     `<name>` wraps an aliased_import like `import x as y`, which
        //     is NOT a single name — it's a compound. Drop the wrapper so
        //     the `<import>` becomes a direct child of `<import_statement>`.
        //   - Flatten when the child is a renamed `from` (import_from_statement)
        //     — same reason: it's a compound, not a name.
        "name" => {
            let element_children: Vec<_> = xot
                .children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            if element_children.len() == 1 {
                let child = element_children[0];
                let ts_kind = get_kind(xot, child);
                let el_name = get_element_name(xot, child);
                if matches!(
                    ts_kind.as_deref(),
                    Some("aliased_import") | Some("import_from_statement"),
                ) || matches!(
                    el_name.as_deref(),
                    Some("import") | Some("from"),
                ) {
                    return Ok(TransformAction::Flatten);
                }
            }
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Type wrappers from Python's tree-sitter grammar contain a single
        // identifier. Inline the identifier text then wrap in <name>
        // for the unified namespace vocabulary (`<type><name>int</name></type>`).
        // If the content is a generic_type (rewritten below into its own
        // `<type>` element) drop the outer wrapper so we don't double-nest.
        "type" => {
            let single_child = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .next();
            if let Some(child) = single_child {
                if get_kind(xot, child).as_deref() == Some("generic_type") {
                    return Ok(TransformAction::Flatten);
                }
            }
            inline_single_identifier(xot, node)?;
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Binary/comparison operators
        "binary_operator" | "comparison_operator" | "boolean_operator"
        | "unary_operator" | "augmented_assignment" => {
            extract_operator(xot, node)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Function definitions — extract async modifier if present;
        // inject a visibility marker when the function lives directly
        // inside a `class_definition` body (Principle #9). Python's
        // convention: `__x` → private, `_x` → protected, bare → public.
        "function_definition" => {
            let texts = get_text_children(xot, node);
            if texts.iter().any(|t| t.contains("async")) {
                prepend_empty_element(xot, node, "async")?;
            }
            if is_inside_class_body(xot, node) {
                if let Some(vis) = python_visibility_from_def(xot, node) {
                    prepend_empty_element(xot, node, vis)?;
                }
            }
            rename(xot, node, "function");
            Ok(TransformAction::Continue)
        }

        // Collections unify with their produced type. The construction
        // form is an exhaustive marker (Principle #9):
        //   <list><literal/>...</list>        -- [1, 2, 3]
        //   <list><comprehension/>...</list>  -- [x for x in xs]
        // Same for dict/set. Generator expressions have no literal
        // form in Python (parens make a tuple), so <generator> is
        // left bare — only one variant, no marker needed.
        // `has_kind` guard — the walker also visits empty marker children
        // named "list" / "set" (e.g. the `<list/>` marker we prepend on
        // `spread`). Only tree-sitter list/set nodes carry a kind attr,
        // so gating on that prevents applying the <literal/> marker to
        // our own markers.
        "list" | "set" if get_kind(xot, node).is_some() => {
            prepend_empty_element(xot, node, "literal")?;
            Ok(TransformAction::Continue)
        }
        "dictionary" => {
            prepend_empty_element(xot, node, "literal")?;
            rename(xot, node, "dict");
            Ok(TransformAction::Continue)
        }
        "list_comprehension" => {
            prepend_empty_element(xot, node, "comprehension")?;
            rename(xot, node, "list");
            Ok(TransformAction::Continue)
        }
        "dictionary_comprehension" => {
            prepend_empty_element(xot, node, "comprehension")?;
            rename(xot, node, "dict");
            Ok(TransformAction::Continue)
        }
        "set_comprehension" => {
            prepend_empty_element(xot, node, "comprehension")?;
            rename(xot, node, "set");
            Ok(TransformAction::Continue)
        }

        // Ternary (conditional_expression) — surgically wrap
        // `alternative` in `<else>`. See transformations.md.
        "conditional_expression" => {
            wrap_field_child(xot, node, "alternative", "else")?;
            rename(xot, node, "ternary");
            Ok(TransformAction::Continue)
        }

        // Identifiers are always names (definitions or references).
        // Tree-sitter uses a separate `type` node for type annotations, so
        // bare identifiers never need a heuristic — they are never types.
        "identifier" => {
            rename(xot, node, "name");
            Ok(TransformAction::Continue)
        }

        _ => {
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }
    }
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Second tuple element is an optional disambiguation marker —
/// lets the map declare "rename to `<spread>` with `<dict/>` child"
/// in one entry.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        "module" => Some(("module", None)),
        "class_definition" => Some(("class", None)),
        "function_definition" => Some(("function", None)),
        "decorated_definition" => Some(("decorated", None)),
        "decorator" => Some(("decorator", None)),
        // parameters is flattened via Principle #12 above
        "default_parameter" | "typed_parameter" | "typed_default_parameter" => Some(("parameter", None)),
        "return_statement" => Some(("return", None)),
        "if_statement" => Some(("if", None)),
        "elif_clause" => Some(("else_if", None)),
        "else_clause" => Some(("else", None)),
        "for_statement" => Some(("for", None)),
        "while_statement" => Some(("while", None)),
        "try_statement" => Some(("try", None)),
        "except_clause" => Some(("except", None)),
        "finally_clause" => Some(("finally", None)),
        "with_statement" => Some(("with", None)),
        "raise_statement" => Some(("raise", None)),
        "pass_statement" => Some(("pass", None)),
        "import_statement" => Some(("import", None)),
        "import_from_statement" => Some(("from", None)),
        "assert_statement" => Some(("assert", None)),
        "delete_statement" => Some(("delete", None)),
        "global_statement" => Some(("global", None)),
        "nonlocal_statement" => Some(("nonlocal", None)),
        "break_statement" => Some(("break", None)),
        "continue_statement" => Some(("continue", None)),
        "match_statement" => Some(("match", None)),
        // Pattern kinds in `match` arms — normalise to `<pattern>`
        // with shape markers for querying by structure.
        "class_pattern" => Some(("pattern", Some("class"))),
        "list_pattern" => Some(("pattern", Some("list"))),
        "dict_pattern" => Some(("pattern", Some("dict"))),
        // Walrus operator — Python's `:=`. Collapses to <assign>;
        // the `<op>` child (`:=`) or the enclosing context marks it.
        "named_expression" => Some(("assign", None)),
        // f-string internals — `format_specifier` is the `:>10` bit.
        "format_specifier" => Some(("format", None)),
        // `lambda_parameters` is a wrapper; flatten handled in match arm above.
        // `keyword_separator` / `positional_separator` are grammar markers
        // for `*` and `/` separators in function signatures — empty markers.
        // Spread / unpack — `*` sequence-style vs `**` mapping-style.
        // The `<list/>` / `<dict/>` marker child survives through
        // argument, pattern, and literal contexts so `//spread[dict]`
        // picks up every `**kwargs`.
        "list_splat" | "list_splat_pattern" => Some(("spread", Some("list"))),
        "dictionary_splat" | "dictionary_splat_pattern" => Some(("spread", Some("dict"))),
        // Type / pattern flavors — shape markers keep queries precise.
        "union_type" => Some(("type", Some("union"))),
        "union_pattern" => Some(("pattern", Some("union"))),
        "splat_pattern" => Some(("pattern", Some("splat"))),
        "as_pattern" => Some(("as", None)),
        "for_in_clause" => Some(("for", None)),
        "call" => Some(("call", None)),
        "attribute" => Some(("member", None)),
        "subscript" => Some(("subscript", None)),
        "assignment" => Some(("assign", None)),
        // augmented_assignment collapses to <assign>; the <op> child (e.g., +=) distinguishes it.
        "augmented_assignment" => Some(("assign", None)),
        "binary_operator" => Some(("binary", None)),
        "unary_operator" => Some(("unary", None)),
        "comparison_operator" => Some(("compare", None)),
        "boolean_operator" => Some(("logical", None)),
        // conditional_expression handled above
        "lambda" => Some(("lambda", None)),
        "await" => Some(("await", None)),
        // Collection literals and comprehensions are handled specially
        // above (renamed to their produced type + <literal/> or
        // <comprehension/> marker).
        "generator_expression" => Some(("generator", None)),
        "string" => Some(("string", None)),
        "integer" => Some(("int", None)),
        "float" => Some(("float", None)),
        "true" => Some(("true", None)),
        "false" => Some(("false", None)),
        "none" => Some(("none", None)),
        _ => None,
    }
}

/// Apply `map_element_name` to a node: rename + prepend marker (if any).
fn apply_rename(xot: &mut Xot, node: XotNode, kind: &str) -> Result<(), xot::Error> {
    if let Some((new_name, marker)) = map_element_name(kind) {
        rename(xot, node, new_name);
        if let Some(m) = marker {
            prepend_empty_element(xot, node, m)?;
        }
    }
    Ok(())
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ':' | '{' | '}' | '[' | ']'))
    });
    if let Some(op) = operator {
        prepend_op_element(xot, node, op)?;
    }
    Ok(())
}

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
///
/// Matches on tree-sitter kind first (walk-order safe — the inner identifier
/// may already have been renamed to `<name>` by the time this fires on the
/// outer wrapper). Also accepts post-rename element names so double-wrapped
/// `<name><name>foo</name></name>` collapses cleanly.
///
/// For `dotted_name` / `relative_import` wrappers containing a single
/// identifier descendant, inline that descendant too — Python's tree-sitter
/// grammar always routes `name` field values through `dotted_name` even
/// for a plain `import os`.
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let ts_kind = get_kind(xot, child);
        let el_name = get_element_name(xot, child);
        let matches_kind = matches!(
            ts_kind.as_deref(),
            Some("identifier") | Some("type_identifier"),
        );
        // Post-rename names — safe because we only enter the "name" arm
        // when the current wrapper IS the field=name wrapper.
        let matches_el = matches!(
            el_name.as_deref(),
            Some("name"),
        );
        // dotted_name/relative_import: Python's grammar routes bare import
        // names through these wrappers. If the wrapper contains a single
        // identifier descendant, inline it.
        let matches_dotted = matches!(
            ts_kind.as_deref(),
            Some("dotted_name") | Some("relative_import"),
        ) && single_identifier_descendant(xot, child);
        if !(matches_kind || matches_el || matches_dotted) {
            continue;
        }
        let text = if matches_dotted {
            descendant_text(xot, child).trim().to_string()
        } else {
            match get_text_content(xot, child) {
                Some(t) => t,
                None => continue,
            }
        };
        if text.is_empty() {
            continue;
        }
        let all_children: Vec<_> = xot.children(node).collect();
        for c in all_children {
            xot.detach(c)?;
        }
        let text_node = xot.new_text(&text);
        xot.append(node, text_node)?;
        return Ok(());
    }
    Ok(())
}

/// Returns true if `node` is directly inside a `class_definition`'s body.
/// Walks up parents looking for a `class_definition` tree-sitter kind,
/// stopping at the first `function_definition` (nested defs don't inherit).
fn is_inside_class_body(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_kind(xot, parent) {
            match kind.as_str() {
                "class_definition" => return true,
                "function_definition" => return false,
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

/// Extract the Python visibility marker name from a function_definition's
/// identifier. Returns None if the identifier can't be found.
///   `__x` / `__x__` → public (dunder methods like `__init__` are special)
///   `__x`          → private
///   `_x`           → protected
///   `x`            → public
fn python_visibility_from_def(xot: &Xot, node: XotNode) -> Option<&'static str> {
    // The identifier child (field="name" on the def) carries the name.
    for child in xot.children(node) {
        if xot.element(child).is_none() { continue; }
        let kind = get_kind(xot, child);
        if kind.as_deref() != Some("identifier") { continue; }
        let text = get_text_content(xot, child)?;
        let name = text.trim();
        // Dunder methods (`__init__`, `__str__`, etc.) are part of the
        // public protocol — they're conventional interface hooks.
        if name.starts_with("__") && name.ends_with("__") && name.len() > 4 {
            return Some("public");
        }
        if name.starts_with("__") {
            return Some("private");
        }
        if name.starts_with('_') {
            return Some("protected");
        }
        return Some("public");
    }
    None
}

/// Returns true if `node` has exactly one element descendant and it's an
/// identifier — the "wrapper contains a single name" case.
fn single_identifier_descendant(xot: &Xot, node: XotNode) -> bool {
    let mut count = 0usize;
    let mut is_ident = false;
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        for c in xot.children(n) {
            if xot.element(c).is_some() {
                count += 1;
                if count > 1 {
                    return false;
                }
                let kind = get_kind(xot, c);
                is_ident = matches!(
                    kind.as_deref(),
                    Some("identifier") | Some("type_identifier"),
                );
                stack.push(c);
            }
        }
    }
    count == 1 && is_ident
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,

        // Literals
        "string" => SyntaxCategory::String,
        "int" | "float" => SyntaxCategory::Number,
        "true" | "false" | "none" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "function" | "module" => SyntaxCategory::Keyword,
        "parameter" | "parameters" => SyntaxCategory::Keyword,
        "import" | "from" => SyntaxCategory::Keyword,
        "decorated" | "decorator" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else_if" | "else" => SyntaxCategory::Keyword,
        "for" | "while" => SyntaxCategory::Keyword,
        "try" | "except" | "finally" | "raise" => SyntaxCategory::Keyword,
        "with" | "pass" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" | "yield" => SyntaxCategory::Keyword,

        // Keywords - async
        "async" | "await" => SyntaxCategory::Keyword,

        // Functions/calls
        "call" => SyntaxCategory::Function,
        "lambda" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "compare" | "logical" => SyntaxCategory::Operator,
        "assign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Collection construction markers
        "literal" | "comprehension" | "generator" => SyntaxCategory::Keyword,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
