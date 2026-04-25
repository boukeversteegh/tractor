// Conditional shape: `else if` chain collapses to flat <else_if>
// siblings. Ternary via if-expression keeps <then>/<else>.

fn classify(n: i32) -> &'static str {
    if n < 0 {
        "neg"
    } else if n == 0 {
        "zero"
    } else if n < 10 {
        "small"
    } else {
        "big"
    }
}

fn label(n: i32) -> &'static str {
    if n > 0 { "positive" } else { "non-positive" }
}
