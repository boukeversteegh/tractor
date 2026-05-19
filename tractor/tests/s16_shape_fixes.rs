//! Tree-level shape tests that pin design contracts for the typed-IR
//! pipeline: each test states a contract from
//! `specs/tractor-parse/tree/design.md` and verifies the typed lowering
//! plus `to_xot` projection emits the contract shape.
//!
//! The tests are organised by the **principle** or **decision** they
//! defend, not by which fix introduced them. The intent is that
//! removing a fix that violates the contract will cause the matching
//! test to fail with a message that names the principle.

#![cfg(feature = "native")]

use tractor::{parse, ParseInput, ParseOptions};

fn parse_to_xml(source: &str, language: &str) -> String {
    let parsed = parse(
        ParseInput::Inline { content: source, file_label: "<x>" },
        ParseOptions { language: Some(language), ..Default::default() },
    )
    .expect("parse");
    let xot = parsed.documents.xot();
    let doc_node = parsed
        .documents
        .document_node(parsed.doc_handle)
        .expect("doc handle");
    let root = if xot.is_document(doc_node) {
        xot.document_element(doc_node).expect("doc")
    } else {
        doc_node
    };
    xot.to_string(root).unwrap()
}

/// Open-tag predicate that tolerates attributes: matches both `<name>`
/// and `<name line="…" id="…">`.
fn has_open_tag(xml: &str, tag: &str) -> bool {
    xml.contains(&format!("<{tag}>")) || xml.contains(&format!("<{tag} "))
}

/// Self-closing-marker predicate: matches `<name/>` and `<name line="…" id="…"/>`.
fn has_self_closing(xml: &str, tag: &str) -> bool {
    if xml.contains(&format!("<{tag}/>")) {
        return true;
    }
    let needle = format!("<{tag} ");
    let mut rest = xml;
    while let Some(idx) = rest.find(&needle) {
        let after = &rest[idx + needle.len()..];
        if let Some(close) = after.find('>') {
            if after.as_bytes().get(close.wrapping_sub(1)) == Some(&b'/') {
                return true;
            }
            rest = &after[close + 1..];
        } else {
            break;
        }
    }
    false
}

fn assert_has_tag(xml: &str, tag: &str, principle: &str) {
    assert!(
        has_open_tag(xml, tag),
        "{principle}: expected open tag <{tag}…>\nfull XML:\n{xml}"
    );
}

fn assert_has_marker(xml: &str, tag: &str, principle: &str) {
    assert!(
        has_self_closing(xml, tag),
        "{principle}: expected self-closing marker <{tag}…/>\nfull XML:\n{xml}"
    );
}

/// Substring predicate that tolerates location attributes between
/// elements, e.g. `parent/<child` matches `<parent line="1">…<child…>`.
/// Splits the contract on `/` and verifies every consecutive pair
/// appears with the parent enclosing the child.
fn assert_contains_nesting(xml: &str, contract: &str, principle: &str) {
    let elements: Vec<&str> = contract.split('/').collect();
    let mut search_from = 0usize;
    let mut last_match_end = 0usize;
    for (i, el) in elements.iter().enumerate() {
        let pat_close = format!("<{el}>");
        let pat_attrs = format!("<{el} ");
        let rest = &xml[search_from..];
        let found = rest
            .find(&pat_close)
            .map(|n| (n, pat_close.len()))
            .or_else(|| rest.find(&pat_attrs).map(|n| (n, pat_attrs.len())));
        match found {
            Some((offset, taglen)) => {
                let abs = search_from + offset;
                if i > 0 && abs < last_match_end {
                    panic!(
                        "{principle}: <{el}> appears before its parent in contract {contract:?}\nfull XML:\n{xml}"
                    );
                }
                last_match_end = abs;
                search_from = abs + taglen;
            }
            None => panic!(
                "{principle}: contract {contract:?} not satisfied — missing <{el}>\nfull XML:\n{xml}"
            ),
        }
    }
}

// =========================================================================
// Principle #9 — Exhaustive Markers for Mutually Exclusive Variations.
// =========================================================================
// "When lifted modifiers represent mutually exclusive choices, all
// variants must have an explicit marker — don't use the absence of a
// marker as a default." Queries are then symmetric: `//yield[from]` and
// `//yield[not(from)]` are both well-formed, never relying on the
// absence of a marker that the consumer might forget to consider.

