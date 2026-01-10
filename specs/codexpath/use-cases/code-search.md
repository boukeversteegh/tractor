---
title: Code Search
priority: 1
---

Find specific code patterns using XPath queries.

Examples:
- Find all method names: //MethodDecl/@name
- Find TODO comments: //Trivia[contains(text(), 'TODO')]
- Find extension methods: //ParameterSyntax[@this='true']
