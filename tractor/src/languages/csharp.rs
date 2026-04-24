//! C# language definitions and transform logic
//!
//! This module owns ALL C#-specific knowledge: element names, modifiers,
//! and transformation rules. The renderer imports constants from here
//! rather than defining its own.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's C# XML vocabulary after transform.
/// These are the names that appear in tractor's output and that the renderer reads.
pub mod semantic {
    // Top-level / structural
    pub const UNIT: &str = "unit";
    pub const NAMESPACE: &str = "namespace";
    pub const IMPORT: &str = "import";
    pub const BODY: &str = "body";

    // Type declarations
    pub const CLASS: &str = "class";
    pub const STRUCT: &str = "struct";
    pub const INTERFACE: &str = "interface";
    pub const ENUM: &str = "enum";
    pub const RECORD: &str = "record";

    // Members
    pub const METHOD: &str = "method";
    pub const CONSTRUCTOR: &str = "constructor";
    pub const PROPERTY: &str = "property";
    pub const FIELD: &str = "field";
    pub const COMMENT: &str = "comment";

    // Shared children
    pub const NAME: &str = "name";
    pub const TYPE: &str = "type";
    pub const ACCESSORS: &str = "accessors";
    pub const ACCESSOR: &str = "accessor";
    pub const ATTRIBUTES: &str = "attributes";
    pub const ATTRIBUTE: &str = "attribute";
    pub const ARGUMENTS: &str = "arguments";
    pub const ARGUMENT: &str = "argument";
    pub const PARAMETERS: &str = "parameters";
    pub const PARAMETER: &str = "parameter";
    pub const VARIABLE: &str = "variable";
    pub const DECLARATOR: &str = "declarator";

    // Type markers
    pub const NULLABLE: &str = "nullable";
    pub const GENERIC: &str = "generic";

    // Comment markers
    pub const TRAILING: &str = "trailing";
    pub const LEADING: &str = "leading";
}

/// Check if kind is a declaration that has a name child
/// Uses original TreeSitter kinds (from `kind` attribute) for robust detection
fn is_named_declaration(kind: &str) -> bool {
    matches!(kind,
        // Types
        "class_declaration"
        | "struct_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "namespace_declaration"
        // Members
        | "method_declaration"
        | "constructor_declaration"
        | "property_declaration"
        | "enum_member_declaration"
        // Parameters & variables
        | "parameter"
        | "variable_declarator"
        | "type_parameter"
        // Attribute applications: `[Foo(…)]` — inline the inner identifier
        // into the `<name>` field wrapper so we get `<name>Foo</name>`
        // not `<name><name>Foo</name></name>`.
        | "attribute"
    )
}

