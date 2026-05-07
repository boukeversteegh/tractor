# Promote `shape` from output filter to a proper `TreeMode` variant

## Context

Tractor's tree rendering selects between three modes via the
`TreeMode` enum (in `tractor/src/tree_mode.rs`, surfaced as `--tree
raw|structure|data` on the CLI):

- `Raw` — every tree-sitter kind, every grammar leaf
- `Structure` — collapsed structural view
- `Data` — the data-tree projection

Separately, there is a `Projection::Shape` (`-p shape`) which is
NOT a tree mode but an output-stage filter: the projection layer
clones `RenderOptions` with `shape_only = true`, and
`query_tree_renderer.rs` checks that flag at three points to strip
text leaves, comments, and processing instructions. The flag is
plumbed through `RenderOptions` in `tractor/src/output/xml_renderer.rs`
(`pub shape_only: bool`).

## Problem

Shape is a *render decision* about how a tree is materialised, but
it lives in the output / projection layer instead of the tree-mode
layer:

1. `Projection::Shape` is wired only into the formats that handle
   tree projections explicitly — `format/text.rs`, `format/json.rs`,
   `format/xml.rs`. Line-oriented and config-style formats (gcc,
   github, claude-code, yaml) do not get shape rendering. The
   user-visible result is "shape only works in some formats."
2. The `Tree` and `Shape` projection variants share the same
   `view_replacement_field` (`ViewField::Tree`) and only differ via
   the side-channel flag (`projection.rs:109-113` comment: "Shape
   uses the same Tree view-field; the shape_only flag is plumbed
   through RenderOptions"). That comment is the smell — the
   projection layer is doing a tree-mode decision in disguise.
3. `update_snapshots.rs` carries a per-fixture `shape_only: bool`
   tuple so feature fixtures can pick shape rendering. With shape
   as a TreeMode, the same fixtures would just declare `--tree
   shape` and the snapshot machinery would not need a special case.

## Desired state

`TreeMode::Shape` is a fourth variant alongside `Raw`, `Structure`,
`Data`. The rendering pipeline treats it like any other tree mode:

- CLI: `--tree shape` selects it (`cli/context.rs:115-117` already
  handles `raw`/`structure`/`data` — add the `"shape"` arm).
- Renderer dispatches on `TreeMode` and produces the text-stripped
  shape representation directly, so every format that can render a
  tree (text, json, xml) renders shape automatically. Line-oriented
  formats remain unaffected — they don't render trees in the first
  place.
- `Projection::Shape` is dropped (or kept as a deprecated alias
  that internally selects `TreeMode::Shape` on the tree projection).
  The `shape_only` flag on `RenderOptions` is removed.
- `update_snapshots.rs` drops the `shape_only` tuple element and
  feature fixtures pass `--tree shape` instead of `-p shape`.

## What to do

1. Add `Shape` to `tractor/src/tree_mode.rs` (or wherever `TreeMode`
   is defined). Plumb through the renderer dispatch.
2. Move the three text-stripping branches in
   `tractor/src/output/query_tree_renderer.rs` (lines 412, 651,
   657, 663) from "if `options.shape_only`" to "if
   `tree_mode == TreeMode::Shape`."
3. Update CLI parsing in `tractor/src/cli/context.rs` to accept
   `--tree shape`.
4. Delete or alias `Projection::Shape` in
   `tractor/src/format/projection.rs`. The format-specific
   `with_shape_only(true)` calls in `format/{text,json,xml}.rs`
   become unnecessary.
5. Remove the `shape_only: bool` field from `RenderOptions` in
   `tractor/src/output/xml_renderer.rs`.
6. Update `update_snapshots.rs` to drop the `shape_only` tuple
   element; rewrite affected feature fixtures to pass `--tree
   shape`.
7. Snapshot regeneration; verify `task test`.

## Open question

Does shape compose with the existing tree modes, or does it replace
them? Two interpretations:

- **Replacement (simpler)**: `TreeMode::Shape` is a fourth variant
  that emits the data-tree projection with text/comments stripped.
  Equivalent to today's `--tree data -p shape`.
- **Modifier (more flexible)**: shape becomes an orthogonal flag
  that composes with any tree mode (`--tree raw --shape`, `--tree
  structure --shape`). Requires extending the CLI surface but lets
  users see the raw-kind shape, the structure shape, etc.

The replacement interpretation matches today's behaviour
(everywhere `-p shape` is used, the underlying tree mode is
`Data`). The modifier interpretation is more general but requires
a CLI-design call. Pick one when starting the work.

## Notes

- Surfaced while reviewing the cross-language transform shape
  tests added in commit `da850e1` — the snapshot-update tooling's
  `shape_only` per-fixture tuple was the smell that motivated this
  todo.
- Related: the `Projection::Shape` description in
  `format/projection.rs:52` says "Project matched tree shape (no
  text)" — that description belongs on a tree mode, not a
  projection.
