# Field-as-role-element for all languages

JS/TS now promotes tree-sitter field names (`function`, `object`, `property`) to
wrapper elements in call/member expressions, replacing the global `identifier` →
`type` rename with a `<ref/>` marker for simple references.

Apply the same pattern to other languages:

- [ ] C# (`csharp.rs`) — `classify_identifier` returns `"type"`, `"name"`, or `"ref"`
- [ ] Java (`java.rs`) — same pattern
- [ ] Python (`python.rs`) — same pattern
- [ ] Go (`go.rs`) — same pattern
- [ ] Rust (`rust_lang.rs`) — same pattern
- [ ] T-SQL (`tsql.rs`) — uses `transform_identifier` with different categories

Each language should:
1. Identify which field names to promote (language-specific — may differ from JS/TS)
2. Call `promote_field_to_wrapper` on appropriate parent nodes
3. Add `inline_identifier_with_ref` handler for the new wrapper elements
4. Stop renaming identifiers to `<type>` in non-type contexts
5. Update integration tests and snapshots

The shared helper `promote_field_to_wrapper` in `xot_transform::helpers` is ready.
