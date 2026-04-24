//! Java transform logic
//!
//! This module owns ALL Java-specific transformation rules.
//! No assumptions about other languages - this is self-contained.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's Java XML vocabulary after transform.
pub mod semantic {
    // Structural — containers.

    // Top-level / declarations
    pub const PROGRAM: &str = "program";
    pub const CLASS: &str = "class";
    pub const INTERFACE: &str = "interface";
    pub const ENUM: &str = "enum";
    pub const RECORD: &str = "record";
    pub const METHOD: &str = "method";
    pub const CONSTRUCTOR: &str = "constructor";
    pub const FIELD: &str = "field";
    pub const VARIABLE: &str = "variable";
    pub const DECLARATOR: &str = "declarator";
    pub const CONSTANT: &str = "constant";

    // Members
    pub const PARAMETER: &str = "parameter";
    pub const GENERIC: &str = "generic";
    pub const GENERICS: &str = "generics";
    pub const EXTENDS: &str = "extends";
    pub const IMPLEMENTS: &str = "implements";

    // Type vocabulary
    pub const TYPE: &str = "type";
    pub const PATH: &str = "path";
    pub const RETURNS: &str = "returns";
    // `String[] args` — array dimensions leaf. Carries the bracket text.
    pub const DIMENSIONS: &str = "dimensions";

    // Control flow
    pub const RETURN: &str = "return";
    pub const IF: &str = "if";
    pub const ELSE: &str = "else";
    pub const ELSE_IF: &str = "else_if";
    pub const FOR: &str = "for";
    pub const FOREACH: &str = "foreach";
    pub const WHILE: &str = "while";
    pub const TRY: &str = "try";
    pub const CATCH: &str = "catch";
    pub const FINALLY: &str = "finally";
    pub const THROW: &str = "throw";
    // `method() throws IOException` — the throws list emitted as a
    // container wrapping exception types.
    pub const THROWS: &str = "throws";
    pub const SWITCH: &str = "switch";
    pub const ARM: &str = "arm";
    pub const LABEL: &str = "label";
    pub const CASE: &str = "case";
    pub const PATTERN: &str = "pattern";
    // `case Foo f when cond` — the `when` guard clause emitted as a
    // container on a pattern arm.
    pub const GUARD: &str = "guard";
    pub const BODY: &str = "body";

    // Expressions
    pub const CALL: &str = "call";
    pub const NEW: &str = "new";
    pub const MEMBER: &str = "member";
    pub const INDEX: &str = "index";
    pub const ASSIGN: &str = "assign";
    pub const BINARY: &str = "binary";
    pub const UNARY: &str = "unary";
    pub const LAMBDA: &str = "lambda";
    pub const TERNARY: &str = "ternary";
    pub const ANNOTATION: &str = "annotation";

    // Imports
    pub const IMPORT: &str = "import";
    pub const PACKAGE: &str = "package";

    // Literals
    pub const STRING: &str = "string";
    pub const INT: &str = "int";
    pub const FLOAT: &str = "float";
    pub const TRUE: &str = "true";
    pub const FALSE: &str = "false";
    pub const NULL: &str = "null";

    // Identifiers / comments
    pub const NAME: &str = "name";
    pub const COMMENT: &str = "comment";

    // Operator child
    pub const OP: &str = "op";

    // Markers — always empty when emitted.

    // Access modifiers (marker-only).
    pub const PUBLIC: &str = "public";
    pub const PRIVATE: &str = "private";
    pub const PROTECTED: &str = "protected";

    // Other modifiers (marker-only).
    pub const STATIC: &str = "static";
    pub const FINAL: &str = "final";
    pub const ABSTRACT: &str = "abstract";
    pub const SYNCHRONIZED: &str = "synchronized";
    pub const VOLATILE: &str = "volatile";
    pub const TRANSIENT: &str = "transient";
    pub const NATIVE: &str = "native";
    pub const STRICTFP: &str = "strictfp";

    // Special markers.
    pub const VOID: &str = "void";
    pub const THIS: &str = "this";
    pub const SUPER: &str = "super";
    pub const ARRAY: &str = "array";
    pub const VARIADIC: &str = "variadic";
    pub const COMPACT: &str = "compact";

    // Ambiguous names — emitted as BOTH structural container AND marker
    // in some contexts. Kept as constants for type-safety but NOT in
    // MARKER_ONLY:
    //   - PACKAGE: structural (package_declaration) + marker (implicit
    //     access when no access modifier is present).
    //   - RECORD: structural (record_declaration) + marker (record_pattern).
    //   - TYPE: structural (type references) + marker (type_pattern).
    //   - THIS: marker on `<call[this]>` AND structural element from the
    //     tree-sitter `this` kind (bare `this` expression).

