---
name: analyze
description: Deep technical analysis of a GitHub issue for complexity assessment and implementation planning. Traces code paths, assesses design impact, finds related work. Use when the user wants to understand an issue before working on it.
allowed-tools: Agent, Read, Write, Edit, Glob, Grep, AskUserQuestion, Bash(gh issue view *), Bash(gh issue list *), Bash(gh pr list *), Bash(gh api graphql *), Bash(tractor *), Bash(task install)
argument-hint: "<issue-number> [or GitHub URL]"
---

# Analyze GitHub Issue

Perform a deep technical analysis of a GitHub issue to assess its complexity, trace affected code paths, identify related work, and produce a detailed report that's useful both for prioritization and for later implementation.

## Input

`$ARGUMENTS` should be a GitHub issue number or URL. If missing, ask the user.

## Process

### Phase 1: Understand the issue

1. Fetch the issue: `gh issue view <number> --json title,body,labels,comments`
2. **Check for parent issue context**: Always check if the issue has a parent via GraphQL:
   ```bash
   gh api graphql -f query='{ repository(owner:"boukeversteegh", name:"tractor") { issue(number:NUMBER) { parent { number title body } } } }'
   ```
   - If a parent exists: read its body for the broader use case, context, and motivation. A sub-issue that looks trivial in isolation may be part of a larger effort.
   - Also check for sibling sub-issues via the parent's `subIssues` field — they may share context or should be tackled together.
   - If no parent exists but the body is very short, search for related issues: `gh issue list --state open --search "<keywords>" --json number,title`
3. Parse the issue body (and parent if found) for:
   - **Sub-issues**: checkbox items (`- [ ]` or `- [x]`) — each may need separate analysis
   - **Linked issues**: `#123` references
   - **Context**: what use case or workflow exposed the problem

### Phase 1.5: Reproduce with tractor

Before diving into code, try to reproduce the reported behavior using the actual `tractor` binary:

1. **Check version**: Run `tractor --version` to verify the installed binary is current. If it seems outdated, run `task install` to rebuild and install.
2. **Reproduce the bug**: Construct a minimal command that triggers the reported issue. Use sample files from `tests/integration/` or create a small inline example.
3. **Observe actual behavior**: Run the command and capture the output. This grounds the analysis in reality rather than speculation from reading code.
4. **Test edge cases**: Try variations — what happens with slightly different flags, different file types, different platforms?

Hands-on reproduction is more reliable than reading code alone. A bug you can trigger is a bug you understand.

### Phase 2: Deep code exploration

For each issue or sub-issue, trace the relevant code paths using direct tools (Grep, Glob, Read). Do not delegate to Agent subagents — use the tools directly so results stay in the main context and don't require per-command approval.

Key questions each exploration should answer:
- **What code path is affected?** Trace from CLI parsing through to the behavior in question. Report file paths and line numbers.
- **How localized is the fix?** Is it contained in one function/module, or does it cross module boundaries?
- **What are the current semantics?** Don't just find the bug — understand what the code currently does and why it might have been written that way.
- **Are there tests?** What's tested, what's not? Would the fix require new test infrastructure?

When there are multiple independent sub-issues, interleave exploration using parallel tool calls (e.g., Grep + Read for different sub-issues in the same message).

### Phase 3: Assess complexity

For each issue/sub-issue, determine the **size label** based on these criteria:

**size/S** — Small:
- Fix is localized to 1-2 functions or a single module
- No design decisions needed — the right approach is obvious
- Documentation-only changes
- Adding a missing error message or validation
- No impact on how tractor's factors compose

**size/M** — Medium:
- Fix crosses 2-3 modules or requires changes at multiple pipeline stages
- Requires a bounded design decision (finite, clear option space)
- Involves propagating a type change or new parameter through a chain
- May affect how two existing factors interact, but doesn't introduce new ones

**size/L** — Large:
- Requires rethinking how tractor's factors compose
- Introduces a new compositional dimension (e.g., format-specific defaults as a new axis)
- Requires a design document or spec before implementation
- Migration of existing code paths into a new architecture
- The "obvious" fix would be a bolt-on that violates tractor's compositional design

**Important nuance:** A feature that seems trivial to "bolt on" may be size/L if doing it properly requires isolating it as a factor that composes cleanly with everything else. Tractor is highly compositional — maintaining that property is the primary complexity driver. Conversely, writing complex code is usually NOT what makes something large — Claude handles that well. Complexity arises from design decisions with broad impact.

### Phase 4: Find related work

Search for related work across multiple sources, **in parallel**:

1. **Local todos**: `Grep` and `Glob` in `todo/` for related topics
2. **Design docs**: `Grep` in `docs/` for related architecture or design documents
3. **Specs**: `Grep` in `specs/` for related specifications
4. **Open GitHub issues**: `gh issue list --state open --limit 50 --json number,title,labels` — scan for related issues
5. **Open PRs**: `gh pr list --state open --json number,title,labels` — check for in-flight related work

For each related item found, note:
- What it is and how it relates
- Whether the current issue is blocked by it, enables it, or should be tackled together
- Whether an existing design doc already proposes a solution direction

### Phase 5: Write the report

Write a detailed analysis to `.issues/<number>-<slug>.md` where `<slug>` is a short hyphenated description derived from the issue title.

#### Report structure

```markdown
# Analysis: <issue title>

**Issue:** #<number>
**Date:** <today>
**Overall size:** size/<S|M|L>

## Summary

<2-3 sentences: what the issue is about and the overall complexity assessment>

## Sub-issue analysis

### <sub-issue title or description>

**Size:** size/<S|M|L>

#### Problem

<What's wrong, traced through the code. Include file:line references.>

#### Current behavior

<What the code currently does and why. Not just "it's broken" but the actual execution path.>

#### Solution direction

<How this could be fixed. If there are multiple approaches, list them with trade-offs.
If this requires design decisions, call that out explicitly.>

#### Key code locations

- `path/to/file.rs:123` — <what this location does>
- `path/to/other.rs:456` — <what this location does>

#### Tests

<What's currently tested, what would need new tests.>

---

<repeat for each sub-issue>

## Related work

| Type | Reference | Relationship |
|------|-----------|-------------|
| Todo | `todo/20-glob-path-resolution.md` | Directly related — addresses same file resolution issues |
| Issue | #42 | Blocked by — needs the new config format first |
| Doc | `docs/design-file-resolver.md` | Proposes solution direction for the file scope redesign |
| PR | #85 | In-flight — touches same code paths |

## Suggested groupings

<Which sub-issues or related items should be tackled together and why.
Note any ordering dependencies.>
```

### Phase 6: Print inline summary

After writing the file, print a concise summary to the conversation:
- File path written
- Size label per sub-issue (one line each)
- Any notable findings (related work, surprising complexity, design decisions needed)

## Guidelines

- **Trace code, don't guess.** Every claim about behavior must be backed by reading the actual source. If an agent reports something surprising, verify it.
- **Don't propose solutions in depth** — that's for implementation time. The analysis should identify the *shape* of the solution (local fix vs. architectural change) without designing it fully.
- **Be honest about uncertainty.** If you can't determine whether something is M or L without a design discussion, say so and default to L.
- **Watch for compositional impact.** The most common mistake is underestimating complexity by proposing a bolt-on fix for something that needs proper factoring.
- **Keep the report useful for implementation.** Someone (likely Claude in a future session) will read this report when actually fixing the issue. The code path traces and file:line references are the most valuable parts.
- **Target ~40k tokens max** for the full report. Be detailed but not exhaustive. Use judgment about what's useful for implementation vs. what's noise.
