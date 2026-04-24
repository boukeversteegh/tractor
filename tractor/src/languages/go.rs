//! Go transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a Go AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        "expression_statement" => Ok(TransformAction::Skip),
        "block" => Ok(TransformAction::Flatten),
        // Struct/interface field lists and miscellaneous groupings —
        // drop the wrappers so each field/spec is a direct child of
        // the enclosing struct/interface/declaration (Principle #12).
        "field_declaration_list" | "expression_list" => {
            Ok(TransformAction::Flatten)
        }
        // Import declarations wrap one or more `import_spec` nodes in
        // parens. Collapse the spec wrapper so each import path is a
        // direct child of the <import>; rename the outer declaration
        // to `import` below via map_element_name.
        //
        // `const_spec` / `var_spec` are the same pattern — a grammar
        // wrapper around `name = value`. Flatten so the declaration
        // reads as `<const>const<name>x</name>=<value>1</value></const>`
        // rather than nesting the assignment inside an opaque spec
        // element.
        "import_spec" | "const_spec" | "var_spec" => Ok(TransformAction::Flatten),

        // Composite literal elements — `literal_element` /
        // `keyed_element` / `literal_value` are grammar wrappers
        // around individual list/map values. Flatten so they become
        // direct siblings inside the enclosing `<literal>`
        // (Principle #12).
        "literal_element" | "keyed_element" | "literal_value" => Ok(TransformAction::Flatten),

        // More Go generic / constraint wrappers — all grammar-only.
        "type_parameter_list" | "type_parameter_declaration"
        | "type_elem" | "type_constraint" | "qualified_type"
        | "type_case" => Ok(TransformAction::Flatten),
        // The content-inside-quotes node on "interpreted" strings —
        // inline as raw text into the enclosing <string>.
        "interpreted_string_literal_content" | "raw_string_literal_content" | "escape_sequence" => Ok(TransformAction::Flatten),

        // Type declarations: move the leading `type` keyword into the
        // inner `type_spec` so it renders as part of the <type> element,
        // then flatten the outer wrapper.
        "type_declaration" => {
            move_type_keyword_into_spec(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Flat lists (Principle #12)
        "parameter_list" => {
            // Go reuses `parameter_list` for both formal parameters AND
            // multi-value return specs. The builder has already wrapped
            // the returns case in a <returns> element (via the
            // `result`→`returns` field normalisation), so we can tell
            // which we're in by looking at the parent.
            let in_returns = get_parent(xot, node)
                .and_then(|p| get_element_name(xot, p))
                .as_deref() == Some("returns");
            if in_returns {
                // Each parameter_declaration here holds a return type.
                // Collapse `<param><type>X</type></param>` to just `<type>X</type>`
                // so the returns list reads as a sequence of types, not params.
                collapse_return_param_list(xot, node)?;
                Ok(TransformAction::Flatten)
            } else {
                distribute_field_to_children(xot, node, "parameters");
                Ok(TransformAction::Flatten)
            }
        }
        "argument_list" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Raw string literal — rename to <string> and prepend <raw/> marker
        "raw_string_literal" => {
            prepend_empty_element(xot, node, "raw")?;
            rename(xot, node, "string");
            Ok(TransformAction::Continue)
        }

        // Short variable declarations (`x := 42`) — render as <variable>
        // with a <short/> marker to distinguish from `var x = 42`.
        "short_var_declaration" => {
            prepend_empty_element(xot, node, "short")?;
            rename(xot, node, "variable");
            Ok(TransformAction::Continue)
        }

        // Name wrappers created by the builder for field="name".
        // Inline the single identifier/type_identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        "name" => {
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Declarations — add <exported/> or <unexported/> based on name capitalization
        "function_declaration" | "method_declaration" => {
            let marker = get_export_marker(xot, node);
            prepend_empty_element(xot, node, marker)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // Type declarations split three ways:
        //
        //   type Hello struct { … }    -> <struct><name>Hello</name>…</struct>
        //   type Greeter interface {…} -> <interface><name>Greeter</name>…</interface>
        //   type MyInt int             -> <type><name>MyInt</name><type>int</type></type>
        //
        // For struct/interface, hoist the inner shape up so a dev reads
        // "I'm declaring a struct named Hello" (Goal #5). The `<type>`
        // wrapper in the tree-sitter grammar is Go-spec terminology, not
        // developer mental model. For defined types over a plain type
        // reference, keep `<type>` — Go's own spec term — with the
        // underlying type as a nested `<type>` child.
        "type_spec" => {
            let marker = get_export_marker(xot, node);
            prepend_empty_element(xot, node, marker)?;

            let inner = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .find(|&c| matches!(
                    get_kind(xot, c).as_deref(),
                    Some("struct_type") | Some("interface_type"),
                ));

            if let Some(inner) = inner {
                let inner_kind = get_kind(xot, inner).unwrap();
                let new_name = if inner_kind == "struct_type" {
                    "struct"
                } else {
                    "interface"
                };
                rename(xot, node, new_name);
                // Hoist inner's children before the inner wrapper itself,
                // then drop the wrapper so the outer element owns them.
                let inner_children: Vec<_> = xot.children(inner).collect();
                for c in inner_children {
                    xot.detach(c)?;
                    xot.insert_before(inner, c)?;
                }
                xot.detach(inner)?;
            } else {
                rename(xot, node, "type");
            }
            Ok(TransformAction::Continue)
        }

        // Type alias declarations — `type Color = int`. Distinct from
        // `type MyInt int` (defined type), which creates a new distinct
        // type. Rename to <alias> — parallel with Rust / TS / C# / Java.
        "type_alias" => {
            let marker = get_export_marker(xot, node);
            prepend_empty_element(xot, node, marker)?;
            rename(xot, node, "alias");
            Ok(TransformAction::Continue)
        }

        // Go's tree-sitter doesn't emit an `else_clause` wrapper: the
        // `alternative` field of an if_statement points directly at
        // the nested if_statement (for `else if`) or a block (for
        // final `else {…}`). Wrap the alternative in `<else>`
        // surgically so the shared conditional-shape post-transform
        // can collapse the chain uniformly (same fix as Java / C#).
        "if_statement" => {
            wrap_field_child(xot, node, "alternative", "else")?;
            rename(xot, node, "if");
            Ok(TransformAction::Continue)
        }

        "binary_expression" | "unary_expression" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // Identifiers are always names (definitions or references).
        // Tree-sitter uses `type_identifier` for type positions, so bare
        // identifiers never need a heuristic — they are never types.
        "identifier" => {
            rename(xot, node, "name");
            Ok(TransformAction::Continue)
        }
        "type_identifier" => {
            rename(xot, node, "type");
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }

        _ => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

/// Move the literal `type` keyword text from a `type_declaration` into
/// its inner `type_spec` / `type_alias` child so the keyword stays
/// attached when the outer wrapper is flattened. Without this,
/// `type Foo struct { … }` becomes a free-floating `"type"` text node
/// sitting next to the bare spec/alias element at the file level.
fn move_type_keyword_into_spec(xot: &mut Xot, decl: XotNode) -> Result<(), xot::Error> {
    let keyword = match xot.children(decl)
        .find(|&c| xot.text_str(c).map(|t| t.trim() == "type").unwrap_or(false))
    {
        Some(k) => k,
        None => return Ok(()),
    };
    let spec = xot.children(decl)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| matches!(
            get_kind(xot, c).as_deref(),
            Some("type_spec") | Some("type_alias"),
        ));
    let spec = match spec {
        Some(s) => s,
        None => return Ok(()),
    };
    xot.detach(keyword)?;
    xot.prepend(spec, keyword)?;
    Ok(())
}

/// Rewrite each `parameter_declaration` in a return-type list to just its
/// inner type node, dropping the `<param>` wrapper so a returns list reads
/// as a sequence of types:
///
/// `<returns><param><type>int</type></param><param><type>error</type></param></returns>`
///   → `<returns><type>int</type><type>error</type></returns>`
fn collapse_return_param_list(xot: &mut Xot, list: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(list)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if get_kind(xot, child).as_deref() != Some("parameter_declaration") {
            continue;
        }
        let type_child = xot.children(child).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("type")
                || matches!(
                    get_kind(xot, c).as_deref(),
                    Some("type_identifier" | "pointer_type" | "slice_type" | "array_type"
                        | "map_type" | "channel_type" | "interface_type" | "struct_type"
                        | "generic_type")
                )
        });
        if let Some(type_node) = type_child {
            xot.detach(type_node)?;
            xot.insert_before(child, type_node)?;
            xot.detach(child)?;
        }
    }
    Ok(())
}

