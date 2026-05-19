---
priority: 2
type: note
---

# Users

<!-- TODO: develop into proper personas with goals/frustrations. -->

## Usage Context

Tractor is designed primarily for teams working in **moderately
large, existing codebases**. These teams have already accumulated
local conventions, lessons from incidents, and maintainability
preferences that are worth applying consistently. The codebase
itself is the reference material rules are written against.

**We optimize for:**

- Repeated patterns that appear many times across an existing
  codebase — the same convention, applied consistently.
- Teams with accumulated lessons (post-incident reviews, recurring
  review feedback, architectural decisions) that need to be
  enforced going forward.
- Rules grounded in real code — written against a real codebase,
  validated against real matches, refined until they capture the
  intended pattern.

**We do not optimize for:**

- **Small codebases** where rules can't be validated against enough
  examples — without a body of code to test against, "does this
  rule actually catch what we want?" is a guess.
- **Speculative rules** written without grounding in actual code.
  Rules earn their keep by matching real instances; rules invented
  in the abstract tend not to.
- **"Universal" rules** meant to apply to any project regardless of
  context. Universal-style rules are the domain of language linters;
  Tractor is for the team-specific conventions a linter can't ship
  out of the box.
- **Complex one-off statement shapes unlikely to recur.** The tree
  shape pays a verbosity cost on deeply nested expressions to keep
  repeated patterns stable; that trade only pays off when the rule
  catches many instances.

Stylistic concerns are **not out of scope** per se — if a concern is
structural and visible in a parse tree, it's expressible as a
Tractor rule. But Tractor isn't trying to compete with formatters
(Prettier, .editorconfig) or replace language linters (ESLint,
Pylint). It fills the gap for team-specific discoveries those tools
don't ship.

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
