// Conditional shape: flat <if>/<else_if>/<else>. Go has no ternary.

package main

func Classify(n int) string {
    if n < 0 {
        return "neg"
    } else if n == 0 {
        return "zero"
    } else if n < 10 {
        return "small"
    } else {
        return "big"
    }
}
