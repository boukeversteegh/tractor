---
name: todo
description: Create, list, or manage todo files in the todo/ directory. Use when the user wants to create a new todo, list existing todos, or update a todo's status.
argument-hint: [create|list|done] [description or id]
allowed-tools: Read, Write, Edit, Glob, Grep, Bash(ls *), Bash(git log *)
---

# Todo Management

Manage todo files in the `todo/` directory. Each todo is a markdown file
named `{id}-{slug}.md` where `id` is a unique incrementing integer.

## Commands

Based on $ARGUMENTS:

- **create [description]** — Create a new todo file
- **list** — List all current todos with their IDs and titles
- **done [id]** — Mark a todo as completed by deleting the file
- No arguments — List all todos (same as `list`)

## Determining the next ID

To assign an ID to a new todo, you MUST check both:

1. Current files in `todo/` directory
2. Previously deleted files in git history:
   ```bash
   git log --all --diff-filter=D --name-only --pretty="" -- 'todo/*.md' | grep 'todo/' | sort -u
   ```
3. Previously created files (in case they exist on other branches):
   ```bash
   git log --all --diff-filter=A --name-only --pretty="" -- 'todo/*.md' | grep 'todo/' | sort -u
   ```

The new ID must be **one higher than the highest ID ever used** across
all of these sources. Never reuse a deleted ID.

### Known ID history

IDs used so far (as of initial skill creation):
- 1 (deleted), 2, 3 (deleted), 4 (deleted), 5-11 (never used — gap),
  12, 12 (two files), 13, 14, 15, 16, 17, 18, 19, 20

Always verify by running the git commands above — do not rely solely on
this list.

## File format

Use this structure for new todos:

```markdown
# {Short title}

## Problem

{What's wrong or what's needed — 1-3 paragraphs}

## Desired state

{What done looks like — concrete and verifiable}

## Notes

{Optional: implementation hints, related files, impact, priority}
```

Keep it concise. The title in the filename slug should be a few
hyphenated keywords (e.g., `21-query-diagnostics-empty-results.md`).

## Slug conventions

- Lowercase, hyphenated
- 3-6 words max
- Descriptive enough to understand without opening the file

## Listing format

When listing todos, show:

```
 2  streaming-mode
12  field-role-elements-all-languages
12  xmlnode-ir-remaining-work
13  semantic-transform-rewrite
...
```

Right-align the ID, then two spaces, then the slug (filename without
ID prefix and `.md` extension).
