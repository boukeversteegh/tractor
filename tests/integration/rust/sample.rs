// Simple Rust example
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    let result = add(5, 3);
    println!("Result: {}", result);
}

pub(crate) fn helper() -> bool {
    true
}

pub struct Point {
    pub x: i32,
    y: i32,
}

enum Color {
    Red,
    Green,
    Blue,
}

pub trait Drawable {
    fn draw(&self);
}

mod internal {
    pub(super) fn secret() {}
}
