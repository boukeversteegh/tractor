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
  <public startLine="2" startCol="3" endLine="2" endCol="9"/>
  <static startLine="2" startCol="10" endLine="2" endCol="16"/>
</method>
```

### Exhaustive markers

When modifiers represent mutually exclusive variations, always include one
marker from the set — never use absence as a default. See
[design principle #9](../design.md#9-exhaustive-markers-for-mutually-exclusive-variations).

```xml
<!-- Declaration kind: always one of const, let, var -->
<variable><const startLine="1" startCol="1" endLine="1" endCol="6"/><name>x</name></variable>
<variable><let startLine="2" startCol="1" endLine="2" endCol="4"/><name>y</name></variable>

<!-- Parameter: always one of required, optional -->
<param><required/><name>id</name><typeof>string</typeof></param>
<param><optional startLine="1" startCol="30" endLine="1" endCol="31"/><name>limit</name><typeof>number</typeof></param>

<!-- Access: always one of public, private, protected, internal -->
<method><public startLine="5" startCol="3" endLine="5" endCol="9"/><name>Foo</name></method>
<method><internal startLine="8" startCol="3" endLine="8" endCol="11"/><name>Bar</name></method>
```
