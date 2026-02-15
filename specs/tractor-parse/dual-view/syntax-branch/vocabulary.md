---
title: Unified Syntax Vocabulary
priority: 1
---

JSON and YAML syntax branches share a common element vocabulary so the same
XPath queries work across both formats.

### Structural elements

| Element      | Meaning                         |
|--------------|---------------------------------|
| `object`     | Map/dict/mapping                |
| `array`      | Sequence/list                   |
| `property`   | Key-value pair                  |
| `key`        | The key within a property       |
| `value`      | The value within a property     |
| `item`       | Array/sequence element wrapper  |
| `document`   | Document boundary (multi-doc)   |

### Scalar elements

| Element  | Meaning               |
|----------|-----------------------|
| `string` | String value          |
| `number` | Numeric value         |
| `bool`   | Boolean (`true`/`false`) |
| `null`   | Null value            |

### Example

```json
{"name": "John", "age": 30}
```

```xml
<syntax>
  <object>
    <property>
      <key><string>name</string></key>
      <value><string>John</string></value>
    </property>
    <property>
      <key><string>age</string></key>
      <value><number>30</number></value>
    </property>
  </object>
</syntax>
```
