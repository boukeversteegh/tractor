---
priority: 2
type: note
---

# Users

<!-- TODO: develop into proper personas with goals/frustrations. -->

## Primary: Team Leads & Project Maintainers

People who set technical standards for their teams and contributors.

- Decide on patterns and conventions
- Want to keep conventions consistent as the codebase grows
- Spend review time on things that could be caught automatically
- Want confidence that agreed patterns are actually followed
- Want to safeguard against high-stakes bugs (e.g., missing
  Authorization attributes on controllers)
- Want to maintain architectural and design specifications

## Secondary: Convention-Minded Developers

Any developer with an interest in documenting and enforcing code
conventions. Think: users of tools like Prettier, .editorconfig,
eslint.

- Care about code consistency
- Want to contribute to team standards
- Value clear documentation of "how we do things here"

## Context: The Problems They Face

1. **Convention drift.** Teams agree on patterns, but implementations
   naturally diverge over time. When looking at existing code for
   guidance, it's not always clear which example reflects the current
   convention.

2. **Bug class prevention.** Post-incident reviews sometimes reveal
   errors that could be caught structurally (forgotten checks, library
   misuse). For high-stakes patterns, automated detection is more
   reliable than relying on review alone.

3. **Architectural consistency.** Design specifications exist but are
   easy to deviate from, especially when the rationale isn't obvious
   from a single file. Automated feedback helps teams stay aligned
   without slowing down reviews.

4. **Knowledge continuity.** Knowledge lives in people's heads,
   scattered across wiki pages and old PR comments. As teams evolve,
   lessons can fade - and get re-learned the hard way.
