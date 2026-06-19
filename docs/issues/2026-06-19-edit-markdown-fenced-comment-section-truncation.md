---
kind: bug
status: open
title: edit_markdown/read_markdown section parser treats `# ` comments inside fenced code blocks as headings
owners: []
tags:
  - edit_markdown
  - read_markdown
  - markdown-parser
severity: medium
opened: 2026-06-19
---

# edit_markdown / read_markdown: `# ` lines inside fenced code blocks end the enclosing section early

## Symptom

`edit_markdown(action="edit", heading="### Task 7 …", old_string=<line>)` fails
with `old_string not found in section` even though the line provably exists in
that section. `read_markdown(heading="### Task 7 …")` of the same file returns a
truncated section that stops partway through.

## Reproduction

In `docs/superpowers/plans/2026-06-19-codescout-pi-integration.md`:

1. The `### Task 7` section contains a fenced `bash` block (an `install.sh`) whose
   body has comment lines like `# Symlink the codescout…` and `#!/usr/bin/env bash`.
2. A paragraph later in the SAME section (L392, `Create \`contrib/pi/README.md\`
   documenting: …`) is the edit target.
3. `grep` confirms the target text is at L392.
4. `read_markdown(path, heading="### Task 7 …")` returns only L352–363 — it stops
   at the `#!/usr/bin/env bash` line, i.e. just before `# Symlink …`.
5. `edit_markdown(action="edit", heading="### Task 7 …", old_string=<L392 text>)`
   → `old_string not found in section`.

## Root cause (high confidence)

The markdown section extractor used by `read_markdown` and `edit_markdown` does
**not** track fenced-code-block (```` ``` ````) state. A line that begins with
`# ` (hash + space) inside a fenced block is misparsed as an ATX heading, which
prematurely terminates the enclosing section. Here the install.sh comment
`# Symlink the codescout↔Pi integration files into the Pi agent dir.` reads as a
new H1, so everything after it (including the L392 edit target) falls outside the
`### Task 7` section as the parser sees it.

This affects ANY markdown doc with shell/python/etc. comment lines (`# …`) in
fenced code blocks — extremely common in this repo's plans and specs.

## Evidence

- `grep` "Step 3: Write the README" → match at L390–392 (text present).
- `read_markdown(heading="### Task 7 …")` → returned range `L352-L363`, last line
  `#!/usr/bin/env bash`; true section extends well past L420.
- Two `edit_markdown(action="edit")` calls with exact, grep-confirmed old_strings
  both returned `old_string not found in section`.

## Hypotheses tried

- "old_string whitespace mismatch" — ruled out: a single-line, grep-verbatim
  old_string (incl. the `≥` char) also failed; and the section read itself is
  truncated, which is independent of any old_string.

## Fix (proposed — not yet implemented)

In the section/heading splitter (the code backing `read_markdown` heading
navigation and `edit_markdown` section scoping), track fenced-code state while
scanning lines: when inside a ```` ``` ````/`~~~` fence, do NOT treat `#`-prefixed
lines as headings. Apply the same guard to the line-slice returned by
`read_markdown(heading=…)`. Add a regression fixture: a section containing a
fenced block with `# comment` lines, then a paragraph after the block; assert
both `read_markdown(heading)` returns the full section and `edit_markdown(edit)`
matches text after the block.

## Workarounds

- Use `edit_file` (exact string replace — no markdown section parsing) for edits
  to markdown sections that contain fenced code blocks with `# ` comment lines.
  This is the IL5 exception: the sanctioned `edit_markdown` is broken on this
  input, so route through `edit_file` until fixed.
- Or `create_file(overwrite=true)` to rewrite the whole file.

## Resume

Splitter likely in the markdown tooling under `src/` (the code serving
`read_markdown`/`edit_markdown`). Start: `grep` for the heading/section
extraction (ATX `#` detection) and add fenced-code tracking. Pair the fix with
the regression fixture above.
