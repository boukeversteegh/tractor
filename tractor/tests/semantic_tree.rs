//! Semantic-tree invariant tests.
//!
//! Each test pins down one design-principle invariant with an explicit
//! XPath assertion. When an assertion fails, consult the cited
//! principle and the invariant description before touching the test.
//! The goal is that a failing assertion names the violated principle
//! clearly enough that a reviewer (or a coding agent) cannot "fix" it
//! by simply flipping the expected value.
//!
//! See `specs/tractor-parse/semantic-tree/design.md` for the principle
//! catalogue referenced in the comments below.
//!
//! Each test owns a minimal inline source and a handful of assertions;
//! no shared fixture files. If coverage feels thin, add a test — the
//! helpers are designed for one-liners.

use std::sync::Arc;
use tractor::{parse, Match, ParseInput, ParseOptions, XPathEngine, XeeParseResult};

fn parse_src(lang: &str, source: &str) -> XeeParseResult {
    parse(
        ParseInput::Inline { content: source, file_label: "<semantic_tree_test>" },
        ParseOptions {
            language: Some(lang),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
        },
    )
    .expect("parse should succeed")
}

fn query(tree: &mut XeeParseResult, xpath: &str) -> Vec<Match> {
    let engine = XPathEngine::new();
    engine
        .query_documents(
            &mut tree.documents,
            tree.doc_handle,
            xpath,
            tree.source_lines.clone(),
            &tree.file_path,
        )
        .unwrap_or_else(|e| panic!("query `{}` failed: {:?}", xpath, e))
}

/// Assert the query matches exactly `expected` nodes. `invariant`
/// names the design rule being enforced — surfaces in the failure
/// message so reviewers know why the assertion exists.
#[track_caller]
fn assert_count(tree: &mut XeeParseResult, xpath: &str, expected: usize, invariant: &str) {
    let got = query(tree, xpath).len();
    assert_eq!(
        got, expected,
        "Invariant violated — {}\n  query: `{}`\n  matched {} nodes, expected {}",
        invariant, xpath, got, expected
    );
}

/// Assert the query returns at least one match whose text value
/// equals `expected`.
#[track_caller]
fn assert_value(tree: &mut XeeParseResult, xpath: &str, expected: &str, invariant: &str) {
    let matches = query(tree, xpath);
    if matches.is_empty() {
        panic!(
            "Invariant violated — {}\n  query: `{}`\n  returned no matches (expected value {:?})",
            invariant, xpath, expected
        );
    }
    let got = &matches[0].value;
    assert_eq!(
        got, expected,
        "Invariant violated — {}\n  query: `{}`\n  first match value = {:?}, expected {:?}",
        invariant, xpath, got, expected
    );
}

/// Silence unused-Arc warning on platforms that don't see all helpers used.
#[allow(dead_code)]
fn _arc_sentinel(_: Arc<Vec<String>>) {}

// ===========================================================================
// C#
// ===========================================================================

mod csharp {
    use super::*;

    /// Principle #14 — every type reference wraps in `<type>` with a
    /// `<name>` text leaf. Regression guard: attribute names used to
    /// double-wrap as `<attribute><name><name>Foo</name></name>`.
    #[test]
    fn attribute_name_is_text_leaf() {
        let mut tree = parse_src(
            "csharp",
            "class X { [MaxLength(50)] public string Name; }",
        );
        assert_count(
            &mut tree,
            "//attribute/name",
            1,
            "attribute must have exactly one <name> child (Principle #14)",
        );
        assert_value(
            &mut tree,
            "//attribute/name",
            "MaxLength",
            "attribute <name> holds the identifier as text",
        );
        assert_count(
            &mut tree,
            "//attribute/name/*",
            0,
            "attribute <name> is a text leaf — no element children (Principle #14)",
        );
    }

    /// Principle #12 — accessor lists flatten: get/set are direct
    /// siblings of the property, no `<accessor_list>` wrapper.
    #[test]
    fn accessor_list_flattens() {
        let mut tree = parse_src("csharp", "class X { int P { get; set; } }");
        assert_count(
            &mut tree,
            "//property/accessor_list",
            0,
            "no <accessor_list> wrapper (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//property/accessor",
            2,
            "accessors are direct property siblings",
        );
    }

