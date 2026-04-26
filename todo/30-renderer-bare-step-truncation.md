# Renderer truncates bare path-step XPaths under `--single`

## Symptom

When a fixture's XPath is a bare path step like `//class` or `//if`,
combining it with `-p tree --single` and `--depth N` truncates the
rendering to just the element header — `class\n` — even though the
full subtree should render.

Name-qualified queries work correctly:

```
tractor query 'samples/Repo.cs' "//class" -p tree --single --depth 5
# → renders only "class"

tractor query 'samples/Repo.cs' "//class[name='Repo']" -p tree --single --depth 5
# → renders the full <class> subtree
```

## Workaround

`update_snapshots.rs` rewrites the `where-clause.cs` fixture XPath to
`//class[name='Repo']` instead of `//class`, with a `TODO` comment
pointing back here.

## Likely cause

The tree-render path interprets `--single` as "show root only" when
the matched node is a path-step result with cardinality > 1, even
though `--single` is supposed to pick the first match and render its
subtree. The name-qualified query returns a single match and avoids
the truncation branch.

## What to do

1. Reproduce with the workaround removed.
2. Trace the renderer dispatch in `tractor/src/format/text.rs` and
   wherever `--single` is interpreted.
3. Fix so that `--single` consistently means "pick first match,
   render full subtree under depth limit," regardless of XPath shape.
4. Drop the `[name='Repo']` workaround from `update_snapshots.rs`.

## Origin

Surfaced during PR #148 (proposal E1).
