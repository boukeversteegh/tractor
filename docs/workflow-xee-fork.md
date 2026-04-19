# Xee Fork Workflow

How we maintain a permanent fork of [Paligo/xee](https://github.com/Paligo/xee)
as a dependency of Tractor, while keeping every change cleanly pull-requestable
to upstream.

## Goals

1. **Tractor always builds** against a version of xee that includes our fixes.
2. **Every fix is upstreamable** as an isolated PR the maintainer can accept,
   reject, or cherry-pick.
3. **No drift on `main`** — our fork's `main` is a pure mirror of
   `upstream/main`, so there is never any ambiguity about the correct base for
   a new fix.

## Branch layout

On the fork (`boukeversteegh/xee`):

| Branch                          | Purpose                                                         |
| ------------------------------- | --------------------------------------------------------------- |
| `main`                          | Mirror of `upstream/main`. No local commits ever.               |
| `fix/<topic>`, `feat/<topic>`   | One per upstream PR. Branched from `upstream/main`.             |
| `tractor`                       | Long-lived integration branch. Merge of `upstream/main` + all currently open `fix/*` branches. Tractor depends on this. |
| `tractor-<topic>`               | Tractor-only change (e.g. CI tweaks, fork config) that must never go upstream. Branched from `tractor`, PR'd to `tractor` on the fork. Hyphen — not `tractor/<topic>` — because git refuses a ref under `tractor/` while a `tractor` branch exists. |

Remotes on every local clone:

```bash
git remote add upstream git@github.com:Paligo/xee.git
git remote add origin   git@github.com:boukeversteegh/xee.git  # the fork
```

## One-time setup

```bash
git clone git@github.com:boukeversteegh/xee.git
cd xee
git remote add upstream git@github.com:Paligo/xee.git
git fetch upstream
git branch --set-upstream-to=upstream/main main   # optional, for pull convenience
```

In Tractor's `Cargo.toml`, depend on the `tractor` branch — but **pin to a
commit SHA**, not the branch name, so a `--force` push on `tractor` can never
silently change Tractor's build:

```toml
[dependencies]
xee-xpath = { git = "https://github.com/boukeversteegh/xee.git", rev = "<sha>" }
```

## Starting a new fix

```bash
git fetch upstream
git switch -c fix/my-bug upstream/main
# ...edit, test, commit...
git push -u origin fix/my-bug
gh pr create --repo Paligo/xee --base main \
  --head boukeversteegh:fix/my-bug \
  --title "..." --body "..."
```

Always branch from `upstream/main`, never from `origin/main` or `tractor`.
Our fork's `main` is just a mirror, but branching from `upstream/main` directly
removes any chance of an out-of-date local tracking branch.

## Starting a tractor-only change

Some changes belong only on the fork — CI tweaks, editor config, anything
specific to how we consume xee in Tractor. These bypass upstream entirely:

```bash
git fetch origin
git switch -c tractor-my-tweak origin/tractor   # note the hyphen
# ...edit, commit...
git push -u origin tractor-my-tweak
gh pr create --base tractor --head tractor-my-tweak \
  --title "..." --body "..."
```

The PR targets `tractor` on the fork (not upstream). Do **not** open a PR
from a `tractor-<topic>` branch against `Paligo/xee:main` — flag the commit
as fork-only in the PR description so a later reviewer doesn't propose it
upstream by mistake.

## Adding a fix to the `tractor` branch

Use a **merge commit** (not a rebase, not a squash) so upstream history doesn't
get rewritten into `tractor`:

```bash
git switch tractor
git fetch upstream
git merge --no-ff upstream/main          # pull in any new upstream work first
git merge --no-ff fix/my-bug             # bring the fix in
git push origin tractor
```

Then bump Tractor's `Cargo.toml` to the new `tractor` SHA.

## When upstream accepts a PR

Upstream usually **squash-merges**, so the commit that lands on
`upstream/main` has a different SHA than the one on your `fix/*` branch. Clean
up `tractor` periodically:

```bash
git fetch upstream
git switch tractor
git rebase upstream/main
```

Git's patch-id detection drops commits whose changes are already present
upstream, in most cases automatically. If the maintainer edited the patch
before merging, you may hit conflicts — resolve by keeping the upstream
version (`git checkout --theirs ...` / delete the now-redundant commit).

After a successful cleanup:

```bash
git push --force-with-lease origin tractor
git branch -d fix/my-bug
git push origin --delete fix/my-bug
```

Because Tractor pins `tractor` by SHA, the force-push affects nothing until
Tractor's `Cargo.toml` is bumped.

## When upstream rejects or ignores a PR

The `fix/*` branch and its merge into `tractor` just stay. Nothing to do —
the fix ships to Tractor via `tractor`, even if it never lands upstream.

## Syncing `main` with upstream

Periodically (and always before starting a new fix):

```bash
git fetch upstream
git switch main
git merge --ff-only upstream/main
git push origin main
```

`--ff-only` guarantees we never accidentally commit to `main` — if there's
anything to reconcile, the command fails loudly.

## Trade-offs and gotchas

- **Squash-merges require occasional manual cleanup.** Budget ~5 minutes per
  upstreamed PR to rebase `tractor` and verify the diff against
  `upstream/main` is only what we still carry.
- **Conflicts between fix branches.** If two open `fix/*` branches touch the
  same code, `tractor` will have merge conflicts. Resolve in the merge commit
  on `tractor`; never rewrite the individual `fix/*` branches to accommodate
  each other — they must stand alone to be PR-able.
- **No direct commits to `tractor`.** Every change lands via a `fix/*`,
  `feat/*`, or `tractor-<topic>` branch so there's always a PR trail. Use
  `fix/*` / `feat/*` when the change is intended for upstream, and
  `tractor-<topic>` when it's fork-only.
- **CI on the fork.** A GitHub Actions job that runs `cargo test` on
  `tractor` after every push is cheap insurance against silent conflicts
  introduced by an upstream merge.

## Quick reference

```bash
# Start a fix (upstream-bound)
git fetch upstream && git switch -c fix/X upstream/main

# Open upstream PR
gh pr create --repo Paligo/xee --base main --head boukeversteegh:fix/X

# Start a tractor-only change
git fetch origin && git switch -c tractor-X origin/tractor
gh pr create --base tractor --head tractor-X    # fork-only PR

# Integrate into tractor
git switch tractor && git merge --no-ff upstream/main && git merge --no-ff fix/X && git push

# After upstream accepts
git switch tractor && git fetch upstream && git rebase upstream/main && git push --force-with-lease

# Sync main
git fetch upstream && git switch main && git merge --ff-only upstream/main && git push
```
