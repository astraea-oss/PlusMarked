# MarkdownPlus Format

MarkdownPlus files use the `.mdp` extension. The first implementation treats them as YAML frontmatter followed by a Markdown-compatible body.

```md
---
id: "018ff6e2-9f4b-7a64-b101-0e2fd6e32f20"
title: "Example note"
created_at: "2026-06-28T23:30:00Z"
updated_at: "2026-06-28T23:30:00Z"
tags: []
aliases: []
type: "note"
---

Markdown body.
```

Required fields:

- `id`
- `title`
- `created_at`
- `updated_at`

The body remains compatible with normal Markdown for the first vertical slice.

