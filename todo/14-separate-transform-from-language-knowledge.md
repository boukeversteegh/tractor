# Separate transform logic from language knowledge

## Background

`languages/csharp.rs` currently contains both:
1. **Language knowledge** — semantic vocabulary (`pub mod semantic`), modifier lists, name mappings
2. **Transform implementation** — `transform_node()` and all its helpers (comment grouping, modifier detection, identifier classification, etc.)

These are conceptually separate concerns. The language knowledge is shared between the transform and the renderer, while the transform implementation is specific to the parse pipeline.

## Current structure

```
tractor-core/src/languages/csharp.rs   (~850 lines, everything in one file)
  pub mod semantic { ... }              — vocabulary consts
  pub const ACCESS_MODIFIERS            — modifier lists
  pub const OTHER_MODIFIERS
  pub fn transform_node()              — transform entry point
  fn is_trailing_comment()             — transform helper
  fn group_line_comments()             — transform helper
  fn classify_identifier()             — transform helper
  fn map_element_name()                — maps raw → semantic names
  fn default_access_modifier()         — transform helper
  ... etc
  pub fn syntax_category()             — highlighting (separate concern too)
```

## Proposed structure

```
tractor-core/src/languages/csharp/
  mod.rs                               — re-exports, language knowledge
    pub mod semantic { ... }           — vocabulary consts
    pub const ACCESS_MODIFIERS
    pub const OTHER_MODIFIERS
    fn map_element_name()              — raw → semantic name mapping
  transform.rs                         — transform implementation
    pub fn transform_node()
    fn is_trailing_comment()
    fn group_line_comments()
    fn classify_identifier()
    fn default_access_modifier()
    ... etc
  highlight.rs                         — syntax highlighting
    pub fn syntax_category()
```

## Benefits

- Clear separation: language knowledge vs transform implementation vs highlighting
- Renderer only depends on `languages::csharp::semantic` (already the case, just cleaner)
- Transform logic can be tested independently
- Other languages follow the same pattern as they grow

## Considerations

- Transform helpers reference vocabulary consts and modifier lists — they'd import from parent `mod.rs`
- `map_element_name` is borderline: it maps raw → semantic, used only by transform. Could live in either place. Since it references semantic consts, keeping it in `mod.rs` makes sense.
- Some functions like `is_named_declaration` are used by both transform and identifier classification — shared private helpers need a home

## Priority

Low. The file is ~850 lines and manageable. Do this when adding a second language's transform or when the file grows beyond comfort.
