# Glob path resolution in config files

**Status**: Mostly resolved by #72 and #82.

## What was done

1. **Empty-glob fatal diagnostic** (#72): If glob patterns match 0
   files, tractor now reports a fatal error instead of silently
   succeeding.

2. **`--verbose` output** (#82): Shows which directory globs are
   resolved relative to, the patterns being expanded, and file counts
   at each intersection step. Printed before expansion starts so
   hangs are diagnosable.

3. **Documentation**: RunCommand docs updated with file resolution
   details including base directory behavior.

## Remaining

- Per-operation empty-intersection diagnostic: when root ∩ operation
  yields 0 files, there's no specific diagnostic (the empty-glob
  check doesn't fire since individual patterns did match). Could be
  a useful warning.
