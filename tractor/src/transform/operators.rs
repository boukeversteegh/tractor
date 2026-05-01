//! Operator marker transformer.
//!
//! Cross-language operator-to-marker logic. Languages call
//! [`prepend_op_element`] from their `extract_operator` paths to attach a
//! semantically-marked `<op>` element to a binary/unary expression node.
//!
//! The marker structure is driven by the declarative [`OPERATOR_MARKERS`]
//! table — one row per canonical operator. Adding an operator here
//! automatically extends `is_operator_marker_name` and the invariant tests
//! in `tests/tree_invariants.rs`.

use xot::{Xot, Node as XotNode};

use super::helpers::append_marker;

/// Attach an `<op>` element to `parent` carrying `op_text`, with
/// semantic marker children driven by [`OPERATOR_MARKERS`].
///
/// Preserves source whitespace by detaching the original operator text
/// node first and reinserting `[before_ws, <op>op_text</op>, after_ws]`
/// in its place. If no matching text node exists (synthetic AST), falls
/// back to prepending a synthesized `<op>` element.
pub fn prepend_op_element(xot: &mut Xot, parent: XotNode, op_text: &str) -> Result<XotNode, xot::Error> {
    let op_name = xot.add_name("op");
    let op_element = xot.new_element(op_name);
    add_operator_markers(xot, op_element, op_text)?;

    // Find the source text node containing the operator — typically
    // `" + "` with surrounding whitespace for binary expressions, or
    // `"== this"` when an adjacent anonymous keyword leaks into the
    // same text leaf (handled by `extract_operator`'s prefix narrowing).
    let source_text: Option<(XotNode, String)> = xot
        .children(parent)
        .find_map(|c| {
            let s = xot.text_str(c)?.to_string();
            let trimmed = s.trim();
            if trimmed == op_text || trimmed.starts_with(op_text) {
                Some((c, s))
            } else {
                None
            }
        });

    match source_text {
        Some((text_node, content)) => {
            // Goal: `<op>` contains exactly `op_text` (so
            // `//binary[op='+']` works) while the surrounding
            // whitespace survives as sibling text (so the source
            // text-preservation invariant holds).
            //
            // Approach: detach the original text FIRST, then
            // insert the three pieces at its former position.
            // Detaching first avoids xot's automatic text
            // consolidation from merging newly-inserted whitespace
            // with the still-present original text.
            let op_pos = content.find(op_text).unwrap_or(0);
            let before = content[..op_pos].to_string();
            let after = content[op_pos + op_text.len()..].to_string();

            // Put the operator char inside the op element.
            let op_inner = xot.new_text(op_text);
            xot.append(op_element, op_inner)?;

            // Create the replacement nodes *before* detaching so
            // xot has no live adjacency to consolidate against.
            let before_node = if before.is_empty() {
                None
            } else {
                Some(xot.new_text(&before))
            };
            let after_node = if after.is_empty() {
                None
            } else {
                Some(xot.new_text(&after))
            };

            // Remember anchor (the next sibling after the text)
            // so we can insert the replacement sequence at the
            // original position. Anchor = None means "append to
            // parent" (i.e., text was the last child).
            let anchor = xot.next_sibling(text_node);
            xot.detach(text_node)?;

            for node in [before_node, Some(op_element), after_node].into_iter().flatten() {
                match anchor {
                    Some(a) => xot.insert_before(a, node)?,
                    None => xot.append(parent, node)?,
                }
            }
        }
        None => {
            // Fallback — synthesize a text node and prepend.
            // Violates source-text-preservation, but the
            // invariant test surfaces this.
            let synth = xot.new_text(op_text);
            xot.append(op_element, synth)?;
            xot.prepend(parent, op_element)?;
        }
    }
    Ok(op_element)
}

