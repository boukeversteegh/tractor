# Glob path resolution in config files

## Problem

When running `tractor run config.yaml`, file glob patterns in the config
are resolved relative to the config file's parent directory. But this
isn't obvious and there's no feedback when globs match nothing.

During development of `tractor-lint.yaml`, the glob
`tractor/src/pipeline/format/*.rs` matched 0 files silently because
paths are relative to the config's directory (`tractor/`), so the
correct path was `src/pipeline/format/*.rs`. This was confusing to
debug — the run completed successfully with 0 results, giving no
indication that the glob was wrong.

## Desired behavior

1. **Better error/warning when globs match no files.** If a `files`
   pattern resolves to 0 files, tractor should warn (or error) rather
   than silently producing an empty report. This catches typos and
   path misunderstandings early.

2. **Document path resolution.** Make it clear in help text and config
   examples that paths are relative to the config file's location.

3. **Consider `--verbose` output.** When `--verbose` is set, show
   which directory globs are resolved relative to and how many files
   matched each pattern.

## Location

- `tractor/src/tractor_config.rs` — config file loading
- `tractor/src/modes/run.rs` — base_dir resolution (lines 45-47)
- `tractor/src/executor.rs` — `resolve_op_files()` expands globs
