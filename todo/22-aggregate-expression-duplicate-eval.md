# Aggregate expressions evaluated per-node instead of once

## Problem

Top-level non-path expressions (function calls, constructors, literals) get evaluated
once per context node in the document, producing duplicate results. This is because
xee's `SequenceQuery` evaluates the expression per context node.

Example: `tractor README.md -x 'array { .//heading ! map { "title": inline/text() } }' -f json`
produces the same array repeated for every node in the document.

**Affected:** `sort()`, `for-each()`, `serialize(array{...})`, `true()`, `map{...}`, `array{...}`
**Not affected:** `for/return`, `count()`, `let/return`, `(true())`

## Workaround

`-n 1` clips the output to a single result, but the expression still evaluates against
every node internally — limit is applied via `truncate()` in the reporting stage
(`executor.rs`), not as a short-circuit on evaluation.

## Proposed heuristic

If the top-level expression is not a path/selector (i.e., it's a function call,
constructor, or literal), evaluate it once on the document root. Path expressions like
`heading` or `.//method` naturally select per-node and keep current behavior.

Per-node intent for non-path expressions is already expressed via `!`
(e.g. `some/node ! array { ... }`), so a bare top-level `array{...}` or `map{...}`
has no reason to run per-node.

## Implementation

Check if xee provides a way to evaluate an expression once against the document root
rather than per-node. Apply this for top-level non-path expressions.

## Priority

Medium.