/// Determine exported/unexported based on name child's first character
fn get_export_marker(xot: &Xot, node: XotNode) -> &'static str {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "name" {
                // Look for identifier text inside the name wrapper
                for grandchild in xot.children(child) {
                    if let Some(text) = get_text_content(xot, grandchild) {
                        if text.starts_with(|c: char| c.is_uppercase()) {
                            return "exported";
                        }
                        return "unexported";
                    }
                }
                // Name wrapper might have text directly
                if let Some(text) = get_text_content(xot, child) {
                    if text.starts_with(|c: char| c.is_uppercase()) {
                        return "exported";
                    }
                    return "unexported";
                }
            }
            // Also check identifier/type_identifier directly (before name wrapping)
            if name == "identifier" || name == "type_identifier" {
                if let Some(field) = get_attr(xot, child, "field") {
                    if field == "name" {
                        if let Some(text) = get_text_content(xot, child) {
                            if text.starts_with(|c: char| c.is_uppercase()) {
                                return "exported";
                            }
                            return "unexported";
                        }
                    }
                }
            }
        }
    }
    "unexported" // default
}

fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "source_file" => Some("file"),
        "package_clause" => Some("package"),
        "function_declaration" => Some("function"),
        "method_declaration" => Some("method"),
        // type_declaration is flattened in the match above.
        "type_spec" => Some("type"),
        "struct_type" => Some("struct"),
        "interface_type" => Some("interface"),
        "const_declaration" => Some("const"),
        "var_declaration" => Some("var"),
        "import_declaration" => Some("import"),
        // parameter_list is flattened via Principle #12 above
        "parameter_declaration" => Some("parameter"),
        "method_elem" => Some("method"),
        "field_declaration" => Some("field"),
        "pointer_type" => Some("pointer"),
        "slice_type" => Some("slice"),
        "map_type" => Some("map"),
        "channel_type" => Some("chan"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "range_clause" => Some("range"),
        "switch_statement" => Some("switch"),
        "case_clause" => Some("case"),
        "default_case" => Some("default"),
        "defer_statement" => Some("defer"),
        "go_statement" => Some("go"),
        "select_statement" => Some("select"),
        "call_expression" => Some("call"),
        "selector_expression" => Some("member"),
        "index_expression" => Some("index"),
        "composite_literal" => Some("literal"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        "interpreted_string_literal" => Some("string"),
        // raw_string_literal is handled in the match above (rename + prepend <raw/>)
        "int_literal" => Some("int"),
        "float_literal" => Some("float"),
        "assignment_statement" => Some("assign"),
        "inc_statement" => Some("unary"),
        "dec_statement" => Some("unary"),
        "labeled_statement" => Some("labeled"),
        "label_name" => Some("label"),
        "send_statement" => Some("send"),
        "communication_case" => Some("case"),
        "receive_statement" => Some("receive"),
        "negated_type" => Some("type"),
        "true" => Some("true"),
        "false" => Some("false"),
        "nil" => Some("nil"),
        // `field_identifier` is a leaf — either the name of a struct field
        // or the method/field being accessed in a selector. Treat it as
        // `<name>` in both contexts (role inferred from tree position).
        "field_identifier" => Some("name"),
        "package_identifier" => Some("name"),
        _ => None,
    }
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

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };
        if !matches!(
            child_name.as_str(),
            "identifier" | "type_identifier" | "field_identifier",
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
        "true" | "false" | "nil" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "function" | "method" => SyntaxCategory::Keyword,
        "struct" | "interface" => SyntaxCategory::Keyword,
        "const" | "var" => SyntaxCategory::Keyword,
        "package" => SyntaxCategory::Keyword,
        "parameter" | "parameters" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" => SyntaxCategory::Keyword,
        "for" | "range" => SyntaxCategory::Keyword,
        "switch" | "case" | "default" => SyntaxCategory::Keyword,
        "select" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" | "goto" => SyntaxCategory::Keyword,
        "defer" | "go" => SyntaxCategory::Keyword,
        "exported" | "unexported" => SyntaxCategory::Keyword,

        // Types
        "pointer" | "slice" | "map" | "chan" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
