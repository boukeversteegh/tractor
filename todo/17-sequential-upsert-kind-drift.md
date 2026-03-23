# Sequential upsert kind drift

## Problem

When `declarative_set` applies multiple upserts sequentially, re-parsing
the intermediate source between operations can lose type information.

Example: `server[port=3000][debug=false()][marker]` on a YAML file.

1. First upsert inserts `port: 3000` — correct.
2. Second upsert inserts `debug: false` — correct (kind="boolean").
3. Third upsert re-parses the file. The YAML parser reads `false` and
   may assign `kind="string"` instead of `kind="boolean"`, causing the
   re-rendered output to become `debug: "false"`.

The root cause: each upsert cycle is parse → mutate → render → splice.
When the modified source is re-parsed for the next operation, the parser
doesn't necessarily recover the original kind annotation. This affects
booleans, numbers, and null values in YAML (and potentially JSON).

## Impact

- Only affects declarative set with 3+ operations (where a previously
  inserted typed value gets re-parsed by a later upsert).
- Single upserts and two-operation sequences typically work correctly.
- JSON may be less affected if its parser preserves types better.

## Possible fixes

1. **Batch mode**: apply all operations in a single parse-mutate-render
   cycle instead of sequential upserts. This avoids re-parsing entirely.
2. **Parser kind preservation**: ensure the YAML/JSON parsers correctly
   annotate `kind` on scalar nodes during re-parse (e.g., YAML `false`
   should get `kind="boolean"`, not `kind="string"`).
3. **Hybrid**: for declarative set, build all mutations on the same
   parsed tree before rendering once.

Option 1 or 3 would also improve performance for expressions with many
predicates.
