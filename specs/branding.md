# Tractor Brand Style Guide

## Theme: Modern Agri-Tech

A fresh, technical take on agricultural aesthetics - combining the harvest/farming metaphor
of "tractor" with clean, modern developer tooling sensibilities.

## Color Palette

| Role             | Color       | ANSI Code | Hex       | Usage                          |
|------------------|-------------|-----------|-----------|--------------------------------|
| **Primary**      | Blue        | 34        | `#0000AA` | Element/tag names              |
| **Secondary**    | Cyan        | 36        | `#00AAAA` | Attribute names                |
| **Accent**       | Yellow      | 33        | `#AAAA00` | Attribute values, highlights   |
| **Content**      | White       | 97        | `#FFFFFF` | Text content, user data        |
| **Punctuation**  | Dim White   | 2;37      | `#AAAAAA` | Brackets `< > = /`, structure  |

## Rationale

- **Blue**: Traditional XML/code syntax highlighting, familiar to developers
- **Cyan**: Modern tech accent, provides visual separation from blue
- **Yellow**: Draws attention to values (the "yield" from code harvesting)
- **White**: Clean, readable content - the actual extracted data
- **Dim White**: Subdued structure, lets the meaningful content stand out

## Application

### XML Output (tractor-parse, tractor)
```
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

**Important**: Reset after each dim character to prevent dim from affecting subsequent colors.
Pattern: `DIM(<) RESET BLUE(element) RESET CYAN(attr) DIM(=) RESET YELLOW("value") DIM(>) RESET`

### CLI Messages
- Errors: Red (standard)
- Warnings: Yellow
- Success: Green
- Info: Default/White

## Color Control

All tools support:
- `--color auto` (default) - Color when outputting to terminal
- `--color always` - Force color output
- `--color never` - Disable colors
- Respects `NO_COLOR` environment variable
