# C# XML Tree Improvements Todo

Status as of the compaction point.

## Completed

- [x] Rename `accessor_list` → `accessors`
- [x] Rename `accessor_declaration` → `accessor`
- [x] Use full element names (no abbreviations)
  - `prop` → `property`
  - `attr` → `attribute`
  - `attrs` → `attributes`
  - `ctor` → `constructor`
  - `param` → `parameter`
  - `params` → `parameters`
  - `arg` → `argument`
  - `args` → `arguments`
  - `decl` → `declarator`
  - `var` → `variable` (for variable_declaration, not the keyword)
- [x] Unify attribute arguments with function call arguments
  - `attribute_argument_list` → `arguments`
  - `attribute_argument` → `argument`

## In Progress

- [ ] Restructure generic types for unified `//type` queries
  - Current: `<generic field="type"><type>List</type><type_argument_list>...</type_argument_list></generic>`
  - Target: `<type><generic/>List<arguments><type>string</type></arguments></type>`
  - Blocked by: XPath string-value whitespace issue (see analysis-xpath-string-value.md)

## Pending

### Priority 1: Type System Consistency
- [ ] Rename `type_argument_list` → `arguments` (unified with function calls)
- [ ] Wrap generic types in `<type>` with `<generic/>` marker
- [ ] Wrap array types in `<type>` with `<array/>` marker
- [ ] Enable `//type[.='Dictionary<string,int>']` style queries (requires whitespace fix)

### Priority 2: Clean Up Redundant Wrappers
- [ ] Flatten or remove redundant `arguments` wrapper around `args`
  - Current: `<call><ref>Foo</ref><arguments><args>(...)</args></arguments></call>`
  - Investigate if we can simplify

### Priority 3: Other Renames
- [ ] Rename `expression_statement` → something shorter or flatten
- [ ] Simplify `qualified_name` in namespaces
- [ ] Convert `implicit_type` → appropriate element (maybe just `<var/>` keyword?)
- [ ] Rename `implicit_parameter` → `parameter`

### Priority 4: String Interpolation (lower priority)
- [ ] Simplify `interpolated_string_expression` → `interpolated` or similar
- [ ] Simplify `string_content` → `text`
- [ ] Simplify/flatten `interpolation` elements
- [ ] Remove `interpolation_start`, `interpolation_brace` noise

### Future Investigation
- [ ] Revisit complex accessors (with bodies, using `value` keyword)
- [ ] Consider how init-only setters should be represented

## Design Documents Updated

- `specs/tractor-parse/semantic-tree/design.md` - Design goals and guiding principles
- `specs/tractor-parse/semantic-tree/element-naming.md` - Full names, no abbreviations
- `specs/tractor-parse/semantic-tree/type-element.md` - Unified type structure spec

## Blocking Issue

The XPath string-value whitespace problem (documented in `analysis-xpath-string-value.md`)
blocks testing of the generic type restructure. The transform can be implemented, but
queries like `//type[.='List<string>']` won't work until whitespace is fixed.
