//! Go: const_spec / var_spec / import_spec wrapper flattening.

use crate::support::semantic::*;

/// Principle #12 — Go's `const_spec` / `var_spec` / `import_spec`
/// are grammar wrappers around `name = value` / `path`. Flatten
/// so a declaration reads as `<const>const<name>x</name>=<value>1</value></const>`
/// rather than burying the assignment inside an opaque spec
/// element.
#[test]
fn go() {
    claim("Go const spec flattens to named declaration with direct value",
        &mut parse_src("go", r#"
        package main

        const x = 1
    "#),
        &multi_xpath(r#"
            //const
                [name='x']
                [value/int='1']
        "#),
        1);

    claim("Go var spec flattens to named declaration with direct value",
        &mut parse_src("go", r#"
        package main

        var y = 2
    "#),
        &multi_xpath(r#"
            //var
                [name='y']
                [value/int='2']
        "#),
        1);
}
