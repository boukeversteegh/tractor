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
  <public start="2:3" end="2:9"/>
  <static start="2:10" end="2:16"/>
</method>
```

### Exhaustive markers

When modifiers represent mutually exclusive variations, always include one
marker from the set — never use absence as a default. See
[design principle #9](../design.md#9-exhaustive-markers-for-mutually-exclusive-variations).

```xml
<!-- Declaration kind: always one of const, let, var -->
<variable><const start="1:1" end="1:6"/><name>x</name></variable>
<variable><let start="2:1" end="2:4"/><name>y</name></variable>

<!-- Parameter: always one of required, optional -->
<param><required/><name>id</name><typeof>string</typeof></param>
<param><optional start="1:30" end="1:31"/><name>limit</name><typeof>number</typeof></param>

<!-- Access: always one of public, private, protected, internal -->
<method><public start="5:3" end="5:9"/><name>Foo</name></method>
<method><internal start="8:3" end="8:11"/><name>Bar</name></method>
```
