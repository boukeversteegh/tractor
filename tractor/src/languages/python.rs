//! Python transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Semantic element names — tractor's Python XML vocabulary shared with the renderer.
pub mod semantic {
    // Top-level / structural
    pub const MODULE: &str = "module";
    pub const IMPORT: &str = "import";
    pub const BODY: &str = "body";

    // Declarations
    pub const CLASS: &str = "class";
    pub const FUNCTION: &str = "function";
    pub const METHOD: &str = "method";
    pub const FIELD: &str = "field";
    pub const COMMENT: &str = "comment";

    // Members / shared children
    pub const NAME: &str = "name";
    pub const TYPE: &str = "type";
    pub const PARAMETERS: &str = "parameters";
    pub const PARAMETER: &str = "parameter";
    pub const RETURNS: &str = "returns";
    pub const DEFAULT: &str = "default";
    pub const BASE: &str = "base";
    pub const REF: &str = "ref";
    pub const DECORATORS: &str = "decorators";
    pub const DECORATOR: &str = "decorator";

    // Type markers
    pub const OPTIONAL: &str = "optional";
    pub const LIST: &str = "list";
}

/// Transform a Python AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        "expression_statement" => Ok(TransformAction::Skip),
        "block" => Ok(TransformAction::Flatten),

        // Name wrappers created by the builder for field="name".
        // Inline the single identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        "name" => {
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Type wrappers from Python's tree-sitter grammar contain a single
        // identifier — inline it so the result is `<type>int</type>`.
        "type" => {
            inline_single_identifier(xot, node)?;
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

        // Annotated assignments — `name: T` or `name: T = default` — are the
        // Python way to declare a field. Rewrite to the semantic `<field>`
        // shape so queries and the renderer treat them like declarations
        // rather than expression statements.
        //
        //   assignment       name
        //   ├─ left          type
        //   │  └─ identifier default?
        //   ├─ type
        //   └─ right?
        "assignment" => {
            if has_child_element(xot, node, "type") {
                rewrite_annotated_assignment_as_field(xot, node)?;
            } else {
                rename(xot, node, "assign");
            }
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
        "parameters" => Some("params"),
        "default_parameter" | "typed_parameter" | "typed_default_parameter" => Some("param"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "elif_clause" => Some("elif"),
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
        "call" => Some("call"),
        "attribute" => Some("member"),
        "subscript" => Some("subscript"),
        "assignment" => Some("assign"),
        "augmented_assignment" => Some("augassign"),
        "binary_operator" => Some("binary"),
        "unary_operator" => Some("unary"),
        "comparison_operator" => Some("compare"),
        "boolean_operator" => Some("logical"),
        "conditional_expression" => Some("ternary"),
        "lambda" => Some("lambda"),
        "await" => Some("await"),
        "list_comprehension" => Some("listcomp"),
        "dictionary_comprehension" => Some("dictcomp"),
        "set_comprehension" => Some("setcomp"),
        "generator_expression" => Some("genexp"),
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

/// Check whether `node` has a direct child element with the given name.
fn has_child_element(xot: &Xot, node: XotNode, name: &str) -> bool {
    xot.children(node)
        .any(|c| get_element_name(xot, c).as_deref() == Some(name))
}

fn find_child_element(xot: &Xot, node: XotNode, name: &str) -> Option<XotNode> {
    xot.children(node)
        .find(|c| get_element_name(xot, *c).as_deref() == Some(name))
}

/// Rewrite an annotated assignment into a `<field>` by lifting `<left>`'s
/// inner identifier directly, renaming `<right>` to `<default>`, and dropping
/// the `<left>` wrapper. The surrounding `<type>` child is preserved in place,
/// and subsequent identifier-rename walks turn the lifted identifier into
/// `<name>` naturally.
fn rewrite_annotated_assignment_as_field(
    xot: &mut Xot,
    node: XotNode,
) -> Result<(), xot::Error> {
    rename(xot, node, "field");

    // Lift the identifier out of <left> so it becomes a direct child of the
    // field. The identifier-rename pass visits it afterwards and renames it
    // to <name>.
    if let Some(left) = find_child_element(xot, node, "left") {
        let inner: Vec<_> = xot
            .children(left)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in inner {
            xot.detach(child)?;
            xot.insert_before(left, child)?;
        }
        xot.detach(left)?;
    }

    // Rename <right>…</right> to <default>…</default>. Queries can then match
    // `field/default` consistently whether the default came from a literal,
    // a call, or another expression.
    if let Some(right) = find_child_element(xot, node, "right") {
        rename(xot, right, "default");
    }

    Ok(())
}

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        if get_element_name(xot, child).as_deref() != Some("identifier") {
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
        "param" | "params" => SyntaxCategory::Keyword,
        "import" | "from" => SyntaxCategory::Keyword,
        "decorated" | "decorator" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "elif" | "else" => SyntaxCategory::Keyword,
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
        "assign" | "augassign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Comprehensions
        "listcomp" | "dictcomp" | "setcomp" | "genexp" => SyntaxCategory::Keyword,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
