# Re-parse per tree mode in multi-rule check

## Problem

When running multi-rule checks (`run_rules`), each file is parsed once
using the first applicable rule's `tree-mode` and `language` overrides.
If different rules specify different overrides for the same file, only
the first rule's settings are used — subsequent rules query against a
tree that may not match their intended mode.

## Location

`tractor/src/pipeline/matcher.rs` — the `run_rules` function, around
the "Resolve per-file language/tree_mode" comment.

## Fix

Group applicable rules by `(effective_language, effective_tree_mode)`.
For each unique combination, parse the file once and run only the rules
in that group against the resulting tree. This means a file could be
parsed multiple times if rules disagree on mode, but each rule sees
the tree it expects.

## Impact

Low — this only matters when a ruleset contains rules with different
`tree-mode` or `language` overrides that apply to overlapping files.
Current real-world rulesets use a single mode for all rules.
