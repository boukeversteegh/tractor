---
name: write-release-notes
description: Write or update GitHub release notes for tractor. Use when creating a new release, updating existing release descriptions, or reviewing release notes quality.
allowed-tools: Bash, Read, Grep, Glob, AskUserQuestion
argument-hint: "[tag or 'all' or 'latest']"
---

# Write Release Notes for Tractor

Write compelling, value-oriented release notes for tractor GitHub releases.

## Process

1. **Identify the release(s)**: Use `$ARGUMENTS` to determine which release(s) to update.
   - A specific tag like `26.0327`
   - `latest` for the most recent release
   - `all` to review and update all releases
   - If no argument, ask the user.

2. **Understand what changed**: For each release, gather context from:
   - `gh release view <tag>` for the current description
   - `git log <prev-tag>..<tag>` for commits in the release
   - PR descriptions: `gh pr view <number>`
   - Relevant design docs in `docs/` and specs in `specs/`

3. **Verify examples**: Before writing release notes that include CLI examples:
   - Actually run the commands to confirm they work
   - Use `tractor` (not `cargo run`) in examples — assume it's installed
   - Show real output, not invented output
   - For tree/XML examples, check snapshots in `tests/integration/languages/` (not the `.raw.xml` ones — those are internal)

4. **Draft and review**: Present the draft to the user via `AskUserQuestion` with a preview before applying.

5. **Apply**: Use `gh release edit <tag> --notes-file <file>` to update. Write notes to a temp file first to preserve markdown formatting.

## Consolidation (past releases)

When reviewing releases from previous days, consolidate multiple same-day releases into one:

1. **Keep the latest tag's release** for each day (e.g., `26.0406.4` if `26.0406`, `.2`, `.3`, `.4` exist).
2. **Merge all changes** from that day into a single set of release notes on the kept release.
3. **Delete redundant releases** with `gh release delete <tag> --yes`. Always keep git tags — never pass `--cleanup-tag`.
4. **Skip internal-only changes** (test infrastructure, internal refactors, Claude skills) — only surface user-facing changes.
5. If an API or syntax changed across the day's releases, use the final version and add: *Syntax updated as of version \<tag\>.*

This only applies to past releases. Today's releases stay as-is until the day is over.

## Voice and Tone

Follow the brand guidelines in `specs/branding.md`:

- **Practical over theoretical.** Lead with what you can do, not what changed internally.
- **Confident, not arrogant.** State the value clearly without overselling.
- **Short sentences. Active voice.** "Check output now shows source context" not "Source context has been added to the check output rendering pipeline."
- **Concrete examples beat abstract descriptions.** Show a command and its output.
- **Use "you" and "your"**, not "users" and "developers."

Avoid: "powerful", "revolutionary", "seamlessly", "leverage", "exciting."

## Release Note Structure by Impact

### Major releases (new capabilities, workflow changes)

```markdown
## [Value-oriented headline — what you can now do]

[1-2 sentences: what this enables and why it matters]

[Input file block if relevant, labeled with filename:]

**config.yaml:**
~~~yaml
example content
~~~

[Command example:]

~~~sh
$ tractor [command with real flags]
~~~

[Output block:]

~~~json
{ actual output }
~~~

[Brief note on additional changes if any]
```

### Mid-tier releases (improvements to existing features)

```markdown
## [What improved]

[1-2 sentences on the improvement]

[Before/after or example if it helps]

[Bullet list of other changes if any]
```

### Minor releases (bug fixes, internal changes)

One or two sentences. No headers needed.

## Key Principles

1. **Lead with value, not implementation.** "Extract structured data from source code" not "Support native JSON output for map/array operator results."

2. **Show, don't tell.** A CLI example with input and output communicates more than a paragraph of description.

3. **Use real tractor syntax.** The tree elements are semantic (`<call>`, `<member>`, `<property>`) not raw tree-sitter names (`call_expression`, `member_expression`). Check snapshots when unsure.

4. **Input files deserve their own block.** Don't inline multi-line source code in a shell command. Show it as a labeled file block, then show the tractor command separately.

5. **Show actual output.** Run the command and paste the real result. Don't invent output.

6. **Internal changes go at the bottom**, if mentioned at all. "Also includes..." or just skip them for minor releases.

7. **Don't lead with XPath.** Per branding guidelines — lead with what you can do. XPath is the engine, not the headline.

8. **Consistent with mission tone.** Use "guide", "feedback", "drifts" — avoid "enforcer", "violation", "vigilance." See `specs/mission.md`.