    /// Principle #13 — accessor kind is an empty marker (`<get/>`,
    /// `<set/>`, `<init/>`). Uniform across auto-form and bodied-form.
    #[test]
    fn accessor_kind_is_queryable_marker() {
        let mut auto = parse_src("csharp", "class X { int P { get; set; } }");
        let mut bodied = parse_src(
            "csharp",
            "class X { int P { get { return 1; } set { } } }",
        );
        assert_count(
            &mut auto,
            "//accessor[get]",
            1,
            "auto-form get accessor carries <get/> marker (Principle #13)",
        );
        assert_count(
            &mut auto,
            "//accessor[set]",
            1,
            "auto-form set accessor carries <set/> marker (Principle #13)",
        );
        assert_count(
            &mut bodied,
            "//accessor[get]",
            1,
            "bodied get accessor carries <get/> marker — uniform with auto-form",
        );
        assert_count(
            &mut bodied,
            "//accessor[set]",
            1,
            "bodied set accessor carries <set/> marker — uniform with auto-form",
        );
    }

    /// Interface members are implicitly public; tractor surfaces this
    /// with a `<public/>` marker on every interface member (Principle
    /// #9 exhaustive markers).
    #[test]
    fn interface_members_default_public() {
        let mut tree = parse_src(
            "csharp",
            "interface IX { double Area(); double Perimeter(); }",
        );
        assert_count(
            &mut tree,
            "//interface/body/method[public]",
            2,
            "interface methods carry <public/> even when not written",
        );
    }

    /// Principle #14 on base lists: `class Foo : Bar` wraps `Bar` in
    /// `<type><name>Bar</name></type>`, not bare text.
    #[test]
    fn base_class_is_typed() {
        let mut tree = parse_src("csharp", "class Dog : Animal {}");
        assert_count(
            &mut tree,
            "//class/extends/type[name='Animal']",
            1,
            "base class wraps in <type><name/></type> (Principle #14)",
        );
    }

    /// `where`-clause constraints attach to the matching `<generic>`.
    /// Shape constraints become empty markers; type bounds wrap in
    /// `<extends><type>…</type></extends>`.
    #[test]
    fn where_clause_attaches_to_generic() {
        let mut tree = parse_src(
            "csharp",
            "class Repo<T, U> where T : class, IComparable<T>, new() where U : struct {}",
        );
        assert_count(
            &mut tree,
            "//type_parameter_constraints_clause",
            0,
            "where clauses dissolve into their generics",
        );
        assert_count(
            &mut tree,
            "//generic[name='T'][class]",
            1,
            "T has <class/> shape-constraint marker",
        );
        assert_count(
            &mut tree,
            "//generic[name='T'][new]",
            1,
            "T has <new/> constructor-constraint marker",
        );
        assert_count(
            &mut tree,
            "//generic[name='T']/extends/type[name='IComparable']",
            1,
            "IComparable bound attaches as <extends><type>…</type></extends>",
        );
        assert_count(
            &mut tree,
            "//generic[name='U'][struct]",
            1,
            "U has <struct/> marker",
        );
    }

    /// Principle #5 — expression_statement renames to `<expression>`
    /// (not the raw tree-sitter kind).
    #[test]
    fn expression_statement_renames() {
        let mut tree = parse_src(
            "csharp",
            "class X { void F() { int y = 0; y = 1; } }",
        );
        assert_count(
            &mut tree,
            "//expression_statement",
            0,
            "no raw tree-sitter kind leak",
        );
        assert_count(
            &mut tree,
            "//expression",
            1,
            "y = 1 renders as <expression>",
        );
    }

    /// Principle #12 — parameters flatten: no `<parameter_list>` wrapper.
    #[test]
    fn parameters_flatten() {
        let mut tree = parse_src("csharp", "class X { void F(int a, string b) {} }");
        assert_count(
            &mut tree,
            "//method/parameter_list",
            0,
            "no <parameter_list> wrapper (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//method/parameter",
            2,
            "parameters are direct method siblings",
        );
    }
}

// ===========================================================================
// TypeScript / JavaScript
// ===========================================================================

mod typescript {
    use super::*;

