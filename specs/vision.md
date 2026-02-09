---
priority: 2
type: note
---

# Vision

<!-- TODO: flesh this out properly. Below are notes from brainstorming. -->

## Approach

Tractor makes coding conventions executable by leveraging a simple but
powerful mechanism: parse source code into an optimized XML tree, then
let users write XPath queries to detect patterns and violations.

Key strategic choices:

- **Low barrier to entry.** The XML tree is heavily optimized to make
  XPath queries as simple as possible. You don't need to learn complex
  syntax - just the basics should be enough.
- **AI-friendly.** XML and XPath are so well-known that AI tools
  (ChatGPT, Claude Code, etc.) are very good at writing XPath queries
  when given an example XML structure. Any developer can quickly
  produce a working query using AI. This is NOT the case for other
  code-grepping tools like ast-grep, which uses a badly documented
  query language on a structure that is not inspectable, with unclear
  semantics.
- **Visual query builders** that completely bypass the fact that
  XML+XPath is used under the hood. (Web demo already exists.)
- **Rule quality matters.** Even when writing a valid query that gives
  the correct result-set is relatively easy, writing *good* rules is
  not obvious. There are many queries that lead to the same result-set,
  but some are robust against variations, some capture the intent of
  the rule better, some are generic while others work only on currently
  known examples. Tractor as a broader project should help developers
  write good rules.
- **Rules as documentation.** Each rule can have a markdown file where
  special syntax or frontmatter defines the tractor parameters, but
  the page content has the description and positive/negative examples.
  This way each convention is clear and can be read and pointed to by
  teammates. (Executable rules are part of vision; documentation is a
  bridge for conventions that can't be automated yet.)

## Not in scope (for now)

- Reusable rule libraries that people can publish and import. Writing
  your own rules is really the core. But if communities sharing rules
  supports the mission, it could be considered later.
