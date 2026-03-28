# Hacker News Launch Checklist

Preparation tasks for posting Tractor on Hacker News.

## Must-fix (blockers)

### Add LICENSE file
There is no LICENSE file in the repo root. MIT is declared in Cargo.toml
but HN readers expect a visible license file. Without it, some people
won't engage at all.

### Ensure `cargo install tractor` works end-to-end
Cargo.toml is missing crates.io metadata: `repository`, `homepage`,
`keywords`, `categories`. Verify the crate is actually published and
installable. Test the full flow on a clean machine.

```toml
# Add to workspace Cargo.toml [workspace.package]:
repository = "https://github.com/boukeversteegh/tractor"
homepage = "https://tractor.fly.dev"
keywords = ["code", "xpath", "query", "ast", "lint"]
categories = ["development-tools", "command-line-utilities"]
```

### Fix README query examples
The README examples use outdated syntax (attributes instead of child
elements). Already fixed on branch `claude/project-code-review-f5nY3`:
- `method[async][type='void']` → `method[async][returns/type='void']`
- `count(params/param)` → `count(parameters/parameter)`

Merge that branch or cherry-pick before launch.

### Merge brand/strategy updates
Branch `claude/project-code-review-f5nY3` has updated mission, strategy,
branding docs, README, and website. Review and merge before launch.


## Should-fix (HN will nitpick)

### Add non-C# samples and examples
The samples/ directory is all Entity Framework / C# migrations. HN skews
toward Rust, Go, Python, TypeScript. Add at least 2-3 sample files in
popular languages with example queries. Consider adding a `samples/`
README showing example commands.

### Warn when no files match the glob
`tractor nonexistent.cs` silently returns nothing. Someone who mistypes
a path will think the tool is broken. Print a warning to stderr when the
file glob matches zero files.

### Polish GitHub release notes
No need for a committed CHANGELOG.md file — it goes stale too easily at
this stage. Instead, edit the auto-generated release notes on each GitHub
release: add a short human-written summary (3-5 bullet points) at the
top. Make sure at least the latest release looks good before posting,
since HN readers will click through to the releases page.

### Query diagnostics for empty results
This was discussed as the single biggest DX improvement. When a query
returns zero matches, show what went wrong:
- Which prefix matched?
- Which segment or predicate killed it?
- What node names exist?

Even a basic version ("0 matches. Hint: 'method' exists but has no child
'type' — did you mean 'returns/type'?") would be hugely impressive in a
demo. This is also a great thing to show in the HN post itself.


## Presentation (the post itself)

### Write the Show HN post
Title suggestion (keep under 80 chars):
> Show HN: Tractor — grep for code structure, not text (20+ languages)

Body should:
- Lead with the problem, not the solution
- Show a concrete before/after (grep vs. tractor)
- Link the playground immediately (people try before installing)
- Mention AI-friendly angle (LLMs can write queries without docs)
- NOT lead with XPath or XML — let people discover that

Draft:
```
I built a CLI tool that lets you query code structure across 20+
languages. Find patterns that text grep can't: async methods missing
await, controllers without authorization, classes with too many
parameters.

Try it in the browser: https://tractor.fly.dev/playground

How it works: tractor parses code into a semantic tree you can inspect
and query. Run `tractor file.cs` to see the tree, then query it with
standard expressions. The tree uses a consistent structure across
languages, so you learn it once.

What makes it different from ast-grep/Semgrep/CodeQL:
- Transparent: you can see exactly what you're querying
- Standard syntax: AI tools (ChatGPT, Claude) can write queries
  without special docs
- CI-native: built-in --expect, exit codes, GCC/GitHub output formats

I've been building this to solve convention enforcement at work —
ensuring teams follow agreed patterns without relying solely on code
review.
```

### Prepare answers to predictable questions

**"Why not ast-grep?"**
ast-grep uses a pattern-matching DSL on an opaque tree. When your
pattern doesn't match, you're stuck guessing. Tractor lets you see the
tree and uses a standard query language that LLMs already know. Also:
a 57-line ast-grep YAML rule = one tractor query.

**"XPath in 2026? Really?"**
XPath is the implementation detail, not the selling point. We use it
because it's the best standard for querying trees with predicates. The
real benefit: AI tools already know it, so any developer can generate
queries without reading docs.

**"How does this compare to Semgrep?"**
Different niche. Semgrep is security-focused with a rule marketplace.
Tractor is for team conventions — lightweight, no cloud dependency, runs
locally. Think eslint-for-any-language rather than security scanner.

**"Does it support [my language]?"**
Currently 22 languages via tree-sitter. Adding a new language is
straightforward since the pipeline is generic. If it has a tree-sitter
grammar, it can be added.


## Nice-to-have (if time permits)

### Record a terminal demo (asciinema/vhs)
A 30-second GIF or asciinema recording showing: (1) inspect tree,
(2) write query, (3) get results, (4) CI check mode. Embed in README.

### Add shell completions
`tractor --completions bash/zsh/fish` would be polished. Clap supports
this out of the box.

### Benchmark against alternatives
If tractor is faster than ast-grep or semgrep on a real codebase, that's
a compelling data point. If it's slower, don't mention it.
