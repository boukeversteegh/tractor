//! Rust kitchen-sink fixture: exhaustive blueprint of language constructs used
//! by tractor's snapshot tests to catch design-principle regressions.
#![allow(dead_code, unused_variables, unused_imports, unused_mut)]

use std::collections::{HashMap, HashSet as Set};
use std::fmt::{self, Debug, Display};
use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;
use std::io::{Read as IoRead, Write as IoWrite};
use std::vec::*;

pub mod outer {
    pub(crate) mod inner {
        pub(super) const INNER_CONST: u32 = 7;
        pub(in crate::outer) fn path_vis() {}
    }
}

/// A named-field struct with generics, lifetimes, and a where clause.
#[derive(Debug, Clone)]
pub struct Point<'a, T>
where
    T: Clone + Send + 'static,
{
    pub x: T,
    pub(crate) y: T,
    label: &'a str,
}

/// A tuple struct.
pub struct Pair<T>(pub T, T);

/// A unit struct.
pub struct Marker;

/// An enum with every variant flavor and explicit discriminants.
#[derive(Debug)]
pub enum Shape<'a> {
    Nothing,
    Dot(i32, i32),
    Rect { w: u32, h: u32 },
    Labeled(&'a str) = 10,
}

pub trait Drawable: Debug {
    type Canvas;
    const MAX: usize = 64;
    fn draw(&self, c: &mut Self::Canvas);
    fn name(&self) -> String {
        String::from("drawable")
    }
}

impl<'a, T: Clone + Send + 'static> Point<'a, T> {
    pub const ORIGIN_LABEL: &'static str = "origin";

    pub fn new(x: T, y: T, label: &'a str) -> Self {
        Self { x, y, label }
    }

    pub async fn async_clone(&self) -> Self {
        self.clone()
    }

    pub const fn const_fn() -> u32 {
        42
    }

    pub unsafe fn unsafe_read(ptr: *const T) -> T {
        (*ptr).clone()
    }
}

impl<'a, T> Drawable for Point<'a, T>
where
    T: Clone + Send + Debug + 'static,
{
    type Canvas = Vec<u8>;
    fn draw(&self, _c: &mut Self::Canvas) {}
}

pub type BoxedDraw<'a> = Box<dyn Drawable<Canvas = Vec<u8>> + 'a>;
pub type Callback = fn(i32) -> i32;

pub const PI: f64 = 3.141_592_653_589f64;
pub static GREETING: &str = "hello";
static mut COUNTER: u32 = 0;

fn numeric_zoo() -> (u32, f64, i32, i32, i32) {
    (1u32, 1.0f64, 0x1F, 0b101, 1_000)
}

fn strings_zoo() {
    let plain = "hello";
    let raw = r"C:\path";
    let raw_hash = r#"she said "hi""#;
    let bytes = b"raw bytes";
    let chr = 'z';
}

fn closures_and_refs() {
    let id = |x| x;
    let add: fn(i32, i32) -> i32 = |a, b| a + b;
    let s = String::from("owned");
    let moved = move |x: i32| -> String { format!("{}-{}", s, x) };
    let r: &i32 = &5;
}

pub fn smart_pointers() {
    let b: Box<i32> = Box::new(1);
    let rc: Rc<String> = Rc::new(String::from("rc"));
    let arc: Arc<u32> = Arc::new(9);
    let cell: RefCell<Vec<i32>> = RefCell::new(vec![1, 2, 3]);
    let parsed = "42".parse::<i32>().unwrap_or(0);
}

pub fn patterns(s: Shape) -> i32 {
    let p = Point::new(1, 2, "p");
    let q = Point { x: 10, y: 20, ..p.clone() };

    let result = match s {
        Shape::Nothing => 0,
        Shape::Dot(0, y) | Shape::Dot(y, 0) => y,
        Shape::Dot(x, y) if x == y => x + y,
        Shape::Rect { w, h } if w > 0 => (w * h) as i32,
        Shape::Labeled(ref name) => name.len() as i32,
        _ => -99,
    };
    let bucket = match 5 {
        0..=9 => "single",
        10..=99 => "double",
        _ => "many",
    };
    if let Some(v) = Some(10) { let _ = v; }
    let mut it = vec![1, 2, 3].into_iter();
    while let Some(x) = it.next() {
        let _shadow = x;
        let _shadow = x as f64;
    }
    result
}

pub fn loops_and_flow() -> i32 {
    'outer: for i in 0..10 {
        for j in 0..10 {
            if i * j > 20 {
                break 'outer;
            }
        }
    }
    let mut n = 0;
    while n < 3 {
        n += 1;
    }
    let found = loop {
        break 42;
    };
    found
}

pub fn try_and_cast() -> Result<i32, std::num::ParseIntError> {
    let parsed: i32 = "10".parse()?;
    let casted = parsed as u8 as i32;
    Ok(casted)
}

pub fn trait_object_and_impl<'a>(d: &'a dyn Drawable<Canvas = Vec<u8>>) -> impl Display + 'a {
    format!("name={}", d.name())
}

pub fn diverges() -> ! {
    panic!("never returns");
}

pub async fn async_demo() -> u32 {
    let block = async { 1u32 + 2 };
    block.await
}

pub fn unsafe_block() {
    let x = 5;
    let r = &x as *const i32;
    unsafe {
        let _ = *r;
        COUNTER += 1;
    }
}

fn main() {
    println!("hello, {}", GREETING);
    let v = vec![1, 2, 3];
    let _ = patterns(Shape::Dot(1, 1));
    let _ = loops_and_flow();
}