/// Python `yield from x` is a distinct construct from `yield x` — the
/// first delegates to a sub-iterator, the second yields a single value.
/// The discriminator is a `<from/>` marker on the stable `<yield>`
/// parent (Principle #15 — markers live on stable hosts; Principle #9
/// — the variant must be explicit).
#[test]
fn python_delegating_yield_carries_from_marker() {
    let xml = parse_to_xml("def g():\n    yield from inner()\n", "python");
    assert_has_tag(&xml, "yield", "Principle #15");
    assert_has_marker(&xml, "from", "Principle #9");
}

/// The complementary half of Principle #9: the absence of the variant
/// must not be expressed by a marker on the other side. Plain `yield x`
/// has no `<from/>` marker, so `//yield[not(from)]` is the precise
/// query for the non-delegating case.
#[test]
fn python_plain_yield_omits_from_marker() {
    let xml = parse_to_xml("def g():\n    yield 1\n", "python");
    assert!(
        has_open_tag(&xml, "yield") && !has_self_closing(&xml, "from"),
        "Principle #9 complement: plain yield must NOT carry <from/>; XML:\n{xml}"
    );
}

// =========================================================================
// Principle #14 — Namespace Vocabulary.
// =========================================================================
// "Every type-reference slot — parameter type, return type, base
// class, implemented interface, generic argument, trait bound, default
// type, field type, variable type — carries a `<type>` child." A
// type-parameter *declaration* (PEP 695 `def f[T]: …`) also occupies a
// type-namespace position and is carried inside a `<generic>` wrapper
// (the type-parameter-context marker) plus the inner `<type>` namespace
// marker.

/// PEP 695 generic-function declarations carry each type parameter as
/// `<generic>/<type>/<name>` — the outer `<generic>` names the
/// declaration context, the inner `<type>` declares the namespace, the
/// `<name>` carries the identifier. This distinguishes the
/// type-parameter declaration `[T]` from a type *annotation* `: T`
/// (which is `<type>/<name>` without the outer `<generic>`).
#[test]
fn python_pep695_function_generic_param_is_wrapped() {
    let xml = parse_to_xml("def identity[T](v: T) -> T:\n    return v\n", "python");
    assert_contains_nesting(&xml, "generic/type/name", "Principle #14");
}

/// Same contract applies to PEP 695 generic *class* declarations: each
/// `[T]` slot is `<generic>/<type>/<name>`.
#[test]
fn python_pep695_class_generic_param_is_wrapped() {
    let xml = parse_to_xml("class Box[T]:\n    pass\n", "python");
    assert_contains_nesting(&xml, "generic/type/name", "Principle #14");
}

// =========================================================================
// Principle #5 — Unified Concepts (cross-language uniform shapes
// where cost-benefit favours unification).
// =========================================================================
// "The structural shape *under* the keyword node (path / alias /
// variant markers) should still be uniform across languages — that's
// where the cost-benefit clearly favors unification."

/// Every import declaration that names a target path uses the same
/// `<import>/<path>/<name>+` shape regardless of the source language's
/// surface syntax. Java's `import java.util.List;`, Python's
/// `import os.path`, and Go's `import "net/http"` all produce the
/// same skeleton — segments are flat `<name>` siblings inside `<path>`
/// (Principle #19 — role-uniform leaves stay flat).
#[test]
fn go_simple_import_uses_canonical_path_shape() {
    let xml = parse_to_xml("package main\nimport \"fmt\"\n", "go");
    assert_contains_nesting(&xml, "import/path/name", "Principle #5");
}

/// Go's `import "net/http/pprof"` is one path with three segments. The
/// segments split on `/` because that's the Go path delimiter, not
/// because of any per-language exception — every multi-segment path
/// across all languages is N flat `<name>` siblings under `<path>`.
#[test]
fn go_multi_segment_import_splits_path_into_name_segments() {
    let xml = parse_to_xml(
        "package main\nimport \"net/http/pprof\"\n",
        "go",
    );
    assert_contains_nesting(&xml, "import/path/name", "Principle #5");
    // Three name siblings under <path>: "net", "http", "pprof".
    let path_open = xml.find("<path").expect("path tag");
    let path_close = xml[path_open..].find("</path>").expect("path close");
    let path_slice = &xml[path_open..path_open + path_close];
    let segment_count = path_slice.matches("<name").count();
    assert_eq!(
        segment_count, 3,
        "expected 3 <name> segments under <path>; XML slice:\n{path_slice}"
    );
}

