//! TypeScript/JavaScript transform logic
//!
//! This module owns ALL TypeScript-specific transformation rules.
//! No assumptions about other languages - this is self-contained.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's TypeScript/JavaScript XML
/// vocabulary after transform.
pub mod semantic {
    // Structural — containers.

    // Top-level / declarations
    pub const PROGRAM: &str = "program";
    pub const CLASS: &str = "class";
    pub const INTERFACE: &str = "interface";
    pub const ENUM: &str = "enum";
    pub const FUNCTION: &str = "function";
    pub const METHOD: &str = "method";
    pub const PROPERTY: &str = "property";
    pub const CONSTRUCTOR: &str = "constructor";
    pub const INDEXER: &str = "indexer";
    pub const ALIAS: &str = "alias";
    pub const VARIABLE: &str = "variable";
    pub const ARROW: &str = "arrow";

    // Members
    pub const FIELD: &str = "field";
    pub const PARAMETER: &str = "parameter";
    pub const EXTENDS: &str = "extends";
    pub const IMPLEMENTS: &str = "implements";

    // Type vocabulary
    pub const TYPE: &str = "type";
    pub const GENERIC: &str = "generic";
    pub const GENERICS: &str = "generics";
    pub const PREDICATE: &str = "predicate";
    pub const ANNOTATION: &str = "annotation";

    // Control flow
    pub const BLOCK: &str = "block";
    pub const RETURN: &str = "return";
    pub const IF: &str = "if";
    pub const ELSE: &str = "else";
    pub const FOR: &str = "for";
    pub const WHILE: &str = "while";
    pub const TRY: &str = "try";
    pub const CATCH: &str = "catch";
    pub const THROW: &str = "throw";
    pub const FINALLY: &str = "finally";
    pub const SWITCH: &str = "switch";
    pub const CASE: &str = "case";
    pub const BREAK: &str = "break";
    pub const CONTINUE: &str = "continue";
    pub const BODY: &str = "body";

    // Expressions
    pub const CALL: &str = "call";
    pub const NEW: &str = "new";
    pub const MEMBER: &str = "member";
    pub const ASSIGN: &str = "assign";
    pub const BINARY: &str = "binary";
    pub const UNARY: &str = "unary";
    pub const TERNARY: &str = "ternary";
    pub const AWAIT: &str = "await";
    pub const YIELD: &str = "yield";
    pub const AS: &str = "as";
    pub const SATISFIES: &str = "satisfies";
    pub const INDEX: &str = "index";
    pub const PATTERN: &str = "pattern";
    pub const SPREAD: &str = "spread";
    pub const REST: &str = "rest";

    // Imports / exports
    pub const IMPORT: &str = "import";
    pub const EXPORT: &str = "export";
    pub const IMPORTS: &str = "imports";
    pub const SPEC: &str = "spec";
    pub const CLAUSE: &str = "clause";
    pub const NAMESPACE: &str = "namespace";

    // Templates
    pub const TEMPLATE: &str = "template";
    pub const INTERPOLATION: &str = "interpolation";

    // JSX
    pub const ELEMENT: &str = "element";
    pub const OPENING: &str = "opening";
    pub const CLOSING: &str = "closing";
    pub const PROP: &str = "prop";
    pub const VALUE: &str = "value";
    pub const TEXT: &str = "text";

    // Enum members (enum_assignment → <constant>)
    pub const CONSTANT: &str = "constant";

    // Literals
    pub const STRING: &str = "string";
    pub const NUMBER: &str = "number";
    pub const BOOL: &str = "bool";
    pub const NULL: &str = "null";

    // Identifiers / comments
    pub const NAME: &str = "name";
    pub const COMMENT: &str = "comment";

    // Switch default (handled in map — NOT a marker here)
    pub const DEFAULT: &str = "default";

    // Operator child
    pub const OP: &str = "op";

    // Markers — always-empty when emitted.

    // Accessibility / modifier markers
    pub const PUBLIC: &str = "public";
    pub const PRIVATE: &str = "private";
    pub const PROTECTED: &str = "protected";
    pub const OVERRIDE: &str = "override";
    pub const READONLY: &str = "readonly";
    pub const ABSTRACT: &str = "abstract";
    pub const OPTIONAL: &str = "optional";
    pub const REQUIRED: &str = "required";

    // Function markers
    pub const ASYNC: &str = "async";
    pub const GENERATOR: &str = "generator";
    pub const GET: &str = "get";
    pub const SET: &str = "set";

