// Go distinguishes defined types (`type MyInt int`) from type aliases
// (`type Color = int`). Defined types create a distinct type with its
// own method set; aliases are just a new name for the same type.
//
// Defined type -> <type> (Go's own spec term; inner <type> is the
// underlying type reference).
// Alias        -> <alias> (parallel with Rust / TS / C# / Java).

package main

type MyInt int
type Color = int
