# Use Case: C# to TypeScript Code Generation with Tractor

This document describes using tractor to generate TypeScript interfaces and builder classes from C# integration test code. The goal is to keep e2e test types in sync with backend models automatically.

## Context

We have a C# integration test framework with:
- **Models** (`Models/**/*.cs`): Data classes representing API entities (ActivityModel, SessionModel, etc.)
- **Builders** (`Builders/**/*.cs`): Fluent builder classes that create domain objects via API calls

Our Cypress e2e tests need TypeScript types matching these C# models. Manually maintaining them is error-prone and drifts over time.

## Approach

### Phase 1: Tractor extracts structured XML from C# source
### Phase 2: A Node.js script parses the XML and generates TypeScript

The script uses tractor twice:
1. Parse C# source files into semantic XML (`tractor "Models/**/*.cs" -x "//class" -o xml`)
2. (Optionally) Re-parse the XML output for further extraction (`tractor --lang xml -x "..." -o value`)

## What We Extract

### Models (C# classes to TypeScript interfaces)

**C# Input:**
```csharp
public class ActivityModel : IEntityModel, IUpsertable
{
    public Guid? Id { get; set; }
    public string? ExternalReference { get; set; }
    public string Name { get; set; } = "default";
    public bool IsConcept { get; set; }
    public int? Capacity { get; set; }
    public List<SessionModel> Sessions { get; set; } = [];
    public ICollection<ActivityLocalizationModel>? Localizations { get; set; }
    public Dictionary<Locale, string> Labels { get; set; } = new();
    public ActivityTypeModel ActivityType { get; set; } = new();
}
```

**Desired TypeScript Output:**
```typescript
export interface ActivityModel {
  id: string | null;           // Guid? -> string | null
  externalReference: string | null;
  name: string;
  isConcept: boolean;          // bool -> boolean
  capacity: number | null;     // int? -> number | null
  sessions: SessionModel[];    // List<T> -> T[]
  localizations: ActivityLocalizationModel[] | null;  // ICollection<T>? -> T[] | null
  labels: Record<string, string>;  // Dictionary<K,V> -> Record<K,V>
  activityType: ActivityTypeModel;
}
```

**Tractor XML for a property (simple type):**
```xml
<property>
  <public/>
  <type>
    Guid
    <nullable/>
  </type>
  <name>Id</name>
  <accessors>{ <accessor>get;</accessor> <accessor>set;</accessor> }</accessors>
</property>
```

**Tractor XML for a property (generic type):**
```xml
<property>
  <public/>
  <type>
    <generic/>
    List
    <arguments>
      &lt;
      <type>SessionModel</type>
      &gt;
    </arguments>
  </type>
  <name>Sessions</name>
  ...
</property>
```

**Tractor XML for a property (nullable generic - the tricky case):**
```xml
<!-- ICollection<ActivityLocalizationModel>? becomes a wrapper type -->
<property>
  <public/>
  <type>
    <type>
      <generic/>
      ICollection
      <arguments>&lt;<type>ActivityLocalizationModel</type>&gt;</arguments>
    </type>
    ?
  </type>
  <name>Localizations</name>
  ...
</property>
```

The nullable generic wraps the inner type in an outer `<type>` element, with `?` as a text node. This makes queries more complex because you need to check both `type//nullable` AND `type/type//generic` patterns.

### Enums (C# enums to TypeScript union types)

**C# Input:**
```csharp
public enum UpsertBy { Both, Id, ExternalReference }
```

**TypeScript Output:**
```typescript
export type UpsertBy = "Both" | "Id" | "ExternalReference";
```

Enum member names sit inside `<name><ref>Both</ref></name>`, so you need to recurse into `<ref>` nodes to get the text.

### Records (C# records to TypeScript interfaces)

Records are nested inside classes and use constructor-style parameters instead of properties:

```csharp
public class FormQuestionOnPage
{
    public record CustomWording(string Label);
    public record AnswerOptionOverride(bool IsHidden, Dictionary<Locale, string> CustomLabels);
}
```

