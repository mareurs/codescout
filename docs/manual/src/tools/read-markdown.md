# `read_markdown` improvements
## What changed

Three related improvements landed together:

### 1. Adaptive output tiers

`read_markdown` now selects output detail based on file size:

| File size | Output |
|---|---|
| Small (< 100 lines) | Full content |
| Medium (100–400 lines) | Heading map + full content |
| Large (> 400 lines) | Heading map only + `@file_*` buffer ref |

For large files the response includes a `must_follow` instruction directing
the agent to use the buffer ref for subsequent reads instead of the original
path. This prevents repeated full-file reads for every heading navigation call.

### 2. `@file_*` buffer ref support

`read_markdown` now accepts `@file_*` buffer refs as the `path` argument for
all subsequent calls (heading navigation, line-range slicing). Once a large
file is loaded into a buffer ref, every subsequent access hits the in-process
cache — no disk re-read.

```json
{ "path": "@file_abc123", "heading": "## Configuration" }
{ "path": "@file_abc123", "start_line": 45, "end_line": 90 }
```

### 3. Heading navigation

Pass `heading` to jump directly to a section without reading the full file:

```json
{ "path": "docs/guide.md", "heading": "## Installation" }
```

The response includes the matched section's content, sibling headings (for
orientation), and breadcrumb path.

## Why this matters

Large markdown files (tracker docs, changelogs, architecture docs) previously
required reading the full file to find a section. With adaptive tiers + heading
nav, agents can navigate to the relevant section in one call and cache the
buffer ref for the rest of the session.

## Known limits

- `@file_*` refs expire when the MCP session ends or the output buffer is
  evicted (LRU, capacity 50). Re-read the original path to get a fresh ref.
- Heading match is case-insensitive prefix search — if two headings share a
  prefix, the first match wins.
