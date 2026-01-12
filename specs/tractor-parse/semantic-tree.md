---
title: Semantic Tree Transform
---

Transforms TreeSitter's raw syntax tree into a semantic XML structure optimized
for intuitive XPath queries.

The raw TreeSitter output is verbose and syntactic - identifiers are child nodes,
modifiers are separate elements, and element names are long snake_case strings.
This transformation creates a tree that mirrors how developers think about code:

- Find a class named Foo: `//class[name='Foo']`
- Find public methods: `//method[public]`
- Find extension methods: `//method[params/param[this]]`
- Find async functions: `//def[async]` (Python) or `//method[async]` (C#)

The goal is zero mental translation - the XPath query reads like the question
you're asking about the code.
