//! Ruby transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's Ruby XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
pub mod semantic {
    use crate::languages::NodeSpec;
    use crate::output::syntax_highlight::SyntaxCategory;

    // Named constants retained for use by the transform code. The NODES
    // table below is the source of truth for marker/container role and
    // syntax category.

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
    pub const THEN: &str = "then";
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
    pub const UNARY: &str = "unary";
    pub const CONDITIONAL: &str = "conditional";
    pub const RANGE: &str = "range";
    pub const LAMBDA: &str = "lambda";
    pub const YIELD: &str = "yield";
    pub const SPREAD: &str = "spread";
    pub const LEFT: &str = "left";

    // Pattern-matching (case/in).
    pub const WHEN: &str = "when";
    pub const IN: &str = "in";
    pub const PATTERN: &str = "pattern";

    // Control-flow keyword leaves.
    pub const NEXT: &str = "next";
    pub const REDO: &str = "redo";
    pub const RETRY: &str = "retry";

    // Rescue / class header metadata.
    pub const EXCEPTIONS: &str = "exceptions";
    pub const SUPERCLASS: &str = "superclass";

    // Collections / atoms
    pub const ARRAY: &str = "array";
    pub const HASH: &str = "hash";
    pub const PAIR: &str = "pair";
    pub const STRING: &str = "string";
    pub const INTERPOLATION: &str = "interpolation";
    pub const SYMBOL: &str = "symbol";
    pub const INT: &str = "int";
    pub const FLOAT: &str = "float";
    pub const REGEX: &str = "regex";

    // Literal atoms.
    pub const TRUE: &str = "true";
    pub const FALSE: &str = "false";
    pub const NIL: &str = "nil";
    pub const SELF: &str = "self";

    // Identifiers
    pub const NAME: &str = "name";
    pub const CONSTANT: &str = "constant";
    pub const COMMENT: &str = "comment";

    // Spread-shape markers.
    pub const LIST: &str = "list";
    pub const DICT: &str = "dict";

    // Parameter-shape markers.
    pub const KEYWORD: &str = "keyword";
    pub const DEFAULT: &str = "default";

    // Block-shape / dual-use markers.
    pub const DO: &str = "do";

    // Symbol-shape marker.
    pub const DELIMITED: &str = "delimited";

    // Class / method singleton markers.
    pub const SINGLETON: &str = "singleton";

    // Dual-use (block container + `<parameter><block/>` marker).
    pub const BLOCK: &str = "block";

    use SyntaxCategory::*;

