// Principle #12: parameter / argument / field lists flatten. Each
// parameter / argument / struct field is a direct sibling with a
// field attribute, no <parameters> / <argument_list> wrapper.

package main

func First(a string, b int, c bool) string {
    return a
}

func Caller() {
    First("x", 1, true)
}

type Config struct {
    Host string
    Port int
    Tls  bool
}
