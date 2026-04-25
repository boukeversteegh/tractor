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

/// Reason-first shape claim — same effect as `assert_count` but the
/// reason reads before the technical XPath, which is much easier to
/// scan in lists of consecutive claims about a single tree.
///
/// Convention: `claim("reason it should hold", tree, xpath, expected)`.
#[track_caller]
fn claim(reason: &str, tree: &mut XeeParseResult, xpath: &str, expected: usize) {
    let got = query(tree, xpath).len();
    assert_eq!(
        got, expected,
        "Shape claim violated — {}\n  query: `{}`\n  matched {} nodes, expected {}",
        reason, xpath, got, expected
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

    /// C# type flavors — array/tuple/nullable — all collapse to
    /// `<type>` with a shape marker. `nullable_type` gets its
    /// `<nullable/>` marker via a direct rewrite (not the map) but
    /// the end shape is the same.
    #[test]
    fn type_shape_markers() {
        let mut tree = parse_src(
            "csharp",
            "class X {\
                int[] a;\
                (int, string) t;\
                int? n;\
            }",
        );
        assert_count(&mut tree, "//type[array]", 1, "array type carries <array/>");
        assert_count(&mut tree, "//type[tuple]", 1, "tuple type carries <tuple/>");
        // nullable_type produces <type>…<nullable/></type>
        assert_count(
            &mut tree,
            "//type[nullable]",
            1,
            "nullable type carries <nullable/>",
        );
    }

    /// C# pattern flavors all collapse to `<pattern>` but carry a
    /// shape marker (declaration / recursive / constant / tuple).
    #[test]
    fn pattern_shape_markers() {
        let mut tree = parse_src(
            "csharp",
            "class X {\
                void F(object o) {\
                    if (o is Point p) {}\
                    if (o is null) {}\
                }\
            }",
        );
        assert_count(
            &mut tree,
            "//pattern[declaration]",
            1,
            "`o is T name` — declaration pattern carries <declaration/>",
        );
        assert_count(
            &mut tree,
            "//pattern[constant]",
            1,
            "`o is null` — constant pattern carries <constant/>",
        );
    }
}

// ===========================================================================
// TypeScript / JavaScript
// ===========================================================================

mod typescript {
    use super::*;

    /// Principle #9 — class members carry an exhaustive visibility
    /// marker: explicit `public/private/protected` keywords lift to
    /// markers, and members without a keyword get an implicit
    /// `<public/>` (the default in TypeScript).
    #[test]
    fn visibility_markers_exhaustive() {
        let mut tree = parse_src(
            "typescript",
            "class X { foo() {} private bar() {} protected baz() {} public qux() {} }",
        );
        assert_count(
            &mut tree,
            "//method[public]",
            2,
            "implicit default and explicit public both carry <public/>",
        );
        assert_count(
            &mut tree,
            "//method[private]",
            1,
            "explicit private carries <private/>",
        );
        assert_count(
            &mut tree,
            "//method[protected]",
            1,
            "explicit protected carries <protected/>",
        );
    }

    /// Principle #9 — class fields follow the same visibility
    /// defaults as methods.
    #[test]
    fn field_visibility_defaults_public() {
        let mut tree = parse_src(
            "typescript",
            "class X { x = 1; private y = 2; }",
        );
        assert_count(
            &mut tree,
            "//field[public]",
            1,
            "unmarked field defaults to <public/>",
        );
        assert_count(
            &mut tree,
            "//field[private]",
            1,
            "explicit private field carries <private/>",
        );
    }

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
            "//parameter/type[name='number']",
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
            "//parameter[required]",
            1,
            "required parameter carries <required/>",
        );
        assert_count(
            &mut tree,
            "//parameter[optional]",
            1,
            "optional parameter carries <optional/>",
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

    /// Type flavors all collapse to `<type>` but carry a shape marker
    /// (Principle #9) so `//type[union]`, `//type[tuple]`, etc. work
    /// uniformly without matching on text.
    #[test]
    fn type_shape_markers() {
        let mut tree = parse_src(
            "typescript",
            "type A = string | number;\n\
             type B = string & object;\n\
             type C = [string, number];\n\
             type D = string[];\n\
             type E = 'idle';\n\
             type F = (x: number) => number;\n\
             type G = { x: number };\n\
             type H = readonly number[];",
        );
        assert_count(&mut tree, "//type[union]", 1, "union type carries <union/>");
        assert_count(
            &mut tree,
            "//type[intersection]",
            1,
            "intersection type carries <intersection/>",
        );
        assert_count(&mut tree, "//type[tuple]", 1, "tuple type carries <tuple/>");
        // `number[]` is array_type; `readonly number[]` wraps in readonly_type.
        assert_count(&mut tree, "//type[array]", 2, "array types carry <array/>");
        assert_count(&mut tree, "//type[literal]", 1, "literal type carries <literal/>");
        assert_count(&mut tree, "//type[function]", 1, "function type carries <function/>");
        assert_count(&mut tree, "//type[object]", 1, "object type carries <object/>");
        assert_count(&mut tree, "//type[readonly]", 1, "readonly type carries <readonly/>");
    }

    /// Destructuring patterns collapse to `<pattern>` but carry an
    /// `<array/>` / `<object/>` marker that distinguishes the shape
    /// without requiring string matching on `[` vs `{`.
    #[test]
    fn destructuring_pattern_markers() {
        let mut tree = parse_src(
            "typescript",
            "const [a, b] = xs;\nconst { x, y } = pt;\n",
        );
        assert_count(
            &mut tree,
            "//pattern[array]",
            1,
            "array destructuring pattern carries <array/>",
        );
        assert_count(
            &mut tree,
            "//pattern[object]",
            1,
            "object destructuring pattern carries <object/>",
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
            "//method/parameter",
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

    /// Identifiers — user-defined AND built-in type names — are
    /// NEVER promoted to element nodes. Nodes are always lowercase,
    /// but identifiers can have distinguishing capitalization
    /// (`List` vs `list`, `Dictionary` vs `dict`), so mapping them to
    /// node names would either lose the case distinction or break
    /// the all-lowercase rule. This applies uniformly to primitives
    /// (`int`, `double`) and user types (`Foo`): they all carry their
    /// identifier as a `<name>` value.
    #[test]
    fn type_names_are_name_child_not_node() {
        let mut tree = parse_src(
            "java",
            "class X { int a; double b; boolean c; Foo e; List l; }",
        );
        for (name, what) in &[
            ("int", "primitive"),
            ("double", "primitive"),
            ("boolean", "primitive"),
            ("Foo", "user-defined type"),
            ("List", "built-in capitalized type"),
        ] {
            assert_count(
                &mut tree,
                &format!("//type[name='{}']", name),
                1,
                &format!("{} {} uses <type><name>{}</name></type>", what, name, name),
            );
        }
    }

    /// `void` is the one primitive special enough to warrant a
    /// shortcut marker — it's return-only and "no value", not a
    /// regular data type. The marker is *additional*, not a
    /// replacement for `<name>`: the name is still there for data
    /// consumers, the marker is a query shortcut.
    #[test]
    fn void_carries_additional_marker() {
        let mut tree = parse_src("java", "class X { void f() {} int g() { return 0; } }");
        assert_count(
            &mut tree,
            "//type[void][name='void']",
            1,
            "void type has both <void/> marker AND <name>void</name>",
        );
        assert_count(
            &mut tree,
            "//type[void]",
            1,
            "exactly one void type in the source",
        );
        assert_count(
            &mut tree,
            "//type[not(void)]",
            1,
            "non-void types have no <void/> marker",
        );
    }

    /// Markers appear in source order (source-reversibility goal):
    /// `public abstract static class X` renders with markers in
    /// source sequence, and the source keywords survive as text so
    /// the enclosing node's string-value still reads like the
    /// source.
    #[test]
    fn modifier_source_order_and_text_preserved() {
        let mut tree = parse_src("java", "public abstract static class X {}");
        // The first three element children of <class> are the
        // modifiers in source order. Predicate-chained position
        // assertions let us pin this without a fragile full-tree
        // assertion.
        assert_count(
            &mut tree,
            "//class/*[1][self::public]",
            1,
            "first marker on class is <public/> (source order)",
        );
        assert_count(
            &mut tree,
            "//class/*[2][self::abstract]",
            1,
            "second marker on class is <abstract/>",
        );
        assert_count(
            &mut tree,
            "//class/*[3][self::static]",
            1,
            "third marker on class is <static/>",
        );
        // Source text survives as a text sibling so //class's
        // string-value still contains "public abstract static".
        assert_count(
            &mut tree,
            "//class[contains(., 'public abstract static')]",
            1,
            "source keywords preserved as dangling text (source-reversibility)",
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
            "//parameter/type[name='i32']",
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

    /// Rust type flavors all collapse to `<type>` with a shape marker
    /// — function, tuple, array, pointer, never, unit, dyn. (The `[T]`
    /// inside `&[T]` is treated as `array_type` by tree-sitter-rust,
    /// so `slice` markers only appear for explicit slice forms — which
    /// the cross-file blueprint snapshot covers separately.)
    #[test]
    fn type_shape_markers() {
        let mut tree = parse_src(
            "rust",
            "fn f(cb: fn(i32) -> i32, t: (i32, i32), a: [u8; 4], p: *const u8) -> ! { loop {} }\n\
             fn g() -> () {}\n\
             fn h(d: &dyn Drawable) {}\n",
        );
        assert_count(&mut tree, "//type[function]", 1, "fn type carries <function/>");
        assert_count(&mut tree, "//type[tuple]", 1, "tuple type carries <tuple/>");
        assert_count(&mut tree, "//type[array]", 1, "array type carries <array/>");
        assert_count(&mut tree, "//type[pointer]", 1, "pointer type carries <pointer/>");
        assert_count(&mut tree, "//type[never]", 1, "never type carries <never/>");
        assert_count(&mut tree, "//type[unit]", 1, "unit type carries <unit/>");
        assert_count(&mut tree, "//type[dynamic]", 1, "dyn trait object carries <dynamic/>");
    }

    /// Pattern flavors in match arms collapse to `<pattern>` but carry
    /// `<or/>`, `<struct/>`, or `<field/>` markers so queries can
    /// pick out the specific shape.
    #[test]
    fn pattern_shape_markers() {
        let mut tree = parse_src(
            "rust",
            "fn f(x: Shape) {\n    match x {\n        Shape::Square(_) | Shape::Circle(_) => {},\n        Shape::Rect { w, h } => {},\n        _ => {},\n    }\n}\n",
        );
        assert_count(
            &mut tree,
            "//pattern[or]",
            1,
            "alternative pattern (`A | B`) carries <or/>",
        );
        assert_count(
            &mut tree,
            "//pattern[struct]",
            1,
            "struct destructure pattern carries <struct/>",
        );
        assert_count(
            &mut tree,
            "//pattern[field]",
            2,
            "each struct field in pattern carries <field/>",
        );
    }

    /// Both bare function calls and method calls collapse to `<call>`.
    /// Tree-sitter-rust doesn't emit a distinct `method_call_expression`
    /// (it uses `call_expression` with a `field_expression` function
    /// child), so `//call/field` finds every `obj.m(args)` site.
    #[test]
    fn method_call_via_field_child() {
        let mut tree = parse_src(
            "rust",
            "fn f() { let y = foo(1); let z = bar.baz(2); }\n",
        );
        assert_count(
            &mut tree,
            "//call/field",
            1,
            "method call has a <field> child function (`obj.m`)",
        );
        assert_count(
            &mut tree,
            "//call",
            2,
            "both function and method calls collapse to <call>",
        );
    }
}

// ===========================================================================
// Python
// ===========================================================================

mod python {
    use super::*;

    /// Principle #9 — class methods carry a visibility marker driven
    /// by Python's naming convention: bare → public, `_x` → protected,
    /// `__x` → private. Dunders (`__init__`) are conventional protocol
    /// hooks and count as public.
    #[test]
    fn visibility_markers_from_underscore_convention() {
        let mut tree = parse_src(
            "python",
            "class X:\n    def foo(self): pass\n    def _bar(self): pass\n    def __baz(self): pass\n    def __init__(self): pass\n",
        );
        assert_count(
            &mut tree,
            "//function[public]",
            2,
            "bare name and dunder both count as public",
        );
        assert_count(
            &mut tree,
            "//function[protected]",
            1,
            "single-underscore prefix means protected",
        );
        assert_count(
            &mut tree,
            "//function[private]",
            1,
            "double-underscore prefix means private",
        );
    }

    /// Module-level functions don't get visibility markers — the
    /// convention only applies to class members.
    #[test]
    fn module_level_functions_no_visibility() {
        let mut tree = parse_src(
            "python",
            "def foo(): pass\ndef _bar(): pass\n",
        );
        assert_count(
            &mut tree,
            "//function[public]",
            0,
            "module-level functions skip the visibility injection",
        );
        assert_count(
            &mut tree,
            "//function[protected]",
            0,
            "module-level functions skip the visibility injection",
        );
    }

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

    /// `*args` and `**kwargs` collapse to `<spread>` but carry a
    /// `<list/>` / `<dict/>` marker that survives argument, pattern,
    /// and literal contexts so shape queries work without string
    /// matching on `*` / `**` operator text.
    #[test]
    fn spread_shape_markers() {
        let mut tree = parse_src(
            "python",
            "def f(*args, **kwargs): pass\ng(*xs, **kw)\n[*a, *b]\n{**a, **b}\n",
        );
        assert_count(
            &mut tree,
            "//spread[list]",
            4,
            "`*args`, `g(*xs)`, `[*a]`, `[*b]` all carry <list/> marker",
        );
        assert_count(
            &mut tree,
            "//spread[dict]",
            4,
            "`**kwargs`, `g(**kw)`, `{**a}`, `{**b}` all carry <dict/> marker",
        );
    }

    /// `list_splat` pattern in a match arm carries the same `<list/>` marker
    /// as a positional `*args` — uniform across contexts.
    #[test]
    fn splat_pattern_carries_marker() {
        let mut tree = parse_src(
            "python",
            "match seq:\n    case [1, *rest]: pass\n    case 'yes' | 'y': pass\n",
        );
        assert_count(
            &mut tree,
            "//pattern[splat]",
            1,
            "`*rest` destructure pattern carries <splat/> marker",
        );
        assert_count(
            &mut tree,
            "//pattern[union]",
            1,
            "`'yes' | 'y'` union pattern carries <union/> marker",
        );
    }
}

// ===========================================================================
// Go
// ===========================================================================

mod go {
    use super::*;

    /// Principle #9 — struct fields carry an exhaustive
    /// `<exported/>`/`<unexported/>` marker based on Go's
    /// name-capitalization export rule.
    #[test]
    fn field_export_markers_exhaustive() {
        let mut tree = parse_src(
            "go",
            "package p\ntype T struct {\n    Public string\n    private string\n}\n",
        );
        assert_count(
            &mut tree,
            "//field[exported]",
            1,
            "capitalised field carries <exported/>",
        );
        assert_count(
            &mut tree,
            "//field[unexported]",
            1,
            "lower-case field carries <unexported/>",
        );
    }

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

    /// Principle #12 — `const_spec` / `var_spec` / `import_spec` are
    /// grammar wrappers around `name = value` / `path`. Flatten so the
    /// declaration reads as `<const>const<name>x</name>=<value>1</value></const>`
    /// rather than nesting the assignment inside an opaque spec element.
    #[test]
    fn const_var_spec_flatten() {
        let mut tree = parse_src(
            "go",
            "package main\nconst x = 1\nvar y = 2\n",
        );
        assert_count(
            &mut tree,
            "//const_spec",
            0,
            "no <const_spec> wrapper (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//var_spec",
            0,
            "no <var_spec> wrapper (Principle #12)",
        );
        assert_count(
            &mut tree,
            "//const[name='x']",
            1,
            "const's name is a direct child, not buried under const_spec",
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

    /// `switch x.(type) { … }` and a regular `switch x { … }` both
    /// collapse to `<switch>`. The type switch carries a `<type/>`
    /// marker so `//switch[type]` picks out every type switch.
    #[test]
    fn type_switch_carries_marker() {
        let mut tree = parse_src(
            "go",
            "package main\nfunc f(x interface{}) {\n    switch x.(type) { case int: }\n    switch x { case 1: }\n}\n",
        );
        assert_count(
            &mut tree,
            "//switch[type]",
            1,
            "type switch carries <type/> marker",
        );
        assert_count(
            &mut tree,
            "//switch",
            2,
            "both regular and type switch collapse to <switch>",
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

    /// Ruby percent-literal arrays collapse to `<array>` with a
    /// `<string/>` / `<symbol/>` marker so the element name matches
    /// a normal array while the flavor stays queryable.
    #[test]
    fn percent_array_shape_markers() {
        let mut tree = parse_src(
            "ruby",
            "A = %w[one two]\nB = %i[alpha beta]\nC = [1, 2]\n",
        );
        assert_count(&mut tree, "//array[string]", 1, "%w[…] carries <string/>");
        assert_count(&mut tree, "//array[symbol]", 1, "%i[…] carries <symbol/>");
        assert_count(&mut tree, "//array", 3, "all three forms collapse to <array>");
    }

    /// Ruby splat parameters distinguish iterable `*args` (list) from
    /// mapping `**kwargs` (dict); keyword parameters (`key:`) carry a
    /// `<keyword/>` marker distinguishing them from positional ones.
    #[test]
    fn parameter_shape_markers() {
        let mut tree = parse_src(
            "ruby",
            "def f(a, *xs, key: 1, **kw)\nend\n",
        );
        assert_count(&mut tree, "//spread[list]", 1, "`*xs` carries <list/>");
        assert_count(&mut tree, "//spread[dict]", 1, "`**kw` carries <dict/>");
        assert_count(
            &mut tree,
            "//parameter[keyword]",
            1,
            "`key:` keyword parameter carries <keyword/>",
        );
    }
}

// ===========================================================================
// PHP
// ===========================================================================

mod php {
    use super::*;

    /// Principle #9 — class members carry an exhaustive visibility
    /// marker. Explicit `public/private/protected` keywords lift to
    /// markers, and members without a keyword get implicit `<public/>`
    /// (PHP's default).
    #[test]
    fn visibility_markers_exhaustive() {
        let mut tree = parse_src(
            "php",
            "<?php class X { function foo() {} private function bar() {} protected function baz() {} public function qux() {} }",
        );
        assert_count(
            &mut tree,
            "//method[public]",
            2,
            "implicit default and explicit public both carry <public/>",
        );
        assert_count(
            &mut tree,
            "//method[private]",
            1,
            "explicit private carries <private/>",
        );
        assert_count(
            &mut tree,
            "//method[protected]",
            1,
            "explicit protected carries <protected/>",
        );
    }

    /// Class properties follow the same defaults as methods.
    #[test]
    fn property_visibility_defaults_public() {
        let mut tree = parse_src(
            "php",
            "<?php class X { public $a; $b; private $c; }",
        );
        assert_count(
            &mut tree,
            "//field[public]",
            2,
            "explicit and implicit public both carry <public/>",
        );
        assert_count(
            &mut tree,
            "//field[private]",
            1,
            "explicit private field carries <private/>",
        );
    }
}
// ===========================================================================
// Cross-language: decorator / annotation / attribute topology
//
// The element name is idiomatic per language (Python uses <decorator>,
// Java <annotation>, C#/PHP/Rust <attribute>) but the STRUCTURAL
// TOPOLOGY is shared: the thing lives as a direct child of the
// decorated/annotated declaration, with a <name> child holding the
// qualifier name as text. No language uses an enclosing wrapper like
// <decorated> or <attributes>.
// ===========================================================================

mod decorator_topology {
    use super::*;

    #[test]
    fn python_decorator_is_direct_child() {
        let mut tree = parse_src("python", "@dataclass\nclass X: pass\n");
        assert_count(
            &mut tree,
            "//class/decorator[name='dataclass']",
            1,
            "Python decorator is a direct child of the decorated <class>",
        );
        assert_count(
            &mut tree,
            "//decorated",
            0,
            "no <decorated> wrapper — topology matches Java/C#/Rust",
        );
    }

    #[test]
    fn java_annotation_is_direct_child() {
        let mut tree = parse_src(
            "java",
            "class X { @Override public void f() {} }",
        );
        assert_count(
            &mut tree,
            "//method/annotation[name='Override']",
            1,
            "Java annotation is a direct child of the annotated <method>",
        );
    }

    #[test]
    fn csharp_attribute_is_direct_child() {
        let mut tree = parse_src(
            "csharp",
            "class X { [Obsolete] public void F() {} }",
        );
        assert_count(
            &mut tree,
            "//method/attribute[name='Obsolete']",
            1,
            "C# attribute is a direct child of the attributed <method>",
        );
    }

    #[test]
    fn rust_attribute_is_flat() {
        let mut tree = parse_src("rust", "#[derive(Debug)] struct S;\n");
        // #[derive] surfaces as a sibling `<attribute>` at the file
        // level — `attribute_item` wrapper was flattened.
        assert_count(
            &mut tree,
            "//attribute[name='derive']",
            1,
            "Rust attribute flattens: <attribute> with <name> child, not nested",
        );
        // Inner attributes (`#![…]`) carry an <inner/> marker to
        // distinguish from outer (`#[…]`) attributes.
        let mut inner = parse_src("rust", "#![allow(dead_code)]\nfn f() {}\n");
        assert_count(
            &mut inner,
            "//attribute[inner][name='allow']",
            1,
            "Rust inner attribute carries <inner/> marker",
        );
    }

    #[test]
    fn php_attribute_is_direct_child() {
        let mut tree = parse_src(
            "php",
            "<?php #[Deprecated] class X {}\n",
        );
        assert_count(
            &mut tree,
            "//class/attribute[name='Deprecated']",
            1,
            "PHP attribute is a direct child of the attributed <class>",
        );
    }
}

// ===========================================================================
// Cross-language: interpolated string shape
//
// Every language that supports string interpolation wraps the
// interpolated expression in an `<interpolation>` element inside
// `<string>` (or `<template>` in TS). The element name is shared;
// the delimiter tokens (`${` / `#{` / `{` / `$`) live as text inside
// the `<string>` (or, for some languages, inside the `<interpolation>`)
// but the queryable shape `//string/interpolation/<expr>` works
// uniformly across languages.
// ===========================================================================

mod interpolation_shape {
    use super::*;

    #[test]
    fn python_fstring() {
        let mut tree = parse_src("python", "x = f\"hi {name}!\"\n");
        assert_count(
            &mut tree,
            "//string/interpolation/name[.='name']",
            1,
            "Python f-string interpolation wraps the expression",
        );
    }

    #[test]
    fn typescript_template() {
        let mut tree = parse_src(
            "typescript",
            "const s = `hello ${name}!`;\n",
        );
        assert_count(
            &mut tree,
            "//template/interpolation/name[.='name']",
            1,
            "TypeScript template interpolation wraps the expression",
        );
    }

    #[test]
    fn ruby_double_quote() {
        let mut tree = parse_src(
            "ruby",
            "s = \"hi #{name}!\"\n",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/name[.='name']",
            1,
            "Ruby double-quote interpolation wraps the expression",
        );
    }

    #[test]
    fn csharp_interpolated_string() {
        let mut tree = parse_src(
            "csharp",
            "class X { string s = $\"hi {Name}!\"; }",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/name[.='Name']",
            1,
            "C# interpolated string wraps the expression",
        );
    }

    #[test]
    fn php_variable_interpolation() {
        let mut tree = parse_src(
            "php",
            "<?php $s = \"hi $name!\";\n",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/variable/name[.='name']",
            1,
            "PHP variable interpolation wraps the expression",
        );
    }

    #[test]
    fn php_complex_interpolation() {
        let mut tree = parse_src(
            "php",
            "<?php $s = \"x {$obj->method()}\";\n",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/call",
            1,
            "PHP complex interpolation wraps the expression",
        );
    }
}

// ===========================================================================
// Feature-grouped shape tests
//
// Reconsidered approach: instead of (or in addition to) per-language
// fixture files in `tests/integration/features/<feature>/`, assert the
// shape claim DIRECTLY in code, grouped by feature.
//
// Each `mod <feature>` collects every language's shape assertions for
// that feature, with multi-line indented XPath strings for legibility.
// XPath is whitespace-insensitive between path steps, so the
// indentation is purely a readability aid.
//
// Convention:
//   - Source code uses raw strings, indented to fit the test.
//   - **Be compact.** A shape claim should fit on one line whenever
//     the path is short and the predicates fit. Only break across
//     lines when the path is genuinely deep, or when several sibling
//     structural conditions need their own line for clarity.
//
//   - When breaking, indent so the path mirrors the tree. Two
//     equivalent styles — pick whichever reads better:
//
//     **Path** — counts the leaf:
//     ```
//     //class
//         /body
//             /method[public][returns/type[name='int']]
//     ```
//
//     **Bracket-predicate** — counts the root; nesting via `[…]`:
//     ```
//     //class[
//         body/method[public][returns/type[name='int']]
//     ]
//     ```
//
//   - Combine sibling predicates on the same node with `and`:
//     `comment[not(leading) and not(trailing)]` — not separate `[…]`
//     blocks. Bracket nesting is for HIERARCHY only.
//
//   - Don't mention things you don't care about. If the test is about
//     trailing comments, write `//comment[trailing]`, not
//     `//class/body/comment[trailing]` — unless the position matters.
// ===========================================================================

/// Pretty-print helper for multi-line XPath. Drops whitespace
/// OUTSIDE of `'…'` and `"…"` string literals so queries can be
/// written with indentation in source. Whitespace inside literals
/// (e.g. `[.='// instance counter']`) is preserved verbatim.
fn multi_xpath(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_quote: Option<char> = None;
    for c in s.chars() {
        match in_quote {
            Some(q) => {
                out.push(c);
                if c == q { in_quote = None; }
            }
            None if c == '\'' || c == '"' => {
                out.push(c);
                in_quote = Some(c);
            }
            None if c.is_whitespace() => {}
            None => out.push(c),
        }
    }
    out
}

mod comments {
    use super::*;

    /// One test per language. The source snippet is a deliberate
    /// kitchen-sink for THIS feature — every comment variant the
    /// language has appears once, and the assertions probe the
    /// resulting shape. One parse, many claims.
    #[test]
    fn csharp() {
        let mut tree = parse_src(
            "csharp",
            r#"
                class Demo {
                    private int _count; // trailing single

                    // leading first
                    // leading second
                    public string Config { get; set; }

                    /* leading block */
                    public void Run() {}

                    // floating

                    public int Solo() => 0;
                }
            "#,
        );

        claim("single-line `//` after `;` on same line is trailing",
            &mut tree, "//comment[trailing][.='// trailing single']", 1);

        claim("adjacent `//` comments merge into one <comment>",
            &mut tree, &multi_xpath("
                //comment[leading]
                    [contains(., 'leading first')]
                    [contains(., 'leading second')]
            "), 1);

        claim("block `/* */` immediately before a decl is leading",
            &mut tree, "//comment[leading][.='/* leading block */']", 1);

        claim("blank-line break: floating comment has no marker",
            &mut tree, "//comment[.='// floating'][not(leading) and not(trailing)]", 1);

        claim("trailing and leading are mutually exclusive",
            &mut tree, "//comment[trailing and leading]", 0);

        claim("no raw tree-sitter `line_comment` / `block_comment` leaks",
            &mut tree, "//line_comment | //block_comment", 0);
    }

    /// TypeScript (and JS) currently emit bare `<comment>` with no
    /// leading/trailing classification — the C# attachment classifier
    /// hasn't been ported yet (see proposal C1). When it lands, add
    /// the classification claims here mirroring `csharp()`.
    #[test]
    fn typescript() {
        let mut tree = parse_src(
            "typescript",
            r#"
                // single
                class X {
                    x: number; // inline
                    /* block */
                    y: string;
                    /** JSDoc */
                    method() {}
                }
            "#,
        );

        claim("`//` line comment becomes <comment>",
            &mut tree, "//comment[.='// single']", 1);

        claim("`/* */` block becomes <comment>",
            &mut tree, "//comment[.='/* block */']", 1);

        claim("JSDoc `/** */` becomes <comment>",
            &mut tree, "//comment[starts-with(., '/**')][contains(., 'JSDoc')]", 1);

        claim("no raw tree-sitter `line_comment` / `block_comment` leaks",
            &mut tree, "//line_comment | //block_comment", 0);
    }

    /// Python `#` comments. Tree-sitter calls them `comment`; tractor
    /// renames to `<comment>` uniformly.
    #[test]
    fn python() {
        let mut tree = parse_src(
            "python",
            r#"
# module-level
class X:
    """docstring stays a string, not a comment"""
    x = 1  # inline
    # before y
    y = 2
"#,
        );

        claim("`#` line comment becomes <comment>",
            &mut tree, "//comment[.='# module-level']", 1);

        claim("inline `#` after code is still <comment>",
            &mut tree, "//comment[.='# inline']", 1);

        claim("docstring is a <string>, NOT a <comment>",
            &mut tree, "//comment[contains(., 'docstring')]", 0);

        claim("docstring lives as a <string> child of <class>",
            &mut tree, "//class//string[contains(., 'docstring')]", 1);
    }

    /// Rust has 4 comment kinds: `//`, `/* */`, doc `///`, inner doc
    /// `//!`. All collapse to `<comment>`.
    #[test]
    fn rust() {
        let mut tree = parse_src(
            "rust",
            r#"
                //! crate-level inner doc
                /// outer doc
                fn x() {}
                // line
                /* block */
                fn y() {}
            "#,
        );

        claim("`//` line comment becomes <comment>",
            &mut tree, "//comment[.='// line']", 1);

        claim("`/* */` block becomes <comment>",
            &mut tree, "//comment[.='/* block */']", 1);

        claim("`///` outer doc becomes <comment>",
            &mut tree, "//comment[starts-with(., '///')]", 1);

        claim("`//!` inner doc becomes <comment>",
            &mut tree, "//comment[starts-with(., '//!')]", 1);

        claim("no raw tree-sitter `line_comment` / `block_comment` / `doc_comment` leaks",
            &mut tree, "//line_comment | //block_comment | //doc_comment", 0);
    }
}

mod accessor_flattening {
    use super::*;

    /// Property accessors are direct siblings of <property>; no
    /// <accessor_list> wrapper. Each accessor carries an empty marker
    /// (<get/>/<set/>/<init/>) uniformly across auto-form and bodied
    /// form (Principles #12, #13).
    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            class Accessors
            {
                public int AutoProp { get; set; }

                private int _backing;
                public int Manual
                {
                    get { return _backing; }
                    set { _backing = value; }
                }

                public int ReadOnly { get; }
                public int WriteOnly { set { _backing = value; } }
            }
        "#);

        claim("no <accessor_list> wrapper anywhere",
            &mut tree, "//accessor_list", 0);

        claim("auto-form get + bodied get + read-only get",
            &mut tree, "//accessor[get]", 3);

        claim("auto-form set + bodied set + write-only set",
            &mut tree, "//accessor[set]", 3);

        claim("AutoProp has 2 accessors as direct siblings of <property>",
            &mut tree, "//property[name='AutoProp']/accessor", 2);

        claim("Manual property has bodied accessors with block bodies",
            &mut tree, "//property[name='Manual']/accessor/body/block", 2);

        claim("ReadOnly has only get",
            &mut tree, "//property[name='ReadOnly']/accessor[set]", 0);
    }
}

mod accessors {
    use super::*;

    /// TypeScript `get foo()` / `set foo(v)` carry <get/>/<set/>
    /// markers on <method>. //method[get] picks them out uniformly
    /// regardless of body shape.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            class Counter {
                private _value = 0;

                get value(): number { return this._value; }
                set value(v: number) { this._value = v; }
                static get singleton(): Counter { return new Counter(); }
            }
        "#);

        claim("two getter methods (instance + static)",
            &mut tree, "//method[get]", 2);

        claim("one setter method",
            &mut tree, "//method[set]", 1);

        claim("get/set on accessor methods imply <public/>",
            &mut tree, "//method[(get or set) and not(public)]", 0);

        claim("get and set markers are mutually exclusive on a method",
            &mut tree, "//method[get and set]", 0);
    }
}

mod async_generator {
    use super::*;

    /// async / generator lift to empty markers on <function> /
    /// <method>. Every async/generator declaration carries the
    /// applicable markers (Principle #9 exhaustive markers).
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            async function fetchOne(): Promise<number> { return 1; }
            function* counter(): Generator<number> { yield 1; }
            async function* stream(): AsyncGenerator<number> { yield 1; }
            class Service {
                async load(): Promise<void> {}
                *keys(): Generator<string> { yield "a"; }
            }
        "#);

        claim("async function fetchOne",
            &mut tree, "//function[async and not(generator)][name='fetchOne']", 1);

        claim("generator function counter",
            &mut tree, "//function[generator and not(async)][name='counter']", 1);

        claim("async generator function stream",
            &mut tree, "//function[async and generator][name='stream']", 1);

        claim("async method load",
            &mut tree, "//method[async and not(generator)][name='load']", 1);

        claim("generator method keys",
            &mut tree, "//method[generator and not(async)][name='keys']", 1);
    }
}

mod augmented_assign {
    use super::*;

    /// Goal #5: augmented_assignment unifies with plain assignment
    /// as <assign> plus an <op> child carrying the compound operator.
    /// A single //assign query matches every assignment.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
def ops():
    x = 0
    x += 1
    x -= 2
    x *= 3
    x //= 2
    x **= 2
    x &= 0xFF
    x |= 0x10
    x ^= 0x01
    x <<= 1
    x >>= 1
"#);

        claim("11 statement-level assignments (1 plain + 10 compound)",
            &mut tree, "//body/assign", 11);

        claim("plain `=` is the only top-level assign without an <op>",
            &mut tree, "//body/assign[not(op)]", 1);

        claim("10 compound assignments carry an <op> child",
            &mut tree, "//body/assign/op", 10);

        claim("`+=` carries assign[plus] marker",
            &mut tree, "//assign/op/assign[plus]", 1);

        claim("`-=` carries assign[minus] marker",
            &mut tree, "//assign/op/assign[minus]", 1);

        claim("`**=` carries assign[power] marker",
            &mut tree, "//assign/op/assign[power]", 1);

        claim("bitwise compound ops carry assign/bitwise[*] markers",
            &mut tree, "//assign/op/assign/bitwise[and] | //assign/op/assign/bitwise[or] | //assign/op/assign/bitwise[xor]", 3);

        claim("shift compound ops carry assign/shift[*] markers",
            &mut tree, "//assign/op/assign/shift[left] | //assign/op/assign/shift[right]", 2);
    }
}

mod collection_markers {
    use super::*;

    /// Python collection literals unify by produced type. <list>,
    /// <dict>, <set>, <generator> carry exhaustive <literal/> or
    /// <comprehension/> markers so queries can distinguish
    /// `[x for x in xs]` from `[1, 2, 3]` without kind-specific
    /// element names.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
nums = [1, 2, 3]
squares = [x * x for x in nums]
pairs = {"a": 1, "b": 2}
inverted = {v: k for k, v in pairs.items()}
unique = {1, 2, 3}
uniq_sq = {x * x for x in nums}
gen = (x for x in nums)
"#);

        claim("list literal carries <literal/>",
            &mut tree, "//list[literal]", 1);

        claim("list comprehension carries <comprehension/>",
            &mut tree, "//list[comprehension]", 1);

        claim("dict literal carries <literal/>",
            &mut tree, "//dict[literal]", 1);

        claim("dict comprehension carries <comprehension/>",
            &mut tree, "//dict[comprehension]", 1);

        claim("set literal carries <literal/>",
            &mut tree, "//set[literal]", 1);

        claim("set comprehension carries <comprehension/>",
            &mut tree, "//set[comprehension]", 1);

        claim("generator expression renders as <generator>",
            &mut tree, "//generator", 1);

        claim("literal and comprehension are mutually exclusive on collections",
            &mut tree, "//*[literal and comprehension]", 0);
    }
}

mod constructor_rename {
    use super::*;

    /// `ctor` -> `<constructor>` (Principle #2: full names over
    /// abbreviations).
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class Point {
                int x, y;
                Point() { this(0, 0); }
                Point(int x, int y) { this.x = x; this.y = y; }
            }
        "#);

        claim("two constructors render as <constructor>",
            &mut tree, "//constructor", 2);

        claim("no abbreviated `ctor` element leaks",
            &mut tree, "//ctor", 0);

        claim("constructor name matches class name",
            &mut tree, "//constructor[name='Point']", 2);

        claim("zero-arg constructor's `this(...)` body is a <call>",
            &mut tree, "//constructor[not(parameter)]/body//call[this]", 1);
    }
}

mod defined_type_vs_alias {
    use super::*;

    /// Go distinguishes defined types (`type MyInt int`) from type
    /// aliases (`type Color = int`). Defined type -> <type>; alias
    /// -> <alias> (parallel with Rust / TS / C# / Java).
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            type MyInt int
            type Color = int
        "#);

        claim("defined type renders as <type>",
            &mut tree, "//type[name='MyInt']", 1);

        claim("alias renders as <alias>",
            &mut tree, "//alias[name='Color']", 1);

        claim("alias inner refers to underlying <type>",
            &mut tree, "//alias[name='Color']/type[name='int']", 1);

        claim("alias does NOT also render as <type> at the top level",
            &mut tree, "//file/type[name='Color']", 0);
    }
}

mod expression_list {
    use super::*;

    /// Principle #12: `expression_list` (tuple-like return/yield
    /// expressions) is a pure grouping node; drop it so the
    /// expressions become direct children of the enclosing
    /// <return>/<yield>/<assign>.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
def pair():
    return 1, 2

def triple():
    return "a", "b", "c"

def unpack():
    a, b = pair()
    return a + b
"#);

        claim("no <expression_list> wrapper leaks anywhere",
            &mut tree, "//expression_list", 0);

        claim("`return 1, 2` puts both ints as direct children of <return>",
            &mut tree, "//return[int='1' and int='2']", 1);

        claim("`return \"a\", \"b\", \"c\"` flattens 3 strings under <return>",
            &mut tree, "//return[count(string)=3]", 1);

        claim("tuple unpack `a, b = pair()` exposes both names directly under <assign>/left",
            &mut tree, "//assign/left[name='a' and name='b']", 1);
    }
}

mod f_strings {
    use super::*;

    /// F-strings render as <string> with <interpolation> children
    /// and bare literal text in between (Principle #12: grammar
    /// wrappers like string_start / string_content / string_end are
    /// flattened). Plain strings collapse to a text-only <string>.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
plain = "hello"
greeting = f"hello {name}"
status = f"hello {name}, you are {age}"
"#);

        claim("3 strings total",
            &mut tree, "//string", 3);

        claim("plain string has no <interpolation> child",
            &mut tree, "//string[not(interpolation)]", 1);

        claim("two f-strings carry interpolations",
            &mut tree, "//string[interpolation]", 2);

        claim("interpolation wraps a <name>",
            &mut tree, "//string/interpolation/name='name'", 1);

        claim("`status` f-string has 2 interpolations",
            &mut tree, "//string[count(interpolation)=2]", 1);

        claim("interpolation can match by interpolated name",
            &mut tree, "//string/interpolation[name='age']", 1);
    }
}

mod match_expression {
    use super::*;

    /// Principle #12: `match_block` (the `{ ... }` wrapper around
    /// match arms) is a pure grouping node; drop it so arms are
    /// direct siblings of <match> via <body>.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn classify(n: i32) -> &'static str {
                match n {
                    0 => "zero",
                    1 | 2 | 3 => "small",
                    _ if n < 0 => "negative",
                    _ => "other",
                }
            }
        "#);

        claim("no `match_block` grammar leaf leaks",
            &mut tree, "//match_block", 0);

        claim("4 arms as siblings under <match>/<body>",
            &mut tree, "//match/body/arm", 4);

        claim("arm with literal pattern `0`",
            &mut tree, "//arm[pattern/int='0']", 1);

        claim("guard arm carries a <condition> child inside <pattern>",
            &mut tree, "//arm/pattern/condition", 1);

        claim("or-pattern uses pattern[or] markers (left-associative nesting)",
            &mut tree, "//arm/pattern/pattern[or]", 1);

        claim("each arm has a <pattern> and a <value>",
            &mut tree, "//arm[pattern and value]", 4);
    }
}

