# with_groups() only called in check mode

## Status: RESOLVED

Implemented in PR #24 (branch `feature/group-by-file`).

## Decisions Made

1. **Flag**: `-g file` / `--group-by file` added to `SharedArgs` (available in all modes).
   `-g none` explicitly disables grouping.

2. **Defaults**: Check mode defaults to `-g file` (preserving existing behavior).
   Query, test, and set default to no grouping.

3. **Implementation**: `RunContext::build()` takes a `default_group_by_file: bool` parameter.
   Each mode passes its default; user's explicit `-g` overrides it.

4. **File field stripping**: `with_groups()` clears `rm.file` on grouped matches.
   The custom `Serialize` impl skips `file` when empty. Group element owns the file.

5. **Renderer changes were needed** (contrary to the original estimate): gcc, github, text,
   and xml renderers all accessed `rm.file` directly. Updated to accept an optional
   `group_file` parameter and resolve from the group when present.

6. **Validation**: Only `"file"` and `"none"` are accepted values for `--group-by`.
   Other values produce a clear error message.
