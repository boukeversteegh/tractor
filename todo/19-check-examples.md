# Check rule examples (expect valid/invalid)

## Problem

Check rules have no way to verify that their XPath query actually matches
what it should. When tractor updates its parser, queries can silently
break. Users also lack a built-in way to document what a rule catches
versus what it allows.

## Solution

Add `expect` entries to check rules — code examples annotated as `valid`
(should not trigger the check) or `invalid` (should trigger the check).
Tractor validates these by reusing the existing `TestOperation` system
with inline source.

## Config format

```yaml
rules:
  - id: no-todo
    xpath: "//comment[contains(.,'TODO')]"
    language: rust
    expect:
      - valid: "// This is a regular comment"
      - invalid: "// TODO: fix this"
```

Each entry is an object with optional `valid` and/or `invalid` fields.

## CLI

```bash
tractor check "src/**/*.rs" -x "//query" --reason "..." -l rust \
  --expect-valid "good code" \
  --expect-invalid "bad code"
```

## Implementation

- `Rule` struct gains `pass_examples` / `fail_examples` Vec fields
- `execute_check()` generates `TestOperation`s from examples before
  running the file check, converting failures into check report matches
- Config parsers (`rules_config.rs`, `tractor_config.rs`) deserialize
  `expect` entries into the Rule fields
- Language resolution: rule `language` → operation `language` → error

## Future

- Minimal pairs: entries with both `valid` and `invalid` for side-by-side
  documentation and mutual exclusivity validation
- Virtual files: embed examples as snippets within source files for
  richer output (highlighting the example in context)

## References

- `specs/rules.md` — "Valid/Invalid Examples" section
- `docs/usecase-integration-test-framework-linting.md` — real-world use case
