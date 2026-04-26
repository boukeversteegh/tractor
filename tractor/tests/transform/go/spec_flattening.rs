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

    claim("Go const/var specs flatten to named declarations with direct values",
        &mut tree,
        &multi_xpath(r#"
            //file
                [const
                    [name='x']
                    [value/int='1']]
                [var
                    [name='y']
                    [value/int='2']]
        "#),
        1);
}