// =========================================================================
// Principle #9 — Exhaustive Markers (continued).
// =========================================================================
// Go's import-kind set is closed: {plain, alias, dot, blank}. Every
// non-default variant must surface as an explicit marker on `<import>`
// so queries like `//import[dot]`, `//import[blank]`, `//import[alias]`
// each match precisely their kind without relying on the absence of a
// marker as a default.

/// `import f "fmt"` is the renamed-import variant — alias marker on
/// `<import>`, original path inside `<path>`, alias inside `<aliased>`.
/// Principle #18 (Name Relationships After the Operator — `<aliased>`
/// for the renamed-target side of `as` clauses).
#[test]
fn go_aliased_import_carries_alias_marker_and_aliased_slot() {
    let xml = parse_to_xml(
        "package main\nimport f \"fmt\"\n",
        "go",
    );
    assert_has_marker(&xml, "alias", "Principle #9");
    assert_contains_nesting(&xml, "import/path/name", "Principle #5");
    assert_contains_nesting(&xml, "import/aliased/name", "Principle #18");
}

/// `import . "strings"` is the dot-import variant.
#[test]
fn go_dot_import_carries_dot_marker() {
    let xml = parse_to_xml(
        "package main\nimport . \"strings\"\n",
        "go",
    );
    assert_has_marker(&xml, "dot", "Principle #9");
    assert_contains_nesting(&xml, "import/path/name", "Principle #5");
}

/// `import _ "x"` is the blank-import (side-effect) variant.
#[test]
fn go_blank_import_carries_blank_marker() {
    let xml = parse_to_xml(
        "package main\nimport _ \"x\"\n",
        "go",
    );
    assert_has_marker(&xml, "blank", "Principle #9");
    assert_contains_nesting(&xml, "import/path/name", "Principle #5");
}

/// Group-form `import (...)` decomposes into N independent `<import>`
/// siblings, one per spec — the grouping parentheses are surface
/// syntax, not a structural concept. A `//import[blank]` query finds
/// the blank-import spec the same way regardless of whether the source
/// used grouped or line-form syntax.
#[test]
fn go_grouped_import_decomposes_into_sibling_imports() {
    let xml = parse_to_xml(
        "package main\nimport (\n    \"fmt\"\n    _ \"x\"\n)\n",
        "go",
    );
    let import_count = xml.matches("<import").count();
    assert!(
        import_count >= 2,
        "expected ≥2 <import> siblings from grouped import; XML:\n{xml}"
    );
    assert_has_marker(&xml, "blank", "Principle #9");
}

// =========================================================================
// Principle #19 — Role-Mixed Text-Leaves Wrap; Role-Uniform Lists Flat.
// =========================================================================
// "Role-mixed — leaves play different roles. Each role gets a distinct
// wrapper element naming the slot. The wrapper holds the text-leaf as
// its primary content and provides a stable host for any current or
// future markers."

/// Every assignment `target = value` has THREE different roles in
/// source order: the target (LHS), the operator `=`, the value (RHS).
/// Pre-IR Python, Go, Ruby, Rust, Java, C#, TypeScript all wrap these
/// in `<left>`, `<op>`, `<right>` slot elements — bare `<name>` /
/// `<expression>` siblings would force consumers to rely on positional
/// order to recover the roles (`//assign/name[1]` for target, etc.),
/// which violates Principle #19. The role-named slots also give each
/// side an `<expression>` host (stable-expression-host decision) so
/// expression modifiers can attach without disturbing the assignment
/// shape.
#[test]
fn ruby_plain_assignment_decomposes_into_left_op_right() {
    let xml = parse_to_xml("MAX = 10\n", "ruby");
    assert_contains_nesting(&xml, "assign/left/expression/name", "Principle #19");
    assert_has_tag(&xml, "op", "Principle #19");
    assert_contains_nesting(&xml, "assign/right/expression/int", "Principle #19");
}

/// `x += 1` is an augmented assignment. The `<op>` element carries the
/// full operator text `+=` so the renderer can round-trip it; the
/// operator markers (`<assign/>`, `<plus/>`) attach as marker children
/// of `<op>` via the shared `add_operator_markers` helper.
#[test]
fn ruby_augmented_assignment_keeps_left_op_right_shape() {
    let xml = parse_to_xml("x += 1\n", "ruby");
    assert_contains_nesting(&xml, "assign/left/expression/name", "Principle #19");
    assert_contains_nesting(&xml, "assign/right/expression/int", "Principle #19");
    assert_has_tag(&xml, "op", "Principle #19");
}

