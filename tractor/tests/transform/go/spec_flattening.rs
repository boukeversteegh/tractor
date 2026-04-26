//! Go: const_spec / var_spec / import_spec wrapper flattening.

use crate::support::semantic::*;

/// Principle #12 — Go's `const_spec` / `var_spec` / `import_spec`
/// are grammar wrappers around `name = value` / `path`. Flatten
/// so a declaration reads as `<const>const<name>x</name>=<value>1</value></const>`
/// rather than burying the assignment inside an opaque spec
/// element.
#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        const x = 1
        var y = 2
    "#);

    claim("no <const_spec> wrapper",
        &mut tree, "//const_spec", 0);

    claim("no <var_spec> wrapper",
        &mut tree, "//var_spec", 0);

    claim("const's name is a direct child, not buried under const_spec",
        &mut tree, "//const[name='x']", 1);
}
