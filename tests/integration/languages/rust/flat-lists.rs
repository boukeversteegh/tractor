// Principle #12: flat parameters / arguments / generics. No wrapping
// <parameters>/<arguments>/<type_parameters> element — children are
// direct siblings with field attributes.

fn first<T, U: Clone>(a: T, b: U, c: i32) -> T {
    a
}

fn caller() {
    first::<String, i32>(String::from("x"), 1, 2);
}
