# Set CLI mode bypasses executor

## Context

While unifying check and run config formats (todo/23), we added
`--config` to all commands via a shared `run_from_config()` executor.
This revealed that the `set` command's CLI path is the only command
that completely bypasses `executor::execute()` — all others (check,
query, test, update) already route through it.

## Problem

`modes/set.rs` implements its own file I/O, matching, upsert, and
report building instead of delegating to `executor::execute_set()`.
This means:

- Two separate implementations of set logic that can diverge
- `set --config` goes through the executor; `set file.yaml -x ...`
  does not
- The CLI path has features the executor lacks (and vice versa)

The CLI path supports features not in the executor:
1. **Declarative mode** — path expressions like `database[host='localhost']`
2. **`--stdout` mode** — print modified content instead of writing files
3. **Per-match source snippets** — the executor only reports file-level status

The executor's `execute_set` is the simpler, config-oriented version.

## Desired state

The CLI set command delegates to the executor for the core
match-and-upsert logic, the same way check/query/test/update do.
The CLI-specific features (declarative parsing, stdout mode) are
handled as pre/post-processing around the executor call.

## Notes

- See comment in `modes/set.rs` line 243: "set mode bypasses the executor"
- Related: todo/21 (unified pipeline architecture)
- Related: todo/22 (stdout bypass report pipeline)
- The executor already has unit tests for `execute_set` via `run` configs