    // Variable-keyword markers
    pub const LET: &str = "let";
    pub const CONST: &str = "const";
    pub const VAR: &str = "var";

    // Type-shape markers (all marker-only in TS — there is no
    // standalone `<union>`/`<intersection>`/etc. element emitted)
    pub const UNION: &str = "union";
    pub const INTERSECTION: &str = "intersection";
    pub const ARRAY: &str = "array";
    pub const LITERAL: &str = "literal";
    pub const TUPLE: &str = "tuple";
    pub const PARENTHESIZED: &str = "parenthesized";
    pub const OBJECT: &str = "object";
    pub const CONDITIONAL: &str = "conditional";
    pub const INFER: &str = "infer";
    pub const LOOKUP: &str = "lookup";
    pub const KEYOF: &str = "keyof";

    // Names emitted as BOTH marker AND structural container — kept as
    // constants for type-safety but NOT in MARKER_ONLY.
    //   - EXPORT: marker via extract_keyword_modifiers + structural from
    //     export_statement.
    //   - DEFAULT: marker via extract_keyword_modifiers + structural from
    //     switch_default.
    //   - FUNCTION: structural (function_declaration) + marker on type
    //     (function_type).
    //   - TEMPLATE: structural (template_string) + marker on type
    //     (template_type / template_literal_type).

    /// Names that, when emitted, are always empty elements.
    pub const MARKER_ONLY: &[&str] = &[
        PUBLIC, PRIVATE, PROTECTED, OVERRIDE, READONLY,
        ABSTRACT, OPTIONAL, REQUIRED,
        ASYNC, GENERATOR, GET, SET,
        LET, CONST, VAR,
        UNION, INTERSECTION, ARRAY, LITERAL, TUPLE, PARENTHESIZED,
        OBJECT, CONDITIONAL, INFER, LOOKUP, KEYOF,
    ];

    /// Every semantic name this language's transform can emit.
    pub const ALL_NAMES: &[&str] = &[
        PROGRAM, CLASS, INTERFACE, ENUM, FUNCTION, METHOD,
        PROPERTY, CONSTRUCTOR, INDEXER, ALIAS, VARIABLE, ARROW,
        FIELD, PARAMETER, EXTENDS, IMPLEMENTS,
        TYPE, GENERIC, GENERICS, PREDICATE, ANNOTATION,
        BLOCK, RETURN, IF, ELSE, FOR, WHILE, TRY, CATCH, THROW, FINALLY,
        SWITCH, CASE, BREAK, CONTINUE, BODY,
        CALL, NEW, MEMBER, ASSIGN, BINARY, UNARY, TERNARY, AWAIT, YIELD,
        AS, SATISFIES, INDEX, PATTERN, SPREAD, REST,
        IMPORT, EXPORT, IMPORTS, SPEC, CLAUSE, NAMESPACE,
        TEMPLATE, INTERPOLATION,
        ELEMENT, OPENING, CLOSING, PROP, VALUE, TEXT,
        CONSTANT,
        STRING, NUMBER, BOOL, NULL,
        NAME, COMMENT, DEFAULT, OP,
        PUBLIC, PRIVATE, PROTECTED, OVERRIDE, READONLY,
        ABSTRACT, OPTIONAL, REQUIRED,
        ASYNC, GENERATOR, GET, SET,
        LET, CONST, VAR,
        UNION, INTERSECTION, ARRAY, LITERAL, TUPLE, PARENTHESIZED,
        OBJECT, CONDITIONAL, INFER, LOOKUP, KEYOF,
    ];
}

