# Tractor Brand Guidelines

## Identity

**Name:** Tractor
**Tagline:** Write a rule once. Enforce it everywhere.
**Category:** Developer tool — convention enforcement

## Voice & Tone

### Personality

Tractor is **practical, direct, and quietly confident**. Like the machine
it's named after — not flashy, but gets the job done reliably.

- **Practical over theoretical.** Show, don't tell. Lead with examples.
- **Confident, not arrogant.** We know what we're good at. We don't need
  to trash competitors to make the point.
- **Technical but approachable.** Assume the reader is smart but busy.
  No jargon without purpose.
- **Warm, not corporate.** We're a tool for teams, not an enterprise
  platform. Casual is fine. Buzzwords are not.

### Writing Style

- Short sentences. Active voice.
- Lead with what the user can *do*, not what the tool *is*.
- Use "you" and "your", not "users" and "developers".
- Concrete examples beat abstract descriptions every time.
- Avoid marketing fluff: "powerful", "revolutionary", "next-generation",
  "seamlessly", "leverage".

### Talking About XPath

XPath is the engine, not the brand. Follow these guidelines:

- **Don't lead with XPath.** Lead with what you can do: "find patterns",
  "enforce conventions", "query code structure."
- **Don't hide XPath either.** When the audience is technical or already
  curious, be direct: "It uses XPath 3.1 under the hood."
- **Frame XPath as a strength, not a quirk.** "Standard syntax that AI
  tools already know" beats "it uses XML and XPath."
- **For AI/LLM audiences:** Emphasize that the tree is XML and queries
  are XPath. This is a feature, not an implementation detail.

**Do say:**
- "Query your code structure with standard expressions."
- "The same syntax works across 20+ languages."
- "AI tools can write queries without special documentation."

**Don't say:**
- "Convert your code to XML and query it with XPath!"
- "XPath-based code analysis tool."
- "Leveraging the power of XML..."

## Color Palette

Based on the Solarized color scheme for readability, with a John Deere
/ agricultural accent for brand personality.

### Terminal / CLI Colors

| Role             | Color       | ANSI Code | Hex       | Usage                          |
|------------------|-------------|-----------|-----------|--------------------------------|
| **Primary**      | Blue        | 34        | `#268bd2` | Element/tag names              |
| **Secondary**    | Cyan        | 36        | `#2aa198` | Attribute names                |
| **Accent**       | Yellow      | 33        | `#b58900` | Attribute values, highlights   |
| **Content**      | White       | 97        | `#FFFFFF` | Text content, user data        |
| **Punctuation**  | Dim White   | 2;37      | `#AAAAAA` | Brackets `< > = /`, structure  |

### Web / UI Colors

| Role             | Hex         | Usage                          |
|------------------|-------------|--------------------------------|
| **Background**   | `#0f1a0f`   | Main content area              |
| **Surface**      | `#1a2e1a`   | Panels, cards                  |
| **Elevated**     | `#243824`   | Hover states, active elements  |
| **Text**         | `#c8e6c8`   | Primary text                   |
| **Text Muted**   | `#8aaa8a`   | Secondary text, labels         |
| **Green**        | `#5cb85c`   | Accent, success, brand color   |
| **Yellow**       | `#d4aa00`   | Highlights, attention, CTAs    |
| **Border**       | `#2a4a2a`   | Separators, outlines           |
| **Error**        | `#dc3545`   | Errors, failures               |

### Application

```
XML output coloring:

<element attr="value">content</element>
│ │      │    │       │       │ │
│ │      │    │       │       │ └── Dim (>)
│ │      │    │       │       └── Blue (element)
│ │      │    │       └── White (content)
│ │      │    └── Yellow (values)
│ │      └── Cyan (attributes)
│ └── Blue (elements)
└── Dim (<)
```

**Important**: Reset after each dim character to prevent dim from affecting
subsequent colors. Pattern:
`DIM(<) RESET BLUE(element) RESET CYAN(attr) DIM(=) RESET YELLOW("value") DIM(>) RESET`

## CLI Messages

- Errors: Red (standard)
- Warnings: Yellow
- Success: Green
- Info: Default/White

## Color Control

All tools support:
- `--color auto` (default) — Color when outputting to terminal
- `--color always` — Force color output
- `--color never` — Disable colors
- Respects `NO_COLOR` environment variable
