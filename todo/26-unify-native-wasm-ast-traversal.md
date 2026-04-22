# Unify native and WASM AST traversal

## Context

`tractor/src/xot/builder.rs`'s `TreeBuilder` holds two sibling methods
that walk a tree-sitter AST and produce the same xot tree:

- `build_node` — takes `tree_sitter::Node<'_>` (native, behind
  `cfg(feature = "native")`). Uses `cursor.walk()`, `start_position()`,
  `utf8_text()`, `field_name()`.
- `build_serialized_node` — takes `&SerializedNode` (WASM, behind
  `cfg(feature = "wasm")`). Uses `&node.children`, `start_row`,
  `text()`, `field_name.as_deref()`.

The file header claims "Both use `TreeBuilder` internally for shared
tree-building logic" but inside `TreeBuilder` the logic is still
duplicated — the only thing that differs between the two methods is
how each one reaches into its input type; everything else (element
creation, kind attribute, location attrs, leaf text, child iteration,
gap text, trailing text, field attribute, append) is line-for-line
identical.

## Problem

The duplication directly causes bugs. Commit `b6ce94e` ("Separate
literal tree-sitter rendering from field wrapping") simplified
`build_node` to drop the old field-wrapping branch. `build_serialized_node`
wasn't updated because it's only compiled behind `cfg(feature = "wasm")`
and native tests couldn't reach it. The web build caught it
— see commit `c40d377` for the fix, and the PR build failure that
surfaced it.

Every future change to the AST-walking logic has to be made twice;
anyone who only runs `cargo test` won't know they've broken the WASM
path.

## Desired state

A single generic `build_node<N: AstNode>(...)` method, driven by a
trait that abstracts "tree-sitter-like node":

```rust
trait AstNode<'src>: Sized {
    fn kind(&self) -> &str;
    fn is_named(&self) -> bool;
    fn start_byte(&self) -> usize;
    fn end_byte(&self) -> usize;
    fn start_position(&self) -> (usize, usize);
    fn end_position(&self) -> (usize, usize);
    fn field_name(&self) -> Option<&str>;
    fn text(&self, source: &'src str) -> &'src str;
    fn named_child_count(&self) -> usize;
    fn children(&self) -> impl Iterator<Item = Self>;
}

impl<'src> AstNode<'src> for tree_sitter::Node<'src> { ... }   // native
impl<'src> AstNode<'src> for &'src SerializedNode { ... }      // wasm
```

Monomorphisation keeps the per-backend cost zero; a single body means
future changes land once.

## Notes

- The file header's claim of "shared tree-building logic" should
  finally be accurate after this lands.
- An `enum AstNode { Native(...), Serialized(...) }` would also work
  and avoid generics, but adds runtime dispatch on every node access.
  Trait + generics is the idiomatic Rust answer for a hot path.
- Delete `build_serialized_node` once the trait is in place.
- No functional change; pure refactor. Should not require fixture
  regens.
- Test plan: `cargo test -p tractor` (native) plus
  `cargo check -p tractor --features wasm` (compile-check the WASM
  cfg) — both should pass unchanged.
- Related: commit `b6ce94e` (the refactor that caused the drift),
  commit `c40d377` (the WASM-path fix).