/// Transform a TypeScript AST node
///
/// This is the main entry point - receives each node during tree walk
/// and decides what transformations to apply.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // ---------------------------------------------------------------------
        // Skip nodes - remove entirely, promote children
        // ---------------------------------------------------------------------
        "expression_statement" => Ok(TransformAction::Skip),

        // TypeScript's `accessibility_modifier` wraps a text token
        // like "public" / "private" / "protected" in a constructor
        // parameter (also `readonly_modifier` / `override_modifier`
        // follow the same pattern). Lift the keyword to an empty
        // marker, preserve the source text as a dangling sibling.
        "accessibility_modifier" | "override_modifier" | "readonly_modifier" => {
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
        // parenthesized_expression — grammar wrapper, flatten so
        // children become direct siblings of the enclosing node
        // (Principle #12).
        "parenthesized_expression" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Flatten nodes - transform children, then remove wrapper
        // ---------------------------------------------------------------------
        "variable_declarator" => Ok(TransformAction::Flatten),
        "class_body" | "interface_body" | "enum_body" => Ok(TransformAction::Flatten),
        // class_heritage is a purely-grouping wrapper around `extends_clause`
        // and `implements_clause`. Drop it so those clauses become direct
        // children of the class, under their renamed forms.
        "class_heritage" => Ok(TransformAction::Flatten),

        // Extends clause: `class Foo extends Bar`. Tree-sitter tags the
        // base-class identifier as `field="value"` (reflecting JS's
        // class-as-value model), so the builder wraps it in `<value>`.
        // Retag as `<type>` for the uniform namespace vocabulary —
        // `<extends><type><name>Bar</name></type></extends>`.
        "extends_clause" => {
            retag_value_as_type(xot, node)?;
            rename(xot, node, EXTENDS);
            Ok(TransformAction::Continue)
        }

        // Type alias declarations: `type Foo = …`. The builder wraps the
        // aliased type in `<value>` (because tree-sitter tags it with
        // `field="value"`). Drop that wrapper so the aliased type lives
        // directly inside `<alias>` — the walker then gives it its own
        // `<type>` wrapper via the normal rename path (predefined_type →
        // <type>, function_type → <type><function/>, etc.).
        "type_alias_declaration" => {
            let value_child = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .find(|&c| get_element_name(xot, c).as_deref() == Some("value"));
            if let Some(v) = value_child {
                flatten_node(xot, v)?;
            }
            rename(xot, node, ALIAS);
            Ok(TransformAction::Continue)
        }

        // function_type now handled uniformly via map_element_name
        // — the rename map declares the marker (Some("function")).
        // Template string parts: inline the raw text into the enclosing
        // `<template>` so a template literal reads as text with interpolation
        // children, not as a soup of grammar-internal wrappers.
        "string_fragment" | "string_start" | "string_end" => {
            Ok(TransformAction::Flatten)
        }

        // `export_clause` is `{ a, b }` inside `export { a, b }` —
        // a pure grouping wrapper, flatten so the specs become direct
        // children of the enclosing `<export>`.
        // `mapped_type_clause` is the key-type portion of a mapped type
        // `{ [K in keyof T]: … }` — flatten so the bindings bubble up.
        "export_clause" | "mapped_type_clause" => {
            Ok(TransformAction::Flatten)
        }

        // private_property_identifier is handled inside `inline_single_identifier`
        // when the enclosing <name> wrapper is processed: the leading `#` is
        // stripped and a <private/> marker is lifted onto the enclosing
        // field/property node.

        // ---------------------------------------------------------------------
        // Name wrappers created by the builder for field="name".
        // Inline the single identifier/property_identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        //
        // Destructuring patterns (`const [a, b] = ...`, `const {x, y} = ...`)
        // appear in the grammar as `name: array_pattern | object_pattern`.
        // A pattern is not a single name — flatten the wrapper so the
        // pattern becomes a direct child of the declarator.
        // ---------------------------------------------------------------------
        "name" => {
            let element_children: Vec<_> = xot
                .children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            if element_children.len() == 1 {
                let child = element_children[0];
                let ts_kind = get_kind(xot, child);
                if matches!(
                    ts_kind.as_deref(),
                    Some("array_pattern") | Some("object_pattern"),
                ) {
                    return Ok(TransformAction::Flatten);
                }
            }
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Call/member expressions — field wrapping (function→callee,
        // object→object, property→property) is handled by apply_field_wrappings
        // per TS_FIELD_WRAPPINGS, so we just rename the outer node here.
        // ---------------------------------------------------------------------
        "call_expression" | "member_expression" => {
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Ternary expression — surgically wrap its `alternative` child in
        // `<else>`. Cannot be a global FIELD_WRAPPINGS rule because
        // if_statement's `alternative` is already an `else_clause` that
        // renames to `<else>` (a global wrap would double-nest there).
        "ternary_expression" => {
            wrap_field_child(xot, node, "alternative", ELSE)?;
            rename(xot, node, TERNARY);
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Binary/unary expressions - extract operator
        // ---------------------------------------------------------------------
        "binary_expression" | "unary_expression" | "assignment_expression"
        | "augmented_assignment_expression" | "update_expression" => {
            extract_operator(xot, node)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Flat lists (Principle #12): drop the list wrapper; children become
        // direct siblings of the enclosing element and carry field="<plural>"
        // so non-XML serializers can group them back into an array.
        //
        // tree-sitter-javascript emits bare `identifier` for untyped params
        // (tree-sitter-typescript wraps them in required_parameter). Promote
        // those to `<param>` here so the semantic tree is consistent across
        // JS and TS — every parameter is a <param>.
        // ---------------------------------------------------------------------
        "formal_parameters" => {
            wrap_bare_identifier_params(xot, node)?;
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "arguments" if has_kind(xot, node) => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "type_arguments" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Type annotations (`:` prefix) are just a colon-prefixed form of
        // the underlying type. Drop the wrapper; the colon stays as a text
        // sibling for renderability and the actual `<type>` appears as a
        // child directly.
        "type_annotation" => Ok(TransformAction::Flatten),

        // Generic type references: apply the C# pattern.
        //   generic_type(name=Foo, type_arguments=[Bar, Baz])
        //     -> <type><generic/>Foo <type field="arguments">Bar</type>
        //                             <type field="arguments">Baz</type></type>
        "generic_type" => {
            rewrite_generic_type(xot, node, &["type_identifier", "identifier"])?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Variable declarations - extract let/const/var modifier
        // ---------------------------------------------------------------------
        "lexical_declaration" | "variable_declaration" => {
            extract_keyword_modifiers(xot, node)?;
            rename(xot, node, VARIABLE);
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Functions/methods — lift `async` keyword and generator `*` prefix
        // to empty marker children on the complex node (Principle #13).
        // Mirrors Python's function_definition handling.
        // ---------------------------------------------------------------------
        "method_definition" | "function_declaration" | "function_expression"
        | "arrow_function" | "generator_function_declaration"
        | "generator_function"
        | "abstract_method_signature" => {
            extract_function_markers(xot, node)?;
            // Class methods default to public (no keyword in source).
            // Inject `<public/>` so the invariant "every class member has
            // an access marker" holds exhaustively (Principle #9).
            if matches!(kind.as_str(), "method_definition" | "abstract_method_signature")
                && !has_visibility_marker(xot, node)
            {
                prepend_empty_element(xot, node, PUBLIC)?;
            }
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Class field (`foo = 1;` / `readonly x: T`) — same default
        // visibility rule as methods.
        "field_definition" | "public_field_definition" => {
            if !has_visibility_marker(xot, node) {
                prepend_empty_element(xot, node, PUBLIC)?;
            }
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Identifiers are always names (definitions or references).
        // Tree-sitter uses `type_identifier` for type positions, so bare
        // identifiers never need a heuristic — they are never types.
        // ---------------------------------------------------------------------
        "identifier" | "property_identifier" => {
            rename(xot, node, NAME);
            Ok(TransformAction::Continue)
        }
        "type_identifier" => {
            rename(xot, node, TYPE);
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Optional parameters - add <optional/> marker to distinguish from required
        // ---------------------------------------------------------------------
        "optional_parameter" => {
            prepend_empty_element(xot, node, OPTIONAL)?;
            rename(xot, node, PARAMETER);
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Required parameters - add <required/> marker (exhaustive with optional)
        // ---------------------------------------------------------------------
        "required_parameter" => {
            prepend_empty_element(xot, node, REQUIRED)?;
            rename(xot, node, PARAMETER);
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Other nodes - just rename if needed
        // ---------------------------------------------------------------------
        _ => {
            if let Some((new_name, marker)) = map_element_name(&kind) {
                rename(xot, node, new_name);
                if let Some(m) = marker {
                    prepend_empty_element(xot, node, m)?;
                }
                if new_name == TYPE && marker.is_none() {
                    // Namespace vocabulary (Principle #14): every named
                    // type reference carries its name in a <name> child.
                    // Shape-marked type variants (union/tuple/…) contain
                    // structure, not a bare name, so we skip wrapping there.
                    wrap_text_in_name(xot, node)?;
                }
            }
            Ok(TransformAction::Continue)
        }
    }
}

/// Map tree-sitter node kinds to semantic element names.
///
/// The second tuple element is an optional disambiguation marker:
/// when two distinct tree-sitter kinds collapse to the same semantic
/// element (e.g. all `*_type` → `<type>`), the marker child preserves
/// the original shape so queries like `//type[union]` remain
/// expressible without resorting to text matching.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        // Declarations
        "program" => Some((PROGRAM, None)),
        "class_declaration" => Some((CLASS, None)),
        "function_declaration" | "function_expression" => Some((FUNCTION, None)),
        "generator_function_declaration" | "generator_function" => Some((FUNCTION, None)),
        "method_definition" => Some((METHOD, None)),
        // Interface members — the shapes that appear inside
        // `interface X { … }`. Rename to the user-facing vocabulary
        // so the invariant tree is shared with class members.
        "method_signature" => Some((METHOD, None)),
        "property_signature" => Some((PROPERTY, None)),
        "construct_signature" => Some((CONSTRUCTOR, None)),
        "index_signature" => Some((INDEXER, None)),
        // Type-position constructs all collapse to `<type>` for a uniform
        // namespace vocabulary. The shape marker child (//type[union],
        // //type[function], //type[array], …) keeps them queryable.
        "union_type" => Some((TYPE, Some(UNION))),
        "intersection_type" => Some((TYPE, Some(INTERSECTION))),
        "array_type" => Some((TYPE, Some(ARRAY))),
        "literal_type" => Some((TYPE, Some(LITERAL))),
        "tuple_type" => Some((TYPE, Some(TUPLE))),
        "readonly_type" => Some((TYPE, Some(READONLY))),
        "parenthesized_type" => Some((TYPE, Some(PARENTHESIZED))),
        "function_type" => Some((TYPE, Some(FUNCTION))),
        "object_type" => Some((TYPE, Some(OBJECT))),
        "template_type" => Some((TYPE, Some(TEMPLATE))),
        "template_literal_type" => Some((TYPE, Some(TEMPLATE))),
        "default_type" => Some((TYPE, Some(DEFAULT))),
        "subscript_expression" => Some((INDEX, None)),
        "shorthand_property_identifier" => Some((NAME, None)),
        "shorthand_property_identifier_pattern" => Some((NAME, None)),
        // JSX — full design still deferred, but rename the obvious
        // tree-sitter kinds so the invariants stop tripping. A
        // proper shape design lives in the open-questions doc.
        "jsx_element" | "jsx_self_closing_element" => Some((ELEMENT, None)),
        "jsx_opening_element" => Some((OPENING, None)),
        "jsx_closing_element" => Some((CLOSING, None)),
        "jsx_attribute" => Some((PROP, None)),
        "jsx_expression" => Some((VALUE, None)),
        "jsx_text" => Some((TEXT, None)),
        // Patterns in destructuring — shape markers distinguish array vs object.
        "rest_pattern" => Some((REST, None)),
        "array_pattern" => Some((PATTERN, Some(ARRAY))),
        "object_pattern" => Some((PATTERN, Some(OBJECT))),
        // Import wrappers.
        "import_specifier" => Some((SPEC, None)),
        "import_clause" => Some((CLAUSE, None)),
        "spread_element" => Some((SPREAD, None)),
        "non_null_expression" => Some((UNARY, None)),
        "for_in_statement" => Some((FOR, None)),
        "enum_assignment" => Some((CONSTANT, None)),
        "update_expression" => Some((UNARY, None)),
        "named_imports" => Some((IMPORTS, None)),
        "switch_case" => Some((CASE, None)),
        "switch_default" => Some((DEFAULT, None)),
        "break_statement" => Some((BREAK, None)),
        "continue_statement" => Some((CONTINUE, None)),
        "switch_statement" => Some((SWITCH, None)),
        "switch_body" => Some((BODY, None)),
        "type_predicate" | "type_predicate_annotation" => Some((PREDICATE, None)),
        "arrow_function" => Some((ARROW, None)),
        "interface_declaration" => Some((INTERFACE, None)),
        // type_alias_declaration handled above (flattens <value> wrapper)
        "enum_declaration" => Some((ENUM, None)),
        "lexical_declaration" | "variable_declaration" => Some((VARIABLE, None)),

        // Parameters — formal_parameters is flattened; individual params below
        "required_parameter" | "optional_parameter" => Some((PARAMETER, None)),
        // accessibility_modifier / override_modifier /
        // readonly_modifier — handled in the main match block as
        // source-backed marker keywords.

        // Blocks
        "statement_block" => Some((BLOCK, None)),

        // Statements
        "return_statement" => Some((RETURN, None)),
        "if_statement" => Some((IF, None)),
        "else_clause" => Some((ELSE, None)),
        "for_statement" => Some((FOR, None)),
        "while_statement" => Some((WHILE, None)),
        "try_statement" => Some((TRY, None)),
        "catch_clause" => Some((CATCH, None)),
        "throw_statement" => Some((THROW, None)),

        // Expressions
        "call_expression" => Some((CALL, None)),
        "new_expression" => Some((NEW, None)),
        "member_expression" => Some((MEMBER, None)),
        // Note: call_expression and member_expression are also handled explicitly
        // in the transform match for field promotion, then renamed via map_element_name.
        "assignment_expression" => Some((ASSIGN, None)),
        "binary_expression" => Some((BINARY, None)),
        "unary_expression" => Some((UNARY, None)),
        // ternary_expression handled above (wraps alternative in <else>)
        "await_expression" => Some((AWAIT, None)),
        "yield_expression" => Some((YIELD, None)),
        "as_expression" => Some((AS, None)),

        // Classes / members
        // class_heritage is flattened in the match above; the inner clauses
        // are the semantic nodes (extends_clause → <extends>, etc.).
        // extends_clause handled above (retag value→type before rename)
        "implements_clause" => Some((IMPLEMENTS, None)),
        "field_definition" | "public_field_definition" => Some((FIELD, None)),

        // Template strings
        "template_string" => Some((TEMPLATE, None)),
        "template_substitution" => Some((INTERPOLATION, None)),

        // Imports/Exports
        "import_statement" => Some((IMPORT, None)),
        "export_statement" => Some((EXPORT, None)),

        // Literals
        "string" => Some((STRING, None)),
        "number" => Some((NUMBER, None)),
        "true" | "false" => Some((BOOL, None)),
        "null" => Some((NULL, None)),

        // Types
        // type_annotation is flattened in the match above.
        "predefined_type" => Some((TYPE, None)),
        "type_parameters" => Some((GENERICS, None)),
        "type_parameter" => Some((GENERIC, None)),

        // Abstract forms — collapse to the concrete shape with an
        // `<abstract/>` marker for querying.
        "abstract_class_declaration" => Some((CLASS, Some(ABSTRACT))),
        "abstract_method_signature" => Some((METHOD, Some(ABSTRACT))),
        // augmented_assignment_expression collapses to <assign>; the
        // <op> child (e.g., +=) distinguishes it.
        "augmented_assignment_expression" => Some((ASSIGN, None)),
        // Type-position flavors missed earlier — roll up to <type>.
        "conditional_type" => Some((TYPE, Some(CONDITIONAL))),
        "infer_type" => Some((TYPE, Some(INFER))),
        "lookup_type" => Some((TYPE, Some(LOOKUP))),
        "index_type_query" => Some((TYPE, Some(KEYOF))),
        "opting_type_annotation" => Some((ANNOTATION, None)),
        // satisfies — TS 4.9 `expr satisfies T` operator.
        "satisfies_expression" => Some((SATISFIES, None)),
        // Optional chain `?.` — collapse to <member> with the `<optional/>`
        // marker like the other member shapes.
        "optional_chain" => Some((MEMBER, Some(OPTIONAL))),
        // Finally-clause leak.
        "finally_clause" => Some((FINALLY, None)),
        // Export bits — export_clause is the `{ a, b }` wrapper;
        // export_specifier is one entry. Flatten the clause handled below.
        "export_specifier" => Some((SPEC, None)),
        // Namespace import `* as ns` — collapse to <namespace> marker.
        "namespace_import" => Some((NAMESPACE, None)),
        // Object destructuring assignment `{ a = 1 } = obj` —
        // collapses to <pattern>[object] with an assign inside.
        "object_assignment_pattern" => Some((PATTERN, Some(DEFAULT))),
        // `#counter` — class private field access; rename to <name>.
        // The leading `#` stays as part of the name text, which already
        // signals "private" at parse time.
        "private_property_identifier" => Some((NAME, None)),

        // Default - no mapping
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

/// Extract operator from text children and add as `<op>` child element
fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);

    // Find operator (skip punctuation)
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    });

    if let Some(op) = operator {
        prepend_op_element(xot, node, op)?;
    }

    Ok(())
}

/// Lift `async` keyword and generator `*` prefix on functions/methods to
/// empty marker children on the node. Leaves all other children intact.
/// The source keyword/token remains as a text sibling for renderability.
///
/// Text children may concatenate multiple tokens (e.g. `"async function"`
/// or `"function*"`), so we scan token-wise for the keywords.
fn extract_function_markers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let mut has_async = false;
    let mut has_star = false;
    let mut accessor_kind: Option<&'static str> = None;
    for t in &texts {
        for tok in t.split_whitespace() {
            if tok == "async" {
                has_async = true;
            }
            // A generator marker may appear as a standalone "*" token or
            // attached to "function" (e.g. "function*"). Either way, if
            // any token contains '*' and is part of a function/method
            // header, treat it as a generator marker.
            if tok == "*" || tok.ends_with('*') || tok.starts_with('*') {
                has_star = true;
            }
            // Property accessor methods: `get value() {}` / `set value(v) {}`.
            // Lift the keyword to a `<get/>` / `<set/>` marker so queries
            // can predicate on the accessor kind uniformly.
            match tok {
                "get" => accessor_kind = Some(GET),
                "set" => accessor_kind = Some(SET),
                _ => {}
            }
        }
    }
    // Prepend in reverse so final order is <async/><generator/><get|set/>...
    if let Some(kind) = accessor_kind {
        prepend_empty_element(xot, node, kind)?;
    }
    if has_star {
        prepend_empty_element(xot, node, GENERATOR)?;
    }
    if has_async {
        prepend_empty_element(xot, node, ASYNC)?;
    }
    Ok(())
}

/// Extract let/const/var keywords and add as empty modifier elements
fn extract_keyword_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);

    // Known keyword modifiers for TypeScript. Source keyword paired with
    // the semantic marker name emitted (for constant-driven type safety).
    const MODIFIERS: &[(&str, &str)] = &[
        ("let", LET),
        ("const", CONST),
        ("var", VAR),
        ("async", ASYNC),
        ("export", EXPORT),
        ("default", DEFAULT),
    ];

    // Find modifiers and prepend as empty elements (in reverse to maintain order)
    let found: Vec<&str> = texts.iter()
        .filter_map(|t| MODIFIERS.iter().find(|(src, _)| *src == t).map(|(_, marker)| *marker))
        .collect();

    for modifier in found.into_iter().rev() {
        prepend_empty_element(xot, node, modifier)?;
    }

    Ok(())
}

/// Wrap each bare `identifier` child of a parameter list in a `<param>`
/// element. Harmonises JS (grammar: `formal_parameters → identifier`)
/// with TS (grammar: `formal_parameters → required_parameter → identifier`)
/// so the semantic tree shape is the same.
fn wrap_bare_identifier_params(xot: &mut Xot, list: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(list)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        let kind = get_element_name(xot, child);
        if kind.as_deref() != Some("identifier") {
            continue;
        }
        let param_name = xot.add_name(PARAMETER);
        let param = xot.new_element(param_name);
        copy_source_location(xot, child, param);
        xot.insert_before(child, param)?;
        xot.detach(child)?;
        xot.append(param, child)?;
    }
    Ok(())
}

/// Check if a node has a `kind` attribute (i.e., it's a tree-sitter node, not a wrapper)
/// Find the `<value>` field-wrapper child (if any) and retag it as
/// `<type>` — both the element name and the `field` attribute. Used
/// where tree-sitter tags a type reference with `field="value"`
/// (e.g. `extends_clause` in TS) and we want the namespace-vocabulary
/// shape `<type>...</type>` instead.
fn retag_value_as_type(xot: &mut Xot, parent: XotNode) -> Result<(), xot::Error> {
    let value_child = xot.children(parent)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("value"));
    if let Some(v) = value_child {
        rename(xot, v, TYPE);
        set_attr(xot, v, "field", "type");
    }
    Ok(())
}

fn has_kind(xot: &Xot, node: XotNode) -> bool {
    get_kind(xot, node).is_some()
}

/// Returns true if `node` has a visibility-related modifier child.
/// Matches both the raw tree-sitter kind (`accessibility_modifier`)
/// AND post-transform marker element names (`public`/`private`/`protected`).
/// Walk-order safe: the accessibility_modifier transform fires before the
/// enclosing method/field sees its children in Continue mode, so either
/// form may be present by the time we look.
fn has_visibility_marker(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if xot.element(child).is_none() { continue; }
        let ts_kind = get_kind(xot, child);
        if ts_kind.as_deref() == Some("accessibility_modifier") {
            return true;
        }
        if let Some(name) = get_element_name(xot, child) {
            if name == PUBLIC || name == PRIVATE || name == PROTECTED {
                return true;
            }
        }
    }
    false
}

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
///
/// For `private_property_identifier` the leading `#` is stripped and a
/// `<private/>` marker is prepended to the enclosing field/property — the
/// text "#foo" is purely a sigil, not part of the name, and the marker
/// follows Principle #7 (modifiers as empty elements).
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };
        // Accept `type_identifier` here too: tree-sitter uses it for the
        // name of class/interface/alias/generic declarations (where the
        // declared thing happens to be a type), but in the tree that's
        // still an identifier — the <name> wrapper's job is just to hold
        // the declared name as text. Without this, the identifier leaks
        // through, later gets renamed to <type>, and we end up with
        // `<name><type><name>Foo</name></type></name>` triple-nesting.
        if !matches!(
            child_name.as_str(),
            "identifier" | "property_identifier" | "private_property_identifier" | "type_identifier",
        ) {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
        let is_private = child_name == "private_property_identifier";
        let clean_text = if is_private {
            text.trim_start_matches('#').to_string()
        } else {
            text
        };
        let all_children: Vec<_> = xot.children(node).collect();
        for c in all_children {
            xot.detach(c)?;
        }
        let text_node = xot.new_text(&clean_text);
        xot.append(node, text_node)?;
        if is_private {
            if let Some(parent) = get_parent(xot, node) {
                let already = xot.children(parent).any(|c| {
                    get_element_name(xot, c).as_deref() == Some(PRIVATE)
                });
                if !already {
                    prepend_empty_element(xot, parent, PRIVATE)?;
                }
            }
        }
        return Ok(());
    }
    Ok(())
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,
        "ref" => SyntaxCategory::Identifier,

        // Literals
        "string" => SyntaxCategory::String,
        "number" => SyntaxCategory::Number,
        "true" | "false" | "null" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "interface" | "enum" | "alias" => SyntaxCategory::Keyword,
        "function" | "method" => SyntaxCategory::Keyword,
        "variable" | "parameter" | "parameters" | "optional" | "required" => SyntaxCategory::Keyword,
        "import" | "export" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" | "for" | "while" | "do" => SyntaxCategory::Keyword,
        "switch" | "case" | "default" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" | "yield" => SyntaxCategory::Keyword,

        // Keywords - modifiers
        "let" | "const" | "var" => SyntaxCategory::Keyword,
        "async" | "await" => SyntaxCategory::Keyword,
        "new" | "this" | "super" => SyntaxCategory::Keyword,

        // Functions/calls
        "call" => SyntaxCategory::Function,
        "arrow" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Types
        "typeof" | "generics" | "generic" => SyntaxCategory::Type,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_string_to_xot;
    use crate::output::{render_document, RenderOptions};
    use super::semantic;

    #[test]
    fn marker_only_names_are_in_all_names() {
        for m in semantic::MARKER_ONLY {
            assert!(
                semantic::ALL_NAMES.contains(m),
                "MARKER_ONLY entry {:?} missing from ALL_NAMES",
                m,
            );
        }
    }

    #[test]
    fn all_names_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for name in semantic::ALL_NAMES {
            assert!(seen.insert(*name), "duplicate name in ALL_NAMES: {:?}", name);
        }
    }

    #[test]
    fn test_typescript_transform() {
        let source = "let x = 1 + 2;";
        let result = parse_string_to_xot(source, "typescript", "<test>".to_string(), None).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Check transforms applied
        assert!(xml.contains("<binary"), "binary_expression should be renamed");
        // Operator wraps the source text in place — marker child
        // plus the original `+` text.
        let normalized = xml.split_whitespace().collect::<Vec<_>>().join("");
        assert!(
            normalized.contains("<op><plus/>+</op>"),
            "operator should be extracted with semantic marker; got:\n{xml}"
        );
        assert!(xml.contains("<let"), "let should be extracted as modifier");
    }

    #[test]
    fn test_optional_parameter_marker() {
        let source = "function f(a: string, b?: number) {}";
        let result = parse_string_to_xot(source, "typescript", "<test>".to_string(), None).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Count occurrences of <parameter> - should have 2
        let param_count = xml.matches("<parameter>").count() + xml.matches("<parameter ").count();
        assert_eq!(param_count, 2, "should have 2 parameters, got: {xml}");

        // Only the optional parameter should have <optional/>
        assert!(xml.contains("<optional/>"), "optional parameter should have <optional/> marker, got: {xml}");

        // The <optional/> should appear exactly once (only for b?)
        let optional_count = xml.matches("<optional/>").count();
        assert_eq!(optional_count, 1, "should have exactly 1 <optional/> marker, got: {xml}");
    }
}
