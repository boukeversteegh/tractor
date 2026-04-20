# Field-as-role-element for all languages

Replaced the per-language `classify_identifier` heuristic (default
`identifier → type`) with the simpler rule: tree-sitter already distinguishes
type positions via `type_identifier`, `primitive_type`, `predefined_type`, etc.,
so bare `identifier` → `name` with no context inspection.

Done:
- [x] Rust (`rust_lang.rs`)
- [x] Python (`python.rs`)
- [x] Java (`java.rs`)
- [x] Go (`go.rs`)
- [x] TypeScript/JavaScript (`typescript.rs`) — kept call/member field promotion
      for `<function>`/`<object>`/`<property>` wrappers with `<ref/>` markers

Not migrated:
- C# (`csharp.rs`) — uses `name`/`type`/`ref` trio with namespace-aware logic;
  not broken, but diverges from the spec's `<name>`-for-references convention.
- T-SQL (`tsql.rs`) — uses its own `transform_identifier` with categories
  specific to SQL; out of scope for the identifier-to-type fix.

JS/TS keeps `promote_field_to_wrapper` for call/member expressions so call
targets and member chains render as `<function><ref/>x</function>` etc. Other
languages do not currently promote fields beyond the builder's defaults.