**Tractor XML for a record:**
```xml
<record>
  <public/>
  record
  <name>AnswerOptionOverride</name>
  <parameters>
    (
    <parameter>
      <type>bool</type>
      <name>IsHidden</name>
    </parameter>
    ,
    <parameter>
      <type><generic/>Dictionary<arguments>&lt;<type>Locale</type>,<type>string</type>&gt;</arguments></type>
      <name>CustomLabels</name>
    </parameter>
    )
  </parameters>
  ;
</record>
```

### Builders (C# fluent builders to TypeScript classes)

**C# Input:**
```csharp
public class ActivityBuilder : Builder(serviceProvider), IModelBuilder<ActivityModel, ActivityBuilder>
{
    public ActivityModel Model { get; set; } = new();

    // Fluent setters - set properties on Model
    public ActivityBuilder WithCapacity(int capacity)
    {
        Model.Capacity = capacity;
        return this;
    }

    public ActivityBuilder Concept()
    {
        Model.IsConcept = true;
        return this;
    }

    // Relationship methods - create child builders (Action<T> parameter)
    public ActivityBuilder Session(Action<SessionBuilder> action)
    {
        AddBuilder<SessionBuilder>().ForActivity(Model).Apply(action);
        return this;
    }
}
```

**Desired TypeScript Output:**
```typescript
export class ActivityBuilder {
  model: ActivityModel = {} as ActivityModel;

  withCapacity(capacity: number): this { this.model.capacity = capacity; return this; }
  concept(): this { this.model.isConcept = true; return this; }
  // session() skipped - has Action<T> parameter (relationship, not setter)
}
```

We extract:
- The class name and model type (from `IModelBuilder<TModel, TBuilder>` in `base_list`)
- Public non-override methods that return the builder type (fluent methods)
- Skip methods with `Action<T>` parameters (relationship builders, not setters)
- For each method, extract `Model.X = value` assignments from the body

## Type Resolution Challenge

Types referenced in models/builders come from three sources:

1. **Our own models** (e.g., `SessionModel`) - defined in `Models/**/*.cs`
2. **Generated API client types** (e.g., `Locale`, `SessionMode`, `MailingType`) - defined in frontend TS client files
3. **Internal utility types** (e.g., `Box<T>`) - defined elsewhere in the test project

We use tractor to index the frontend client files too:
```bash
tractor "frontend/.../eduConfigurationServiceClient.ts" \
  -x "//class/name | //enum/name | //interface/name" -o json
```
This gives us a lookup table of `typeName -> file` to auto-generate imports.

## Two-Pass Tractor Pipeline

Tractor can parse its own XML output, enabling a powerful two-pass pipeline: C# -> XML -> tractor again with `--lang xml` to extract structured data:

```bash
tractor "Models/**/*.cs" -x "//class" -o xml | \
  tractor --lang xml -x "//property/concat(...)" -o value
```

### Simple extraction with concat

```bash
tractor "Models/ActivityModel.cs" -x "//class[name='ActivityModel']" -o xml | \
  tractor --lang xml -x "$(cat <<'XPATH'
//property/concat(
  lower-case(substring(name, 1, 1)), substring(name, 2), ': ',
  normalize-space(type)
)
XPATH
)" -o value
```

Output:
```
id: Guid
name: string
locations: List < ActivityLocationModel >
isConcept: bool
```

