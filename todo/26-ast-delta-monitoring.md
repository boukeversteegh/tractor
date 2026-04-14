# AST delta monitoring — learn lint rules from code edits

## Context

We already support AI guardrailing by running manually-written rules
against AI-generated code (see `UseCases.tsx#ai-guard-railing`). That
workflow assumes someone writes the rules upfront. This idea flips the
direction: observe what humans actually fix, extract the pattern, and
generate rules automatically.

## Idea

When developers edit code — during a Claude Code hook, in CI, in a
pre-commit hook, or any other monitored moment — we can diff the
before/after ASTs to extract **AST deltas**: small, localized structural
changes that represent an improvement.

Each AST delta is a candidate for automatic conversion into a Tractor
lint rule. For example:

- A developer renames `data` → `user_response` after an AI generates code
  → suggests a "no generic variable names in API handlers" rule
- A developer adds a null check around a database call
  → suggests a "database calls must handle null" rule
- A developer moves an import from inside a function to module level
  → suggests a "no function-scoped imports" rule

Over time, this builds a corpus of team-specific conventions learned
from real corrections, not guessed from style guides.

## Use case: AI guardrailing feedback loop

This is an extension of the AI guardrailing use case:

1. AI generates code
2. Human reviews and makes corrections
3. Tractor observes the AST delta (before → after)
4. Delta is converted into a candidate lint rule
5. Rule is reviewed/approved by the team
6. Next time AI generates code, the rule catches the same mistake automatically

This closes the loop — instead of guardrails being a one-time manual
effort, they **evolve from the team's actual editing behavior**.

## Integration points

- **Claude Code hooks** — `PostToolUse` or custom hooks can capture
  file state before/after edits and send deltas to Tractor
- **Pre-commit hooks** — diff staged changes against HEAD to extract
  deltas at commit time
- **CI pipelines** — compare PR branch against base to find recurring
  fix patterns across PRs
- **Editor plugins** — capture real-time edits for immediate delta
  extraction

## Open questions

- How to distinguish meaningful patterns from one-off edits?
  (frequency threshold, similarity clustering)
- What's the right granularity for AST deltas? (statement-level,
  expression-level, block-level)
- How to present candidate rules for human review/approval?
- Should rules be generated as Tractor rule YAML, or as natural
  language descriptions that get refined?
- How to handle language-specific vs cross-language patterns?

## Related

- `web/src/pages/docs/UseCases.tsx` — AI Guard Railing section
- `specs/codexpath/use-cases/ast-inspection.md` — AST inspection
- `specs/codexpath/use-cases/refactoring-support.md` — pattern detection
- `specs/mission.md` — "Write a rule once. Enforce it everywhere."
- todo/21 — unified pipeline architecture
