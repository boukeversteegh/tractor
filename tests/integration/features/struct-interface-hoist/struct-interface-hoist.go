// The `type Foo struct { … }` / `type Foo interface { … }` grammar
// wrappers hoist: the outer element becomes <struct> or <interface>
// directly instead of the Go-spec `<type>` wrapper. A developer reads
// "I'm declaring a struct named Foo" (Goal #5 — developer mental
// model), not "I'm declaring a type that happens to be a struct."

package main

type Config struct {
    Host string
    Port int
}

type Greeter interface {
    Greet() string
}