The `normalize-space(type)` works but produces `List < ActivityLocationModel >` with spaces around angle brackets due to pretty-print whitespace collapsing (see challenge #3 below).

### Full structured extraction with `for` + nested `let`

Single `let` works. Multiple `let` bindings require nesting (`let ... return let ... return`). The `let` keyword cannot be used directly inside a path step (`//property/let $x := ...` fails), but works with `for`:

```bash
tractor "Models/ActivityModel.cs" -x "//class[name='ActivityModel']" -o xml | \
  tractor --lang xml -x "$(cat <<'XPATH'
for $p in //property
return
  let $name := concat(lower-case(substring($p/name, 1, 1)), substring($p/name, 2))
  return
  let $raw := normalize-space($p/type)
  return
  let $nullable := exists($p/type//nullable) or contains($raw, '?')
  return
  let $container := normalize-space(string-join(($p/type/text(), $p/type/type/text()), ''))
  return
  let $args := string-join(
    for $t in ($p/type/arguments/type, $p/type/type/arguments/type) return normalize-space($t), ','
  )
  return concat($name, '|', $nullable, '|', $container, '|', $args)
XPATH
)" -o value
```

Output:
```
id|true|Guid|
externalReference|true|string|
upsertBy|false|UpsertBy|
name|false|string|
activityType|false|ActivityTypeModel|
locations|false|List|ActivityLocationModel
studyPrograms|false|List|StudyProgramModel
localizations|true|? ICollection|ActivityLocalizationModel
isConcept|false|bool|
capacity|true|int|
notifications|true|ActivityNotificationConfigurationModel|
```

This cleanly separates name, nullable, container type, and generic arguments. A tiny Node script can then do the final type mapping (`Guid->string`, `bool->boolean`, `List->[]`, etc.).

### `let` behavior notes (tractor 0.1.0)

| Pattern | Works? | Notes |
|---------|--------|-------|
| `let $x := "a" return $x` | Yes | Single let at top level |
| `let $x := "a" let $y := "b" return ...` | No | Multi-let needs nesting |
| `let $x := "a" return let $y := "b" return concat($x,$y)` | Yes | Nested let works |
| `//property/let $x := name return $x` | No | Let inside path step fails |
| `for $p in //property return let $x := $p/name return $x` | Yes | Use for + let instead |

## Challenges and Feature Requests

### 1. `let` in path steps and multi-let syntax

**Status**: `let` was recently fixed and works at top level. However:

- **Multi-let** requires nested `let ... return let ... return` syntax. XPath 3.1 spec allows `let $a := 1, $b := 2 return ...` — this comma-separated form doesn't work yet.
- **`let` inside path steps** (`//property/let $name := name return ...`) fails with a parse error. The workaround is `for $p in //property return let $name := $p/name return ...`, which works but is more verbose.

Supporting multi-let with commas and let-in-path-steps would make complex extractions much more readable.

### 2. Structured JSON output (`-o json` with tree structure)

**Problem**: `-o json` currently returns `{file, line, column, value}` where `value` is flattened source text. To do structural analysis (is this a generic type? what are its type arguments?) we need the tree.

**Current workaround**: Either parse the XML with a library (fast-xml-parser), or use the two-pass pipeline to extract individual fields.

**Desired**: A JSON output mode that preserves the XML tree structure:
```json
{
  "property": {
    "name": "Locations",
    "type": {
      "generic": true,
      "text": "List",
      "arguments": [{"type": {"text": "ActivityLocationModel"}}]
    }
  }
}
```

This would eliminate the need for any XML parser in the consuming script.

### 2. Nullable wrapper type is hard to query

**Problem**: `ICollection<T>?` produces a double-wrapped type:
```xml
<type>
  <type><generic/>ICollection<arguments>...</arguments></type>
  ?
</type>
```

To get the container name, you need to check BOTH `type/text()` AND `type/type/text()`. To get type arguments: both `type/arguments/type` AND `type/type/arguments/type`. Every query doubles in complexity.

**Suggestion**: Represent nullable as an attribute or consistent child element rather than a wrapper layer:
```xml
<!-- Option A: attribute -->
<type nullable="true">
  <generic/>ICollection<arguments>...</arguments>
</type>

<!-- Option B: nullable child at same level (current style for simple types) -->
<type>
  <generic/>ICollection
  <arguments>...</arguments>
  <nullable/>
</type>
```

Both options keep the nullable info without requiring callers to navigate an extra tree level.

### 3. `normalize-space(type)` includes formatting artifacts

**Problem**: `normalize-space(type)` on a generic type gives `List < ActivityLocationModel >` with spaces around angle brackets. These come from the pretty-printed XML indentation being collapsed.

**Desired**: A way to get the original source text of a node, without pretty-print whitespace. Either:
- A function like `source-text()` that returns the original source
- Or `-o value` already does this for matched nodes, but it's not available inside XPath `concat()` expressions

### 4. Multi-file context loss in piped mode

**Problem**: When piping output through a second tractor invocation:
```bash
tractor "Models/**/*.cs" -x "//class" -o xml | tractor --lang xml -x "//property/name" -o json
```
All results show `"file": "<stdin>"`. The original file path is lost. This makes it impossible to group results by source file.

**Suggestion**: When outputting XML, optionally preserve the file path as an attribute on root elements:
```xml
<class _file="Models/ActivityModel.cs" _line="5">
  ...
</class>
```
This would survive the pipe and be queryable in the second pass.

### 5. Per-property output within a class context

**Problem**: When extracting properties, you lose the class context. If you query `//property`, you get all properties from all classes mixed together. You can query `//class` and then pipe to get properties, but then you lose which class each property belongs to.

**Desired**: Ability to group or nest output. For example:
```bash
tractor "Models/**/*.cs" \
  -x "//class" \
  -o json \
  --project "name, body/property/(name, type)"
```

Or the structured JSON from request #1 would solve this naturally.

### 6. `string-join()` with `normalize-space()` on sequences

**Problem**: `string-join(type/arguments/type/normalize-space(), ',')` fails with a type error. The workaround is the more verbose `for $t in ... return normalize-space($t)` form.

This might be an XPath spec thing, but it was a stumbling block.

### 7. String interpolation / template strings

**Problem**: Building output strings with `concat()` is extremely verbose. A typical extraction looks like:

```xpath
concat($name, '|', $nullable, '|', $container, '|', $args)
```

But for JSON output it becomes unreadable:
```xpath
concat('{"name":"', $name, '","type":"', $type, '","nullable":', $nullable, '}')
```

**Desired**: Some form of string interpolation, e.g.:
```xpath
`{"name": "{$name}", "type": "{$type}", "nullable": {$nullable}}`
```

Or even just a shorter alias for `concat()`. This is the single most common operation in codegen-style queries and the verbosity adds up fast.

### 8. Case conversion functions

**Problem**: Converting PascalCase to camelCase requires:
```xpath
concat(lower-case(substring($name, 1, 1)), substring($name, 2))
```

This is needed for virtually every property name in a C#-to-TypeScript conversion and it clutters every query.

**Desired**: Built-in case conversion functions:
```xpath
camel-case($name)    (: PascalCase -> camelCase :)
pascal-case($name)   (: camelCase -> PascalCase :)
kebab-case($name)    (: PascalCase -> pascal-case :)
snake-case($name)    (: PascalCase -> pascal_case :)
```

These are universally useful for any cross-language codegen scenario (C# to TypeScript, Rust to Python, etc.).

## Results

The current approach (tractor + Node.js with fast-xml-parser) successfully generates:
- 190 TypeScript interfaces from C# classes and records
- 7 enum union types
- 64 builder classes with fluent setter methods and Model assignments
- Auto-resolved imports from 2 frontend client files (563 types indexed)

Remaining issues (23 TypeScript errors):
- 6 duplicate method names from C# overloads (TypeScript doesn't support same-name overloads this way)
- 8 `Record<Locale, ...>` where Locale is a class, not a string (invalid Record key constraint)
- 5 types that exist in neither our models nor the frontend clients (external/utility types)

## Priority Summary

| # | Feature | Impact | Notes |
|---|---------|--------|-------|
| 1 | Multi-let + let-in-path | High | Makes complex queries readable; nested let workaround exists |
| 2 | Structured JSON output | High | Eliminates XML parser dependency entirely |
| 3 | Nullable wrapper normalization | High | Every type query doubles in complexity without this |
| 4 | normalize-space source text | Medium | `List < T >` vs `List<T>` |
| 5 | File context in piped mode | Medium | Needed for multi-file codegen pipelines |
| 6 | Per-class property grouping | Medium | Natural with structured JSON (#2) |
| 7 | String interpolation | Medium | `concat()` verbosity dominates codegen queries |
| 8 | Case conversion functions | Medium | camelCase conversion needed on every property |

## Ideal Tractor-Native Pipeline

With structured JSON output (#2), the nullable fix (#3), and case conversion (#8), the entire pipeline could become:

```bash
tractor "Models/**/*.cs" -x "//class" -o structured-json | node generate-ts.mjs
```

Where `generate-ts.mjs` would be ~50 lines of pure type mapping logic, with zero XML parsing dependency.

Alternatively, with string interpolation (#7), tractor could do nearly everything itself:

```bash
tractor "Models/**/*.cs" -x "//class" -o xml | \
  tractor --lang xml -x "$(cat <<'XPATH'
for $p in //property
return
  let $name := camel-case($p/name)
  return
  let $type := normalize-space($p/type)
  return `  {$name}: {$type};`
XPATH
)" -o value
```
