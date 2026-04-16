# Implicit default config file

## Problem

Every command that accepts a configuration file (`tractor run`, etc.)
currently requires the path as a mandatory argument. This adds
friction — users must type `tractor run tractor.yml` even when the
config sits right next to them in the project root.

## Proposal

Make `tractor.yml` (and `tractor.yaml`) the implicit default for any
command that accepts a config file. When the argument is omitted,
tractor looks for `tractor.yml` or `tractor.yaml` in the current
directory and uses it automatically.

This means:
- `tractor run` is equivalent to `tractor run tractor.yml`
- An explicit path still overrides the default
- If neither `tractor.yml` nor `tractor.yaml` exists and no path is
  given, tractor should report a clear error

## Why `tractor.yml` instead of `.tractor.yml`

Dotfiles are hidden by default on Unix — users need `ls -a` or
explicit IDE settings to see them. That creates unnecessary friction:
the config is invisible unless you know to look for it.

A plain `tractor.yml` is immediately visible in the project root.
It feels accessible and approachable — just a simple file you can
open and edit. Dotfiles carry the connotation of complex, low-level
configuration scripts that are hard to write and debug. Tractor's
config should feel like the opposite of that.

## Scope

All commands that accept a `<CONFIG>` argument.