mod method_call {
    use super::*;

    /// Both function calls and method calls render as <call>. Method
    /// calls are distinguished by a <field> child that names the
    /// receiver and method (Rust uses field-call syntax).
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn use_calls() {
                let v: Vec<i32> = Vec::new();
                let n = v.len();
                let s = "hi".to_string();
                s.to_uppercase();
            }
        "#);

        claim("4 unified <call> nodes (1 path-call + 3 method-calls)",
            &mut tree, "//call", 4);

        claim("path-call `Vec::new()` has a <path> child",
            &mut tree, "//call[path[name='Vec' and name='new']]", 1);

        claim("3 method calls expose a <field> child for receiver.method",
            &mut tree, "//call/field", 3);

        claim("method `len` on receiver `v`",
            &mut tree, "//call/field[value/name='v' and name='len']", 1);

        claim("method `to_string` on a string-literal receiver",
            &mut tree, "//call/field[value/string and name='to_string']", 1);
    }
}

mod modifiers {
    use super::*;

    /// Modifiers lift as empty markers on the declaration. Every
    /// access modifier is exhaustive — package-private gets an
    /// explicit <package/> marker.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class Modifiers {
                public static final int PUB = 1;
                private int priv = 2;
                protected int prot = 3;
                int pkg = 4;
                public synchronized void sync() {}
                public abstract static class AbsStatic {}
            }
        "#);

        claim("public static final field marks all 3 modifiers",
            &mut tree, "//field[public and static and final][declarator/name='PUB']", 1);

        claim("private field carries <private/>",
            &mut tree, "//field[private]", 1);

        claim("protected field carries <protected/>",
            &mut tree, "//field[protected]", 1);

        claim("implicit package-private surfaces as <package/>",
            &mut tree, "//field[package]", 1);

        claim("synchronized method also marks public",
            &mut tree, "//method[public and synchronized][name='sync']", 1);

        claim("nested class composes public + abstract + static markers",
            &mut tree, "//class[public and abstract and static][name='AbsStatic']", 1);
    }
}

