# Markdown Tools: read_markdown & edit_markdown

Two dedicated tools for navigating and editing Markdown files using
heading-based addressing. They replace the need to read raw line ranges
or construct fragile string replacements against unstructured text.

---

## read_markdown

Navigate a Markdown file by heading. Without `heading`/`headings` params,
returns a **heading map** — the document outline with line numbers.

### Parameters

| Param | Type | Description |
|---|---|---|
| `path` | string | Markdown file path (relative to project root) |
| `heading` | string | Single section to read (fuzzy matched) |
| `headings` | string[] | Multiple sections in one call (mutually exclusive with `heading`) |
| `start_line` / `end_line` | int | Raw line range fallback (1-indexed, inclusive) |

### Usage

```
// Step 1: get the heading map
read_markdown("docs/guide.md")
→ heading map with line numbers

// Step 2: read specific sections
read_markdown("docs/guide.md", headings=["## Auth", "## Config"])
→ both sections in one response
```

The heading map is the starting point for any markdown edit workflow — always
read it first so you know which headings exist before targeting one.

---

## edit_markdown

Edit a Markdown document section by heading. Heading matching is **fuzzy** —
`## Auth` matches `## Authentication` — so you don't need to quote headings
exactly.

### Actions

| Action | Description |
|---|---|
| `replace` | Replace section body (heading line is preserved) |
| `insert_before` | Insert content before the heading |
| `insert_after` | Insert content after the section (before next heading) |
| `remove` | Delete the section and its body |
| `edit` | Surgical string replacement within a section (`old_string` → `new_string`) |

### Parameters

| Param | Type | Description |
|---|---|---|
| `path` | string | Markdown file path |
| `heading` | string | Target section heading (fuzzy matched) |
| `action` | string | One of the actions above |
| `content` | string | New body for `replace`/`insert_*` (heading not included) |
| `old_string` | string | For `edit`: exact text to find |
| `new_string` | string | For `edit`: replacement text |
| `replace_all` | bool | For `edit`: replace all occurrences (default: false) |
| `edits` | array | Batch mode — multiple operations applied atomically |

### Examples

```
// Replace a section body
edit_markdown("docs/guide.md",
  heading="## Configuration",
  action="replace",
  content="See project.toml for all options.\n")

// Surgical fix inside a section
edit_markdown("docs/guide.md",
  heading="## Auth",
  action="edit",
  old_string="secret_key = \"\"",
  new_string="secret_key = \"<your-key>\"")

// Batch: two edits in one atomic call
edit_markdown("docs/guide.md",
  edits=[
    { heading: "## Usage", action: "replace", content: "..." },
    { heading: "## License", action: "remove" }
  ])
```

### Batch Mode

Pass an `edits` array instead of `heading`/`action` to apply multiple operations
atomically. All edits are validated before any are applied — if one heading is
missing, nothing changes.

---

### `at` Parameter for `insert_after`

When the section's body contains nested sub-headings, `insert_after` needs to
know whether you want the new content **at the end of the section** (after
all sub-sections) or **immediately after the heading line** (before any
sub-section). Pass `at` to disambiguate:

| `at` value | Placement |
|---|---|
| `"end-of-section"` (default) | After all nested sub-sections — useful for adding a new H3 to an existing H2. |
| `"after-heading-line"` | Immediately after the heading line itself — useful when a top-level H1 wraps the entire document and "end-of-section" would mean EOF. |

```text
// Add a new H3 inside an existing H2 section
edit_markdown("docs/guide.md",
  heading="## Configuration",
  action="insert_after",
  content="\n### Environment Variables\n\n...\n",
  at="end-of-section")

// Add a top-of-page note right under a wrapping H1
edit_markdown("docs/guide.md",
  heading="# Guide",
  action="insert_after",
  content="\n> Note: requires v0.5+.\n",
  at="after-heading-line")
```

### Frontmatter Mutation

> **Status:** experimental — see [Experimental Features](../experimental/index.md).

Pass `frontmatter: { set, delete }` to mutate the YAML frontmatter block in
the same atomic call as any body edits. The mutator preserves existing key
order — updated keys stay in place; new keys append at the end of the
block.

| Field | Type | Effect |
|---|---|---|
| `frontmatter.set` | object | Key → value pairs to write. Scalars, strings, booleans, null, or inline arrays only — flat structure, no nested objects. |
| `frontmatter.delete` | string[] | Keys to remove. Missing keys are silently ignored (idempotent). |

At least one of `set` / `delete` must be non-empty.

```text
// Close a bug file: flip status, set closed date, drop a legacy field
edit_markdown("docs/issues/2026-05-18-foo.md",
  edits=[
    { heading: "## Fix", action: "replace", content: "Shipped in abc1234.\n" }
  ],
  frontmatter={
    set: { status: "fixed", closed: "2026-05-18" },
    delete: ["legacy_field"]
  })
```

The frontmatter mutation runs **alongside** any `heading`+`action` or
`edits[]` block in the same call — all changes are validated and applied
atomically. If the file has no existing frontmatter block, `set` operations
prepend one; `delete` is a no-op.

**Constraints:**

- Flat YAML only — one key per line, scalar / string / inline-array values.
  Nested objects raise an error.
- Keys must be non-empty and contain no whitespace or colons.
- If the file starts with `---` but the closing delimiter is missing, the
  mutator refuses to guess and returns a "frontmatter is malformed" error.

## Why Not edit_file?

`edit_file` works on raw strings and requires exact whitespace/newline matching.
For Markdown, heading-scoped edits are both safer and more resilient:

| Scenario | edit_file | edit_markdown |
|---|---|---|
| Replace a section body | Error-prone: must match surrounding blank lines exactly | `action=replace` — heading preserved automatically |
| Edit text inside a section | Works, but edits anywhere in the file | `action=edit` scoped to one section |
| Remove a section | Must know exact start/end lines | `action=remove` — no line numbers needed |
| Multiple edits | Multiple calls, each can conflict | `edits=[]` batch — atomic |
