//! PHP transform logic.
//!
//! Applies the shared design principles:
//!   - Renames tree-sitter kinds to short, developer-friendly names.
//!   - Lifts visibility / static / final / abstract keywords to
//!     empty markers while preserving the source keyword as a
//!     dangling text sibling.
//!   - Flattens grammar wrappers (Principle #12) — parameter_list,
//!     arguments, declaration_list, property_element, ...
//!
//! Still rough — focuses on the most-visible constructs so queries
//! work uniformly. Refine as blueprint snapshots surface specifics.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's PHP XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
pub mod semantic {
    // Structural — containers.

    // Top-level / declarations
    pub const PROGRAM: &str = "program";
    pub const NAMESPACE: &str = "namespace";
    pub const USE: &str = "use";
    pub const CLASS: &str = "class";
    pub const INTERFACE: &str = "interface";
    pub const TRAIT: &str = "trait";
    pub const ENUM: &str = "enum";
    pub const METHOD: &str = "method";
    pub const FUNCTION: &str = "function";
    pub const FIELD: &str = "field";
    pub const CONST: &str = "const";
    pub const CONSTANT: &str = "constant";

    // Members / parameters
    pub const PARAMETER: &str = "parameter";
    pub const ARGUMENT: &str = "argument";

    // Inheritance
    pub const EXTENDS: &str = "extends";
    pub const IMPLEMENTS: &str = "implements";
    pub const TYPES: &str = "types";

    // Statements / control flow
    pub const RETURN: &str = "return";
    pub const IF: &str = "if";
    pub const ELSE: &str = "else";
    pub const ELSE_IF: &str = "else_if";
    pub const FOR: &str = "for";
    pub const FOREACH: &str = "foreach";
    pub const WHILE: &str = "while";
    pub const DO: &str = "do";
    pub const SWITCH: &str = "switch";
    pub const CASE: &str = "case";
    pub const TRY: &str = "try";
    pub const CATCH: &str = "catch";
    pub const FINALLY: &str = "finally";
    pub const THROW: &str = "throw";
    pub const ECHO: &str = "echo";
    pub const CONTINUE: &str = "continue";
    pub const BREAK: &str = "break";
    pub const MATCH: &str = "match";
    pub const ARM: &str = "arm";
    pub const YIELD: &str = "yield";
    pub const REQUIRE: &str = "require";
    pub const PRINT: &str = "print";
    pub const EXIT: &str = "exit";
    pub const DECLARE: &str = "declare";
    pub const GOTO: &str = "goto";

    // Expressions
    pub const CALL: &str = "call";
    pub const MEMBER: &str = "member";
    pub const INDEX: &str = "index";
    pub const NEW: &str = "new";
    pub const CAST: &str = "cast";
    pub const ASSIGN: &str = "assign";
    pub const BINARY: &str = "binary";
    pub const UNARY: &str = "unary";
    pub const TERNARY: &str = "ternary";
    pub const ARRAY: &str = "array";
    pub const SPREAD: &str = "spread";

    // Types / atoms
    pub const TYPE: &str = "type";
    pub const STRING: &str = "string";
    pub const INT: &str = "int";
    pub const FLOAT: &str = "float";
    pub const BOOL: &str = "bool";
    pub const NULL: &str = "null";
    pub const VARIABLE: &str = "variable";

    // Misc structural
    pub const TAG: &str = "tag";
    pub const INTERPOLATION: &str = "interpolation";
    pub const ATTRIBUTE: &str = "attribute";

    // Identifiers
    pub const NAME: &str = "name";

    // Operator child (from prepend_op_element).
    pub const OP: &str = "op";

    // Markers — always empty when emitted.

    // Visibility / access modifiers (marker-only).
    pub const PUBLIC: &str = "public";
    pub const PRIVATE: &str = "private";
    pub const PROTECTED: &str = "protected";

    // Other modifiers (marker-only).
    pub const FINAL: &str = "final";
    pub const ABSTRACT: &str = "abstract";
    pub const READONLY: &str = "readonly";

    // Call / member flavor markers.
    pub const INSTANCE: &str = "instance";

    // Type-shape markers.
    pub const PRIMITIVE: &str = "primitive";
    pub const UNION: &str = "union";
    pub const OPTIONAL: &str = "optional";

    // Parameter-shape markers.
    pub const VARIADIC: &str = "variadic";

    // Anonymous / arrow function shape markers.
    pub const ANONYMOUS: &str = "anonymous";
    pub const ARROW: &str = "arrow";