mod name_inlining {
    use super::*;

    /// (1) Every Ruby `identifier` becomes <name> unconditionally.
    /// (2) When a <name> wrapper sits inside method/class/module and
    /// contains a single identifier, the transform inlines its text
    /// directly, so <method><name>foo</name>… not
    /// <method><name><identifier>foo</identifier></name>….
    #[test]
    fn ruby() {
        let mut tree = parse_src("ruby", r#"
            class Calculator
              def add(a, b)
                a + b
              end
            end

            module Utils
              def self.greet(name)
                "hi, #{name}"
              end
            end
        "#);

        claim("class name is inlined text on <name>",
            &mut tree, "//class/name='Calculator'", 1);

        claim("class <name> has no <identifier> child",
            &mut tree, "//class/name/identifier", 0);

        claim("module name is inlined text on <name>",
            &mut tree, "//module/name='Utils'", 1);

        claim("method name `add` is inlined text on <name>",
            &mut tree, "//method/name='add'", 1);

        claim("singleton method `self.greet` carries [singleton] marker",
            &mut tree, "//method[singleton][name='greet']", 1);

        claim("method parameters are <name> elements (identifier renamed)",
            &mut tree, "//method[name='add']/name[. ='a' or .='b']", 2);

        claim("no raw <identifier> nodes leak from Ruby grammar",
            &mut tree, "//identifier", 0);
    }
}

mod parameter_marking {
    use super::*;

