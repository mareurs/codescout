# Document Section Editing

Structured markdown operations: read, edit, and manage document sections by
heading instead of line numbers or string matching.

---

## Overview

Seven features built on a shared heading-parsing foundation:

| Feature | Tool | Purpose |
|---------|------|---------|
| `edit_section` | New tool | Replace, insert, or remove entire sections by heading |
| `headings=[]` | `read_file` param | Read multiple sections in one call |
| `heading=` | `edit_file` param | Scope string matching to a section |
| `edits=[]` | `edit_file` param | Atomic batch edits, optionally heading-scoped |
| `mode="complete"` | `read_file` param | Full plan file inline with delivery receipt |
| Fuzzy heading matching | All heading params | Strips formatting, prefix/substring fallback |
| Section coverage | Automatic | Tracks which sections you've read, hints on writes |

## Recommended Workflow

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `read_file(path)` | Get heading map — see all sections |
| 2 | `read_file(path, headings=[...])` | Read target sections (one call) |
| 3a | `edit_section(path, heading, action, content)` | Whole-section: replace, insert, remove |
| 3b | `edit_file(path, heading=, old_string, new_string)` | Surgical: scoped string replacement |
| 3c | `edit_file(path, edits=[...])` | Batch: multiple edits, atomic |

---

## `edit_markdown`

**Purpose:** Whole-section operations on markdown files — replace content, insert
new sections, or remove existing ones. Addresses sections by heading, not line
numbers.

> The tool was renamed from `edit_section` to `edit_markdown` in v0.11 to
> mirror `read_markdown`. The behavior is unchanged.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | File path relative to project root |
| `heading` | string | yes | Section heading to target (e.g. `## Auth`) |
| `action` | string | yes | `replace`, `insert_before`, `insert_after`, or `remove` |
| `content` | string | for replace/insert | New content |

**Actions:**

- **`replace`** — Replaces the section body. Pass body only — the heading is
  preserved automatically. If your content includes a heading line, smart
  detection kicks in and replaces the heading too.
- **`insert_before`** — Inserts content as a new section before the target.
- **`insert_after`** — Inserts content as a new section after the target.
- **`remove`** — Deletes the entire section (heading + body).

**Example — replace a section's body:**

```json
{
  "path": "docs/ROADMAP.md",
  "heading": "## What's Next",
  "action": "replace",
  "content": "- Feature A\n- Feature B\n- Feature C\n"
}
```

**Example — insert a new section after an existing one:**

```json
{
  "path": "docs/ROADMAP.md",
  "heading": "## What's Built",
  "action": "insert_after",
  "content": "## What's In Progress\n\n- Working on X\n- Prototyping Y\n"
}
```

**Example — remove a section:**

```json
{
  "path": "docs/ROADMAP.md",
  "heading": "## Deprecated",
  "action": "remove"
}
```

---

## `read_file` — Heading Navigation

### Single heading: `heading=`

Read one section by heading. Returns the section content with line range and
breadcrumb (parent headings).

```json
{
  "path": "docs/ROADMAP.md",
  "heading": "## What's Next"
}
```

### Multiple headings: `headings=[]`

Read multiple sections in a single call. More efficient than separate calls.

```json
{
  "path": "docs/ROADMAP.md",
  "headings": ["## What's Built", "## What's Next"]
}
```

### Complete mode: `mode="complete"`

Returns the entire file inline (bypasses the output buffer) with a delivery
receipt showing section count and checkbox progress. Scoped to files in `plans/`
directories only.

```json
{
  "path": "plans/implementation-plan.md",
  "mode": "complete"
}
```

**When to use:** Only when you truly need the full plan. For targeted reads,
prefer the heading map (`read_file(path)`) followed by `headings=[]`.

---

## `edit_file` — Heading-Scoped Editing

### Scoped matching: `heading=`

Restricts `old_string` matching to the lines within a specific section. Prevents
accidental matches in other parts of the file.

```json
{
  "path": "docs/ROADMAP.md",
  "heading": "## What's Next",
  "old_string": "Feature A",
  "new_string": "Feature A ✅"
}
```

### Batch mode: `edits=[]`

Multiple edits applied atomically in a single write. Each edit can optionally
have its own `heading` scope.

```json
{
  "path": "docs/plan.md",
  "edits": [
    {
      "old_string": "- [ ] Step 1",
      "new_string": "- [x] Step 1",
      "heading": "## Task A"
    },
    {
      "old_string": "- [ ] Step 2",
      "new_string": "- [x] Step 2",
      "heading": "## Task B"
    }
  ]
}
```

---

## Fuzzy Heading Matching

All heading parameters (`heading=` on `read_file`, `edit_file`, `edit_section`)
use a 4-tier matching strategy:

1. **Exact match** — `## Auth` matches `## Auth`
2. **Format-stripped** — `## \`Auth\`` matches `## Auth` (backticks, bold, italic stripped)
3. **Prefix match** — `## Auth` matches `## Authentication & Authorization`
4. **Substring match** — `Auth` matches `## Authentication & Authorization`

Headings inside fenced code blocks are ignored.

---

## Section Coverage Tracking

The server tracks which markdown sections you've read during a session. This
powers two hints:

- **On reads:** When you read part of a file, the response includes an `unread`
  list showing sections you haven't seen yet.
- **On writes:** When you edit a file with unread sections, a warning appears
  so you can verify your edit doesn't conflict with unseen content.

Coverage resets when the file is modified on disk (mtime-based invalidation).

---

## Choosing the Right Tool

| You want to… | Use |
|--------------|-----|
| Replace an entire section's content | `edit_section(action="replace")` |
| Add a new section | `edit_section(action="insert_before/after")` |
| Delete a section | `edit_section(action="remove")` |
| Fix a typo in a section | `edit_file(heading=, old_string, new_string)` |
| Toggle multiple checkboxes | `edit_file(edits=[...])` with per-edit `heading` |
| Read specific sections | `read_file(headings=[...])` |
| Read a full plan file | `read_file(mode="complete")` |
| See what sections exist | `read_file(path)` — returns heading map |
