//! Cross-language: error-handling shapes.
//!
//! Most languages share `<try>` / `<catch>` / `<finally>` / `<throw>`.
//! Python distinguishes `<except>` / `<raise>`. Ruby uses
//! `<begin>` / `<rescue>` / `<ensure>` (its own keywords; `raise`
//! is currently a plain call, not a dedicated element). Rust's
//! `<try>` is a different construct — the `?` suffix operator —
//! and is exercised separately in the Rust quirk files.
//!
//! These tests pin the rename decisions documented in the
//! per-language transformation specs. The catch-parameter and
//! call-body shapes are intentionally NOT asserted; only the
//! top-level structural rename and child-clause ordering are
//! pinned.

use crate::support::semantic::*;

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class Service {
            void run() {
                try { flakyMethod(); }
                catch (Exception err) { handleError(); }
                finally { cleanup(); }
            }
            void boom() { throw new RuntimeException(); }
        }
    "#);

    claim("Java try statement carries body, catch, and finally as direct children",
        &mut tree, "//try[body][catch][finally]", 1);

    claim("Java throw statement renders as <throw>",
        &mut tree, "//throw", 1);
}

#[test]
fn csharp() {
    let mut tree = parse_src("csharp", r#"
        class Service {
            void Run() {
                try { FlakyMethod(); }
                catch (System.Exception err) { HandleError(); }
                finally { Cleanup(); }
            }
            void Boom() { throw new System.Exception(); }
        }
    "#);

    claim("C# try statement carries body, catch, and finally as direct children",
        &mut tree, "//try[body][catch][finally]", 1);

    claim("C# throw statement renders as <throw>",
        &mut tree, "//throw", 1);
}

#[test]
fn typescript() {
    let mut tree = parse_src("typescript", r#"
        function run() {
            try { flakyMethod(); }
            catch (err) { handleError(); }
            finally { cleanup(); }
        }
        function boom() { throw new Error("network down"); }
    "#);

    claim("TypeScript try statement carries body, catch, and finally as direct children",
        &mut tree, "//try[body][catch][finally]", 1);

    claim("TypeScript throw statement renders as <throw>",
        &mut tree, "//throw", 1);
}

#[test]
fn php() {
    let mut tree = parse_src("php", r#"<?php
        function run() {
            try { flakyMethod(); }
            catch (Exception $err) { handleError(); }
            finally { cleanup(); }
        }
        function boom() { throw new Exception("network down"); }
    "#);

    claim("PHP try statement carries body, catch, and finally as direct children",
        &mut tree, "//try[body][catch][finally]", 1);

    claim("PHP throw statement renders as <throw>",
        &mut tree, "//throw", 1);
}

/// Python uses `except` (not `catch`) and `raise` (not `throw`) —
/// the rename keeps the language-idiomatic keyword. Multiple
/// `except` clauses are flat siblings of `<try>`.
#[test]
fn python() {
    let mut tree = parse_src("python", r#"
        def run():
            try:
                flaky_method()
            except ValueError as err:
                handle_error()
            except Exception:
                handle_unknown()
            finally:
                cleanup()

        def boom():
            raise ValueError("bad input")
    "#);

    claim("Python try carries body, two except clauses, and a finally as flat siblings",
        &mut tree, "//try[body][count(except)=2][finally]", 1);

    claim("Python except with `as` binding exposes both the type and the binding name",
        &mut tree,
        &multi_xpath(r#"
            //try/except/value/expression/as
                [name='ValueError']
                [name='err']
        "#),
        1);

    claim("Python type-only except has the bare type as value with no binding",
        &mut tree,
        &multi_xpath(r#"
            //try/except
                [value/expression/name='Exception']
                [not(value/expression/as)]
        "#),
        1);

    claim("Python raise statement renders as <raise>",
        &mut tree, "//raise", 1);
}

/// Ruby uses `begin` / `rescue` / `ensure` — distinct keywords
/// from the C-family `try` / `catch` / `finally` set. Each becomes
/// its own element. `raise` does NOT yet have a dedicated element
/// — it currently renders as a plain `<call>` to a name "raise"
/// — so it's not asserted here.
#[test]
fn ruby() {
    claim("Ruby begin block carries the body, a rescue clause, and an ensure clause",
        &mut parse_src("ruby", r#"
        def run
          begin
            flaky_method
          rescue StandardError => err
            handle_error
          ensure
            cleanup
          end
        end
    "#),
        &multi_xpath(r#"
            //begin
                [rescue/exceptions/name='StandardError']
                [ensure]
        "#),
        1);
}