    /// Principle #5 — arrow_function renames to `<arrow>` (JS-native
    /// vocabulary; distinct from `<function>` declarations).
    #[test]
    fn arrow_function_renames() {
        let mut tree = parse_src(
            "typescript",
            "const f = (x: number) => x + 1;",
        );
        assert_count(
            &mut tree,
            "//arrow",
            1,
            "arrow_function renames to <arrow>",
        );
        assert_count(
            &mut tree,
            "//arrow_function",
            0,
            "no raw tree-sitter kind leak",
        );
    }

    /// Principle #14 — type references wrap in `<type><name>…</name></type>`.
    #[test]
    fn type_reference_is_wrapped() {
        let mut tree = parse_src(
            "typescript",
            "function f(x: number): string { return \"\"; }",
        );
        assert_count(
            &mut tree,
            "//param/type[name='number']",
            1,
            "parameter type wraps name in <name> (Principle #14)",
        );
        assert_count(
            &mut tree,
            "//returns/type[name='string']",
            1,
            "return type wraps name in <name> (Principle #14)",
        );
    }

    /// Principle #13 — `<function/>` marker distinguishes function types.
    /// `(x: T) => U` as a type renders as `<type><function/>…`.
    #[test]
    fn function_type_carries_marker() {
        let mut tree = parse_src(
            "typescript",
            "type Handler = (x: number) => void;",
        );
        assert_count(
            &mut tree,
            "//alias/type[function]",
            1,
            "function type carries <function/> marker (Principle #13)",
        );
        assert_count(
            &mut tree,
            "//function_type",
            0,
            "no raw function_type leak",
        );
    }

    /// Principle #9 — parameters carry exhaustive required/optional markers.
    #[test]
    fn parameters_marked_required_or_optional() {
        let mut tree = parse_src(
            "typescript",
            "function f(a: string, b?: number) {}",
        );
        assert_count(
            &mut tree,
            "//param[required]",
            1,
            "required param carries <required/>",
        );
        assert_count(
            &mut tree,
            "//param[optional]",
            1,
            "optional param carries <optional/>",
        );
    }

    /// Principle #13 — async and generator become empty markers.
    #[test]
    fn async_generator_markers() {
        let mut tree = parse_src(
            "typescript",
            "async function a() {} function* b() {} async function* c() {}",
        );
        assert_count(
            &mut tree,
            "//function[async]",
            2,
            "two async functions",
        );
        assert_count(
            &mut tree,
            "//function[generator]",
            2,
            "two generator functions",
        );
        assert_count(
            &mut tree,
            "//function[async][generator]",
            1,
            "one async-generator function — markers compose",
        );
    }

    /// Principle #13 — get/set on class methods become markers.
    #[test]
    fn accessor_methods_marked() {
        let mut tree = parse_src(
            "typescript",
            "class X { get a(): number { return 1; } set a(v: number) {} }",
        );
        assert_count(
            &mut tree,
            "//method[get]",
            1,
            "get accessor carries <get/> marker",
        );
        assert_count(
            &mut tree,
            "//method[set]",
            1,
            "set accessor carries <set/> marker",
        );
    }

    /// Conditional shape: else-if chain flattens; ternary keeps
    /// `<then>`/`<else>` via surgical field wrap.
    #[test]
    fn conditional_shape_flat() {
        let mut tree = parse_src(
            "typescript",
            "function f(n: number) { if (n < 0) {} else if (n == 0) {} else if (n < 10) {} else {} }",
        );
        assert_count(
            &mut tree,
            "//if/else_if",
            2,
            "else_if siblings are flat children of outer <if>",
        );
        assert_count(
            &mut tree,
            "//if/else",
            1,
            "final <else> is also a flat sibling",
        );
        assert_count(
            &mut tree,
            "//else/if",
            0,
            "no nested <else><if>…",
        );
    }

    /// Ternary keeps `<then>`/`<else>` children.
    #[test]
    fn ternary_has_then_and_else() {
        let mut tree = parse_src(
            "typescript",
            "const x = cond ? 1 : 2;",
        );
        assert_count(
            &mut tree,
            "//ternary/then",
            1,
            "ternary keeps <then>",
        );
        assert_count(
            &mut tree,
            "//ternary/else",
            1,
            "ternary keeps <else>",
        );
    }

