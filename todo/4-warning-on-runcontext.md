# `warning` field on RunContext is test-mode specific

## RESOLVED

Removed `--warning` flag from `tractor test` entirely, along with the `warning` field
on `RunContext` and the `warning` parameter on `render_test_report`.

Rationale: the `--warning` flag was a boolean toggle for something that `check` already
models as `--severity warning|error`. Rather than unifying via severity on test (which
has no real use case for it — test mode is single-assertion, and our own test suite
relies on the error exit code), we removed warning mode from test altogether.

If non-fatal test output is needed in the future, it should use `--severity` like check.