/// Declarative spec for one operator's marker structure.
///
/// `text`:   the exact operator string (e.g. `"+"`, `"+="`, `"not in"`).
/// `primary`: the top-level marker child of `<op>`. `None` means "no
///           marker — graceful degradation" (e.g. bare `=` alone).
/// `children`: empty-element children appended *inside* the primary
///             marker. Each is a distinct flag (e.g. `<less/><or-equal/>`
///             inside a `<compare>`).
/// `nested`: for compound assignments only — a second marker nested
///           inside `<assign>` with its own children
///           (e.g. `&&=` → `<assign><logical><and/></logical></assign>`).
pub struct OperatorSpec {
    pub text: &'static str,
    pub primary: Option<&'static str>,
    pub children: &'static [&'static str],
    pub nested: Option<(&'static str, &'static [&'static str])>,
}

/// Canonical cross-language operator table. Every entry pins down
/// the EXACT marker shape that a transform MUST emit when an `<op>`
/// has the corresponding text content.
///
/// Invariants (enforced by `tests/tree_invariants.rs::op_marker_matches_text`):
///   1. For every `<op>` whose trimmed text equals an entry's `text`,
///      if `primary` is `Some`, the `<op>` has that marker as a
///      direct element child.
///   2. `<op>` whose text is NOT in the table is accepted without
///      requirements (language-specific operator — graceful
///      degradation).
///   3. The `children` / `nested` fields describe the sub-structure
///      the current `add_operator_markers` produces; the invariant
///      only checks the primary layer for now.
///
/// Languages share this table by construction — there is exactly
/// one source of truth, and every language's `extract_operator`
/// routes through `prepend_op_element` which consults it.
pub const OPERATOR_MARKERS: &[OperatorSpec] = &[
    // Equality
    OperatorSpec { text: "==",  primary: Some("equals"),     children: &[],         nested: None },
    OperatorSpec { text: "===", primary: Some("equals"),     children: &["strict"], nested: None },
    OperatorSpec { text: "!=",  primary: Some("not-equals"), children: &[],         nested: None },
    OperatorSpec { text: "!==", primary: Some("not-equals"), children: &["strict"], nested: None },
    // Comparison
    OperatorSpec { text: "<",   primary: Some("compare"),    children: &["less"],                 nested: None },
    OperatorSpec { text: ">",   primary: Some("compare"),    children: &["greater"],              nested: None },
    OperatorSpec { text: "<=",  primary: Some("compare"),    children: &["less", "or-equal"],     nested: None },
    OperatorSpec { text: ">=",  primary: Some("compare"),    children: &["greater", "or-equal"],  nested: None },
    // Arithmetic
    OperatorSpec { text: "+",   primary: Some("plus"),     children: &[], nested: None },
    OperatorSpec { text: "-",   primary: Some("minus"),    children: &[], nested: None },
    OperatorSpec { text: "*",   primary: Some("multiply"), children: &[], nested: None },
    OperatorSpec { text: "/",   primary: Some("divide"),   children: &[], nested: None },
    OperatorSpec { text: "%",   primary: Some("modulo"),   children: &[], nested: None },
    OperatorSpec { text: "**",  primary: Some("power"),    children: &[], nested: None },
    // Logical
    OperatorSpec { text: "&&",  primary: Some("logical"), children: &["and"], nested: None },
    OperatorSpec { text: "and", primary: Some("logical"), children: &["and"], nested: None },
    OperatorSpec { text: "||",  primary: Some("logical"), children: &["or"],  nested: None },
    OperatorSpec { text: "or",  primary: Some("logical"), children: &["or"],  nested: None },
    OperatorSpec { text: "!",   primary: Some("logical"), children: &["not"], nested: None },
    OperatorSpec { text: "not", primary: Some("logical"), children: &["not"], nested: None },
    OperatorSpec { text: "??",  primary: Some("nullish-coalescing"), children: &[], nested: None },
    // Bitwise
    OperatorSpec { text: "&",   primary: Some("bitwise"), children: &["and"], nested: None },
    OperatorSpec { text: "|",   primary: Some("bitwise"), children: &["or"],  nested: None },
    OperatorSpec { text: "^",   primary: Some("bitwise"), children: &["xor"], nested: None },
    OperatorSpec { text: "~",   primary: Some("bitwise"), children: &["not"], nested: None },
    // Shift
    OperatorSpec { text: "<<",  primary: Some("shift"), children: &["left"],                 nested: None },
    OperatorSpec { text: ">>",  primary: Some("shift"), children: &["right"],                nested: None },
    OperatorSpec { text: ">>>", primary: Some("shift"), children: &["right", "unsigned"],    nested: None },
    // Compound assignment (arithmetic)
    OperatorSpec { text: "+=",  primary: Some("assign"), children: &["plus"],     nested: None },
    OperatorSpec { text: "-=",  primary: Some("assign"), children: &["minus"],    nested: None },
    OperatorSpec { text: "*=",  primary: Some("assign"), children: &["multiply"], nested: None },
    OperatorSpec { text: "/=",  primary: Some("assign"), children: &["divide"],   nested: None },
    OperatorSpec { text: "%=",  primary: Some("assign"), children: &["modulo"],   nested: None },
    OperatorSpec { text: "**=", primary: Some("assign"), children: &["power"],    nested: None },
    // Compound assignment (logical / bitwise / shift) — nested form.
    OperatorSpec { text: "&&=", primary: Some("assign"), children: &[], nested: Some(("logical", &["and"])) },
    OperatorSpec { text: "||=", primary: Some("assign"), children: &[], nested: Some(("logical", &["or"]))  },
    OperatorSpec { text: "??=", primary: Some("assign"), children: &[], nested: Some(("nullish-coalescing", &[])) },
    OperatorSpec { text: "<<=", primary: Some("assign"), children: &[], nested: Some(("shift", &["left"]))  },
    OperatorSpec { text: ">>=", primary: Some("assign"), children: &[], nested: Some(("shift", &["right"])) },
    OperatorSpec { text: "&=",  primary: Some("assign"), children: &[], nested: Some(("bitwise", &["and"])) },
    OperatorSpec { text: "|=",  primary: Some("assign"), children: &[], nested: Some(("bitwise", &["or"]))  },
    OperatorSpec { text: "^=",  primary: Some("assign"), children: &[], nested: Some(("bitwise", &["xor"])) },
    // Python-specific
    OperatorSpec { text: "in",     primary: Some("contains"), children: &[],      nested: None },
    OperatorSpec { text: "not in", primary: Some("contains"), children: &["not"], nested: None },
    OperatorSpec { text: "is",     primary: Some("identity"), children: &[],      nested: None },
    OperatorSpec { text: "is not", primary: Some("identity"), children: &["not"], nested: None },
    // Python floor-divide and matmul (binary + augmented-assign)
    OperatorSpec { text: "//",  primary: Some("floor-divide"), children: &[],               nested: None },
    OperatorSpec { text: "@",   primary: Some("matmul"),       children: &[],               nested: None },
    OperatorSpec { text: "//=", primary: Some("assign"),       children: &["floor-divide"], nested: None },
    OperatorSpec { text: "@=",  primary: Some("assign"),       children: &["matmul"],       nested: None },
    // TypeScript / JavaScript unary type operators
    OperatorSpec { text: "typeof", primary: Some("typeof"), children: &[], nested: None },
    OperatorSpec { text: "void",   primary: Some("void"),   children: &[], nested: None },
    // Ruby unary defined-test
    OperatorSpec { text: "defined?", primary: Some("defined"), children: &[], nested: None },
    // Ruby spaceship comparator: `a <=> b` returns -1 / 0 / 1.
    // Marker `compare-three-way` is more explicit than the source
    // sigil and parallels the existing `compare` family.
    OperatorSpec { text: "<=>", primary: Some("compare-three-way"), children: &[], nested: None },
    // Ruby regex match operators.
    OperatorSpec { text: "=~", primary: Some("match"), children: &[],      nested: None },
    OperatorSpec { text: "!~", primary: Some("match"), children: &["not"], nested: None },
    // Python walrus / assignment expression.
    OperatorSpec { text: ":=", primary: Some("assign"), children: &["walrus"], nested: None },
    // PHP / Java / C# / Ruby type-test (PHP source: `instanceof`,
    // Java/C# source: `instanceof` / `is`, Ruby `is_a?`/`kind_of?`
    // are method-style — only the keyword form is in this table).
    OperatorSpec { text: "instanceof", primary: Some("instanceof"), children: &[], nested: None },
    // Go channel receive (binary form is a separate `<send>` shape)
    OperatorSpec { text: "<-", primary: Some("receive"), children: &[], nested: None },
    // Unary prefix / postfix
    OperatorSpec { text: "++", primary: Some("increment"), children: &[], nested: None },
    OperatorSpec { text: "--", primary: Some("decrement"), children: &[], nested: None },
    // Bare `=` — no marker by design (the parent element's name is
    // already `<assign>`). Recording here with `primary: None` so
    // the invariant knows `=` is a known operator that intentionally
    // has no marker.
    OperatorSpec { text: "=", primary: None, children: &[], nested: None },
];

/// Look up the declarative spec for `op_text` in `OPERATOR_MARKERS`.
/// Returns `None` for operators not in the canonical table (language-
/// specific operators that get no marker).
pub fn lookup_operator_spec(op_text: &str) -> Option<&'static OperatorSpec> {
    OPERATOR_MARKERS.iter().find(|spec| spec.text == op_text)
}