    /// Every <param> carries an exhaustive marker: <required/> or
    /// <optional/>. Covers required, optional (?), defaulted, and
    /// rest parameters; also the JS-style untyped param shape.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            function call(
                required: string,
                optional?: number,
                defaulted: boolean = true,
                ...rest: string[]
            ): void {}

            function noTypes(x, y) {}
        "#);

        claim("every parameter is either required or optional",
            &mut tree, "//parameter[not(required) and not(optional)]", 0);

        claim("required and optional are mutually exclusive",
            &mut tree, "//parameter[required and optional]", 0);

        claim("required: 1 (required) + defaulted + rest + 2 untyped = 5",
            &mut tree, "//parameter[required]", 5);

        claim("optional `?` is the only <parameter[optional]>",
            &mut tree, "//parameter[optional]", 1);

        claim("rest parameter exposes a <rest> child",
            &mut tree, "//parameter[rest]", 1);

        claim("defaulted parameter has a <value> child",
            &mut tree, "//parameter[name='defaulted'][value]", 1);
    }
}

mod reference_type {
    use super::*;

    /// Reference types `&T` / `&mut T` / `&'a T` render as a single
    /// <type> with a <borrowed/> marker (Principles #14 + #13). The
    /// inner referenced type is a nested <type> child.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn read(s: &str) -> &str { s }
            fn write(buf: &mut Vec<u8>) {}
            fn static_ref() -> &'static str { "" }
        "#);