    /// Per-name metadata — single source of truth for every element
    /// name this language's transform can emit.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - STRING — `<string>` literal container + `<array><string/>` shape marker.
    ///   - SYMBOL — `<symbol>` literal container + `<array><symbol/>` shape marker.
    ///   - BLOCK  — `<block>` container (do/begin blocks) +
    ///              `<parameter><block/>` shape marker.
    ///   - BEGIN  — `<begin>` container + `<block><begin/>` marker.
    ///   - DO     — `<block><do/>` marker + structural `do` container
    ///              (body of while/until/for loops).
    pub const NODES: &[NodeSpec] = &[
        // Top-level / declarations
        NodeSpec { name: PROGRAM, marker: false, container: true, syntax: Default },
        NodeSpec { name: MODULE,  marker: false, container: true, syntax: Default },
        NodeSpec { name: CLASS,   marker: false, container: true, syntax: Keyword },
        NodeSpec { name: METHOD,  marker: false, container: true, syntax: Keyword },

        // Statements / control flow (BEGIN dual-use)
        NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
        NodeSpec { name: UNLESS,   marker: false, container: true, syntax: Keyword },
        NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
        NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Keyword },
        NodeSpec { name: CASE,     marker: false, container: true, syntax: Keyword },
        NodeSpec { name: THEN,     marker: false, container: true, syntax: Default },
        NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
        NodeSpec { name: UNTIL,    marker: false, container: true, syntax: Keyword },
        NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
        NodeSpec { name: BEGIN,    marker: true,  container: true, syntax: Keyword },
        NodeSpec { name: RESCUE,   marker: false, container: true, syntax: Keyword },
        NodeSpec { name: ENSURE,   marker: false, container: true, syntax: Keyword },
        NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
        NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Default },

        // Members / parameters
        NodeSpec { name: PARAMETER, marker: false, container: true, syntax: Default },
        NodeSpec { name: VARIABLE,  marker: false, container: true, syntax: Default },

        // Expressions
        NodeSpec { name: CALL,        marker: false, container: true, syntax: Function },
        NodeSpec { name: ASSIGN,      marker: false, container: true, syntax: Operator },
        NodeSpec { name: BINARY,      marker: false, container: true, syntax: Operator },
        NodeSpec { name: UNARY,       marker: false, container: true, syntax: Operator },
        NodeSpec { name: CONDITIONAL, marker: false, container: true, syntax: Default },
        NodeSpec { name: RANGE,       marker: false, container: true, syntax: Default },
        NodeSpec { name: LAMBDA,      marker: false, container: true, syntax: Function },
        NodeSpec { name: YIELD,       marker: false, container: true, syntax: Keyword },
        NodeSpec { name: SPREAD,      marker: false, container: true, syntax: Default },
        NodeSpec { name: LEFT,        marker: false, container: true, syntax: Default },

        // Pattern-matching
        NodeSpec { name: WHEN,    marker: false, container: true, syntax: Keyword },
        NodeSpec { name: IN,      marker: false, container: true, syntax: Default },
        NodeSpec { name: PATTERN, marker: false, container: true, syntax: Default },

        // Control-flow keyword leaves
        NodeSpec { name: NEXT,  marker: false, container: true, syntax: Keyword },
        NodeSpec { name: REDO,  marker: false, container: true, syntax: Keyword },
        NodeSpec { name: RETRY, marker: false, container: true, syntax: Keyword },

        // Rescue / class header metadata
        NodeSpec { name: EXCEPTIONS, marker: false, container: true, syntax: Default },
        NodeSpec { name: SUPERCLASS, marker: false, container: true, syntax: Default },

        // Collections / atoms (STRING, SYMBOL dual-use)
        NodeSpec { name: ARRAY,         marker: false, container: true, syntax: Type },
        NodeSpec { name: HASH,          marker: false, container: true, syntax: Type },
        NodeSpec { name: PAIR,          marker: false, container: true, syntax: Default },
        NodeSpec { name: STRING,        marker: true,  container: true, syntax: String },
        NodeSpec { name: INTERPOLATION, marker: false, container: true, syntax: Default },
        NodeSpec { name: SYMBOL,        marker: true,  container: true, syntax: String },
        NodeSpec { name: INT,           marker: false, container: true, syntax: Number },
        NodeSpec { name: FLOAT,         marker: false, container: true, syntax: Number },
        NodeSpec { name: REGEX,         marker: false, container: true, syntax: Default },

        // Literal atoms
        NodeSpec { name: TRUE,  marker: false, container: true, syntax: Keyword },
        NodeSpec { name: FALSE, marker: false, container: true, syntax: Keyword },
        NodeSpec { name: NIL,   marker: false, container: true, syntax: Keyword },
        NodeSpec { name: SELF,  marker: false, container: true, syntax: Keyword },

        // Identifiers
        NodeSpec { name: NAME,     marker: false, container: true, syntax: Identifier },
        NodeSpec { name: CONSTANT, marker: false, container: true, syntax: Default },
        NodeSpec { name: COMMENT,  marker: false, container: true, syntax: Comment },

        // Spread-shape markers
        NodeSpec { name: LIST, marker: true, container: false, syntax: Default },
        NodeSpec { name: DICT, marker: true, container: false, syntax: Default },

        // Parameter-shape markers
        NodeSpec { name: KEYWORD, marker: true, container: false, syntax: Default },
        NodeSpec { name: DEFAULT, marker: true, container: false, syntax: Default },

        // Block-shape / dual-use: DO is both marker (on block) and
        // container (loop body).
        NodeSpec { name: DO, marker: true, container: true, syntax: Keyword },

        // Symbol-shape marker
        NodeSpec { name: DELIMITED, marker: true, container: false, syntax: Default },

        // Class / method singleton markers
        NodeSpec { name: SINGLETON, marker: true, container: false, syntax: Default },

        // Dual-use: block container + `<parameter><block/>` marker.
        NodeSpec { name: BLOCK, marker: true, container: true, syntax: Default },
    ];

    pub fn spec(name: &str) -> Option<&'static NodeSpec> {
        NODES.iter().find(|n| n.name == name)
    }

    pub fn all_names() -> impl Iterator<Item = &'static str> {
        NODES.iter().map(|n| n.name)
    }

    pub fn is_marker_only(name: &str) -> bool {
        spec(name).map_or(false, |s| s.marker && !s.container)
    }

    pub fn is_declared(name: &str) -> bool {
        spec(name).is_some()
    }
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

/// Map a transformed element name to a syntax category for highlighting.
///
/// Consults the per-name NODES table first (one source of truth);
/// falls back to cross-cutting rules for names not in NODES.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = semantic::spec(element) {
        return spec.syntax;
    }
    match element {
        // Raw tree-sitter kinds / builder wrappers not in NODES:
        "type" => SyntaxCategory::Type,
        "raise" | "return" => SyntaxCategory::Keyword,
        "def" | "end" | "super" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::semantic::NODES;

    #[test]
    fn no_duplicate_node_names() {
        let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
        names.sort();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate NODES entry");
    }

    #[test]
    fn no_unused_role() {
        for n in NODES {
            assert!(
                n.marker || n.container,
                "<{}> is neither marker nor container — dead entry?",
                n.name,
            );
        }
    }
}
