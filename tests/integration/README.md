# Integration Tests

Integration tests for tractor are written in Rust and live in
`tractor/tests/cli/`. They run the `tractor` binary as an external subprocess
(true black-box testing) and avoid bash entirely for cross-platform reliability.

## Running Tests

```bash
# Run all tests (unit + integration)
cargo test --workspace --release

# Run only CLI integration tests
cargo test --test cli --release

# Run a specific test module
cargo test --test cli --release languages
cargo test --test cli --release set
cargo test --test cli --release run_cmd
```

## Test Fixtures

This directory still contains the fixture files (sample source files, config
YAML, XML snapshots) used by the Rust integration tests. The test code
references them via `env!("CARGO_MANIFEST_DIR")`.

## Directory Structure

```
tests/integration/
├── languages/         # Language-specific fixtures and snapshots
│   ├── rust/          # sample.rs, sample.rs.xml, sample.rs.raw.xml
│   ├── python/        # sample.py, multiline-string-*.py, ...
│   ├── typescript/    # sample.ts, ...
│   └── ...            # 15 languages total
├── formats/           # Output format fixtures and snapshots
│   └── set/           # Set command format snapshots
├── run/               # Batch execution (tractor run) fixtures
│   ├── mixed-language/
│   ├── scope-intersection/
│   └── absolute-paths/
├── replace/           # (fixtures for set command tests)
├── update/            # (fixtures for update command tests)
├── string-input/      # (no fixtures needed, uses --string flag)
├── view-modifiers/    # (uses formats/sample.cs)
├── xpath-expressions/ # (no fixtures needed, uses --string flag)
└── README.md          # This file
```

## Adding New Tests

1. Add fixtures to the appropriate `tests/integration/` subdirectory
2. Add test functions in `tractor/tests/cli/` (the relevant module)
3. Run `cargo test --test cli --release` to verify