    /// Principle #14: `extends`/`implements` wrap in `<type><name/></type>`.
    #[test]
    fn extends_implements_typed() {
        let mut tree = parse_src(
            "typescript",
            "class Dog extends Animal implements Barker {}",
        );
        assert_count(
            &mut tree,
            "//class/extends/type[name='Animal']",
            1,
            "extends target is typed",
        );
        assert_count(
            &mut tree,
            "//class/implements/type[name='Barker']",
            1,
            "implements target is typed",
        );
    }

    /// Type parameter declaration: `<generic><name>T</name></generic>`.
    #[test]
    fn type_parameter_inner_shape() {
        let mut tree = parse_src(
            "typescript",
            "class Box<T> { value: T; }",
        );
        assert_count(
            &mut tree,
            "//generic[name='T']",
            1,
            "type parameter has <name> child holding T (not nested type)",
        );
        assert_count(
            &mut tree,
            "//generic/name/type",
            0,
            "no spurious <type> wrapper inside the <name>",
        );
    }
}

// ===========================================================================
// Java
// ===========================================================================

mod java {
    use super::*;

    /// Principle #2 — constructor_declaration renames to `<constructor>`,
    /// not the abbreviation `<ctor>`.
    #[test]
    fn constructor_is_full_word() {
        let mut tree = parse_src(
            "java",
            "class Point { Point(int x, int y) {} }",
        );
        assert_count(
            &mut tree,
            "//constructor",
            1,
            "constructor_declaration renames to <constructor> (Principle #2)",
        );
        assert_count(
            &mut tree,
            "//ctor",
            0,
            "no abbreviated <ctor>",
        );
    }

    /// Principle #14: extends/implements wrap in `<type>`.
    #[test]
    fn extends_implements_typed() {
        let mut tree = parse_src(
            "java",
            "class Dog extends Animal implements Barker {}",
        );
        assert_count(
            &mut tree,
            "//class/extends/type[name='Animal']",
            1,
            "extends target typed",
        );
        assert_count(
            &mut tree,
            "//class/implements/type[name='Barker']",
            1,
            "implements target typed",
        );
    }

    /// Interface members default to `<public/>`.
    #[test]
    fn interface_members_default_public() {
        let mut tree = parse_src(
            "java",
            "interface Shape { double area(); double perimeter(); }",
        );
        assert_count(
            &mut tree,
            "//interface/body/method[public]",
            2,
            "interface methods are implicitly public",
        );
    }

    /// Principle #1 — package-private access renders as `<package/>`
    /// (matches Java's own term; earlier spelling `<package-private/>`
    /// broke XPath predicate syntax).
    #[test]
    fn package_private_marker() {
        let mut tree = parse_src(
            "java",
            "class X { int pkg; private int priv; }",
        );
        assert_count(
            &mut tree,
            "//field[package]",
            1,
            "package-private field carries <package/> marker",
        );
        assert_count(
            &mut tree,
            "//field[private]",
            1,
            "private field carries <private/> marker",
        );
    }

    /// Type parameter with bound: `<generic><name>T</name><extends>…</extends></generic>`.
    #[test]
    fn type_parameter_with_bound() {
        let mut tree = parse_src(
            "java",
            "class Box<T extends Comparable<T>> {}",
        );
        assert_count(
            &mut tree,
            "//generic[name='T']/extends/type[name='Comparable']",
            1,
            "bound attaches as <extends><type>…</type></extends>",
        );
    }

    /// Principle #12 — parameters flatten.
    #[test]
    fn parameters_flatten() {
        let mut tree = parse_src(
            "java",
            "class X { void f(int a, String b) {} }",
        );
        assert_count(
            &mut tree,
            "//method/parameter_list",
            0,
            "no parameter_list wrapper (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//method/param",
            2,
            "params are direct method siblings",
        );
    }

    /// Conditional shape: flat else-if chain.
    #[test]
    fn conditional_shape_flat() {
        let mut tree = parse_src(
            "java",
            "class X { String f(int n) { if (n<0) return \"\"; else if (n==0) return \"\"; else return \"\"; } }",
        );
        assert_count(
            &mut tree,
            "//if/else_if",
            1,
            "else_if is flat sibling of <if>",
        );
        assert_count(
            &mut tree,
            "//if/else",
            1,
            "final else is flat sibling",
        );
    }