// =========================================================================
// Goal #7 — Source Reversibility.
// =========================================================================
// "The semantic tree's text, when concatenated in document order and
// the element tags stripped, should reproduce the original source."
// Heredoc markers are part of the source; dropping them silently means
// the assignment's right-hand side disappears from the tree.

/// `HEREDOC = <<-TEXT` — the heredoc opener `<<-TEXT` is the value
/// expression's surface token. Lower it to a `<string>` literal so the
/// assignment's right slot has something to host (Principle #19: every
/// role-named slot wraps a concrete value) and so the source bytes
/// flow through the renderer (Goal #7).
#[test]
fn ruby_heredoc_opener_lowers_to_string_value() {
    let source = "HEREDOC = <<-TEXT\n  body\nTEXT\n";
    let xml = parse_to_xml(source, "ruby");
    assert_contains_nesting(&xml, "assign/right/expression/string", "Goal #7");
}

// =========================================================================
// Principle #5 / chain-inversion implicit-receiver decision —
// languages with implicit `self` / `super` / `this` receivers surface
// them as dedicated elements, not as identifier siblings.
// =========================================================================

/// Ruby's `def self.build` declares a singleton method on the class
/// object. The `self` token is a *receiver* in source-position terms,
/// not a regular identifier — so it gets its own `<self>` element name
/// (Principle #5 — unified concept per language). Otherwise
/// `method[singleton]/{name="self", name="build"}` is role-mixed
/// (Principle #19 violation): two same-named siblings whose roles
/// (receiver / method-name) differ only by sibling order.
#[test]
fn ruby_singleton_method_self_receiver_is_dedicated_element() {
    let xml = parse_to_xml(
        "class Foo\n  def self.build\n  end\nend\n",
        "ruby",
    );
    assert_has_tag(&xml, "self", "chain-inversion implicit-receiver");
}

// =========================================================================
// Principle #18 — Name Relationships After the Operator.
// =========================================================================
// "When a syntactic construct expresses a relationship between a host
// element and one or more target types or values (`extends`,
// `implements`, `throws`, `where`, type bounds, etc.), name the
// relationship element after the **operator** that introduces it,
// never after the target's role. Multiple targets ⇒ multiple
// siblings, never a list container."

/// Java's `void f() throws E1, E2` declares each thrown exception as
/// one `<throws>/<type>/<name>` sibling on the method. The `<throws>`
/// element name comes from the source operator keyword (Principle #18
/// + Principle #1 — use language keywords); the inner `<type>` carries
/// the namespace marker (Principle #14 — every type-reference slot
/// carries `<type>`). Two declared exceptions produce two siblings,
/// not one list container (Principle #12 — flat siblings).
#[test]
fn java_method_throws_emits_per_target_sibling() {
    let xml = parse_to_xml(
        "class C { void f() throws java.io.IOException, IllegalArgumentException {} }\n",
        "java",
    );
    let throws_count = xml.matches("<throws").count();
    assert!(
        throws_count >= 2,
        "expected 2 <throws> siblings, found {throws_count}; XML:\n{xml}"
    );
    assert_contains_nesting(&xml, "throws/type", "Principle #14");
}

/// A method with no `throws` clause has no `<throws>` siblings — the
/// absence of the relationship is the absence of the marker, never an
/// empty container.
#[test]
fn java_method_without_throws_has_no_throws_element() {
    let xml = parse_to_xml("class C { void f() {} }\n", "java");
    assert!(
        !xml.contains("<throws"),
        "method without throws clause should not emit <throws>; XML:\n{xml}"
    );
}

// =========================================================================
// Principle #9 — TypeScript import classification (continued).
// =========================================================================
// TypeScript's import-kind set is closed: every import is exactly one
// of {sideeffect, default, namespace, group, or a mix of default+other}.
// Each variant carries an explicit marker on `<import>` so queries
// like `//import[group]`, `//import[namespace]`, `//import[default]`
// each pinpoint their kind without relying on the absence of a marker
// as a default.

/// `import "./side-effect-only"` carries the `<sideeffect/>` marker —
/// no specifiers, the import exists for the module's evaluation
/// side-effects only.
#[test]
fn typescript_side_effect_import_carries_sideeffect_marker() {
    let xml = parse_to_xml("import \"./barrel\";\n", "typescript");
    assert_has_marker(&xml, "sideeffect", "Principle #9");
}

