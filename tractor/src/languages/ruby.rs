//! Ruby transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's Ruby XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
pub mod semantic {
    // Structural — containers.

    // Top-level / declarations
    pub const PROGRAM: &str = "program";
    pub const MODULE: &str = "module";
    pub const CLASS: &str = "class";
    pub const METHOD: &str = "method";

    // Statements / control flow
    pub const IF: &str = "if";
    pub const UNLESS: &str = "unless";
    pub const ELSE: &str = "else";
    pub const ELSE_IF: &str = "else_if";
    pub const CASE: &str = "case";
    pub const WHILE: &str = "while";
    pub const UNTIL: &str = "until";
    pub const FOR: &str = "for";
    pub const BEGIN: &str = "begin";
    pub const RESCUE: &str = "rescue";
    pub const ENSURE: &str = "ensure";
    pub const BREAK: &str = "break";
    pub const CONTINUE: &str = "continue";

    // Members / parameters
    pub const PARAMETER: &str = "parameter";
    pub const VARIABLE: &str = "variable";

    // Expressions
    pub const CALL: &str = "call";
    pub const ASSIGN: &str = "assign";
    pub const BINARY: &str = "binary";
    pub const SPREAD: &str = "spread";
    pub const LEFT: &str = "left";

    // Collections / atoms
    pub const ARRAY: &str = "array";
    pub const HASH: &str = "hash";
    pub const STRING: &str = "string";
    pub const SYMBOL: &str = "symbol";
    pub const INT: &str = "int";
    pub const FLOAT: &str = "float";

    // Identifiers
    pub const NAME: &str = "name";

    // Markers — always empty when emitted.

    // Array-shape markers (for %w[…] / %i[…] percent-literals).
    //   - STRING also doubles as a structural container.
    //   - SYMBOL also doubles as a structural container.
    // Both stay as constants but are OMITTED from MARKER_ONLY.

    // Spread-shape markers (`*args` vs `**kwargs`).
    pub const LIST: &str = "list";
    pub const DICT: &str = "dict";

    // Parameter-shape markers.
    pub const KEYWORD: &str = "keyword";
    //   - BLOCK doubles as `<parameter><block/>` marker AND `<block>` container
    //     (do/begin blocks). Kept as a constant but NOT in MARKER_ONLY.
    //   - DEFAULT (optional parameter) doesn't collide with a container here
    //     but stays consistent with other languages: keep it a marker.
    pub const DEFAULT: &str = "default";

    // Block-shape markers (do … end vs begin … end).
    //   - DO is marker-only.
    //   - BEGIN doubles as `<begin>` structural container AND `<block><begin/>`
    //     marker. Kept as constant but OMITTED from MARKER_ONLY.
    pub const DO: &str = "do";

    // Symbol-shape marker (`:"dyn#{foo}"` → `<symbol><delimited/>`).
    pub const DELIMITED: &str = "delimited";

    // Class / method singleton markers (`class << self`, `def self.foo`).
    pub const SINGLETON: &str = "singleton";

    // Block parameter marker (`&block`).
    pub const BLOCK: &str = "block";

    // Ambiguous names — emitted as BOTH structural container AND marker
    // in different contexts. Kept as constants for type-safety but NOT in
    // MARKER_ONLY:
    //   - STRING: `<string>` literal AND `<array><string/>` shape marker.
    //   - SYMBOL: `<symbol>` literal AND `<array><symbol/>` shape marker.
    //   - BLOCK: `<block>` container (do_block/begin_block) AND
    //     `<parameter><block/>` shape marker.
    //   - BEGIN: `<begin>` container AND `<block><begin/>` marker.
    //   - DO: marker on `<block><do/>` AND structural container — the
    //     Ruby grammar has a `do` kind used as the body of `while` /
    //     `until` / `for` loops.

    /// Names that, when emitted, are always empty elements (no text,
    /// no element children). Used by the markers-stay-empty invariant.
    pub const MARKER_ONLY: &[&str] = &[
        LIST, DICT,
        KEYWORD, DEFAULT,
        DELIMITED,
        SINGLETON,
    ];

    /// Every semantic name this language's transform can emit.
    pub const ALL_NAMES: &[&str] = &[
        PROGRAM, MODULE, CLASS, METHOD,
        IF, UNLESS, ELSE, ELSE_IF, CASE, WHILE, UNTIL, FOR,
        BEGIN, RESCUE, ENSURE, BREAK, CONTINUE,
        PARAMETER, VARIABLE,
        CALL, ASSIGN, BINARY, SPREAD, LEFT,
        ARRAY, HASH, STRING, SYMBOL, INT, FLOAT,
        NAME,
        LIST, DICT,
        KEYWORD, DEFAULT,
        DO,
        DELIMITED,
        SINGLETON,
        BLOCK,
    ];
}