    /// Primitive types render as `<type>` with an empty marker
    /// carrying the keyword — `<type><int/>int</type>`. User-defined
    /// types keep `<type><name>Foo</name></type>`.
    #[test]
    fn primitive_types_use_markers() {
        let mut tree = parse_src(
            "java",
            "class X { int a; double b; boolean c; void d() {} Foo e; }",
        );
        assert_count(
            &mut tree,
            "//type[int]",
            1,
            "int primitive carries <int/> marker",
        );
        assert_count(
            &mut tree,
            "//type[double]",
            1,
            "double primitive carries <double/> marker",
        );
        assert_count(
            &mut tree,
            "//type[boolean]",
            1,
            "boolean primitive carries <boolean/> marker",
        );
        assert_count(
            &mut tree,
            "//type[void]",
            1,
            "void carries <void/> marker",
        );
        assert_count(
            &mut tree,
            "//type[name='Foo']",
            1,
            "user-defined type keeps <name> for the identifier",
        );
    }

    /// Principle #12 — parenthesized_expression is grammar bleed-through;
    /// drop the wrapper so inner expressions sit directly under their
    /// enclosing node. The parens remain as text children.
    #[test]
    fn parenthesized_expression_flattens() {
        let mut tree = parse_src(
            "java",
            "class X { boolean f(int n) { return (n + 1) > 0; } }",
        );
        assert_count(
            &mut tree,
            "//parenthesized_expression",
            0,
            "no parenthesized_expression wrapper (Principle #12)",
        );
    }

    /// `this(…)` / `super(…)` in constructors render as `<call>` with
    /// a `<this/>` or `<super/>` marker — uniform with other call sites.
    #[test]
    fn explicit_constructor_invocation_is_call() {
        let mut tree = parse_src(
            "java",
            "class X { X() { this(1); } X(int a) {} class Y extends X { Y() { super(2); } } }",
        );
        assert_count(
            &mut tree,
            "//call[this]",
            1,
            "this(…) renders as <call> with <this/> marker",
        );
        assert_count(
            &mut tree,
            "//call[super]",
            1,
            "super(…) renders as <call> with <super/> marker",
        );
        assert_count(
            &mut tree,
            "//explicit_constructor_invocation",
            0,
            "no raw tree-sitter kind leak",
        );
    }

    /// Principle #2 — `variable_declarator` renames to `<declarator>`
    /// (no underscores in the final vocabulary, short but not
    /// abbreviated).
    #[test]
    fn variable_declarator_renames() {
        let mut tree = parse_src("java", "class X { void f() { int x = 1, y = 2; } }");
        assert_count(
            &mut tree,
            "//variable_declarator",
            0,
            "no raw kind leak",
        );
        assert_count(
            &mut tree,
            "//variable/declarator",
            2,
            "each declarator in a multi-variable declaration is its own <declarator>",
        );
    }
}

// ===========================================================================
// Rust
// ===========================================================================

mod rust {
    use super::*;

    /// Principle #14: every type reference wraps in `<type><name>…</name></type>`.
    #[test]
    fn type_reference_is_wrapped() {
        let mut tree = parse_src(
            "rust",
            "fn f(x: i32) -> String { String::new() }",
        );
        assert_count(
            &mut tree,
            "//param/type[name='i32']",
            1,
            "parameter type wraps name in <name>",
        );
        assert_count(
            &mut tree,
            "//returns/type[name='String']",
            1,
            "return type wraps name in <name>",
        );
    }

    /// Reference types render as `<type><borrowed/>…<type>T</type></type>`.
    #[test]
    fn reference_type_uses_borrowed_marker() {
        let mut tree = parse_src(
            "rust",
            "fn read(s: &str) -> &str { s } fn write(b: &mut Vec<u8>) {}",
        );
        assert_count(
            &mut tree,
            "//type[borrowed]",
            3,
            "every reference type carries <borrowed/> (Principle #14 + #13)",
        );
        assert_count(
            &mut tree,
            "//type[borrowed][mut]",
            1,
            "mutable borrow carries both markers — they compose",
        );
        assert_count(
            &mut tree,
            "//ref",
            0,
            "no legacy <ref> element",
        );
    }

