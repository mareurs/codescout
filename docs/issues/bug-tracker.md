---
id: '0ed68e66d69ceec0'
kind: tracker
status: active
title: Bug Tracker
owners:
- '@mareurs'
tags:
- bugs
- tracker
topic: null
time_scope: null
---

## Audit scope and methodology

Tracks bugs noticed while working on codescout — its MCP server, its tools, the
companion plugin's hooks, LSP behavior, build scripts, and anything else that
misbehaves. Each issue gets a row in `params.issues`. Substantial investigations
(multi-session work, complex repro, evidence to gather) live in
`docs/issues/<date>-<slug>.md` and are linked via the row's `path` field;
trivial bugs (one-line fix) need only the param row + the fix commit.

The per-bug file skeleton lives at `docs/issues/_TEMPLATE.md`. Use it for any
issue where `path` is set.

## Per-issue detail

Short summaries (under 100 words per issue). Long investigations belong in
the per-bug file, not here.

### #1 — `edit_markdown insert_after` on H1 places content at section end

- **Symptom:** `edit_markdown(action="insert_after", heading="# Title")` on a
  top-level H1 inserts content at the END of the file (EOF area), not
  immediately after the heading line. For files where the H1 wraps the entire
  document, the content lands at the bottom.
- **Root cause:** The "insert_after a heading" semantic targets the END of that
  heading's section. For an H1 spanning the whole doc, the section ends at EOF.
  Defensible but counter-intuitive for top-level headings.
- **Workaround:** Use `action="edit"` with `old_string` matching the heading +
  the next non-blank line; place the new content between them in `new_string`.
- **Fix:** Open. No commit yet.

## History

### 2026-05-09 — Tracker bootstrapped

Created from the `audit_issues` archetype via `librarian(tracker_design)`.
Replaces the static `docs/issues/INDEX.md` shipped earlier on `experiments`
(commit b3b063b). Inaugural issue (#1) filed for the `edit_markdown` H1
footgun observed during this very bootstrap session.