/// Transform a Ruby AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        "body_statement"
        | "parenthesized_statements"
        | "block_body"
        | "heredoc_content"
        | "heredoc_beginning"
        | "heredoc_body"
        | "heredoc_end"
        | "hash_key_symbol"
        | "block_parameters"
        // `->(...)` lambda parameter list — grouping wrapper; flatten
        // so the parameters bubble up to the `<lambda>`.
        | "lambda_parameters" => Ok(TransformAction::Flatten),

        // Hash/keyword parameter syntax handled via map_element_name
        // with markers — `**kwargs` → `<spread><dict/>`, `key:` → parameter+keyword.

        // Trailing `if` / `unless` modifier — still a conditional,
        // same vocabulary as the full form.
        "if_modifier" => {
            rename(xot, node, IF);
            Ok(TransformAction::Continue)
        }
        "unless_modifier" => {
            rename(xot, node, UNLESS);
            Ok(TransformAction::Continue)
        }
        "while_modifier" => {
            rename(xot, node, WHILE);
            Ok(TransformAction::Continue)
        }
        "until_modifier" => {
            rename(xot, node, UNTIL);
            Ok(TransformAction::Continue)
        }

        // Ruby instance / class / global variables (`@x`, `@@y`, `$z`)
        // are distinct node kinds in the grammar but they're all
        // "variable references" at the semantic layer. Render as
        // `<name>` — the leading sigil survives as text so the
        // source is preserved. A future refinement could add a
        // `<instance/>` / `<class/>` / `<global/>` marker child.
        "instance_variable" | "class_variable" | "global_variable" => {
            rename(xot, node, NAME);
            Ok(TransformAction::Continue)
        }

        // String internals — grammar wrappers around the literal
        // text. Flatten so `<string>` reads as text + interpolations
        // (Principle #12).
        "string_content"
        | "escape_sequence"
        | "simple_symbol"
        | "bare_string"
        | "bare_symbol" => Ok(TransformAction::Flatten),

        // Flat lists (Principle #12)
        "method_parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "argument_list" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Ruby's grammar has no type_identifier — every identifier is a
        // value reference, so the rename is unconditional. Matches Python
        // and the rest of the languages on the value-namespace side
        // (Principle #14).
        "identifier" => {
            rename(xot, node, NAME);
            Ok(TransformAction::Continue)
        }

        // Name wrappers - inline identifier text directly
        // Inline the single identifier/constant child into plain
        // text. Applies everywhere a `<name>` field wrapper wraps a
        // single renamable child — declarations (method/class/module)
        // AND references (singleton method, call receiver, etc.) —
        // so the design-doc "identifiers are a single <name> text
        // leaf" rule holds uniformly.
        "name" => {
            let children: Vec<_> = xot.children(node).collect();
            let element_children: Vec<_> = children
                .iter()
                .copied()
                .filter(|&c| xot.element(c).is_some())
                .collect();
            if element_children.len() == 1 {
                let child = element_children[0];
                let child_name = get_element_name(xot, child).unwrap_or_default();
                // `identifier` for methods, `constant` for classes/
                // modules (Ruby uses constant for capitalized
                // identifiers); `operator` for `def ==(other)` and
                // friends — Ruby's tree-sitter grammar tags the
                // operator token as an element inside `<name>`.
                // Also accept already-renamed <name> when walk order
                // leaves one around.
                if matches!(child_name.as_str(), "identifier" | "constant" | "name" | "operator") {
                    if let Some(text) = get_text_content(xot, child) {
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

        _ => {
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }
    }
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Second tuple element is an optional disambiguation marker — lets
/// the map declare inline that e.g. `string_array` renames to
/// `<array>` with a `<string/>` marker child so `//array[string]`
/// finds every `%w[…]` literal.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        "program" => Some((PROGRAM, None)),
        "method" => Some((METHOD, None)),
        "class" => Some((CLASS, None)),
        "module" => Some((MODULE, None)),
        "if" => Some((IF, None)),
        "unless" => Some((UNLESS, None)),
        // Ruby's tree-sitter nests `elsif` chains (each `elsif`/`else`
        // lives inside the previous `elsif`). The post-transform in
        // `languages/mod.rs` lifts them to flat children of `<if>` per
        // the cross-cutting conditional shape; here we just rename.
        "elsif" => Some((ELSE_IF, None)),
        "else" => Some((ELSE, None)),
        "case" => Some((CASE, None)),
        "while" => Some((WHILE, None)),
        "until" => Some((UNTIL, None)),
        "for" => Some((FOR, None)),
        "begin" => Some((BEGIN, None)),
        "rescue" => Some((RESCUE, None)),
        "ensure" => Some((ENSURE, None)),
        "call" => Some((CALL, None)),
        "method_call" => Some((CALL, None)),
        "assignment" => Some((ASSIGN, None)),
        "binary" => Some((BINARY, None)),
        "string" => Some((STRING, None)),
        "integer" => Some((INT, None)),
        "float" => Some((FLOAT, None)),
        "symbol" => Some((SYMBOL, None)),
        "array" => Some((ARRAY, None)),
        "hash" => Some((HASH, None)),
        "operator_assignment" => Some((ASSIGN, None)),
        "break_statement" => Some((BREAK, None)),
        "continue_statement" | "next_statement" => Some((CONTINUE, None)),
        // Percent-literal arrays — %w[…] gives a string_array, %i[…]
        // gives a symbol_array. Both collapse to <array> with a shape
        // marker so the element kind is uniform but queryable.
        "string_array" => Some((ARRAY, Some(STRING))),
        "symbol_array" => Some((ARRAY, Some(SYMBOL))),
        // Splat parameters — `*args` vs `**kwargs` distinguished by
        // list/dict marker, matching Python's shape.
        "splat_parameter" => Some((SPREAD, Some(LIST))),
        "hash_splat_parameter" => Some((SPREAD, Some(DICT))),
        // `key:` keyword parameter — a parameter variant; the marker
        // lets us find every keyword parameter without matching on text.
        "keyword_parameter" => Some((PARAMETER, Some(KEYWORD))),
        // `&block` capture — identifies the block parameter as a
        // <parameter> with <block/> marker.
        "block_parameter" => Some((PARAMETER, Some(BLOCK))),
        // `arg = default` — optional parameter shape.
        "optional_parameter" => Some((PARAMETER, Some(DEFAULT))),
        // `do … end` block — collapse to <block> with a <do/> marker
        // so `//block[do]` finds the do-style, `//block[brace]` the
        // `{ … }` style (once we add it).
        "do_block" => Some((BLOCK, Some(DO))),
        // `begin … end` — explicit Ruby block.
        "begin_block" => Some((BLOCK, Some(BEGIN))),
        // Splat call-site args — `*args` / `**kwargs` distinguished by
        // list/dict marker, matching the parameter shape above.
        "splat_argument" => Some((SPREAD, Some(LIST))),
        "hash_splat_argument" => Some((SPREAD, Some(DICT))),
        // `:"dyn#{foo}"` delimited symbol — shape-marker the collapse
        // so `//symbol[delimited]` finds them.
        "delimited_symbol" => Some((SYMBOL, Some(DELIMITED))),
        // `rescue => e` — the `=> e` binding the exception.
        "exception_variable" => Some((VARIABLE, None)),
        // `class << self` — singleton class body.
        "singleton_class" => Some((CLASS, Some(SINGLETON))),
        // `def self.foo` — singleton method.
        "singleton_method" => Some((METHOD, Some(SINGLETON))),
        // `a, b, c = …` — LHS of a multi-assignment.
        "left_assignment_list" => Some((LEFT, None)),
        // `*rest = …` — the splat position on the LHS.
        "rest_assignment" => Some((SPREAD, None)),
        // `->(...) { ... }` — Ruby lambdas; flatten the parameter list.
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

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,

        // Literals
        "string" => SyntaxCategory::String,
        "int" | "float" => SyntaxCategory::Number,
        "symbol" => SyntaxCategory::String,
        "true" | "false" | "nil" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "module" | "method" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "unless" | "else" | "else_if" => SyntaxCategory::Keyword,
        "case" | "when" => SyntaxCategory::Keyword,
        "while" | "until" | "for" => SyntaxCategory::Keyword,
        "begin" | "rescue" | "ensure" | "raise" => SyntaxCategory::Keyword,
        "return" | "break" | "next" | "redo" | "retry" => SyntaxCategory::Keyword,
        "yield" => SyntaxCategory::Keyword,

        // Keywords - other
        "def" | "end" | "do" => SyntaxCategory::Keyword,
        "self" | "super" => SyntaxCategory::Keyword,

        // Collections
        "array" | "hash" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
