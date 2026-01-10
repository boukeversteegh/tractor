---
title: Node-Specific Attributes
priority: 1
---

Add semantic attributes based on syntax node type:
- name: identifier text for declarations, types, parameters, variables
- modifiers: space-separated list of modifiers (public, static, etc.)
- this="true": for extension method parameters with 'this' modifier
- textValue: parsed string content for string literals (use with XPath 2.0 functions like `ends-with(@textValue, ';')`)