    /// Names that, when emitted, are always empty elements.
    pub const MARKER_ONLY: &[&str] = &[
        PUBLIC, PRIVATE, PROTECTED,
        STATIC, FINAL, ABSTRACT, SYNCHRONIZED,
        VOLATILE, TRANSIENT, NATIVE, STRICTFP,
        VOID, SUPER, ARRAY, VARIADIC, COMPACT,
    ];

    /// Every semantic name this language's transform can emit.
    pub const ALL_NAMES: &[&str] = &[
        PROGRAM, CLASS, INTERFACE, ENUM, RECORD,
        METHOD, CONSTRUCTOR, FIELD, VARIABLE, DECLARATOR, CONSTANT,
        PARAMETER, GENERIC, GENERICS, EXTENDS, IMPLEMENTS,
        TYPE, PATH, RETURNS, DIMENSIONS,
        RETURN, IF, ELSE, ELSE_IF, FOR, FOREACH, WHILE, TRY, CATCH, FINALLY, THROW, THROWS,
        SWITCH, ARM, LABEL, CASE, PATTERN, GUARD, BODY,
        CALL, NEW, MEMBER, INDEX, ASSIGN, BINARY, UNARY, LAMBDA, TERNARY, ANNOTATION,
        IMPORT, PACKAGE,
        STRING, INT, FLOAT, TRUE, FALSE, NULL,
        NAME, COMMENT, OP,
        PUBLIC, PRIVATE, PROTECTED,
        STATIC, FINAL, ABSTRACT, SYNCHRONIZED,
        VOLATILE, TRANSIENT, NATIVE, STRICTFP,
        VOID, THIS, SUPER, ARRAY, VARIADIC, COMPACT,
    ];
}