        claim("4 reference types: 2x &str (param + return) + &mut Vec<u8> + &'static str",
            &mut tree, "//type[borrowed]", 4);

        claim("only the &mut Vec<u8> carries the mut marker",
            &mut tree, "//type[borrowed and mut]", 1);

        claim("borrowed type wraps the referenced type as a nested <type>",
            &mut tree, "//type[borrowed]/type", 4);

        claim("`&'static` exposes a <lifetime> child",
            &mut tree, "//type[borrowed]/lifetime[name='static']", 1);

        claim("inner type of &mut is the generic Vec<u8>",
            &mut tree, "//type[borrowed and mut]/type[generic][name='Vec']", 1);
    }
}

mod strings {
    use super::*;

    /// Go strings: interpreted (double-quoted, escapes) and raw
    /// (backtick, no escapes). Both render as <string>; raw strings
    /// carry a <raw/> marker (Principle #13).
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            const normal = "hello\nworld"
            const raw = `hello world`
            const pattern = `^\d+$`
        "#);

        claim("3 strings total — bare //string catches both forms",
            &mut tree, "//string", 3);

        claim("interpreted string has no <raw/> marker",
            &mut tree, "//string[not(raw)]", 1);

        claim("two backtick strings carry <raw/>",
            &mut tree, "//string[raw]", 2);