    /// struct_expression renders as `<literal><name>Point</name>…</literal>`.
    #[test]
    fn struct_expression_is_literal() {
        let mut tree = parse_src(
            "rust",
            "struct Point { x: i32 } fn make() { let p = Point { x: 1 }; }",
        );
        assert_count(
            &mut tree,
            "//literal[name='Point']",
            1,
            "struct construction renders as <literal><name>Point</name>…",
        );
        assert_count(
            &mut tree,
            "//literal/body/field",
            1,
            "initializers are <field> children inside the body",
        );
        assert_count(
            &mut tree,
            "//struct_expression",
            0,
            "no raw tree-sitter kind leak",
        );
    }

    /// Principle #12: match_block flattens; arms live under match's body.
    #[test]
    fn match_block_flattens() {
        let mut tree = parse_src(
            "rust",
            "fn f(n: i32) { match n { 0 => {}, _ => {} } }",
        );
        assert_count(
            &mut tree,
            "//match/match_block",
            0,
            "no <match_block> wrapper (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//match/body/arm",
            2,
            "arms live directly under match's body — no extra wrapper",
        );
    }

    /// Principle #5: method_call_expression and call_expression both
    /// render as `<call>` (unified; member access child distinguishes).
    #[test]
    fn method_call_unifies_with_call() {
        let mut tree = parse_src(
            "rust",
            "fn f() { let s = String::new(); s.len(); }",
        );
        assert_count(
            &mut tree,
            "//call",
            2,
            "both free call and method call render as <call>",
        );
        assert_count(
            &mut tree,
            "//methodcall",
            0,
            "no legacy <methodcall>",
        );
    }

    /// Principle #2: type_item renders as `<alias>` (parallel with
    /// TS / Java / C#).
    #[test]
    fn type_item_is_alias() {
        let mut tree = parse_src("rust", "type Id = u32;");
        assert_count(
            &mut tree,
            "//alias[name='Id']",
            1,
            "type_item renames to <alias>",
        );
        assert_count(
            &mut tree,
            "//typedef",
            0,
            "no legacy <typedef>",
        );
    }

    /// Visibility is exhaustive: every declaration carries `<private/>`
    /// (implicit) or `<pub/>` (explicit).
    #[test]
    fn visibility_is_exhaustive() {
        let mut tree = parse_src(
            "rust",
            "fn priv_fn() {} pub fn pub_fn() {} pub(crate) fn crate_fn() {}",
        );
        assert_count(
            &mut tree,
            "//function[private]",
            1,
            "implicit-private function carries <private/>",
        );
        assert_count(
            &mut tree,
            "//function[pub]",
            2,
            "pub and pub(crate) both carry <pub/>",
        );
        assert_count(
            &mut tree,
            "//function/pub/crate",
            1,
            "pub(crate) carries <crate/> restriction child",
        );
    }

    /// Raw string literal renders as `<string><raw/>…</string>`.
    #[test]
    fn raw_string_has_marker() {
        let mut tree = parse_src(
            "rust",
            "fn f() { let _ = r\"raw\"; let _ = \"normal\"; }",
        );
        assert_count(
            &mut tree,
            "//string[raw]",
            1,
            "raw string carries <raw/> marker",
        );
        assert_count(
            &mut tree,
            "//string",
            2,
            "both raw and normal strings use <string>",
        );
    }
}

// ===========================================================================
// Python
// ===========================================================================

mod python {
    use super::*;

    /// `elif` renames to `<else_if>` and flattens under the outer `<if>`.
    #[test]
    fn conditional_shape_flat() {
        let mut tree = parse_src(
            "python",
            "def f(n):\n    if n < 0:\n        return 1\n    elif n == 0:\n        return 2\n    else:\n        return 3\n",
        );
        assert_count(
            &mut tree,
            "//if/else_if",
            1,
            "elif renames to <else_if> and is a flat sibling",
        );
        assert_count(
            &mut tree,
            "//if/else",
            1,
            "else is a flat sibling",
        );
        assert_count(
            &mut tree,
            "//elif_clause",
            0,
            "no raw elif_clause leak",
        );
    }

    /// Collections carry exhaustive literal/comprehension markers.
    #[test]
    fn collection_markers_exhaustive() {
        let mut tree = parse_src(
            "python",
            "a = [1, 2]\nb = [x for x in a]\nc = {1: 2}\nd = {k: v for k, v in c.items()}\n",
        );
        assert_count(
            &mut tree,
            "//list[literal]",
            1,
            "list literal carries <literal/>",
        );
        assert_count(
            &mut tree,
            "//list[comprehension]",
            1,
            "list comprehension carries <comprehension/>",
        );
        assert_count(
            &mut tree,
            "//dict[literal]",
            1,
            "dict literal carries <literal/>",
        );
        assert_count(
            &mut tree,
            "//dict[comprehension]",
            1,
            "dict comprehension carries <comprehension/>",
        );
    }

