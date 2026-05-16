# Parity-track variant retirement (C5)

`Atom`, `SimpleStatement`, and `FieldWrap` are the three open-set
string-discriminator variants left in `SyntaxTree`. They were
introduced as parity-track wrappers during the legacy XML pipeline
migration: a CST kind whose old rule was "rename to X" lowers to
`SimpleStatement { element_name: "X", … }`; arbitrary field wraps
go through `FieldWrap { wrapper: "X", … }`; T-SQL identifier-role
classifications use `Atom { element_name: "X", … }`.

After C8 these are the last remaining sources of leakage between
the variant identity and the output element name. The codegen still
has a special-case rule for `Atom` / `SimpleStatement` / `FieldWrap`
in `element_name_of` that reads the discriminator field; everything
else gets variant-tag-as-name.

## Why retirement isn't a single commit

Current scale:
- ~351 `SyntaxTree::SimpleStatement` construction sites
- ~14 `SyntaxTree::Atom` construction sites
- ~15 `SyntaxTree::FieldWrap` construction sites

`SimpleStatement` is the workhorse used for:
1. **Slot wrappers** — `<left>`, `<right>`, `<condition>`,
   `<value>`, `<type>`, `<op>`, `<as>`, `<filter>` … emitted by
   `SyntaxTree::wrap_slot` / `wrap_clause` / `wrap_type` /
   `wrap_extends`.
2. **Keyword statements** — `<assert>`, `<raise>`, `<delete>`,
   `<global>`, `<nonlocal>`, `<yield>`, `<throw>`, `<break>`,
   `<continue>`, `<return>` (where typed Return isn't used) …
3. **Operator nodes** — `<op>+<plus/></op>` inside Binary/Unary.
4. **Container wrappers** — extends/implements clauses, where
   clauses, throws clauses, …

Each category has its own ideal typed shape; the retirement plan
is multi-pass.

## Retirement plan (future work)

**Pass 1: dedicated slot variants.** ✅ **Landed 2026-05-16** (commit
`e8d52779`). `SlotKind` closed enum (`Left`/`Right`/`Condition`/
`Then`/`Else`/`As`/`Filter`) plus typed `SyntaxTree::Slot` variant.
`wrap_slot(&str)` validates the name and routes to the typed
`wrap_typed_slot(SlotKind)`. All 49 existing `wrap_slot` call sites
transitioned automatically; `unwrap_slot` recognises both the typed
and legacy forms during the migration window. 26 raw
`SimpleStatement { element_name: "left" | ... }` construction sites
in lowering still emit the legacy form — they render identically
to the typed shape so the snapshots are unaffected, but they're
the remaining sweep for Pass 1B.

**Pass 2: typed keyword statements.**
Introduce `Assert`, `Raise`, `Delete`, `Global`, `Nonlocal`,
`Yield`, `Throw` variants. Each carries a `value: Option<…>` or
`targets: Vec<…>` per its semantics. About 20 new variants
covering ~100 SimpleStatement uses.

**Pass 3: dedicated `Op` variant.**
`Op { text: String, marker: &'static str, range, span }` for
Binary's `<op>+<plus/></op>` slot.

**Pass 4: typed clause containers.**
`Extends`, `Implements`, `Where`, `Throws` variants for class /
function clause lists.

**Pass 5: retire `FieldWrap`.**
Each FieldWrap.wrapper value becomes a typed variant (`Type`,
`Returns`, `Value`, etc.). Many of these already exist or have
clear mappings.

**Pass 6: retire `Atom`.**
T-SQL's per-role classification (`<var>`, `<schema>`, `<alias>`,
`<name>`) becomes typed variants in the SqlTree, not SyntaxTree.

## After retirement

The codegen rule for `element_name_of` simplifies to:
- If variant is `Inline` / `Skip`: None.
- Otherwise: snake_case of variant tag.

Per-language overrides (C9) handle native vocabulary differences.
No more field-value-as-name rules. The variant identity IS the
output type, mechanically.
