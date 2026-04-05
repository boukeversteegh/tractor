# Centralized FileResolver

## Problem

File resolution logic is scattered across `merge_scope`,
`SharedFileScope::build`, `resolve_files`, `resolve_op_files`,
and `resolve_input`. This makes it hard to reason about as a whole
and leads to duplicated glob expansion between CLI and run paths.

## Proposal

Extract a `FileResolver` struct that owns all glob expansion,
caching, intersection, exclusion, and filtering. Operations describe
what they need via a `FileRequest`; the resolver decides how.

Key benefits:
- Automatic dedup: identical glob patterns across operations expand once
- Single pipeline: expansion → intersection → excludes → language filter → diff → limits
- Testable in isolation
- Clear API boundary between "what files do I need" and "how to get them"

## Design

See `docs/design-file-resolver.md` for the full design document.

## Location

New file: `tractor/src/file_resolver.rs`

Current code to consolidate:
- `tractor/src/executor.rs` — `SharedFileScope`, `resolve_files`, `resolve_op_files`
- `tractor/src/tractor_config.rs` — `merge_scope` (files logic removed in #82, exclude/diff remains)
- `tractor/src/pipeline/input.rs` — `resolve_input` (optional, for CLI/run unification)