/// Return true if `name` is an element name emitted by the shared
/// operator marker machinery — either `"op"` itself or any primary /
/// child / nested marker declared in `OPERATOR_MARKERS`.
///
/// Derived from `OPERATOR_MARKERS` so there is exactly one source of
/// truth: adding an operator to the table automatically extends this
/// allowlist. The `all_names_declared_in_semantic_module` invariant
/// uses this to treat cross-cutting operator markers as universally
/// allowed (they're shared by every language, so declaring them in
/// each language's `ALL_NAMES` would duplicate the source of truth).
pub fn is_operator_marker_name(name: &str) -> bool {
    if name == "op" {
        return true;
    }
    OPERATOR_MARKERS.iter().any(|spec| {
        spec.primary == Some(name)
            || spec.children.iter().any(|c| *c == name)
            || match spec.nested {
                Some((nested_name, nested_children)) => {
                    nested_name == name
                        || nested_children.iter().any(|c| *c == name)
                }
                None => false,
            }
    })
}

/// Add semantic marker children inside an `<op>` element based on
/// operator text — drives off the declarative `OPERATOR_MARKERS`
/// table. Unknown operators get no markers (graceful degradation).
fn add_operator_markers(xot: &mut Xot, op: XotNode, text: &str) -> Result<(), xot::Error> {
    let spec = match lookup_operator_spec(text) {
        Some(s) => s,
        None => return Ok(()),
    };
    let primary = match spec.primary {
        Some(p) => p,
        None => return Ok(()),
    };
    let primary_el = append_marker(xot, op, primary, spec.children)?;
    if let Some((nested_name, nested_children)) = spec.nested {
        append_marker(xot, primary_el, nested_name, nested_children)?;
    }
    Ok(())
}