        claim("raw and not-raw partition the strings",
            &mut tree, "//string[raw and not(raw)]", 0);
    }
}

mod struct_expression {
    use super::*;

    /// Struct construction `Point { x: 1, y: 2 }` renders as
    /// <literal> with a <name> child for the struct name and
    /// <field> siblings for each initializer. Symmetric with JS/C#
    /// object construction: //literal[name='Point'] finds every
    /// Point construction site.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            struct Point { x: i32, y: i32 }

            fn make() {
                let p = Point { x: 1, y: 2 };
                let q = Point { x: 0, ..p };
            }
        "#);

        claim("two Point construction sites",
            &mut tree, "//literal[name='Point']", 2);

        claim("struct name lives as <name> on <literal> (NOT a <type>)",
            &mut tree, "//literal/type", 0);

        claim("first construction has 2 plain fields, no [base]",
            &mut tree, "//literal[name='Point'][not(body/field[base])]/body/field", 2);

        claim("second construction has a [base] field for `..p`",
            &mut tree, "//literal/body/field[base][name='p']", 1);

        claim("field initializers carry <value> children",
            &mut tree, "//literal/body/field[name='x']/value/int", 2);
    }
}

mod struct_interface_hoist {
    use super::*;

    /// Goal #5 mental model — `type Foo struct { … }` and
    /// `type Foo interface { … }` hoist: the outer element becomes
    /// <struct> or <interface> directly instead of the Go-spec
    /// `<type>` wrapper.
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            type Config struct {
                Host string
                Port int
            }

            type Greeter interface {
                Greet() string
            }
        "#);

        claim("struct hoists to top level (no enclosing <type>)",
            &mut tree, "//file/struct[name='Config']", 1);

        claim("interface hoists to top level (no enclosing <type>)",
            &mut tree, "//file/interface[name='Greeter']", 1);

        claim("uppercase struct name carries <exported/>",
            &mut tree, "//struct[exported][name='Config']", 1);

        claim("uppercase interface name carries <exported/>",
            &mut tree, "//interface[exported][name='Greeter']", 1);

        claim("the `type` wrapper does NOT also surface a <type> for the struct",
            &mut tree, "//file/type[name='Config']", 0);
    }
}

mod type_declaration {
    use super::*;

    /// Go's `type_declaration` wrapper is dropped; `type_spec`
    /// renders as <type> directly. Parallel with struct/interface
    /// declarations so //type queries find every declared type.
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            type ID uint64

            type User struct {
                Name string
                Age  int
            }

            type Greeter interface {
                Greet() string
            }
        "#);

        claim("plain `type ID uint64` renders as <type>",
            &mut tree, "//file/type[name='ID']", 1);

        claim("struct/interface forms do NOT also produce a <type> wrapper",
            &mut tree, "//file/type[name='User'] | //file/type[name='Greeter']", 0);

        claim("no `type_declaration` grammar wrapper leaks",
            &mut tree, "//type_declaration", 0);

        claim("inner referenced type of `type ID uint64`",
            &mut tree, "//type[name='ID']/type[name='uint64']", 1);
    }
}

mod typedef {
    use super::*;

