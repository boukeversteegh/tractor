---
title: Lift Modifiers
priority: 1
---

When a node has `<modifier>text</modifier>` children, convert to empty elements.
If the marker corresponds to a keyword in the source, it carries the keyword's
source location:

```xml
<!-- Before -->
<method_declaration>
  <modifier>public</modifier>
  <modifier>static</modifier>
</method_declaration>

<!-- After -->
<method>
  <public line="2" column="3" end_line="2" end_column="9"/>
  <static line="2" column="10" end_line="2" end_column="16"/>
</method>
```

### Exhaustive markers

When modifiers represent mutually exclusive variations, always include one
marker from the set — never use absence as a default. See
[design principle #9](../design.md#9-exhaustive-markers-for-mutually-exclusive-variations).

```xml
<!-- Declaration kind: always one of const, let, var -->
<variable><const line="1" column="1" end_line="1" end_column="6"/><name>x</name></variable>
<variable><let line="2" column="1" end_line="2" end_column="4"/><name>y</name></variable>

<!-- Parameter: always one of required, optional -->
<param><required/><name>id</name><typeof>string</typeof></param>
<param><optional line="1" column="30" end_line="1" end_column="31"/><name>limit</name><typeof>number</typeof></param>

<!-- Access: always one of public, private, protected, internal -->
<method><public line="5" column="3" end_line="5" end_column="9"/><name>Foo</name></method>
<method><internal line="8" column="3" end_line="8" end_column="11"/><name>Bar</name></method>
```