    // php_tag marker.
    pub const OPEN: &str = "open";

    // Ambiguous names — emitted as BOTH structural container AND marker
    // in different contexts. Kept as constants for type-safety but NOT in
    // MARKER_ONLY:
    //   - STATIC: `static_modifier` keyword marker AND `scoped_call_expression`
    //     shape marker. Also doubles with static property-access shape.
    //   - CONSTANT: `enum_case` / `const_element` (structural) AND
    //     `class_constant_access_expression` member-shape marker.
    //   - DEFAULT: `default_statement` (structural `<default>` clause) AND
    //     `match_default_expression` arm-shape marker (`<arm><default/>`).
    //   - FUNCTION: function_definition (container) AND anonymous/arrow
    //     function shape markers.
    pub const STATIC: &str = "static";
    pub const DEFAULT: &str = "default";

    /// Names that, when emitted, are always empty elements (no text,
    /// no element children). Used by the markers-stay-empty invariant.
    pub const MARKER_ONLY: &[&str] = &[
        PUBLIC, PRIVATE, PROTECTED,
        FINAL, ABSTRACT, READONLY,
        INSTANCE,
        PRIMITIVE, UNION, OPTIONAL,
        VARIADIC,
        ANONYMOUS, ARROW,
        OPEN,
    ];

    /// Every semantic name this language's transform can emit.
    pub const ALL_NAMES: &[&str] = &[
        PROGRAM, NAMESPACE, USE, CLASS, INTERFACE, TRAIT, ENUM,
        METHOD, FUNCTION, FIELD, CONST, CONSTANT,
        PARAMETER, ARGUMENT,
        EXTENDS, IMPLEMENTS, TYPES,
        RETURN, IF, ELSE, ELSE_IF, FOR, FOREACH, WHILE, DO,
        SWITCH, CASE, TRY, CATCH, FINALLY, THROW, ECHO, CONTINUE, BREAK,
        MATCH, ARM, YIELD, REQUIRE, PRINT, EXIT, DECLARE, GOTO,
        CALL, MEMBER, INDEX, NEW, CAST, ASSIGN, BINARY, UNARY, TERNARY,
        ARRAY, SPREAD,
        TYPE, STRING, INT, FLOAT, BOOL, NULL, VARIABLE,
        TAG, INTERPOLATION, ATTRIBUTE,
        NAME, OP,
        PUBLIC, PRIVATE, PROTECTED,
        FINAL, ABSTRACT, READONLY,
        INSTANCE,
        PRIMITIVE, UNION, OPTIONAL,
        VARIADIC,
        ANONYMOUS, ARROW,
        OPEN,
        STATIC, DEFAULT,
    ];
}

