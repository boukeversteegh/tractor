// Principle #14: type references wrap their name in a <name> child.
// Covers: primitive types, paths, generic types, reference types,
// type parameter declarations with <extends> bounds.

use std::collections::HashMap;

trait Barker { fn bark(&self); }

struct Dog<T: Barker> {
    owner: T,
    tags: Vec<String>,
    scores: HashMap<String, i32>,
    parent: Option<Box<Dog<T>>>,
}

impl<T: Barker> Barker for Dog<T> {
    fn bark(&self) {}
}