    /// Rust `type_item` renders as <alias> (parallel with
    /// TS / Java / C#).
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            type Id = u32;
            type Mapping<T> = std::collections::HashMap<String, T>;
        "#);

        claim("two aliases declared",
            &mut tree, "//alias", 2);

        claim("no raw `type_item` grammar leaf leaks",
            &mut tree, "//type_item", 0);

        claim("aliases default to <private/>",
            &mut tree, "//alias[private]", 2);

        claim("simple alias resolves to <type>",
            &mut tree, "//alias[name='Id']/type[name='u32']", 1);

        claim("generic alias declares a <generic> parameter",
            &mut tree, "//alias[name='Mapping']/generic[name='T']", 1);
    }
}

mod visibility {
    use super::*;

    /// Visibility is exhaustive: every declaration carries either
    /// <private/> (implicit default) or <pub/> with optional
    /// restriction details.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn private_fn() {}
            pub fn public_fn() {}
            pub(crate) fn crate_fn() {}
            pub(super) fn super_fn() {}

            struct PrivateStruct;
            pub struct PublicStruct;

            const PRIV: i32 = 1;
            pub const PUB: i32 = 2;
        "#);

        claim("4 functions total, every one has visibility info",
            &mut tree, "//function[private or pub]", 4);

        claim("plain `pub` produces a <pub/> marker (no restriction)",
            &mut tree, "//function[pub][name='public_fn']", 1);

        claim("`pub(crate)` exposes <pub><crate/></pub>",
            &mut tree, "//function/pub[crate]", 1);

        claim("`pub(super)` exposes <pub><super/></pub>",
            &mut tree, "//function/pub[super]", 1);

        claim("private struct carries <private/>",
            &mut tree, "//struct[private][name='PrivateStruct']", 1);

        claim("private const carries <private/>",
            &mut tree, "//const[private][name='PRIV']", 1);
    }
}

mod where_clause {
    use super::*;

    /// C# `where` clause constraints attach to the matching
    /// <generic> element. Shape constraints (class / struct /
    /// notnull / unmanaged / new) become empty markers that
    /// compose; type bounds wrap in <extends><type>…</type></extends>.
    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            using System;

            class Repo<T, U, V>
                where T : class, IComparable<T>, new()
                where U : struct
                where V : notnull
            {
            }
        "#);

        claim("3 generics declared on Repo (T, U, V)",
            &mut tree, "//class[name='Repo']/generic", 3);

        claim("T composes class + new shape markers",
            &mut tree, "//generic[class and new][name='T']", 1);

        claim("U has the struct constraint",
            &mut tree, "//generic[struct][name='U']", 1);

        claim("V has the notnull constraint",
            &mut tree, "//generic[notnull][name='V']", 1);

        claim("T's IComparable<T> bound wraps in <extends><type>...",
            &mut tree, "//generic[name='T']/extends/type[name='IComparable']", 1);

        claim("U has no <extends> bound",
            &mut tree, "//generic[name='U']/extends", 0);
    }
}

mod flat_lists {
    use super::*;

    /// Principle #12: parameters / arguments / generics / accessors
    /// render as flat siblings — no <parameters> / <accessor_list> /
    /// <argument_list> / <type_parameters> wrapper element.

    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            class FlatLists
            {
                public T First<T, U>(T a, U b, int c) where T : class
                {
                    return a;
                }

                public int Count { get; set; }

                public void Caller()
                {
                    First<string, int>("x", 1, 2);
                }
            }
        "#);

        claim("no parameter-list wrapper element",
            &mut tree, "//parameter_list | //parameters", 0);

        claim("no argument-list wrapper element",
            &mut tree, "//argument_list | //arguments", 0);

        claim("no accessor-list wrapper element",
            &mut tree, "//accessor_list", 0);

        claim("First has 3 parameters as direct siblings",
            &mut tree, "//method[name='First']/parameter", 3);

        claim("First has 2 generics as direct siblings",
            &mut tree, "//method[name='First']/generic", 2);

        claim("Property accessors are direct siblings of <property>",
            &mut tree, "//property[name='Count']/accessor", 2);
    }

    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            func First(a string, b int, c bool) string { return a }

            func Caller() { First("x", 1, true) }

            type Config struct { Host string; Port int; Tls bool }
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameter_list | //parameters", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("no field-list wrapper around struct fields",
            &mut tree, "//field_declaration_list | //field_list", 0);

        claim("First has 3 parameters as direct siblings",
            &mut tree, "//function[name='First']/parameter", 3);

        claim("Caller's call has 3 argument siblings (not wrapped)",
            &mut tree, "//function[name='Caller']//call/*[self::string or self::int or self::true]", 3);

        claim("Config struct has 3 fields",
            &mut tree, "//struct[name='Config']/field", 3);
    }

    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class FlatLists {
                <T, U extends Comparable<U>> T first(T a, U b, int c) { return a; }

                void caller() { first("x", "y", 1); }
            }
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameter_list | //parameters", 0);

        claim("no type-parameter wrapper",
            &mut tree, "//type_parameters", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("first has 3 parameters as direct siblings",
            &mut tree, "//method[name='first']/parameter", 3);

        claim("first has 2 generics as direct siblings",
            &mut tree, "//method[name='first']/generic", 2);
    }

    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn first<T, U: Clone>(a: T, b: U, c: i32) -> T { a }

            fn caller() {
                first::<String, i32>(String::from("x"), 1, 2);
            }
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameters | //parameter_list", 0);

        claim("no type-parameter wrapper",
            &mut tree, "//type_parameters", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("first has 3 parameters as direct siblings",
            &mut tree, "//function[name='first']/parameter", 3);

        claim("first has 2 generics as direct siblings",
            &mut tree, "//function[name='first']/generic", 2);
    }

    /// TypeScript currently retains a thin <generics> grouping
    /// element — pin that as current behaviour. Parameters and
    /// arguments still flatten.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            function first<T, U>(a: T, b: U, c: number): T { return a; }
            first<string, number>("x", 1, 2);
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameters | //parameter_list", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("first has 3 parameters as direct siblings",
            &mut tree, "//function[name='first']/parameter", 3);

        claim("TS keeps a thin <generics> wrapper for type parameters",
            &mut tree, "//function[name='first']/generics/generic", 2);
    }
}

mod interface_public {
    use super::*;

    /// Interface members without an explicit access modifier default
    /// to <public/>. C# and Java both lift this to an exhaustive
    /// marker so a single //method[public] hits every visible
    /// interface method.

    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            interface IShape
            {
                double Area();
                double Perimeter();
                string Name => "shape";
                public void Stroke();
            }
        "#);

        claim("3 interface methods all carry <public/>",
            &mut tree, "//interface/body/method[public]", 3);

        claim("expression-bodied property carries <public/>",
            &mut tree, "//interface/body/property[public]", 1);

        claim("no interface member is missing visibility",
            &mut tree, "//interface/body/*[(self::method or self::property) and not(public)]", 0);
    }

    /// Java pins current behaviour: implicit-public abstract methods
    /// surface as <public/>; the explicit `public void stroke()` also
    /// gets <public/>. Default methods (`default String name() {...}`)
    /// currently render with <package/> rather than <public/> — pin
    /// that as the actual current shape rather than aspiration.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            interface Shape {
                double area();
                double perimeter();
                default String name() { return "shape"; }
                public void stroke();
            }
        "#);

        claim("implicit-public abstract methods carry <public/>",
            &mut tree, "//interface/body/method[public][not(body)]", 3);

        claim("explicit `public` method `stroke` also carries <public/>",
            &mut tree, "//interface/body/method[public][name='stroke']", 1);

        claim("`default` method is not classified as <public/> (current behaviour)",
            &mut tree, "//interface/body/method[name='name'][public]", 0);
    }
}

mod type_vocabulary {
    use super::*;

    /// Principle #14: every type reference wraps its name in a
    /// <name> child. No bare-text <type> nodes; type parameters use
    /// <generic>; bounds wrap in <extends>; collection-of-T uses
    /// <type[generic]> with nested <type> children.

    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            using System.Collections.Generic;

            interface IBarker { void Bark(); }
            class Animal {}

            class Dog<T> : Animal, IBarker where T : Animal
            {
                public T Owner;
                public List<string> Tags;
                public void Bark() {}
            }
        "#);

        claim("every <type> has a <name> child (no bare-text types)",
            &mut tree, "//type[not(name)]", 0);