/// `import { a, b } from "./mod"` is a named/group import.
#[test]
fn typescript_named_import_carries_group_marker() {
    let xml = parse_to_xml(
        "import { foo, bar } from \"./mod\";\n",
        "typescript",
    );
    assert_has_marker(&xml, "group", "Principle #9");
}

/// `import * as ns from "./mod"` is a namespace import.
#[test]
fn typescript_namespace_import_carries_namespace_marker() {
    let xml = parse_to_xml(
        "import * as ns from \"./mod\";\n",
        "typescript",
    );
    assert_has_marker(&xml, "namespace", "Principle #9");
}

/// `import foo from "./mod"` is a default import.
#[test]
fn typescript_default_import_carries_default_marker() {
    let xml = parse_to_xml(
        "import foo from \"./mod\";\n",
        "typescript",
    );
    assert_has_marker(&xml, "default", "Principle #9");
}

// =========================================================================
// Principle #15 — Markers Live in Stable, Predictable Locations.
// =========================================================================
// "Markers carry meaning by their presence on a parent. For that to
// work, the parent has to be a stable, predictable location — the same
// parent shape whether or not the marker is present, and across all
// surface variants of the same concept."

/// TypeScript's `<T = number>` declares a type parameter with a
/// default constraint. The default branch surfaces as a
/// `<type[default]>` slot on the `<generic>` parent — a stable place
/// to attach default-bound markers without disturbing the
/// type-parameter's identity. Without the slot, `<T = number>` is
/// indistinguishable from a bare `<T>` declaration (Goal #7 — Source
/// Reversibility — the `= number` text disappears from the tree).
#[test]
fn typescript_generic_parameter_default_lowers_to_type_default() {
    let xml = parse_to_xml(
        "type Shape<T = number> = { value: T };\n",
        "typescript",
    );
    assert_has_tag(&xml, "generic", "Principle #14");
    assert!(
        xml.contains("<type ") || xml.contains("<type>"),
        "expected inner <type> wrapper; XML:\n{xml}"
    );
    // The default branch carries a `[default]` marker on its `<type>`
    // wrapper. The marker is empty (Principle #13 — markers stay
    // empty).
    assert_has_marker(&xml, "default", "Principle #15");
}

// =========================================================================
// Principle #11 — Specific Names Over Type Hierarchies.
// =========================================================================
// "Use the most specific semantic name for each node." A `T | U` union
// type IS a union — the construct deserves a concrete name (Principle
// #11) rather than an anonymous `<type>/<type>/...` double-wrap. The
// `[union]` marker on the outer `<type>` names the construct so
// `//type[union]` finds every union site.

/// `x: T | None` lowers to a `<type[union]>` wrapper around the
/// alternative types. Without the marker, the outer `<type>` is an
/// anonymous wrapper whose meaning is opaque.
#[test]
fn python_union_type_annotation_carries_union_marker() {
    let xml = parse_to_xml(
        "def f(x: int | None) -> int:\n    return 0\n",
        "python",
    );
    assert_has_marker(&xml, "union", "Principle #11");
}

// =========================================================================
// Principle #18 — operator-named relationships (continued).
// =========================================================================
// Go's `interface { io.Reader; ... }` embeds another interface. That
// embedding IS a hierarchy relationship at the type level — same
// semantics as Java's `extends`, just expressed via co-location inside
// the body rather than a dedicated keyword. The element name is
// `<extends>` (the cross-language operator-naming choice from
// Principle #18) so the same query `//interface/extends/type` finds
// every interface base across all supported languages.

/// `interface { io.Reader }` lifts the embedded `io.Reader` into an
/// `<extends>/<type>` slot on the interface, distinguishing the
/// embedding from a method signature (which lives inline as a
/// `<method>` sibling).
#[test]
fn go_interface_embedding_lifts_to_extends_slot() {
    let xml = parse_to_xml(
        "package main\ntype R interface { io.Reader }\n",
        "go",
    );
    assert_has_tag(&xml, "interface", "structural");
    assert_has_tag(&xml, "extends", "Principle #18");
    assert_contains_nesting(&xml, "extends/type", "Principle #14");
}

