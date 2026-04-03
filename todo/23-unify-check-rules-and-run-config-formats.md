# Unify check rules file and run config file formats

## Problem

The `check --rules` flag and the `run` command accept different file formats
that are almost identical but diverge in small ways:

1. **Root scope naming**: rules file uses `include`/`exclude`, tractor config
   uses `files`/`exclude`.
2. **Diff support**: `diff-files` and `diff-lines` exist only in tractor config.
3. **Per-rule `language`/`tree_mode`**: exist only in the rules file, not in
   the tractor config's `CheckRuleConfig`.
4. **Nesting**: rules file has `rules: [...]` at the root; tractor config nests
   it under `check: { rules: [...] }`.

Having two formats is unnecessary complexity. There is no conceptual reason for
a separate "rules file" — it's just a tractor config that only contains a
`check` operation.

## Desired behavior

- **One file format**: the tractor config format (`check:`, `set:`, `query:`,
  `test:`, `operations:`).
- **`check --rules`** accepts a tractor config and extracts the `check`
  operation from it. The flag could be renamed to `--config` or kept as
  `--rules` for familiarity.
- **No more separate rules file concept** — it's always a "tractor config".
- **Output structure is determined by the command**, not the file contents.
  `check` always produces file-grouped output; `run` always produces
  command+file-grouped output. This avoids content-dependent output structure,
  which is hard for consumers to parse reliably.

## Migration

A minimal check-only tractor config:

```yaml
check:
  rules:
    - id: no-unwrap
      xpath: "//call_expression[function='unwrap']"
      severity: warning
```

One extra level of nesting compared to the old rules file, but conceptually
simpler — one format to learn, one parser to maintain.

## Tasks

- [ ] Remove `rules_config.rs` parser; make `check --rules` reuse
      `tractor_config.rs` parsing.
- [ ] Reconcile field differences (add per-rule `language`/`tree_mode` to
      `CheckRuleConfig`; drop `include` as root scope key or alias it to
      `files`).
- [ ] Update integration tests and example files.
- [ ] Update CLI help text and any documentation.

## Location

- `tractor/src/rules_config.rs` — to be removed
- `tractor/src/tractor_config.rs` — becomes the single config parser
- `tractor/src/modes/check.rs` — update to use tractor config parsing
- `tractor/src/modes/run.rs` — no changes expected
- `tractor/src/cli.rs` — possibly rename `--rules` flag
