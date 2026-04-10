# CLI Integration Tests

The CLI integration suite is now Rust-native and lives in `tractor/tests/cli.rs` with shared helpers in `tractor/tests/support/mod.rs`.

Run it with:

```bash
cargo test -p tractor --test cli
```

Or run the full tractor package test suite:

```bash
cargo test -p tractor
```

## Why this replaced the shell harness

- The tests invoke the compiled `tractor` binary directly, so there is no bash, `cygpath`, or `wslpath` logic to keep in sync.
- Temp fixtures, path normalization, stdout snapshots, and in-place file assertions are all centralized in one helper layer.
- `cargo test` builds the binary and runs the integration suite cross-platform from the same entrypoint.

## Adding Tests

Simple query/assertion cases should be one line inside a `cli_suite!` block:

```rust
functions_exist => expect("sample.rs", "function", "4");
```

More complex cases should still fit in one fluent statement:

```rust
case(["set", "sample.yaml", "-x", "//database/host", "--value", "db.example.com"])
    .in_fixture("formats/set")
    .temp_fixture()
    .replace_output("sample.yaml", "tests/integration/formats/set/sample.yaml")
    .stdout_snapshot("formats/set/set.txt")
    .run();
```

## Fixture Layout

The fixtures and snapshot files under `tests/integration/` are still the source data for the Rust suite:

- `languages/` holds language-specific sample files.
- `formats/` holds output snapshots.
- `run/`, `replace/`, `update/`, `string-input/`, `view-modifiers/`, and `xpath-expressions/` hold command-specific fixtures.

The old shell harness has been removed. `cargo test` is now the supported entrypoint for Rust-side integration coverage.