// =========================================================================
// Principle #9 + Principle #19 — Ruby ranges.
// =========================================================================
// Ruby's `1..10` (inclusive) and `1...10` (exclusive) are
// mutually-exclusive variants of the same construct. The exhaustive
// markers `<inclusive/>` / `<exclusive/>` make the boundary policy
// queryable (Principle #9). The two endpoints play different roles
// (start of range / end of range), so role-named `<from>` / `<to>`
// slots wrap each (Principle #19 — otherwise two same-named `<int>`
// siblings rely on sibling order to mean different things).

/// `1..10` carries the `<inclusive/>` marker and the `<from>`/`<to>`
/// slots are emitted regardless of the leaf type at each endpoint.
#[test]
fn ruby_inclusive_range_carries_inclusive_marker_and_slot_wrappers() {
    let xml = parse_to_xml("r = (1..10)\n", "ruby");
    assert_has_marker(&xml, "inclusive", "Principle #9");
    assert_contains_nesting(&xml, "range/from/int", "Principle #19");
    assert_contains_nesting(&xml, "range/to/int", "Principle #19");
}

/// `1...10` carries the `<exclusive/>` marker — the complementary
/// half of the exhaustive pair.
#[test]
fn ruby_exclusive_range_carries_exclusive_marker() {
    let xml = parse_to_xml("r = (1...10)\n", "ruby");
    assert_has_marker(&xml, "exclusive", "Principle #9");
    assert_contains_nesting(&xml, "range/from/int", "Principle #19");
    assert_contains_nesting(&xml, "range/to/int", "Principle #19");
}

// =========================================================================
// Principle #19 — TypeScript conditional types.
// =========================================================================
// `T extends X ? Y : Z` has four typed slots playing four different
// roles. Bare children disambiguated by position would force every
// consumer to remember the ordering — role-named slots make the
// shape self-describing.

/// `T extends X ? Y : Z` lowers to a `<type[conditional]>` with four
/// role-named slot children: `<left>` (the type being tested), `<right>`
/// (the type being matched against), `<then>` (true branch),
/// `<else>` (false branch). Each slot wraps an inner `<type>` element
/// per Principle #14.
#[test]
fn typescript_conditional_type_has_role_named_slot_wrappers() {
    let xml = parse_to_xml(
        "type Unwrap<T> = T extends Array<infer U> ? U : never;\n",
        "typescript",
    );
    assert_has_marker(&xml, "conditional", "Principle #11");
    assert_contains_nesting(&xml, "type/left/type", "Principle #19");
    assert_contains_nesting(&xml, "type/right/type", "Principle #19");
    assert_contains_nesting(&xml, "type/then/type", "Principle #19");
    assert_contains_nesting(&xml, "type/else/type", "Principle #19");
}

// =========================================================================
// Principle #14 — Namespace Vocabulary (continued, type-only).
// =========================================================================
// "Every type-reference slot carries a `<type>` child." The rule is
// uniform — tuple elements, function-return slots, parameter types,
// generic arguments, base classes, trait bounds. Without the wrapper,
// a name-leaf inside a type position is indistinguishable from a
// name-leaf in a value position (`//type//name='string'` vs.
// `//name='string'` becomes ambiguous).

/// TypeScript tuple-type elements (`[string, number]`) are each
/// type-references in their own right and must each carry a `<type>`
/// wrapper.
#[test]
fn typescript_tuple_type_wraps_each_element_in_type() {
    let xml = parse_to_xml(
        "type Pair = [string, number];\n",
        "typescript",
    );
    assert_has_marker(&xml, "tuple", "Principle #11");
    // Each tuple element renders as `<type><name>string</name></type>`
    // / `<type><name>number</name></type>` — three `<type>` elements
    // total (the outer `<type[tuple]>` plus one per element).
    let count = xml.matches("<type ").count() + xml.matches("<type>").count();
    assert!(
        count >= 3,
        "expected ≥3 <type> elements (tuple + 2 elements), found {count}; XML:\n{xml}"
    );
    // Each element's inner shape is `<type>/<name>`.
    assert_contains_nesting(&xml, "type/name", "Principle #14");
}

/// `(x: T) => U` is a TS function type. The return-type slot wraps in
/// `<returns>/<type>`, matching the slot used by regular function
/// declarations.
#[test]
fn typescript_function_type_return_lowers_to_returns_type_slot() {
    let xml = parse_to_xml(
        "type Mapper<T> = (input: T) => T;\n",
        "typescript",
    );
    assert_has_marker(&xml, "function", "Principle #11");
    assert_contains_nesting(&xml, "returns/type", "Principle #14");
}
