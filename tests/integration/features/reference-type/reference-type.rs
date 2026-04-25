// Reference types (`&T`, `&mut T`, `&'a T`) render as a single
// <type> with a <borrowed/> marker (Principle #14: every type
// reference wraps in <type>; Principle #13: empty markers compose).
// The inner referenced type is a nested <type> child.
//
// Queries:
//   //type[borrowed]         -> every reference type
//   //type[borrowed][mut]    -> every mutable borrow
//   //type[borrowed]/type    -> the referenced type

fn read(s: &str) -> &str {
    s
}

fn write(buf: &mut Vec<u8>) {}

fn static_ref() -> &'static str {
    ""
}
