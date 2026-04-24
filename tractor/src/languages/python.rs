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
        "block" => Ok(TransformAction::Flatten),
        // Purely-grouping wrappers (Principle #12):
        //   as_pattern_target — the target of `with x as y` / `except E as y`.
        //   pattern_list — `a, b = ...` unpacking; drop wrapper so the
        //     underlying patterns are direct children of the assignment.
        //   expression_list — tuple-like returns/yields (`return x, y`).
        //     Drop the wrapper so expressions are direct children of the
        //     enclosing statement; matches Go's behavior.
        "as_pattern_target" | "pattern_list" | "expression_list" => Ok(TransformAction::Flatten),

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
        "name" => {
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
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // Function definitions — extract async modifier if present
        "function_definition" => {
            let texts = get_text_children(xot, node);
            if texts.iter().any(|t| t.contains("async")) {
                prepend_empty_element(xot, node, "async")?;
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
        "list" | "set" => {
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
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "module" => Some("module"),
        "class_definition" => Some("class"),
        "function_definition" => Some("function"),
        "decorated_definition" => Some("decorated"),
        "decorator" => Some("decorator"),
        // parameters is flattened via Principle #12 above
        "default_parameter" | "typed_parameter" | "typed_default_parameter" => Some("parameter"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "elif_clause" => Some("else_if"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "while_statement" => Some("while"),
        "try_statement" => Some("try"),
        "except_clause" => Some("except"),
        "finally_clause" => Some("finally"),
        "with_statement" => Some("with"),
        "raise_statement" => Some("raise"),
        "pass_statement" => Some("pass"),
        "import_statement" => Some("import"),
        "import_from_statement" => Some("from"),
        "list_splat_pattern" => Some("splat"),
        "dictionary_splat_pattern" => Some("kwsplat"),
        "as_pattern" => Some("as"),
        "for_in_clause" => Some("for"),
        "call" => Some("call"),
        "attribute" => Some("member"),
        "subscript" => Some("subscript"),
        "assignment" => Some("assign"),
        // augmented_assignment collapses to <assign>; the <op> child (e.g., +=) distinguishes it.
        "augmented_assignment" => Some("assign"),
        "binary_operator" => Some("binary"),
        "unary_operator" => Some("unary"),
        "comparison_operator" => Some("compare"),
        "boolean_operator" => Some("logical"),
        // conditional_expression handled above
        "lambda" => Some("lambda"),
        "await" => Some("await"),
        // Collection literals and comprehensions are handled specially
        // above (renamed to their produced type + <literal/> or
        // <comprehension/> marker).
        "generator_expression" => Some("generator"),
        "string" => Some("string"),
        "integer" => Some("int"),
        "float" => Some("float"),
        "true" => Some("true"),
        "false" => Some("false"),
        "none" => Some("none"),
        _ => None,
    }
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
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        // Also accept `type_identifier` — same rationale as TS/Java.
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