        claim("Dog declares one <generic> type parameter",
            &mut tree, "//class[name='Dog']/generic[name='T']", 1);

        claim("generic T with where-clause `: Animal` exposes <extends><type>",
            &mut tree, "//class[name='Dog']/generic[name='T']/extends/type[name='Animal']", 1);

        claim("class extends list combines base + interface as siblings",
            &mut tree, "//class[name='Dog']/extends/type[name='Animal' or name='IBarker']", 2);

        claim("List<string> field uses generic type with inner <type>",
            &mut tree, "//field//type[generic][name='List']/type[name='string']", 1);
    }

    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            import java.util.List;

            class Animal {}
            interface Barker { void bark(); }
            interface Runner { void run(); }

            class Dog<T extends Animal> extends Animal implements Barker, Runner {
                T owner;
                List<String> tags;

                public void bark() {}
                public void run() {}
            }
        "#);

        claim("every <type> has a <name> child",
            &mut tree, "//type[not(name)]", 0);

        claim("type parameter T has an <extends> bound on Animal",
            &mut tree, "//class[name='Dog']/generic[name='T']/extends/type[name='Animal']", 1);

        claim("extends list points to <type[name='Animal']>",
            &mut tree, "//class[name='Dog']/extends/type[name='Animal']", 1);

        claim("implements list has 2 <type> entries",
            &mut tree, "//class[name='Dog']/implements/type", 2);

        claim("List<String> field uses generic type with inner <type>",
            &mut tree, "//field//type[generic][name='List']/type[name='String']", 1);
    }

    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            use std::collections::HashMap;

            trait Barker { fn bark(&self); }

            struct Dog<T: Barker> {
                owner: T,
                tags: Vec<String>,
                scores: HashMap<String, i32>,
                parent: Option<Box<Dog<T>>>,
            }
        "#);

        claim("every <type> has a <name> child",
            &mut tree, "//type[not(name)]", 0);

        claim("Dog declares <generic> with a `: Barker` bound",
            &mut tree, "//struct[name='Dog']/generic[name='T']/bounds/type[name='Barker']", 1);

        claim("Vec<String>: generic with inner <type>",
            &mut tree, "//field[name='tags']/type[generic][name='Vec']/type[name='String']", 1);

        claim("HashMap<String, i32>: generic with two inner <type> children",
            &mut tree, "//field[name='scores']/type[generic][name='HashMap']/type", 2);

        claim("Option<Box<Dog<T>>> nests 3 levels of <type[generic]>",
            &mut tree, "//field[name='parent']/type[generic]/type[generic]/type[generic]", 1);
    }

    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            type Id = number;
            type Handler = (x: number) => void;
            type Box<T> = Array<T>;

            class Animal {}
            interface Barker { bark(): void; }
            class Dog extends Animal implements Barker {
                bark(): void {}
            }
        "#);

        claim("only <type[function]> may lack a <name> (it's defined by signature)",
            &mut tree, "//type[not(name) and not(function)]", 0);

        claim("plain alias points at a single <type>",
            &mut tree, "//alias[name='Id']/type[name='number']", 1);

        claim("function-type alias carries <type[function]>",
            &mut tree, "//alias[name='Handler']/type[function]", 1);

        claim("generic alias carries a <generic> child via <generics> wrapper",
            &mut tree, "//alias[name='Box']/generics/generic[name='T']", 1);

        claim("Dog extends and implements both wrap base types",
            &mut tree, "//class[name='Dog']/extends/type[name='Animal'] | //class[name='Dog']/implements/type[name='Barker']", 2);
    }
}

mod conditionals {
    use super::*;

    /// Conditional shape: `else if` chains collapse to flat
    /// <else_if> siblings of <if>; ternary keeps <then>/<else>
    /// wrappers via surgical field-wrap in languages that have a
    /// dedicated <ternary> node. Python ternary is FLAT (no
    /// then/else wrappers); Ruby uses <conditional> rather than
    /// <ternary>.

    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            class Conditionals
            {
                public string Classify(int n)
                {
                    if (n < 0) { return "neg"; }
                    else if (n == 0) { return "zero"; }
                    else if (n < 10) { return "small"; }
                    else { return "big"; }
                }

                public string Label(int n) => n > 0 ? "positive" : "non-positive";
            }
        "#);

        claim("one <if> at the chain root",
            &mut tree, "//if", 1);

        claim("two <else_if> siblings flattened under <if>",
            &mut tree, "//if/else_if", 2);

        claim("one trailing <else> sibling under <if>",
            &mut tree, "//if/else", 1);

        claim("ternary surgically wraps then/else",
            &mut tree, "//ternary[then and else]", 1);
    }

    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            func Classify(n int) string {
                if n < 0 { return "neg" } else if n == 0 { return "zero" } else if n < 10 { return "small" } else { return "big" }
            }
        "#);

        claim("one <if> at the chain root",
            &mut tree, "//if", 1);

        claim("two flat <else_if> siblings",
            &mut tree, "//if/else_if", 2);

        claim("one <else> sibling",
            &mut tree, "//if/else", 1);

        claim("Go has no <ternary> (no ternary in the language)",
            &mut tree, "//ternary", 0);
    }

    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class Conditionals {
                String classify(int n) {
                    if (n < 0) { return "neg"; }
                    else if (n == 0) { return "zero"; }
                    else if (n < 10) { return "small"; }
                    else { return "big"; }
                }

                String label(int n) {
                    return n > 0 ? "positive" : "non-positive";
                }
            }
        "#);

        claim("one <if> + 2 <else_if> + 1 <else>",
            &mut tree, "//if[count(else_if)=2 and count(else)=1]", 1);

        claim("ternary has <then> and <else> via surgical wrap",
            &mut tree, "//ternary[then and else]", 1);
    }

    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
def classify(n):
    if n < 0:
        return "neg"
    elif n == 0:
        return "zero"
    elif n < 10:
        return "small"
    else:
        return "big"


def label(n):
    return "positive" if n > 0 else "non-positive"
"#);

        claim("one <if> at the chain root",
            &mut tree, "//if", 1);

        claim("`elif` becomes <else_if> (underscore naming)",
            &mut tree, "//if/else_if", 2);

        claim("no `elif` raw element leaks",
            &mut tree, "//elif", 0);

        claim("Python ternary is FLAT (no then/else wrappers)",
            &mut tree, "//ternary[then or else]", 0);

        claim("Python ternary still produces a <ternary> node",
            &mut tree, "//ternary", 1);
    }

    #[test]
    fn ruby() {
        let mut tree = parse_src("ruby", r#"
            def classify(n)
              if n < 0
                "neg"
              elsif n == 0
                "zero"
              elsif n < 10
                "small"
              else
                "big"
              end
            end

            def label(n)
              n > 0 ? "positive" : "non-positive"
            end
        "#);

        claim("one <if> with 2 flat <else_if> siblings",
            &mut tree, "//if[count(else_if)=2]", 1);

        claim("`elsif` renames to <else_if>",
            &mut tree, "//if/else_if", 2);

        claim("no raw `elsif` element leaks",
            &mut tree, "//elsif", 0);

        claim("Ruby ternary uses <conditional> (not <ternary>)",
            &mut tree, "//conditional", 1);
    }

    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn classify(n: i32) -> &'static str {
                if n < 0 { "neg" }
                else if n == 0 { "zero" }
                else if n < 10 { "small" }
                else { "big" }
            }

            fn label(n: i32) -> &'static str {
                if n > 0 { "positive" } else { "non-positive" }
            }
        "#);

        claim("classify: one <if> with 2 <else_if> + 1 <else>",
            &mut tree, "//function[name='classify']/body/if[count(else_if)=2 and count(else)=1]", 1);

        claim("label: if-expression as ternary keeps <then>/<else>",
            &mut tree, "//function[name='label']//if[then and else]", 1);
    }

    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            function classify(n: number): string {
                if (n < 0) { return "neg"; }
                else if (n === 0) { return "zero"; }
                else if (n < 10) { return "small"; }
                else { return "big"; }
            }

            const label = (n: number) => n > 0 ? "positive" : "non-positive";
        "#);

        claim("one <if> + 2 <else_if> + 1 <else>",
            &mut tree, "//if[count(else_if)=2 and count(else)=1]", 1);

        claim("ternary surgically wraps then/else",
            &mut tree, "//ternary[then and else]", 1);
    }
}
