---
priority: 1
type: note
---

# Strategy

## Positioning

**One sentence:** Tractor lets development teams write rules that keep their
codebase consistent — across any language, enforced automatically.

**Elevator pitch:** Tractor parses source code into a clean, inspectable tree
and lets you query it with standard expressions. Write a rule once, enforce it
in CI forever. No custom DSL to learn — AI tools and developers can both write
queries immediately.

## Target Niche

**Primary:** Convention enforcement for development teams.

Teams that have coding standards, architectural rules, or patterns they want
to enforce automatically — especially across large or multi-language codebases.

**Adjacent:** Custom linting, code search, and structural code analysis.

Developers who need to find patterns that text-based grep can't express:
"all async methods missing await", "all controllers without authorization",
"all classes with more than N public methods."

## What We Lead With

Tractor is a **convention enforcement tool** that happens to use XPath under
the hood. The positioning should emphasize:

1. **Write a rule, enforce it everywhere.** The core value proposition.
2. **Transparent.** You can see exactly what you're querying. No black box.
3. **AI-friendly.** LLMs already know XPath and XML. Any developer can
   generate queries with AI assistance — no special docs needed.
4. **Multi-language.** One tool, one syntax, 20+ languages.
5. **CI-native.** Built for pipelines. GCC/GitHub output formats, exit codes,
   expectations.

## Concentric Circles: Expansion Strategy

Build features for the broader use cases, but don't lead with them in
messaging. Let users discover the breadth organically.

```
Circle 1 — Core positioning (what we say):
  "Enforce coding conventions across any language."

Circle 2 — Power user discovery:
  "Also works on config files, data formats, schemas."
  JSON, YAML, XML, INI are already supported.

Circle 3 — Emerging potential:
  "Universal structured data querying and transformation."
  Code synthesis, transpilation, ETL on structured files.
```

**Rule of thumb:** Build Circle 2 and 3 features when they're natural
extensions of the existing pipeline. Don't market them until Circle 1
positioning is established and there's organic pull.

## Competitive Differentiation

| Competitor    | Their angle           | Our advantage                                   |
|---------------|-----------------------|-------------------------------------------------|
| ast-grep      | Pattern matching DSL  | Transparent tree, standard query language, AI-friendly |
| Semgrep       | Security-focused rules| Lighter weight, broader language support, no cloud dependency |
| CodeQL        | Database-style queries| No build step, instant results, simpler setup    |
| eslint/clippy | Language-specific     | Cross-language, one tool for all                 |
| grep/ripgrep  | Text search           | Structural queries, not text patterns            |

**Key differentiators (in priority order):**

1. **Transparency.** Run `tractor file.cs` to see the tree. No other tool
   lets you inspect what you're querying this easily.
2. **AI-native.** XPath + XML are in every LLM's training data. Custom DSLs
   are not. This is a compounding advantage as AI-assisted development grows.
3. **Standard syntax.** No lock-in to a proprietary query language. XPath 3.1
   is a W3C specification with 25 years of tooling.
4. **Multi-language consistency.** Same semantic tree structure across
   languages. Learn it once, apply everywhere.

## What We Say Yes To

- Features that make convention enforcement easier and more reliable.
- Better diagnostics: query debugging, suggestions, schema discovery.
- New language support when there's user demand.
- Output formats that integrate with developer workflows (CI, IDEs, editors).
- Data format support (JSON, YAML, CSV) when it naturally extends the
  existing pipeline.

## What We Say No To (For Now)

- Reusable rule libraries / marketplace. Writing your own rules is the core
  experience. Community sharing may come later if there's organic demand.
- Full IDE plugin with real-time feedback. Focus on CLI + CI first.
- Competing with security scanners (Semgrep, CodeQL) on vulnerability
  detection. Our niche is team conventions, not CVE databases.
- Marketing Tractor as a "universal data tool" or ETL platform. The
  capability can exist; the positioning should not.

## Success Metrics (Qualitative)

- A team can go from "we have a convention" to "CI enforces it" in under
  10 minutes.
- A developer using AI can write a working query on their first attempt,
  given only the tree output.
- Query diagnostics are good enough that empty results always come with
  actionable suggestions.
