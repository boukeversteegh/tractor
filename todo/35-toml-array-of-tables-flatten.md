# Flatten TOML `[[arrays-of-tables]]` and YAML repeated keys into a single element

## Context

Several data-language constructs that semantically describe a
single keyed array render as multiple sibling elements with the
same name:

**TOML — `[[servers]]`** (array of tables):
```toml
[[servers]]
name = "web-1"
port = 8080

[[servers]]
name = "web-2"
port = 8081
```
Semantically: `servers = [{name: "web-1", port: 8080}, {name: "web-2", port: 8081}]`.

**YAML — repeated mapping under a sequence**:
```yaml
servers:
  - name: web-1
    port: 8080
  - name: web-2
    port: 8081

features:
  - auth
  - logging
  - metrics
```
Semantically: `servers` is one sequence with two items; `features` is one
sequence with three string items.

## Problem

The current semantic transform renders both forms as **multiple
sibling elements with the same name**.

For TOML `[[servers]]` — two sibling `<servers>` elements,
each with one `<item>` inside:

```xml
<document>
  <servers>
    <item>
      <name>web-1</name>
      <port>8080</port>
    </item>
  </servers>
  <servers>
    <item>
      <name>web-2</name>
      <port>8081</port>
    </item>
  </servers>
</document>
```

For YAML `servers:` with two list items — two sibling `<servers>`
elements, each holding one item's fields directly:

```xml
<document>
  <servers><name>web-1</name><port>8080</port></servers>
  <servers><name>web-2</name><port>8081</port></servers>
</document>
```

And YAML `features:` with three string items — three sibling
`<features>` elements with the strings as text:

```xml
<document>
  <features>auth</features>
  <features>logging</features>
  <features>metrics</features>
</document>
```

This contradicts the way every *other* array form in tractor's
data-language transforms collapses repeats into a single parent
with multiple `<item>` children — see e.g. TOML's own
`features = [...]` (one `<features>` with three `<item>`s) and
JSON arrays. A query like `//servers/item` happens to work either
way today (in TOML), but `count(//servers)` returns N when
intuitively it should return 1, and `//servers[count(item)=N]`
fails to match when it should match.

## Desired state

`[[servers]]` collapses into a single `<servers>` element with
one `<item>` per repetition, matching the inline-array shape:

```xml
<document>
  <servers>
    <item>
      <name>web-1</name>
      <port>8080</port>
    </item>
    <item>
      <name>web-2</name>
      <port>8081</port>
    </item>
  </servers>
</document>
```

After the fix:
- `count(//servers) = 1` (one logical key).
- `count(//servers/item) = N` (one item per `[[servers]]` block).
- The element-or-array distinction at the document level is consistent
  with TOML's data model and with every other array shape in tractor.

## What to do

1. In the TOML semantic transform (likely
   `tractor-core/src/languages/toml/...` — confirm exact path),
   detect adjacent same-named array-of-tables headers and merge their
   items under a single parent element.
2. Update the test in `tractor/tests/transform/toml.rs::toml_array_of_tables`:
   currently pinned to the buggy "two sibling `<servers>`" shape
   so the suite stays green (with a `TODO #35` comment); flip it
   to assert the unified shape once the transform is fixed.
3. Snapshot regeneration; verify `task test`.

## Notes

- Surfaced while migrating the toml cli_suite block to
  `tests/transform/toml.rs` (Phase F). The pre-existing cli_suite
  used `//servers/item[name='web-1']` which silently worked under
  both shapes, masking the bug.
- Fix in JSON / YAML / inline-table TOML paths to confirm none of
  them need similar treatment.