/// Transform a PHP AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Purely-grouping wrappers — Principle #12. Drop the
        // container so children become direct siblings of the
        // enclosing class / method / …
        "declaration_list"
        | "compound_statement"
        | "property_element"
        | "match_block"
        | "match_condition_list"
        | "namespace_name"
        | "namespace_use_clause"
        | "namespace_use_group"
        | "string_content"
        | "escape_sequence"
        | "array_element_initializer"
        // `attribute_group` = `#[Attr1, Attr2]` wrapper; `attribute_list` =
        // the list of attribute_group for a declaration. Both are pure
        // grouping wrappers — flatten so individual attributes surface as
        // direct siblings.
        | "attribute_group"
        | "attribute_list"
        // `anonymous_function_use_clause` = `use ($x, $y)` on a closure —
        // grouping wrapper for captured variables; flatten so the captured
        // names become direct siblings with their field role intact.
        | "anonymous_function_use_clause"
        // `declare_directive` = the `strict_types=1` bit inside
        // `declare(strict_types=1);` — wrapper around the assignment.
        | "declare_directive"
        // `enum_declaration_list` = the `{ … }` body of `enum E { … }` —
        // grouping wrapper, flatten so `case` entries surface as siblings.
        | "enum_declaration_list"
        => Ok(TransformAction::Flatten),

        // Expression statement / parenthesized expression —
        // grammar wrappers, flatten so children become siblings of
        // the enclosing node (Principle #12). Flatten is safer than
        // Skip for parenthesized expressions (the walker's Skip
        // path trips xot's text consolidation on nested ternaries).
        "expression_statement" => Ok(TransformAction::Skip),
        "parenthesized_expression" => Ok(TransformAction::Flatten),

        // PHP interpolated string — `"hello $name"` or `"x {$obj->y}"`.
        // Tree-sitter nests the interpolated expressions (variable_name /
        // member_access_expression / …) directly inside the string; every
        // other language we support wraps these in an `<interpolation>`
        // element so the shape is uniform. Match that shape here:
        // wrap every element child of the string in `<interpolation>`
        // so `//string/interpolation/name` works cross-language.
        //
        // Complex interpolation (`{$expr}`) keeps `{` / `}` in the
        // surrounding string text — absorbing them into the
        // interpolation element would require scanning adjacent text
        // tokens and is deferred. The existing delimiters still yield
        // a correct round-trip via `text_preservation`.
        "encapsed_string" => {
            // Tree-sitter PHP nests interpolated expressions (variable_name /
            // member_access_expression / …) directly inside the string,
            // alongside `string_content` / `escape_sequence` text-fragment
            // wrappers. To match the uniform cross-language shape
            // (`<string>…<interpolation>EXPR</interpolation>…</string>`),
            // wrap each real expression in an `<interpolation>`. Skip the
            // text-fragment kinds; those are just literal string text and
            // get flattened in their own handler.
            let children: Vec<_> = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            for child in children {
                let ts_kind = get_kind(xot, child);
                // Skip text fragments and already-renamed interpolation wrappers.
                if matches!(
                    ts_kind.as_deref(),
                    Some("string_content") | Some("string_value") | Some("escape_sequence")
                        | Some("text_interpolation") | None,
                ) {
                    continue;
                }
                let interp_name = xot.add_name("interpolation");
                let interp = xot.new_element(interp_name);
                copy_source_location(xot, child, interp);
                xot.insert_before(child, interp)?;
                xot.detach(child)?;
                xot.append(interp, child)?;
            }
            rename(xot, node, STRING);
            Ok(TransformAction::Continue)
        }

        // Qualified names (`App\Hello\Greeter`) collapse to a single
        // text leaf inside their enclosing <name> — same design as
        // C# qualified_name. The outer <name> field wrapper handles
        // the collapse; here we just flatten the inner wrapper so
        // its segments become siblings of the enclosing <name>,
        // which then consolidates.
        "qualified_name" => Ok(TransformAction::Flatten),

        // Comments — normalise tree-sitter's distinction between
        // line and block into the shared `<comment>` name.
        "comment" => Ok(TransformAction::Continue),

        // Flat lists (Principle #12) — parameters and arguments
        // become direct siblings with field="parameters" / "arguments".
        "formal_parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "arguments" if has_kind(xot, node) => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Modifier wrappers. PHP's grammar gives us
        // `visibility_modifier`, `static_modifier`, `final_modifier`,
        // `abstract_modifier`, `readonly_modifier` — each a text
        // token like "public" / "static". Convert to empty markers
        // with the source keyword preserved as a dangling sibling.
        "visibility_modifier"
        | "static_modifier"
        | "final_modifier"
        | "abstract_modifier"
        | "readonly_modifier"
        | "class_modifier" => {
            if let Some(text) = get_text_content(xot, node) {
                let text = text.trim().to_string();
                if !text.is_empty() {
                    rename_to_marker(xot, node, &text)?;
                    insert_text_after(xot, node, &text)?;
                    return Ok(TransformAction::Done);
                }
            }
            Ok(TransformAction::Continue)
        }

        // Base class / implements — wrap the type reference in <type>
        // (Principle #14).
        "base_clause" => {
            rename(xot, node, EXTENDS);
            Ok(TransformAction::Continue)
        }
        "class_interface_clause" => {
            rename(xot, node, IMPLEMENTS);
            Ok(TransformAction::Continue)
        }

        // PHP emits `name` directly on identifiers — our field
        // wrappings already produce <name>foo</name>, so nothing to
        // rewrite here except collapsing wrappers that sit inside a
        // <name> field wrapper: `<name><name>foo</name></name>` (from
        // field+identifier double-wrapping) and `<name><variable>$foo</variable></name>`
        // (from field-on-variable_name — tree-sitter tags `$foo` as a
        // `variable_name` kind, but in any field slot it's still just
        // the bound name, so the outer <name> should be the text leaf).
        //
        // Multi-segment qualified names (`App\Blueprint`) are flattened
        // — each segment becomes a direct sibling of the enclosing
        // namespace / use / etc. (Principle #12). This matches C#'s
        // qualified_name handling.
        "name" => {
            let children: Vec<_> = xot.children(node).collect();
            let element_children: Vec<_> = children
                .iter()
                .copied()
                .filter(|&c| xot.element(c).is_some())
                .collect();
            if element_children.len() == 1 {
                let child = element_children[0];
                // Match on the original tree-sitter kind (stable across
                // the walk order) and on post-rename element names for
                // the `<name><name>…</name></name>` case.
                let ts_kind = get_kind(xot, child);
                let el_name = get_element_name(xot, child);
                // If the single child is a `namespace_name` / `qualified_name`,
                // that child will flatten into multiple segments + "\"
                // separators. Flattening the outer wrapper now hoists the
                // segments to the enclosing namespace/use so each becomes a
                // direct `<name>` sibling.
                if matches!(
                    ts_kind.as_deref(),
                    Some("namespace_name") | Some("qualified_name"),
                ) {
                    return Ok(TransformAction::Flatten);
                }
                let inlineable = matches!(
                    ts_kind.as_deref(),
                    Some("name") | Some("variable_name"),
                ) || matches!(
                    el_name.as_deref(),
                    Some("name") | Some("variable"),
                );
                if inlineable {
                    let text = descendant_text(xot, child);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        for c in children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&trimmed);
                        xot.append(node, text_node)?;
                        return Ok(TransformAction::Done);
                    }
                }
            } else if element_children.len() > 1 {
                // Multiple element children — this is a qualified name
                // that flattened into segments + separators. Flatten
                // the outer <name> wrapper so each segment becomes a
                // direct child of the enclosing node.
                return Ok(TransformAction::Flatten);
            }
            Ok(TransformAction::Continue)
        }

        // Binary / assignment / unary expressions — lift the operator.
        "binary_expression" | "assignment_expression" | "unary_op_expression" => {
            extract_operator(xot, node)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Class members default to public when no visibility modifier
        // is written (PHP spec). Inject `<public/>` so the invariant
        // "every class member has an access marker" holds exhaustively
        // (Principle #9).
        "method_declaration" | "property_declaration" => {
            if !has_visibility_marker(xot, node) {
                prepend_empty_element(xot, node, PUBLIC)?;
            }
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        _ => {
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }
    }
}