    /// Goal #5 — augmented_assignment unifies with assignment as
    /// `<assign>` plus an `<op>` child. Uses `left` child to
    /// discriminate statement `<assign>` from any `<assign>` marker
    /// that may appear inside `<op>`.
    #[test]
    fn augmented_assignment_unifies() {
        let mut tree = parse_src("python", "x = 0\nx += 1\nx *= 2\n");
        assert_count(
            &mut tree,
            "//assign[left]",
            3,
            "plain and augmented assignments both render as <assign> (with <left> child)",
        );
        assert_count(
            &mut tree,
            "//assign[left]/op",
            2,
            "augmented assignments carry an <op> child",
        );
        assert_count(
            &mut tree,
            "//augmented_assignment",
            0,
            "no raw kind leak",
        );
    }

    /// Principle #12 — expression_list flattens: tuple returns render
    /// as `<return>` with expressions as direct children.
    #[test]
    fn expression_list_flattens() {
        let mut tree = parse_src("python", "def f():\n    return 1, 2\n");
        assert_count(
            &mut tree,
            "//return/expression_list",
            0,
            "no <expression_list> wrapper (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//return/int",
            2,
            "expressions are direct return children",
        );
    }

    /// f-string internals flatten: string_start / string_content /
    /// string_end become bare text. Interpolation preserved.
    #[test]
    fn fstring_flattens() {
        let mut tree = parse_src("python", "m = f\"hi {name}\"\n");
        assert_count(
            &mut tree,
            "//string_content",
            0,
            "string_content flattens to text (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//string_start",
            0,
            "string_start flattens",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/name[.='name']",
            1,
            "<interpolation> preserved as wrapper around the expression",
        );
    }
}

// ===========================================================================
// Go
// ===========================================================================

mod go {
    use super::*;

    /// `type Foo struct { … }` hoists: outer element is `<struct>`,
    /// not `<type>`.
    #[test]
    fn struct_hoists_out_of_type_wrapper() {
        let mut tree = parse_src(
            "go",
            "package main\ntype Config struct { Host string }\n",
        );
        assert_count(
            &mut tree,
            "//struct[name='Config']",
            1,
            "struct declaration renders as <struct> (Goal #5)",
        );
        assert_count(
            &mut tree,
            "//struct/field[name='Host']",
            1,
            "struct fields are flat children",
        );
    }

    /// `type Foo interface { … }` hoists: outer element is `<interface>`.
    #[test]
    fn interface_hoists_out_of_type_wrapper() {
        let mut tree = parse_src(
            "go",
            "package main\ntype Greeter interface { Greet() string }\n",
        );
        assert_count(
            &mut tree,
            "//interface[name='Greeter']",
            1,
            "interface declaration renders as <interface>",
        );
    }

    /// `type MyInt int` → `<type>`; `type Color = int` → `<alias>`.
    #[test]
    fn defined_type_vs_alias() {
        let mut tree = parse_src(
            "go",
            "package main\ntype MyInt int\ntype Color = int\n",
        );
        assert_count(
            &mut tree,
            "//type[name='MyInt']",
            1,
            "defined type renders as <type>",
        );
        assert_count(
            &mut tree,
            "//alias[name='Color']",
            1,
            "type alias (with =) renders as <alias>",
        );
    }

    /// Raw string literal carries a `<raw/>` marker.
    #[test]
    fn raw_string_has_marker() {
        let mut tree = parse_src(
            "go",
            "package main\nvar a = `raw`\nvar b = \"normal\"\n",
        );
        assert_count(
            &mut tree,
            "//string[raw]",
            1,
            "raw string carries <raw/> marker",
        );
        assert_count(
            &mut tree,
            "//string",
            2,
            "both string forms render as <string>",
        );
    }

