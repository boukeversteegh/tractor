---
title: Expect Option (-e, --expect)
priority: 1
---

Assert expected match count for CI checks. Sets exit code to 1 if not met.

Values:
- none: expect zero matches (fail if any found)
- some: expect at least one match (fail if none)
- N (number): expect exactly N matches

Example: --expect none to fail if any matches found
