# XPath as value specification (value-less set)

## Goal

Allow `tractor set` to work without a `--value` flag by extracting the
target value from the XPath expression itself. The XPath query becomes a
declarative spec: "ensure this is true after the set completes."

## Syntax

The natural way to set a property is with an attribute predicate on the parent:

```sh
# Set db's host to "localhost"
tractor set -x "//db[@host='localhost']"

# Set server's port to 8080
tractor set -x "//server[@port='8080']"

# Set nested value
tractor set -x "//database[@connection='postgres://...']"
```

This reads as: "find (or create) `db` and ensure it has `host = localhost`."

The `[@key='value']` predicate on the parent is more natural than using a
child path with a dot-predicate:

```sh
# Less natural (child path + value predicate):
tractor set -x "//db/host[.='localhost']"

# More natural (attribute predicate on parent):
tractor set -x "//db[@host='localhost']"
```

## Semantics

The XPath predicates become postconditions — after the set completes, the
query should match. This means:

1. **Node exists, value matches** — no-op
2. **Node exists, value differs** — update the value
3. **Node doesn't exist** — insert with the value from the predicate

If the set would produce an invalid syntax tree for the target language,
it should fail with a clear error.

## Interaction with --value flag

- When `--value` is provided, it takes precedence (current behavior)
- When `--value` is omitted, extract the value from the XPath predicate
- Error if neither `--value` nor a value predicate is present

## Implementation notes

- Extend the insert path's XPath parsing to extract `[@key='value']`
  predicates as key-value pairs
- The update path already handles arbitrary XPath via the full xee engine;
  value extraction is only needed when `--value` is omitted
- Consider supporting multiple predicates:
  `//db[@host='localhost'][@port='5432']` to set several properties at once