    /// Principle #9 — every declaration carries an exhaustive
    /// <exported/> or <unexported/> marker, inferred from the name's
    /// first character. No declaration is "unmarked" / silently-default.
    #[test]
    fn exported_unexported_markers_exhaustive() {
        let mut tree = parse_src(
            "go",
            "package main\nfunc Public() {}\nfunc private() {}\ntype Exported int\ntype unexported int\n",
        );
        assert_count(
            &mut tree,
            "//function[exported]",
            1,
            "exported function carries <exported/>",
        );
        assert_count(
            &mut tree,
            "//function[unexported]",
            1,
            "unexported function carries <unexported/>",
        );
        assert_count(
            &mut tree,
            "//type[exported]",
            1,
            "exported type carries <exported/>",
        );
        assert_count(
            &mut tree,
            "//type[unexported]",
            1,
            "unexported type carries <unexported/>",
        );
        // No declaration is silently unmarked.
        assert_count(
            &mut tree,
            "//function[not(exported) and not(unexported)]",
            0,
            "every function carries one of the two markers",
        );
    }

    /// Principle #12 — parameters flatten.
    #[test]
    fn parameters_flatten() {
        let mut tree = parse_src(
            "go",
            "package main\nfunc f(a string, b int) {}\n",
        );
        assert_count(
            &mut tree,
            "//function/parameter_list",
            0,
            "no parameter_list wrapper",
        );
        assert_count(
            &mut tree,
            "//function/parameter",
            2,
            "parameters are flat function siblings (Principle #2 — full name, not `param`)",
        );
    }

    /// Conditional shape: Go's tree-sitter doesn't wrap the `else`
    /// branch in an `else_clause` element, so the transform has to
    /// surgically wrap the `alternative` field in <else>. Without
    /// that, the shared collapse post-transform leaves else-if as
    /// a nested <if> sibling of the outer, producing a broken chain.
    #[test]
    fn conditional_shape_flat() {
        let mut tree = parse_src(
            "go",
            "package main\nfunc f(n int) string {\n    if n < 0 { return \"\" } else if n == 0 { return \"\" } else { return \"\" }\n}\n",
        );
        assert_count(
            &mut tree,
            "//if/else_if",
            1,
            "else_if is a flat sibling of <if>",
        );
        assert_count(
            &mut tree,
            "//if/else",
            1,
            "final else is a flat sibling",
        );
        assert_count(
            &mut tree,
            "//else/if",
            0,
            "no nested <else><if>...",
        );
    }
}

// ===========================================================================
// Ruby
// ===========================================================================

mod ruby {
    use super::*;

    /// Principle #14 — Ruby identifiers unconditionally rename to
    /// `<name>` (Ruby has no type_identifier, every identifier is a
    /// value reference).
    #[test]
    fn identifier_renames_to_name() {
        let mut tree = parse_src("ruby", "def f(a, b)\n  a + b\nend\n");
        assert_count(
            &mut tree,
            "//identifier",
            0,
            "no raw identifier leak",
        );
        assert_count(
            &mut tree,
            "//binary/left/name[.='a']",
            1,
            "identifiers render as <name>",
        );
    }

    /// Class and module names inline the `<constant>` child (Ruby
    /// uses `constant` for capitalized identifiers).
    #[test]
    fn class_and_module_name_inline() {
        let mut tree = parse_src(
            "ruby",
            "class Calculator\nend\nmodule Utils\nend\n",
        );
        assert_count(
            &mut tree,
            "//class/name[.='Calculator']",
            1,
            "class name inlines as text",
        );
        assert_count(
            &mut tree,
            "//class/name/constant",
            0,
            "no nested <constant> inside <name>",
        );
        assert_count(
            &mut tree,
            "//module/name[.='Utils']",
            1,
            "module name inlines as text",
        );
    }

    /// Ruby elsif chain flattens: `<else_if>` is a direct sibling of
    /// `<if>` (tree-sitter nests elsif inside the previous elsif/else).
    #[test]
    fn conditional_shape_flat() {
        let mut tree = parse_src(
            "ruby",
            "def f(n)\n  if n < 0\n    1\n  elsif n == 0\n    2\n  else\n    3\n  end\nend\n",
        );
        assert_count(
            &mut tree,
            "//if/else_if",
            1,
            "elsif renames to <else_if> and lifts to flat sibling",
        );
        assert_count(
            &mut tree,
            "//if/else",
            1,
            "else is a flat sibling",
        );
        assert_count(
            &mut tree,
            "//elsif",
            0,
            "no raw <elsif> leaks",
        );
    }
}