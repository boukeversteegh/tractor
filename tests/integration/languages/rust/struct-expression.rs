// Struct construction: `Point { x: 1, y: 2 }` renders as <literal>
// with a <name> child for the struct name and <field> siblings for
// each initializer. Symmetric with JS/C# object construction:
// //literal[name='Point'] finds every Point construction site.
//
// The name of the struct is a <name>, not a <type>, because this is
// a reference-by-name to the struct being instantiated — not a type
// annotation.

struct Point {
    x: i32,
    y: i32,
}

fn make() {
    let p = Point { x: 1, y: 2 };
    let q = Point { x: 0, ..p };
}