/// Transform a Java AST node
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

        // ---------------------------------------------------------------------
        // Flatten nodes - transform children, then remove wrapper
        // ---------------------------------------------------------------------
        "class_body" | "interface_body" | "block"
        | "enum_body" | "field_declaration_list" | "type_list"
        | "constructor_body"
        // `@Foo(key = value)` — `annotation_argument_list` is the
        // parenthesised wrapper around the arguments. Flatten so
        // arguments become direct siblings of the `<annotation>`.
        | "annotation_argument_list"
        // `enum E { A, B; void foo() {} }` puts the field/method list
        // after the `;` into `enum_body_declarations` — grouping only.
        | "enum_body_declarations" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Flat lists (Principle #12)
        // ---------------------------------------------------------------------
        "formal_parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "argument_list" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "type_arguments" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "type_parameters" => {
            distribute_field_to_children(xot, node, "generics");
            rename(xot, node, GENERICS);
            Ok(TransformAction::Flatten)
        }
        "type_parameter" => {
            // Tree-sitter puts the parameter's name as a sibling
            // `type_identifier`; bounds follow as sibling `type_bound`
            // elements. Replace the identifier with a `<name>TEXT</name>`
            // element so the eventual shape is
            // `<generic><name>T</name><bound>...</bound></generic>`,
            // not the over-wrapped `<generic><type><name>T</name></type>...`.
            replace_identifier_with_name_child(xot, node, &["type_identifier"])?;
            rename(xot, node, GENERIC);
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Generic type references: apply the C# pattern.
        //   generic_type(<type_identifier>Foo</type_identifier>, type_arguments)
        //     -> <type><generic/>Foo <type field="arguments">Bar</type>...</type>
        // ---------------------------------------------------------------------
        "generic_type" => {
            rewrite_generic_type(xot, node, &["type_identifier", "scoped_type_identifier"])?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Name wrappers created by the builder for field="name".
        // Inline the single identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        // ---------------------------------------------------------------------
        "name" => {
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Modifier wrappers - Java wraps modifiers in a "modifiers"
        // element containing space-separated keyword tokens. Lift each
        // keyword to an empty marker in source order, then flatten the
        // wrapper so the literal `public abstract static` text survives
        // as dangling siblings — the enclosing declaration's XPath
        // string-value then contains the actual source keywords.
        // Also inserts <package/> if no access modifier was found
        // (Principle #9 — mutually-exclusive access is exhaustive).
        "modifiers" => {
            let words: Vec<String> = match get_text_content(xot, node) {
                Some(text) => text.split_whitespace().map(String::from).collect(),
                None => Vec::new(),
            };
            let has_access = words.iter().any(|w| is_access_modifier(w));

            // Build final marker list in source order. The implicit
            // <package/> (when no access keyword was written) lives at
            // the head — conventionally access modifiers come first.
            let mut markers: Vec<&str> = Vec::new();
            if !has_access {
                markers.push(PACKAGE);
            }
            for word in &words {
                if is_known_modifier(word) {
                    markers.push(word.as_str());
                }
            }

            for marker in &markers {
                insert_empty_before(xot, node, marker)?;
            }

            // Flatten <modifiers> so its text content lifts to the
            // parent, preserving the source keywords next to the
            // markers we just inserted.
            Ok(TransformAction::Flatten)
        }

        // ---------------------------------------------------------------------
        // Binary/unary expressions - extract operator
        // ---------------------------------------------------------------------
        "binary_expression" | "unary_expression" | "assignment_expression" => {
            extract_operator(xot, node)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Ternary expression — surgically wrap `alternative` in `<else>`.
        // See TS transform for rationale.
        "ternary_expression" => {
            wrap_field_child(xot, node, "alternative", ELSE)?;
            rename(xot, node, TERNARY);
            Ok(TransformAction::Continue)
        }

        // Java's tree-sitter doesn't emit an `else_clause` wrapper: the
        // `alternative` field of an if_statement points directly at the
        // nested if_statement (for `else if`) or a block (for final
        // `else {…}`). Wrap the alternative in `<else>` surgically so
        // the shared conditional-shape post-transform can collapse the
        // chain uniformly.
        "if_statement" => {
            wrap_field_child(xot, node, "alternative", ELSE)?;
            rename(xot, node, IF);
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Identifiers are always names (definitions or references).
        // Tree-sitter uses `type_identifier` for type positions, so bare
        // identifiers never need a heuristic — they are never types.
        // ---------------------------------------------------------------------
        "identifier" => {
            rename(xot, node, NAME);
            Ok(TransformAction::Continue)
        }
        // Tree-sitter Java emits `line_comment` and `block_comment` —
        // both are just "comment" in every semantic query. Rename to
        // the shared `<comment>` vocabulary. Principle #1 / #2.
        "line_comment" | "block_comment" => {
            rename(xot, node, COMMENT);
            Ok(TransformAction::Continue)
        }
        // `string_fragment` is tree-sitter's wrapper around the
        // unescaped body chars of a string literal — lift the text
        // up to the enclosing <string> (Principle #12 flat strings).
        "string_fragment" => Ok(TransformAction::Flatten),
        "type_identifier" | "integral_type" | "floating_point_type"
        | "boolean_type" => {
            rename(xot, node, TYPE);
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }
        // `void_type` gets the same `<type><name>void</name></type>`
        // shape as any other type PLUS a `<void/>` marker — void is
        // the one primitive that's special enough to warrant a
        // shortcut predicate (`//type[void]`) because it's
        // return-only and conceptually "no value", not a regular
        // data type. The marker is *additional*, not a replacement
        // for `<name>`: JSON keeps `"name": "void"` for data
        // consumers and adds `"void": true` as the shortcut flag.
        "void_type" => {
            rename(xot, node, TYPE);
            wrap_text_in_name(xot, node)?;
            prepend_empty_element(xot, node, VOID)?;
            Ok(TransformAction::Continue)
        }

        // Parenthesized expressions are a grammar wrapper — just `"("`
        // / the inner expression / `")"`. The parens carry no semantic,
        // so skip the wrapper: its children become direct siblings of
        // the enclosing node (Principle #12).
        "parenthesized_expression" => Ok(TransformAction::Skip),

        // `this(args)` / `super(args)` at the start of a constructor
        // body. Render as `<call>` with a `<this/>` or `<super/>`
        // marker so `//call[this]` / `//call[super]` work uniformly
        // with other call sites.
        "explicit_constructor_invocation" => {
            // Find the "this" or "super" keyword child and lift it to
            // an empty marker with the keyword as dangling sibling text.
            let children: Vec<_> = xot.children(node).collect();
            for child in children {
                let child_kind = get_kind(xot, child);
                let tag = match child_kind.as_deref() {
                    Some("this") => THIS,
                    Some("super") => SUPER,
                    _ => continue,
                };
                let text = get_text_content(xot, child).unwrap_or_default();
                xot.detach(child)?;
                let marker = prepend_empty_element(xot, node, tag)?;
                insert_text_after(xot, marker, &text)?;
                break;
            }
            rename(xot, node, CALL);
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Declarations — prepend <package/> if no modifiers child
        // ---------------------------------------------------------------------
        "class_declaration" | "interface_declaration" | "enum_declaration"
        | "method_declaration" | "constructor_declaration" | "field_declaration" => {
            if !has_modifiers_child(xot, node) {
                // Members declared inside an interface are implicitly public
                // (Java spec §9.4). Top-level types and class members default
                // to package access.
                let default = if is_inside_interface(xot, node) {
                    PUBLIC
                } else {
                    PACKAGE
                };
                prepend_empty_element(xot, node, default)?;
            }
            // Java's grammar tags the method return type as field="type"
            // (the same field name used on parameters), so the builder
            // can't wrap it by name. Do it here for methods only.
            if kind == "method_declaration" {
                wrap_method_return_type(xot, node)?;
            }
            apply_rename(xot, node, &kind)?;
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
                    wrap_text_in_name(xot, node)?;
                }
            }
            Ok(TransformAction::Continue)
        }
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

/// Walk up from `node` looking for an enclosing `interface_declaration`.
/// Stops at the first class/enum/record (which would override the default).
fn is_inside_interface(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_kind(xot, parent) {
            match kind.as_str() {
                "interface_declaration" => return true,
                "class_declaration" | "enum_declaration" | "record_declaration" => return false,
                // interface_body and class_body are transparent wrappers
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

/// Wrap a method's return type (the child with field="type") in a `<returns>`
/// element so it's symmetric with C#/Rust/TS. Java's tree-sitter grammar
/// uses the ambiguous field name `type` for both return types and parameter
/// types, so this can't be done generically by the builder.
fn wrap_method_return_type(xot: &mut Xot, method: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(method).collect();
    for child in children {
        if xot.element(child).is_none() {
            continue;
        }
        if get_attr(xot, child, "field").as_deref() != Some("type") {
            continue;
        }
        let returns_name = xot.add_name(RETURNS);
        let wrapper = xot.new_element(returns_name);
        copy_source_location(xot, child, wrapper);
        set_attr(xot, wrapper, "field", "returns");
        xot.insert_before(child, wrapper)?;
        xot.detach(child)?;
        xot.append(wrapper, child)?;
        // Drop field="type" on the inner type — it's now "returns" at the wrapper level
        remove_attr(xot, child, "field");
        break;
    }
    Ok(())
}

/// Check if text is an access modifier keyword
fn is_access_modifier(text: &str) -> bool {
    matches!(text, PUBLIC | PRIVATE | PROTECTED)
}

/// Check if a declaration node has a `modifiers` child element
fn has_modifiers_child(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "modifiers" {
                return true;
            }
        }
    }
    false
}

/// Known Java modifiers
fn is_known_modifier(text: &str) -> bool {
    matches!(text,
        PUBLIC | PRIVATE | PROTECTED |
        STATIC | FINAL | ABSTRACT | SYNCHRONIZED |
        VOLATILE | TRANSIENT | NATIVE | STRICTFP
    )
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Second tuple element is an optional disambiguation marker.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        "program" => Some((PROGRAM, None)),
        "class_declaration" => Some((CLASS, None)),
        "interface_declaration" => Some((INTERFACE, None)),
        "enum_declaration" => Some((ENUM, None)),
        "method_declaration" => Some((METHOD, None)),
        "constructor_declaration" => Some((CONSTRUCTOR, None)),
        // constructor_body is flattened (handled above) — the `body` wrapper
        // already comes from the field-wrapping pass.
        "field_declaration" => Some((FIELD, None)),
        "variable_declarator" => Some((DECLARATOR, None)),
        "local_variable_declaration" => Some((VARIABLE, None)),
        "enum_constant" => Some((CONSTANT, None)),
        // formal_parameters and argument_list are flattened via Principle #12 above
        "formal_parameter" => Some((PARAMETER, None)),
        "generic_type" => Some((GENERIC, None)),
        // array_type collapses to <type><array/> so it joins the
        // uniform namespace of `<type>` references.
        "array_type" => Some((TYPE, Some(ARRAY))),
        "scoped_identifier" | "scoped_type_identifier" => Some((PATH, None)),
        "super_interfaces" => Some((IMPLEMENTS, None)),
        "superclass" => Some((EXTENDS, None)),
        "type_bound" => Some((EXTENDS, None)),
        "type_parameter" => Some((GENERIC, None)),
        "return_statement" => Some((RETURN, None)),
        "if_statement" => Some((IF, None)),
        "else_clause" => Some((ELSE, None)),
        "for_statement" => Some((FOR, None)),
        "enhanced_for_statement" => Some((FOREACH, None)),
        "while_statement" => Some((WHILE, None)),
        "try_statement" => Some((TRY, None)),
        "catch_clause" => Some((CATCH, None)),
        "finally_clause" => Some((FINALLY, None)),
        "throw_statement" => Some((THROW, None)),
        "switch_expression" => Some((SWITCH, None)),
        "switch_rule" => Some((ARM, None)),
        "switch_label" => Some((LABEL, None)),
        "switch_block_statement_group" => Some((CASE, None)),
        "method_invocation" => Some((CALL, None)),
        "object_creation_expression" => Some((NEW, None)),
        "field_access" => Some((MEMBER, None)),
        "array_access" => Some((INDEX, None)),
        "assignment_expression" => Some((ASSIGN, None)),
        "binary_expression" => Some((BINARY, None)),
        "unary_expression" => Some((UNARY, None)),
        // ternary_expression handled above
        "lambda_expression" => Some((LAMBDA, None)),
        "string_literal" => Some((STRING, None)),
        "decimal_integer_literal" | "hex_integer_literal"
        | "octal_integer_literal" | "binary_integer_literal" => Some((INT, None)),
        // Patterns — `instanceof T x` and record patterns both collapse
        // to `<pattern>` with a shape marker so the flavor is queryable.
        "type_pattern" => Some((PATTERN, Some(TYPE))),
        "record_pattern" => Some((PATTERN, Some(RECORD))),
        "switch_block" => Some((BODY, None)),
        // varargs `T... name` — flag with <variadic/> so queries can
        // distinguish from a regular parameter.
        "spread_parameter" => Some((PARAMETER, Some(VARIADIC))),
        "decimal_floating_point_literal" => Some((FLOAT, None)),
        // `record R(...)` — Java 14+ records. Same slot as <class>/etc.
        "record_declaration" => Some((RECORD, None)),
        // `record R(...) { R { … } }` — compact constructor syntax
        // without an explicit parameter list.
        "compact_constructor_declaration" => Some((CONSTRUCTOR, Some(COMPACT))),
        // `@Override` bare marker annotation — collapse to <annotation>.
        "marker_annotation" => Some((ANNOTATION, None)),
        "annotation" => Some((ANNOTATION, None)),
        "true" => Some((TRUE, None)),
        "false" => Some((FALSE, None)),
        "null_literal" => Some((NULL, None)),
        "import_declaration" => Some((IMPORT, None)),
        "package_declaration" => Some((PACKAGE, None)),
        _ => None,
    }
}

/// Extract operator from text children and add as `<op>` child element
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

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        // Also accept `type_identifier` — tree-sitter uses it for the
        // name of class/interface/enum/generic declarations, where the
        // declared thing is a type but its name is still an identifier.
        let child_name = get_element_name(xot, child);
        if !matches!(
            child_name.as_deref(),
            Some("identifier") | Some("type_identifier"),
        ) {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
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

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,

        // Literals
        "string" => SyntaxCategory::String,
        "int" | "float" => SyntaxCategory::Number,
        "true" | "false" | "null" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "interface" | "enum" => SyntaxCategory::Keyword,
        "method" | "constructor" | "field" => SyntaxCategory::Keyword,
        "parameter" | "parameters" => SyntaxCategory::Keyword,
        "import" | "package" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" => SyntaxCategory::Keyword,
        "for" | "foreach" | "while" | "do" => SyntaxCategory::Keyword,
        "switch" | "case" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" => SyntaxCategory::Keyword,

        // Keywords - modifiers (note: "package" is covered earlier)
        "public" | "private" | "protected" => SyntaxCategory::Keyword,
        "static" | "final" | "abstract" | "synchronized" => SyntaxCategory::Keyword,
        "volatile" | "transient" | "native" | "strictfp" => SyntaxCategory::Keyword,
        "new" | "this" | "super" => SyntaxCategory::Keyword,

        // Types
        "generic" | "array" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,
        "lambda" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::semantic::*;

    #[test]
    fn marker_only_names_are_in_all_names() {
        for m in MARKER_ONLY {
            assert!(
                ALL_NAMES.contains(m),
                "MARKER_ONLY entry {:?} missing from ALL_NAMES",
                m,
            );
        }
    }

    #[test]
    fn all_names_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for name in ALL_NAMES {
            assert!(seen.insert(*name), "duplicate name in ALL_NAMES: {:?}", name);
        }
    }
}