/// Returns true if `node` has a PHP visibility modifier child.
/// Walk order: when we enter method/property_declaration, the
/// visibility_modifier child may still be raw (pre-rename) or already
/// transformed to a marker element — check both.
fn has_visibility_marker(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if xot.element(child).is_none() { continue; }
        let ts_kind = get_kind(xot, child);
        if ts_kind.as_deref() == Some("visibility_modifier") {
            return true;
        }
        if let Some(name) = get_element_name(xot, child) {
            if matches!(name.as_str(), "public" | "private" | "protected") {
                return true;
            }
        }
    }
    false
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

fn has_kind(xot: &Xot, node: XotNode) -> bool {
    get_kind(xot, node).is_some()
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    });
    if let Some(op) = operator {
        prepend_op_element(xot, node, op)?;
    }
    Ok(())
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Second tuple element is an optional disambiguation marker —
/// lets entries like `union_type → <type><union/>` declare the
/// marker inline so shape queries work across collapsed variants.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        "program" => Some(("program", None)),
        "namespace_definition" => Some(("namespace", None)),
        "namespace_use_declaration" => Some(("use", None)),
        "class_declaration" => Some(("class", None)),
        "interface_declaration" => Some(("interface", None)),
        "trait_declaration" => Some(("trait", None)),
        "enum_declaration" => Some(("enum", None)),
        "method_declaration" => Some(("method", None)),
        "function_definition" => Some(("function", None)),
        "property_declaration" => Some(("field", None)),
        "const_declaration" => Some(("const", None)),
        "enum_case" => Some(("constant", None)),
        "formal_parameter" | "simple_parameter" => Some(("parameter", None)),
        "variadic_parameter" => Some(("parameter", Some("variadic"))),
        // property_element / formal_parameters flattened above
        "argument" => Some(("argument", None)),
        // arguments flattened above when has kind
        "return_statement" => Some(("return", None)),
        "if_statement" => Some(("if", None)),
        "else_clause" => Some(("else", None)),
        "else_if_clause" | "elseif_clause" => Some(("else_if", None)),
        "for_statement" => Some(("for", None)),
        "foreach_statement" => Some(("foreach", None)),
        "while_statement" => Some(("while", None)),
        "do_statement" => Some(("do", None)),
        "switch_statement" => Some(("switch", None)),
        "case_statement" => Some(("case", None)),
        "default_statement" => Some(("default", None)),
        "try_statement" => Some(("try", None)),
        "catch_clause" => Some(("catch", None)),
        "finally_clause" => Some(("finally", None)),
        "throw_expression" => Some(("throw", None)),
        "echo_statement" => Some(("echo", None)),
        "continue_statement" => Some(("continue", None)),
        "break_statement" => Some(("break", None)),
        "match_expression" => Some(("match", None)),
        "match_conditional_expression" => Some(("arm", None)),
        "match_default_expression" => Some(("arm", Some("default"))),
        "class_constant_access_expression" => Some(("member", Some("constant"))),
        "subscript_expression" => Some(("index", None)),
        "yield_expression" => Some(("yield", None)),
        "require_expression" | "require_once_expression" | "include_expression" | "include_once_expression" => Some(("require", None)),
        "type_cast_expression" => Some(("cast", None)),
        "print_intrinsic" => Some(("print", None)),
        "exit_intrinsic" | "exit_statement" => Some(("exit", None)),
        "use_declaration" => Some(("use", None)),
        "variadic_unpacking" => Some(("spread", None)),
        "const_element" => Some(("constant", None)),
        "type_list" => Some(("types", None)),
        // Call flavors — `foo()` is a bare function call, `$obj->m()`
        // is an instance method, `Class::m()` is a static method. All
        // three collapse to `<call>` with a shape marker so
        // `//call[static]` finds every scoped call regardless of the
        // textual operator.
        "function_call_expression" => Some(("call", None)),
        "member_call_expression" => Some(("call", Some("instance"))),
        "scoped_call_expression" => Some(("call", Some("static"))),
        // Access flavors — `$obj->prop` vs `Class::$prop` (static
        // property) vs `Class::CONST`. Marker preserves the scoped /
        // static / constant distinction.
        "member_access_expression" => Some(("member", Some("instance"))),
        "scoped_property_access_expression" => Some(("member", Some("static"))),
        "object_creation_expression" => Some(("new", None)),
        "cast_expression" => Some(("cast", None)),
        "assignment_expression" => Some(("assign", None)),
        "binary_expression" => Some(("binary", None)),
        "unary_op_expression" => Some(("unary", None)),
        "conditional_expression" => Some(("ternary", None)),
        "array_creation_expression" => Some(("array", None)),
        "string" | "encapsed_string" => Some(("string", None)),
        "integer" => Some(("int", None)),
        "float" => Some(("float", None)),
        "boolean" => Some(("bool", None)),
        "null" => Some(("null", None)),
        "variable_name" => Some(("variable", None)),
        // Type flavors — shape marker keeps them queryable after the
        // collapse to `<type>`.
        "primitive_type" => Some(("type", Some("primitive"))),
        "named_type" => Some(("type", None)),
        "union_type" => Some(("type", Some("union"))),
        "optional_type" => Some(("type", Some("optional"))),
        // Anonymous function / arrow function — collapse to <function>
        // with a shape marker so `//function[anonymous]` finds them.
        "anonymous_function_creation_expression" | "anonymous_function" => {
            Some(("function", Some("anonymous")))
        }
        "arrow_function" => Some(("function", Some("arrow"))),
        // declare_statement — `declare(strict_types=1);`. The
        // `declare_directive` wrapper flattens (handled in match arm).
        "declare_statement" => Some(("declare", None)),
        // `goto LABEL;` — rare, but rename for completeness.
        "goto_statement" => Some(("goto", None)),
        // PHP opening/closing tags.
        "php_tag" => Some(("tag", Some("open"))),
        "text_interpolation" => Some(("interpolation", None)),
        // `attribute` (PHP 8+ attributes) — `#[Foo(1)]`. The grouping
        // wrappers around it flatten; here just rename.
        "attribute" => Some(("attribute", None)),
        _ => None,
    }
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
        "bool" | "null" => SyntaxCategory::Keyword,

        // Keywords
        "namespace" | "use" | "class" | "interface" | "trait" | "enum" => SyntaxCategory::Keyword,
        "function" | "method" | "field" | "const" | "constant" => SyntaxCategory::Keyword,
        "parameter" | "parameters" | "argument" | "arguments" => SyntaxCategory::Keyword,
        "if" | "else" | "else_if" | "switch" | "case" | "default" => SyntaxCategory::Keyword,
        "for" | "foreach" | "while" | "do" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" => SyntaxCategory::Keyword,
        "extends" | "implements" => SyntaxCategory::Keyword,
        "public" | "private" | "protected" | "static" | "final" | "abstract"
        | "readonly" => SyntaxCategory::Keyword,

        // Functions/calls
        "call" | "new" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" | "cast" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
