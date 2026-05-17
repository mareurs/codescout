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

### #2 — `read_file(@buf, json_path="$.array[N].field")` returns 0 lines for array-element paths

- **Symptom:** `read_file(path="@tool_xxx", json_path="$.symbols[0].body")` returns `lines: 0` even though the buffer contains a populated `body` at `symbols[0]`. Object access on the same buffer (e.g. `json_path="$.context"`) works correctly.
- **Root cause:** Unknown — likely jsonpath dispatch difference between array-index-with-property access vs. plain object property access.
- **Workaround:** Use `read_file(path=@tool_, start_line, end_line)` and parse manually, or `read_file(path="<real-filesystem-path>", force=true)`.
- **Fix:** Open. Promoted from F-1 in `docs/trackers/i1-session-friction.md`.

### #3 — `read_file(@buf, start_line=N, end_line=M)` returns empty content past buffer midpoint

- **Symptom:** Reading `@tool_*` buffer line ranges beyond roughly the buffer midpoint returns empty `content` with no error. The same buffer reads correctly with smaller `start_line`.
- **Root cause:** Unknown. Possible pagination or buffer-chunk offset miscalculation. Buffer total bytes reported correctly; only `start_line` past a threshold triggers the empty response.
- **Workaround:** Read filesystem path directly (`force=true`), or fetch from line 1 in two passes.
- **Fix:** Open. Promoted from F-2 in `docs/trackers/i1-session-friction.md`.

### #4 — `grep(pattern, path="@tool_*")` false-negatives on strings present in the buffer

- **Symptom:** `grep` on an `@tool_*` buffer returns `{"matches": [], "total": 0}` for patterns verifiably present in the buffer (confirmed via `read_file` line-range on the same buffer immediately afterward). The tool emits a misleading suggestion: *"Pattern looks like a symbol name. Consider: symbols(name='…')."*
- **Root cause:** Likely two layered causes. (1) `grep` on `@tool_*` may not operate on raw buffer text the way it does on filesystem paths. (2) The symbol-name-suggestion router may intercept queries containing underscores/identifier-shaped tokens before the search runs.
- **Workaround:** Use `read_file(path=@tool_, json_path=…)` for structured fields, or `read_file(@tool_, start_line, end_line)` for sequential inspection. Reserve `grep` for filesystem paths.
- **Fix:** Open. Promoted from F-11 in `docs/trackers/i1-session-friction.md`.

## History

### 2026-05-09 — Tracker bootstrapped

Created from the `audit_issues` archetype via `librarian(tracker_design)`.
Replaces the static `docs/issues/INDEX.md` shipped earlier on `experiments`
(commit b3b063b). Inaugural issue (#1) filed for the `edit_markdown` H1
footgun observed during this very bootstrap session.