/// Transform a C# AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    // Use get_kind() for robust detection - original TreeSitter kind doesn't change after renames
    // Fall back to element name for field wrappers (like <name>, <body>) which don't have kind attr
    let kind = get_kind(xot, node)
        .or_else(|| get_element_name(xot, node))
        .unwrap_or_default();

    match kind.as_str() {
        // ---------------------------------------------------------------------
        // Flatten nodes - transform children, then remove wrapper
        // ---------------------------------------------------------------------
        "declaration_list" | "parameters" => Ok(TransformAction::Flatten),

        // String internals — grammar wrappers with no semantic
        // beyond their text value. Flatten so `<string>` reads as
        // text with `<interpolation>` children where relevant
        // (Principle #12).
        "string_content"
        | "string_literal_content"
        | "verbatim_string_literal_content"
        | "raw_string_literal_content"
        | "interpolation_brace"
        | "interpolation_start"
        | "escape_sequence"
        | "interpolated_string_expression"
        | "qualified_name" => Ok(TransformAction::Flatten),

        // `implicit_type` is C#'s `var` keyword in a type position.
        // Render as `<type><name>var</name></type>` for uniform
        // querying — users already learn type[name='int'] etc.
        "parenthesized_expression" => Ok(TransformAction::Flatten),

        "implicit_type" => {
            rename(xot, node, "type");
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Postfix unary (`x!`, `x++`) is still a unary expression —
        // map to the shared `<unary>` element.
        "postfix_unary_expression" => {
            extract_operator(xot, node)?;
            rename(xot, node, "unary");
            Ok(TransformAction::Continue)
        }
        // enum_member_declaration_list is a pure grouping wrapper around
        // enum members (the `{ Red, Green }` list inside `enum Color`).
        // local_declaration_statement wraps `type name = value;` inside a
        // method body; the inner `variable_declaration` already becomes
        // `<variable>`, so the outer wrapper adds no semantic info.
        // arrow_expression_clause is the `=>` body of an expression-bodied
        // method/property — flatten so its expression becomes body content.
        "enum_member_declaration_list" | "local_declaration_statement"
        | "arrow_expression_clause" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Flat lists (Principle #12): drop purely-grouping wrappers;
        // children become siblings with field="<plural>".
        // ---------------------------------------------------------------------
        "parameter_list" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "argument_list" | "attribute_argument_list" | "type_argument_list" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "attribute_list" => {
            distribute_field_to_children(xot, node, "attributes");
            Ok(TransformAction::Flatten)
        }
        "accessor_list" => {
            distribute_field_to_children(xot, node, "accessors");
            Ok(TransformAction::Flatten)
        }
        // Accessor declarations carry their kind (get / set / init / add /
        // remove) as a text token. Lift it to an empty marker element so
        // queries can predicate on the kind uniformly across the auto-form
        // (`{ get; set; }`) and the bodied form (`get { … }`).
        "accessor_declaration" => {
            const KINDS: &[&str] = &["get", "set", "init", "add", "remove"];
            let children: Vec<_> = xot.children(node).collect();
            for child in children {
                let raw = match xot.text_str(child) {
                    Some(t) => t.to_string(),
                    None => continue,
                };
                let stripped = raw.trim().trim_end_matches(';').trim();
                if let Some(&kind) = KINDS.iter().find(|&&k| k == stripped) {
                    // Prepend an empty marker so `//accessor[get]`
                    // matches uniformly across auto-form and bodied
                    // form. The original `get;` / `set;` / `get`
                    // text token is left untouched on the accessor,
                    // so its XPath string-value is source-accurate.
                    prepend_empty_element(xot, node, kind)?;
                    break;
                }
            }
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
        "type_parameter_list" => {
            distribute_field_to_children(xot, node, "generics");
            Ok(TransformAction::Flatten)
        }

        // ---------------------------------------------------------------------
        // Name wrappers - inline the single identifier child as text.
        //   <name><identifier>Foo</identifier></name>    →  <name>Foo</name>
        //   <name><type_identifier>Foo</type_identifier> →  <name>Foo</name>
        //   <name><name>Foo</name></name>                →  <name>Foo</name>
        //
        // Applies uniformly — declaration context and reference
        // context both want the same flat "identifier as a single
        // <name> text leaf" shape per the design doc.
        "name" => {
            let children: Vec<_> = xot.children(node).collect();
            let element_children: Vec<_> = children
                .iter()
                .copied()
                .filter(|&c| xot.element(c).is_some())
                .collect();
            if element_children.len() == 1 {
                let child = element_children[0];
                let child_kind = get_kind(xot, child);
                let is_identifier = matches!(
                    child_kind.as_deref(),
                    Some("identifier") | Some("type_identifier") | Some("property_identifier")
                );
                let is_inlined_name =
                    get_element_name(xot, child).as_deref() == Some("name");
                // For qualified / scoped names (`System.Text`,
                // `MyApp.Services.Logger`) concat the descendant
                // text so the outer <name> holds the full dotted
                // path as a single text leaf — Principle #14's
                // uniform `<name>X</name>` shape.
                let is_qualified = matches!(
                    child_kind.as_deref(),
                    Some("qualified_name") | Some("generic_name") | Some("alias_qualified_name")
                );
                if is_identifier || is_inlined_name {
                    if let Some(text) = get_text_content(xot, child) {
                        for c in children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&text);
                        xot.append(node, text_node)?;
                        return Ok(TransformAction::Done);
                    }
                } else if is_qualified {
                    let text = descendant_text(xot, child);
                    if !text.is_empty() {
                        for c in children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&text);
                        xot.append(node, text_node)?;
                        return Ok(TransformAction::Done);
                    }
                }
            }
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Modifier wrappers - C# wraps modifiers in "modifier" elements
        // Convert <modifier>public</modifier> to <public/>
        // ---------------------------------------------------------------------
        "modifier" => {
            if let Some(text) = get_text_content(xot, node) {
                let text = text.trim().to_string();
                if is_known_modifier(&text) {
                    rename_to_marker(xot, node, &text)?;
                    // Keep the source keyword as a dangling sibling so
                    // the enclosing declaration's XPath string-value
                    // still contains `public` / `static` / `this` / ...
                    // The marker element itself stays empty (Principle #7).
                    insert_text_after(xot, node, &text)?;
                    return Ok(TransformAction::Done);
                }
            }
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Nullable types - convert to <type>X<nullable/></type>
        // TreeSitter: <nullable_type><identifier>Guid</identifier>?</nullable_type>
        // We want: <type kind="nullable_type">Guid<nullable/></type>
        // ---------------------------------------------------------------------
        "nullable_type" => {
            // Find the inner type (identifier or predefined_type)
            let children: Vec<_> = xot.children(node).collect();
            for child in children {
                if let Some(child_kind) = get_kind(xot, child) {
                    if matches!(child_kind.as_str(), "identifier" | "predefined_type" | "type_identifier") {
                        if let Some(type_text) = get_text_content(xot, child) {
                            // Remove all children
                            let all_children: Vec<_> = xot.children(node).collect();
                            for c in all_children {
                                xot.detach(c)?;
                            }
                            // Rename to "type" (kind="nullable_type" is preserved)
                            rename(xot, node, "type");
                            // Add the type text
                            let text_node = xot.new_text(&type_text);
                            xot.append(node, text_node)?;
                            // Add <nullable/> element
                            let nullable_name = xot.add_name("nullable");
                            let nullable_el = xot.new_element(nullable_name);
                            xot.append(node, nullable_el)?;
                            return Ok(TransformAction::Done);
                        }
                    }
                }
            }
            // No recognized inner type - continue with children processing
            // kind="nullable_type" will be preserved for debugging
            rename(xot, node, "type");
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Binary/unary expressions - extract operator
        // ---------------------------------------------------------------------
        "binary_expression" | "unary_expression" | "assignment_expression" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Identifiers - classify as name or type based on context
        // ---------------------------------------------------------------------
        "identifier" => {
            let classification = classify_identifier(xot, node);
            rename(xot, node, classification);
            // If classified as a type reference, wrap the text in <name>
            // for the unified namespace vocabulary (Principle #14).
            if classification == "type" {
                wrap_text_in_name(xot, node)?;
            }
            Ok(TransformAction::Continue)
        }
        "type_identifier" | "predefined_type" => {
            rename(xot, node, "type");
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Generic types - wrap in <type> with <generic/> marker
        // TreeSitter: <generic_name><identifier>List</identifier><type_argument_list>...</type_argument_list></generic_name>
        // We want: <type><generic/>List<arguments>...</arguments></type>
        // ---------------------------------------------------------------------
        "generic_name" => {
            // Find the identifier child and extract its text
            let mut type_name = String::new();
            let children: Vec<_> = xot.children(node).collect();

            for child in &children {
                if let Some(child_kind) = get_kind(xot, *child) {
                    if child_kind == "identifier" {
                        if let Some(text) = get_text_content(xot, *child) {
                            type_name = text;
                        }
                        // Remove the identifier element (we'll add text directly)
                        xot.detach(*child)?;
                    }
                }
            }

            // Rename to "type"
            rename(xot, node, "type");

            // Add <generic/> marker as first child
            let generic_name = xot.add_name("generic");
            let generic_el = xot.new_element(generic_name);
            xot.prepend(node, generic_el)?;

            // Wrap the type name in a <name> child (Principle #14) so
            // `//type[name='IComparable']` matches uniformly across
            // declaration and reference sites.
            if !type_name.is_empty() {
                let name_id = xot.add_name("name");
                let name_el = xot.new_element(name_id);
                let text_node = xot.new_text(&type_name);
                xot.append(name_el, text_node)?;
                xot.insert_after(generic_el, name_el)?;
            }

            // Continue to process type_argument_list children
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Declarations — prepend default access modifier if none present
        // ---------------------------------------------------------------------
        "class_declaration" | "struct_declaration" | "interface_declaration"
        | "enum_declaration" | "record_declaration"
        | "method_declaration" | "constructor_declaration"
        | "property_declaration" | "field_declaration" => {
            if !has_access_modifier_child(xot, node) {
                let default = default_access_modifier(xot, node);
                prepend_empty_element(xot, node, default)?;
            }
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // Ternary expression — surgically wrap `alternative` in `<else>`.
        // See transformations.md (conditional shape) for rationale.
        "conditional_expression" => {
            wrap_field_child(xot, node, "alternative", "else")?;
            rename(xot, node, "ternary");
            Ok(TransformAction::Continue)
        }

        // C#'s tree-sitter doesn't emit an `else_clause` wrapper: the
        // `alternative` field of an if_statement points directly at
        // the nested if_statement (for `else if`) or a block (for
        // final `else {…}`). Wrap the alternative in `<else>`
        // surgically so the shared conditional-shape post-transform
        // can collapse the chain uniformly.
        "if_statement" => {
            wrap_field_child(xot, node, "alternative", "else")?;
            rename(xot, node, "if");
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Comments - detect attachment and group adjacent line comments
        //
        // Attachment classification:
        //   <trailing/> — comment on same line as previous sibling's end
        //   <leading/>  — comment (block) immediately followed by a declaration
        //   (no marker) — floating/standalone comment
        //
        // Grouping: consecutive // line comments on adjacent lines are merged
        // into a single <comment> with multiline text content.
        // ---------------------------------------------------------------------
        "comment" => {
            // Skip if already consumed by a preceding comment's grouping
            if xot.parent(node).is_none() {
                return Ok(TransformAction::Done);
            }

            // Trailing comments are attached to the previous sibling — no grouping
            if is_inline_node(xot, node) {
                prepend_empty_element(xot, node, "trailing")?;
                return Ok(TransformAction::Done);
            }

            // Group consecutive line comments into this node
            let consumed = group_line_comments(xot, node)?;

            // Classify the (possibly merged) comment
            if is_leading_comment(xot, node) {
                prepend_empty_element(xot, node, "leading")?;
            }

            // Detach consumed siblings (they've been merged into this node)
            for sibling in consumed {
                xot.detach(sibling)?;
            }

            Ok(TransformAction::Done)
        }

        // ---------------------------------------------------------------------
        // Other nodes - just rename if needed
        // ---------------------------------------------------------------------
        _ => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

/// C# access modifiers in canonical declaration order
pub const ACCESS_MODIFIERS: &[&str] = &["public", "private", "protected", "internal"];

/// C# non-access modifiers in canonical declaration order
pub const OTHER_MODIFIERS: &[&str] = &[
    "static", "abstract", "virtual", "override", "sealed",
    "readonly", "const", "partial", "async", "extern", "unsafe", "new",
];

fn is_access_modifier(text: &str) -> bool {
    ACCESS_MODIFIERS.contains(&text)
}

/// Check if a declaration node has any access modifier children (using raw kind)
fn has_access_modifier_child(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if let Some(kind) = get_kind(xot, child) {
            if kind == "modifier" {
                if let Some(text) = get_text_content(xot, child) {
                    if is_access_modifier(text.trim()) {
                        return true;
                    }
                }
            }
        }
        // Also check already-transformed markers
        if let Some(name) = get_element_name(xot, child) {
            if is_access_modifier(&name) {
                return true;
            }
        }
    }
    false
}

/// Determine the default access modifier for a C# declaration based on context.
/// Looks through `declaration_list` wrappers (which get Flatten'd, so children are
/// processed while still inside the wrapper).
///
/// Per C# spec: members of interfaces are public by default; members of classes,
/// structs, and records are private by default; top-level types are internal.
fn default_access_modifier(xot: &Xot, node: XotNode) -> &'static str {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(parent_kind) = get_kind(xot, parent).as_deref().map(str::to_owned) {
            match parent_kind.as_str() {
                "interface_declaration" => return "public",
                "class_declaration" | "struct_declaration"
                | "record_declaration" => return "private",
                // declaration_list is a transparent wrapper — look through it
                "declaration_list" => {}
                _ => break,
            }
        }
        current = get_parent(xot, parent);
    }
    "internal"
}

/// Known C# modifiers (access + other + "this" for extension methods)
fn is_known_modifier(text: &str) -> bool {
    ACCESS_MODIFIERS.contains(&text) || OTHER_MODIFIERS.contains(&text) || text == "this"
}

/// Map tree-sitter node kinds to semantic element names
fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "compilation_unit" => Some(UNIT),
        "class_declaration" => Some(CLASS),
        "struct_declaration" => Some(STRUCT),
        "interface_declaration" => Some(INTERFACE),
        "enum_declaration" => Some(ENUM),
        "record_declaration" => Some(RECORD),
        "method_declaration" => Some(METHOD),
        "constructor_declaration" => Some(CONSTRUCTOR),
        "property_declaration" => Some(PROPERTY),
        "field_declaration" => Some(FIELD),
        "namespace_declaration" => Some(NAMESPACE),
        "expression_statement" => Some("expression"),
        "parameter_list" => Some(PARAMETERS),
        "parameter" => Some(PARAMETER),
        "argument_list" => Some(ARGUMENTS),
        "argument" => Some(ARGUMENT),
        // generic_name is handled specially - becomes <type><generic/>Name<arguments>...</arguments></type>
        "type_argument_list" => Some(ARGUMENTS),
        // nullable_type is handled specially - becomes <type>X<nullable/></type>
        "array_type" => Some("array"),
        "block" => Some("block"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "foreach_statement" => Some("foreach"),
        "while_statement" => Some("while"),
        "try_statement" => Some("try"),
        "catch_clause" => Some("catch"),
        "throw_statement" => Some("throw"),
        "using_statement" => Some("using"),
        "invocation_expression" => Some("call"),
        "member_access_expression" => Some("member"),
        "object_creation_expression" => Some("new"),
        "assignment_expression" => Some("assign"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        // conditional_expression handled above
        "lambda_expression" => Some("lambda"),
        "await_expression" => Some("await"),
        "variable_declaration" => Some(VARIABLE),
        "variable_declarator" => Some(DECLARATOR),
        // local_declaration_statement is flattened (handled above); the
        // inner variable_declaration already becomes <variable>.
        "base_list" => Some("extends"),
        "type_parameter" => Some("generic"),
        "enum_member_declaration" => Some("constant"),
        "string_literal" => Some("string"),
        "integer_literal" => Some("int"),
        "real_literal" => Some("float"),
        "boolean_literal" => Some("bool"),
        "null_literal" => Some("null"),
        "attribute_list" => Some(ATTRIBUTES),
        "attribute" => Some(ATTRIBUTE),
        "attribute_argument_list" => Some(ARGUMENTS),
        "attribute_argument" => Some(ARGUMENT),
        "accessor_list" => Some(ACCESSORS),
        "accessor_declaration" => Some(ACCESSOR),
        "using_directive" => Some(IMPORT),
        // C# 8+ switch expression rules/labels — normalise to the
        // shared vocabulary (`<case>` like other languages).
        "switch_rule" => Some("arm"),
        "switch_label" => Some("label"),
        "switch_section" => Some("section"),
        "element_binding_expression" => Some("index"),
        "declaration_pattern" => Some("pattern"),
        "switch_expression_arm" => Some("arm"),
        "operator_declaration" => Some("operator"),
        "is_pattern_expression" => Some("is"),
        "implicit_object_creation_expression" => Some("new"),
        "event_field_declaration" => Some("event"),
        "constructor_initializer" => Some("chain"),
        "tuple_element" => Some("element"),
        "recursive_pattern" => Some("pattern"),
        "property_pattern_clause" => Some("properties"),
        "member_binding_expression" => Some("member"),
        "implicit_parameter" => Some("parameter"),
        "break_statement" => Some("break"),
        "continue_statement" => Some("continue"),
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

/// Classify an identifier as "name" or "type" based on context
/// Uses get_kind() for robust parent detection
fn classify_identifier(xot: &Xot, node: XotNode) -> &'static str {
    // Check if this identifier has field="type" attribute (e.g., parameter type)
    if let Some(field) = get_attr(xot, node, "field") {
        if field == "type" {
            return "type";
        }
    }

    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return "type",  // Default for C#
    };

    let parent_kind = get_kind(xot, parent).unwrap_or_default();

    // If parent is a field wrapper (like <name>), check grandparent
    // TreeSitter wraps identifiers in field elements like: <name><identifier>Foo</identifier></name>
    if parent_kind == "name" {
        if let Some(grandparent) = get_parent(xot, parent) {
            let grandparent_kind = get_kind(xot, grandparent).unwrap_or_default();
            // If grandparent is a declaration, this identifier IS the name
            if is_named_declaration(&grandparent_kind) {
                return "name";
            }
        }
    }

    // Check if in namespace declaration path
    let in_namespace = is_in_namespace_context(xot, node);
    if parent_kind == "qualified_name" && in_namespace {
        return "name";
    }

    // Check if followed by parameter list (method/ctor name)
    let siblings = get_following_siblings(xot, node);
    let has_param_sibling = siblings.iter().any(|&s| {
        get_kind(xot, s)
            .map(|n| matches!(n.as_str(), "parameter_list" | "parameters"))
            .unwrap_or(false)
    });

    match parent_kind.as_str() {
        // Method/constructor names followed by params
        "method_declaration" | "constructor_declaration" if has_param_sibling => "name",

        // Type declarations - the identifier IS the name
        "class_declaration" | "struct_declaration" | "interface_declaration"
        | "enum_declaration" | "record_declaration" | "namespace_declaration" => "name",

        // Variable declarator - the identifier is the name
        "variable_declarator" => "name",

        // Parameter - the identifier is the parameter name
        "parameter" => "name",

        // Generic name - the identifier is the generic type name
        "generic_name" => "type",

        // Type annotations - use type
        "type_argument_list" | "type_parameter" => "type",

        // Base list (`class Foo : Bar, IBaz`) — each entry is a type
        // reference (base class or interface). Classifying as "type"
        // means the identifier becomes `<type>` and gets its text
        // wrapped in `<name>` by `wrap_text_in_name`, producing
        // `<extends><type><name>Bar</name></type>...</extends>`.
        "base_list" => "type",

        // Default: all other identifiers are <name>. The post-transform
        // pass marks each <name> as <bind/> or <use/> by context, so we
        // no longer need a separate <ref> element for value references.
        // See Principle #13.
        _ => "name",
    }
}

/// Check if node is in a namespace declaration context
fn is_in_namespace_context(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_kind(xot, parent) {
            match kind.as_str() {
                "namespace_declaration" => return true,
                // Stop if we hit a type declaration
                "class_declaration" | "struct_declaration" | "interface_declaration"
                | "enum_declaration" | "record_declaration" => return false,
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

// =============================================================================
// Comment attachment helpers
// =============================================================================

/// Check if a comment (or comment block) immediately precedes a non-comment sibling.
/// "Immediately" means the next non-comment element sibling starts on the line
/// right after this comment ends, with no blank-line gap.
fn is_leading_comment(xot: &Xot, node: XotNode) -> bool {
    let comment_end_line = match get_line(xot, node, "end_line") {
        Some(l) => l,
        None => return false,
    };

    // Find next element sibling that is NOT a comment (skip self — following_siblings includes node)
    let next = xot.following_siblings(node)
        .filter(|&s| s != node)
        .find(|&s| {
            xot.element(s).is_some()
                && get_kind(xot, s).as_deref() != Some("comment")
        });

    match next {
        Some(next) => {
            let next_start_line = get_line(xot, next, "line").unwrap_or(0);
            // Next declaration starts on the very next line (no blank line gap)
            next_start_line == comment_end_line + 1
        }
        None => false,
    }
}

/// Group consecutive `//` line comments on adjacent lines into a single comment node.
///
/// Merges the text content of following comment siblings into `node` and returns
/// the consumed sibling nodes (caller should detach them after classification).
///
/// Only groups `//` style comments (not `/* */` block comments).
fn group_line_comments(xot: &mut Xot, node: XotNode) -> Result<Vec<XotNode>, xot::Error> {
    let text = match get_text_content(xot, node) {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };

    // Only group line comments (start with //)
    let trimmed = text.trim();
    if !trimmed.starts_with("//") {
        return Ok(Vec::new());
    }

    let mut end_line = match get_line(xot, node, "end_line") {
        Some(l) => l,
        None => return Ok(Vec::new()),
    };
    let mut end_column = get_attr(xot, node, "end_column")
        .unwrap_or_else(|| "1".to_string());

    let mut consumed: Vec<XotNode> = Vec::new();
    let mut merged_text = text.clone();

    // Walk following siblings looking for adjacent // comments (skip self)
    let following: Vec<XotNode> = xot.following_siblings(node)
        .filter(|&s| s != node && xot.element(s).is_some())
        .collect();

    for sibling in following {
        let sibling_kind = match get_kind(xot, sibling) {
            Some(k) => k,
            None => break,
        };
        if sibling_kind != "comment" {
            break;
        }

        let sibling_text = match get_text_content(xot, sibling) {
            Some(t) => t,
            None => break,
        };

        // Must also be a // comment
        if !sibling_text.trim().starts_with("//") {
            break;
        }

        let sibling_start_line = match get_line(xot, sibling, "line") {
            Some(l) => l,
            None => break,
        };

        // Must be on the very next line (adjacent)
        if sibling_start_line != end_line + 1 {
            break;
        }

        // Merge: append text with newline
        merged_text.push('\n');
        merged_text.push_str(&sibling_text);

        // Update end line to the consumed sibling's end
        end_line = get_line(xot, sibling, "end_line").unwrap_or(end_line + 1);
        end_column = get_attr(xot, sibling, "end_column")
            .unwrap_or_else(|| end_column.clone());

        consumed.push(sibling);
    }

    if !consumed.is_empty() {
        // Replace text content of node with merged text
        // Remove existing text children
        let text_children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.text_str(c).is_some())
            .collect();
        for child in text_children {
            xot.detach(child)?;
        }
        // Add merged text
        let new_text = xot.new_text(&merged_text);
        xot.append(node, new_text)?;

        // Update end attribute to reflect the last consumed comment
        set_attr(xot, node, "end_line", &end_line.to_string());
        set_attr(xot, node, "end_column", &end_column);
    }

    Ok(consumed)
}

/// Map a transformed element name to a syntax category for highlighting
/// This is called by the highlighter to determine what color to use
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers and references
        "name" => SyntaxCategory::Identifier,
        "ref" => SyntaxCategory::Identifier,

        // Types
        "type" => SyntaxCategory::Type,
        "implicit_type" => SyntaxCategory::Type,  // var keyword in C#
        "generic" => SyntaxCategory::Type,
        "nullable" => SyntaxCategory::Type,
        "array" => SyntaxCategory::Type,

        // Literals
        "string" => SyntaxCategory::String,
        "int" => SyntaxCategory::Number,
        "float" => SyntaxCategory::Number,
        "bool" => SyntaxCategory::Keyword,
        "null" => SyntaxCategory::Keyword,

        // Keywords - declarations (actual keyword tokens, not structural wrappers)
        "class" | "struct" | "interface" | "enum" | "record" | "namespace" => SyntaxCategory::Keyword,
        "import" => SyntaxCategory::Keyword,

        // Note: "method", "constructor", "property", "field", "parameter", "variable",
        // "local", "declarator" are structural wrappers, not keywords. Leave as Default
        // so punctuation inside them doesn't get colored.

        // Keywords - control flow
        "if" | "else" | "for" | "foreach" | "while" | "do" => SyntaxCategory::Keyword,
        "switch" | "case" | "default" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" | "goto" | "yield" => SyntaxCategory::Keyword,
        "using" | "lock" => SyntaxCategory::Keyword,

        // Keywords - modifiers (these become empty elements like <public/>)
        "public" | "private" | "protected" | "internal" => SyntaxCategory::Keyword,
        "static" | "abstract" | "virtual" | "override" | "sealed" => SyntaxCategory::Keyword,
        "readonly" | "const" | "volatile" => SyntaxCategory::Keyword,
        "async" | "await" => SyntaxCategory::Keyword,
        "partial" | "extern" | "unsafe" => SyntaxCategory::Keyword,
        "new" | "this" | "base" => SyntaxCategory::Keyword,

        // Functions/calls - lambda gets Function color, but call/member are structural
        // (the actual function name is a ref/name inside, which gets Identifier color)
        "lambda" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Attributes
        "attribute" | "attributes" => SyntaxCategory::Type,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{parse_string_to_xee, parse_string_to_xot};
    use crate::output::{render_document, RenderOptions};
    use crate::XPathEngine;

    #[test]
    fn test_csharp_transform() {
        let source = r#"
public class Foo {
    public void Bar() { }
}
"#;
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Check transforms applied
        assert!(xml.contains("<class"), "class_declaration should be renamed");
        assert!(xml.contains("<method"), "method_declaration should be renamed");
        assert!(xml.contains("<public"), "public modifier should be extracted");
    }

    // =========================================================================
    // Comment attachment tests
    // =========================================================================

    #[test]
    fn test_trailing_comment() {
        let source = "public class Foo {\n    int x; // trailing\n}\n";
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();
        let xml = render_document(&result.xot, result.root, &RenderOptions::default());
        assert!(
            xml.contains("<trailing/>"),
            "same-line comment should get <trailing/> marker, got:\n{}", xml
        );
    }

    #[test]
    fn test_leading_comment() {
        let source = "public class Foo {\n    // describes y\n    int y;\n}\n";
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();
        let xml = render_document(&result.xot, result.root, &RenderOptions::default());
        assert!(
            xml.contains("<leading/>"),
            "comment above declaration should get <leading/> marker, got:\n{}", xml
        );
    }

    #[test]
    fn test_floating_comment() {
        // Comment with blank line before next declaration = floating (no marker)
        let source = "public class Foo {\n    // floating\n\n    int y;\n}\n";
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();
        let xml = render_document(&result.xot, result.root, &RenderOptions::default());
        assert!(
            !xml.contains("<trailing/>") && !xml.contains("<leading/>"),
            "floating comment should have no marker, got:\n{}", xml
        );
        assert!(xml.contains("<comment>"), "comment should still be present");
    }

    #[test]
    fn test_comment_block_grouping() {
        let source = "public class Foo {\n    // line 1\n    // line 2\n    // line 3\n    int y;\n}\n";
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();
        let xml = render_document(&result.xot, result.root, &RenderOptions::default());
        // Should be grouped into a single comment
        let comment_count = xml.matches("<comment>").count() + xml.matches("<comment ").count();
        assert_eq!(
            comment_count, 1,
            "three adjacent // comments should be grouped into one, got {} comments in:\n{}", comment_count, xml
        );
        // Should contain all lines
        assert!(xml.contains("// line 1"), "merged comment should contain line 1");
        assert!(xml.contains("// line 3"), "merged comment should contain line 3");
        // Should be leading (immediately before int y)
        assert!(xml.contains("<leading/>"), "grouped comment block should be leading");

        let mut parsed = parse_string_to_xee(source, "csharp", "<test>".to_string(), None).unwrap();
        let engine = XPathEngine::new();
        let matches = engine.query_documents(
            &mut parsed.documents,
            parsed.doc_handle,
            "//comment",
            parsed.source_lines.clone(),
            "<test>",
        ).unwrap();
        assert_eq!(matches.len(), 1, "grouped comments should query as a single match");
        assert_eq!(
            matches[0].extract_source_snippet(),
            "// line 1\n    // line 2\n    // line 3".to_string(),
            "grouped comment should extract the full merged source span"
        );
    }

    #[test]
    fn test_trailing_not_grouped_with_following() {
        // Trailing comment should NOT absorb the following line comments
        let source = "public class Foo {\n    int x; // trailing\n    // block 1\n    // block 2\n    int y;\n}\n";
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();
        let xml = render_document(&result.xot, result.root, &RenderOptions::default());
        // Should have 2 comments: one trailing, one grouped leading block
        let comment_count = xml.matches("<comment>").count() + xml.matches("<comment ").count();
        assert_eq!(
            comment_count, 2,
            "should have trailing + grouped block = 2 comments, got {} in:\n{}", comment_count, xml
        );
        assert!(xml.contains("<trailing/>"), "first comment should be trailing");
        assert!(xml.contains("<leading/>"), "block comment should be leading");
        // Block should contain both lines
        assert!(xml.contains("// block 1"), "block should contain line 1");
        assert!(xml.contains("// block 2"), "block should contain line 2");
    }

    #[test]
    fn test_block_comment_not_grouped() {
        // /* */ style comments should NOT be grouped with // comments
        let source = "public class Foo {\n    /* block */\n    // line\n    int y;\n}\n";
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();
        let xml = render_document(&result.xot, result.root, &RenderOptions::default());
        let comment_count = xml.matches("<comment>").count() + xml.matches("<comment ").count();
        assert!(
            comment_count >= 2,
            "/* */ and // comments should not be grouped, got {} comments in:\n{}", comment_count, xml
        );
    }

    #[test]
    fn test_leading_comment_at_unit_level() {
        // Comment at compilation_unit level, before a class
        let source = "// describes Foo\npublic class Foo { }\n";
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();
        let xml = render_document(&result.xot, result.root, &RenderOptions::default());
        assert!(
            xml.contains("<leading/>"),
            "top-level comment before class should be leading, got:\n{}", xml
        );
    }

    // =========================================================================

    #[test]
    fn test_extension_method_this_modifier() {
        let source = r#"
public static class Mapper {
    public static UserDto Map(this User user) { return new UserDto(); }
}
"#;
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), None).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Empty marker with the source keyword kept as a dangling
        // sibling text node so `-v value` preserves "this" in the
        // enclosing declaration's XPath string-value. The marker
        // itself stays empty (Principle #7).
        assert!(
            xml.contains("<this/>this"),
            "this modifier should be converted to <this/> marker with source keyword as sibling, got: {}",
            xml
        );
        assert!(!xml.contains("<modifier>this</modifier>"), "this should not remain as <modifier>this</modifier>");
    }
}
