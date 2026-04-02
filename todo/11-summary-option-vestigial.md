# Summary is always Some — the Option wrapper is vestigial

## Problem

`Report` declares `summary: Option<Summary>`, but every constructor always sets it to
`Some(...)`:

```rust
pub fn query(matches, summary) -> Self {
    Report { ..., summary: Some(summary), ... }
}
pub fn check(matches, summary) -> Self {
    Report { ..., summary: Some(summary), ... }
}
pub fn test(matches, summary) -> Self {
    Report { ..., summary: Some(summary), ... }
}
```

No code path ever produces a Report with `summary: None`. Yet every renderer guards
access with `if let Some(ref summary) = report.summary { ... }`, suggesting the summary
might be absent. This is misleading — the guard can never be false.

## Why it matters

- **Misleading API**: a reader of the Report struct sees `Option<Summary>` and reasonably
  asks "when is this None?". The answer is never, but you have to trace all constructors
  to learn that.
- **Unnecessary branching**: every renderer has a dead branch for the None case.
- **serde output**: the `#[serde(skip_serializing_if = "Option::is_none")]` annotation
  suggests the summary can be absent from serialized output. It can't.

## Recommendation

Two valid directions:

1. **Make it non-optional**: change `summary: Option<Summary>` to `summary: Summary`.
   Remove all `if let Some(...)` guards in renderers. The serde annotation goes away and
   summary is always in the serialized output.

2. **Make query's summary actually optional**: query mode's summary (`{total, files}`) is
   only shown when `-v summary` is in the ViewSet. If we made query produce
   `summary: None` when summary is not in the view, the Option would be meaningful. Check
   and test always produce `Some(...)` since their summary drives pass/fail semantics.

Option 1 is simpler and matches current behavior exactly. Option 2 is more semantically
correct (query reports don't inherently have a pass/fail summary) but adds a code path
that currently doesn't exist.

## Priority

Low. Cosmetic/clarity. No behavior impact either way.
