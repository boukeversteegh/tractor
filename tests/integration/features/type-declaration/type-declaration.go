// Go's `type_declaration` wrapper is dropped; `type_spec` renders as
// <type> directly. Parallel with struct/interface declarations so
// //type queries find everything.

package main

type ID uint64

type User struct {
    Name string
    Age  int
}

type Greeter interface {
    Greet() string
}
