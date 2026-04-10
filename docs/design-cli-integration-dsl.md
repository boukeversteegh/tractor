# CLI Integration Test DSL

This note documents the design of the Rust-native CLI integration test DSL in
`tractor/tests/support/mod.rs` and `tractor/tests/cli.rs`.

## Goal

The DSL should let integration tests read like real Tractor commands while
keeping test-harness assertions explicit and separate.

The intended reading model is:

1. run a Tractor command
2. assert on the result

It is intentionally not modeled as "everything is really `tractor test`".

## Why The DSL Is Split Into Command + Assertions

Only native `tractor test` has built-in expectation semantics.

Other commands such as:

- `query`
- `check`
- `set`
- `update`
- `run`

do not.

So the integration DSL models a real CLI invocation plus harness assertions:

```text
tractor query "sample.cs" -x "//method"
=> count 5
```

That keeps the command side honest about what Tractor actually accepts on the
CLI and keeps the assertion side honest about what the test harness is adding.

## Surface Forms

The DSL supports two user-facing forms.

### One-liner

For simple single-assertion cases:

```rust
methods_exist => tractor query "sample.cs" -x "//method" => count 5;
todo_check => tractor check "sample.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO found" => exit 1;
```

### Block Form

For commands with richer outcomes or multiple assertions:

```rust
set_host => {
    tractor set "config.yaml" -x "//database/host" --value "db.example.com";
    expect => {
        exit 0;
        file_contains "config.yaml" "db.example.com";
    }
}
```

There is also a block shorthand for a single assertion:

```rust
methods_exist => {
    tractor query "sample.cs" -x "//method";
    expect => count 5;
}
```

The one-liner is conceptually shorthand for the block form.

## Why Shorthand And Block Forms Are Normalized Internally

The macro accepts both:

```rust
expect => count 5
```

and:

```rust
expect => {
    count 5;
}
```

Both normalize into the same internal path before execution. This avoids
duplicating assertion parsing logic and keeps the grammar easier to extend.

The same principle applies to the one-liner case form: the macro lowers the
surface syntax into a single `TestCase { command, assertions }` model.

## Internal Model

The implementation is organized around:

```rust
struct TestCase {
    command: TractorInvocation,
    assertions: Vec<Assertion>,
}
```

Conceptually:

- `TractorInvocation` is the CLI command under test
- `Assertion` is a harness-side expectation
- fixture setup and output normalization are harness concerns around execution,
  not part of the command syntax

## Layering

The implementation is kept in four visible layers.

### 1. Suite Structure

`cli_suite!` groups named cases under a fixture/module.

### 2. Command Capture

`TractorInvocation` stores CLI arguments, stdin, and path-handling details.

The command side remains visually close to real Tractor CLI usage so commands
can be copied into a terminal and checked directly.

### 3. Assertion Parsing

Assertions such as:

- `count 5`
- `count some`
- `exit 1`
- `stdout "..."`
- `file_contains "config.yaml" "..."`

belong to the harness, not Tractor itself.

### 4. Execution

The harness:

1. prepares the fixture context
2. runs the Tractor invocation
3. evaluates assertions against the result

## Conservative Grammar

The DSL is intentionally small and regular.

It does not try to recreate arbitrary shell parsing. In particular, it avoids:

- many equivalent spellings
- command-specific magic when not needed
- ambiguous parsing rules
- hidden defaults that are hard to infer from examples

This makes the DSL easier to read, maintain, and generate from examples.

## Why Quoted Literals Are Required

Paths, XPath expressions, and similar values stay quoted because that is a good
Rust-macro tradeoff:

- it keeps the syntax close to the real CLI
- it keeps tokenization predictable
- it avoids fragile shell-like parsing in macro rules

The command should feel CLI-shaped, but it still needs to be robust inside Rust
syntax.

## Count Assertions

`count` is treated as a harness assertion, even though Tractor can also print a
count via `-v count`.

This is deliberate:

- the test still reads as the command the user would actually run
- the harness is free to observe count output as part of assertion evaluation
- expectations are not encoded back into the command syntax for commands that do
  not natively own them

That keeps the command/assertion boundary intact while still using normal CLI
behavior under the hood.
