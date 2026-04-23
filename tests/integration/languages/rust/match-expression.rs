// Principle #12: `match_block` (the `{ … }` wrapper around match arms)
// is a pure grouping node; drop it so arms are direct siblings of
// <match>. Parallel with `block` / `declaration_list`.

fn classify(n: i32) -> &'static str {
    match n {
        0 => "zero",
        1 | 2 | 3 => "small",
        _ if n < 0 => "negative",
        _ => "other",
    }
}