/// Check if an element name is an operator semantic marker
pub fn is_operator_marker(name: &str) -> bool {
    matches!(name,
        "equals" | "not-equals" | "compare" | "less" | "greater" | "or-equal"
        | "plus" | "minus" | "multiply" | "divide" | "modulo" | "power"
        | "floor-divide" | "matmul"
        | "logical" | "bitwise" | "shift" | "nullish-coalescing"
        | "assign" | "increment" | "decrement"
        | "strict" | "left" | "right" | "unsigned" | "xor"
        | "contains" | "identity" | "not" | "and" | "or"
        | "typeof" | "void" | "defined" | "receive"
    )
}

/// Detect prefix form for `update_expression`-style nodes that
/// conflate `++x` (prefix) and `x++` (postfix) under one tree-sitter
/// kind. Inspects child order BEFORE operator extraction: in prefix
/// forms the operator text appears as the first non-whitespace child;
/// in postfix forms an element child (the operand) appears first.
///
/// Used by TS/Java/PHP `update_expression` transforms so a single
/// `<prefix/>` marker pattern works cross-language and queries like
/// `//unary[prefix][op[increment]]` distinguish `++x` from `x++`
/// uniformly. C# splits prefix vs postfix at the kind level
/// (`prefix_unary_expression` / `postfix_unary_expression`) so it
/// doesn't need this helper.
pub fn is_prefix_form(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if let Some(text) = xot.text_str(child) {
            if !text.trim().is_empty() {
                return true;
            }
        } else if xot.element(child).is_some() {
            return false;
        }
    }
    false
}

/// Find the operator text inside a binary/unary expression node (the
/// first non-pure-punctuation text child) and prepend an `<op>` element
/// for it. No-op if no operator-like text exists.
///
/// Identical to the per-language `extract_operator` helpers — pulled
/// here so `Rule::ExtractOpThenRename` can call it without forcing
/// language-specific function pointers through the rule table.
///
/// When the candidate text contains a known operator from
/// [`OPERATOR_MARKERS`] as a token-bounded prefix (e.g. `"== this"`
/// when `this` leaks as anonymous text from tree-sitter), only the
/// operator prefix is extracted. The trailing tokens stay as sibling
/// text via `prepend_op_element`'s before/after slicing.
pub fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    use super::helpers::get_text_children;
    let texts = get_text_children(xot, node);
    let candidate = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    });
    if let Some(text) = candidate {
        let op_text = refine_operator_text(text);
        prepend_op_element(xot, node, op_text)?;
    }
    Ok(())
}

/// Narrow a candidate operator string to a known operator prefix when
/// the text leaks adjacent anonymous keywords (e.g. C# `"== this"`).
///
/// Strategy: prefer the longest [`OPERATOR_MARKERS`] entry that is a
/// token-bounded prefix of `text` (followed by whitespace or
/// end-of-string). Multi-word operators like `"is not"` win over
/// single-word ones like `"is"`. If no match, return the trimmed text
/// unchanged — preserving graceful degradation for language-specific
/// operators absent from the table.
fn refine_operator_text(text: &str) -> &str {
    let trimmed = text.trim();
    let mut best: Option<&'static str> = None;
    for spec in OPERATOR_MARKERS.iter() {
        if !trimmed.starts_with(spec.text) {
            continue;
        }
        let rest = &trimmed[spec.text.len()..];
        let token_bounded = rest.is_empty()
            || rest.chars().next().map_or(false, char::is_whitespace);
        if !token_bounded {
            continue;
        }
        if best.map_or(true, |b| spec.text.len() > b.len()) {
            best = Some(spec.text);
        }
    }
    best.unwrap_or(trimmed)
}
