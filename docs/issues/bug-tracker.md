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
- **Fix (2026-05-11):** In `perform_section_edit`, when `end_idx == lines.len() && range.level == 1`, insert at `heading_idx + 1` instead of EOF. H2+ last-sections keep the append-at-end semantics. Tests: `insert_after_h1_spanning_whole_doc_goes_below_heading_not_eof`, `insert_after_bounded_section_still_appends_at_section_end`.
- **Status:** Fixed.

### #2 — BUG-021: parallel write calls leave files half-applied

- **Symptom:** Dispatching two write tool calls in parallel can leave one unapplied if the user denies the permission dialog. Previously also caused MCP server crash via rmcp cancellation race.
- **Root cause:** No transaction semantics across independent tool calls; rmcp 0.1.5 cancellation race sent a response for a cancelled request ID.
- **Fix:** rmcp 1.2.0 fixed the crash.
- **Rule:** Never dispatch parallel write tool calls — still unsafe by design.
- **Status:** Mitigated.

### #3 — BUG-030: `replace_symbol` on `mod tests` can eat adjacent function body

- **Symptom:** Replacing `mod tests` overwrote the body of the immediately following function.
- **Root cause:** Stale LSP symbol positions after prior edits; range for `mod tests` extended into adjacent code.
- **Fix (2026-03-20):** `validate_symbol_position` guard detects stale positions and returns `RecoverableError`.
- **Status:** Mitigated. Still watch for stale positions on large files mid-edit — `/mcp` reconnect re-indexes.

### #4 — BUG-032: `remove_symbol` leaves orphaned `impl` block after enum removal

- **Symptom:** Removing an enum left a dangling `impl` block whose type no longer existed.
- **Root cause:** Stale LSP positions (same as BUG-030); range computation grabbed wrong brace set near adjacent `impl Trait for Type`.
- **Fix (2026-03-20):** `validate_symbol_position` guard.
- **Workaround:** Use `create_file` for adjacent/nested `impl` blocks.
- **Status:** Mitigated.

### #5 — BUG-047: `ResilientStdin` spinning `Poll::Pending` floods logs to 268 GB

- **Symptom:** Log files grew to 268 GB each; disk exhausted. Observed at `mirela/deployment` 2026-04-22.
- **Root cause:** `wake_by_ref()` on EAGAIN — canonical spinning-pending async anti-pattern. `WARN`-level log inside spin + no size-based rotation made it catastrophic.
- **Fix (2026-04-22):** 1ms `tokio::time::Sleep` backoff on EAGAIN; `SizeRotatingFile` (50 MiB cap, 3 backups); `WARN` → `TRACE`.
- **Status:** Fixed.

### #6 — BUG-048: `find_symbol` hangs ~60s during LSP cold-start indexing

- **Symptom:** `workspace/symbol` blocked on cold-start retry budget, hanging up to 60s.
- **Root cause:** `uses_cold_start_retry_budget` incorrectly `true` for `workspace/symbol`; tree-sitter fallback not triggered.
- **Fix (2026-04-24):** `workspace/symbol` bypasses cold-start budget; falls back to tree-sitter in ~1s. Test: `workspace_symbol_skips_cold_start_retry_budget`.
- **Status:** Mitigated. Still watch for `/mcp` reconnects on large projects during rust-analyzer reindex.

### #7 — BUG-049: `find_symbol` hangs ~90s when kotlin-lsp hits "Multiple editing sessions"

- **Symptom:** Any kotlin `find_symbol` hung ~90s when another editor held the kotlin-lsp workspace lock.
- **Root cause:** `detect_fatal_stderr` did not fast-fail on kotlin-lsp's multi-session error; cold-start budget ran to exhaustion.
- **Fix (2026-04-24):** Per-language 8s hard budget in JoinSet; `detect_fatal_stderr` fast-fails on multi-session error. Tests: `detect_fatal_stderr_flags_kotlin_multi_session`, `detect_fatal_stderr_ignores_benign_lines`.
- **Status:** Mitigated. First call still pays up to ~8s. Pin `path=` to a non-Kotlin file to skip kotlin-lsp entirely.

### #8 — BUG-050: `edit_file` batch silently injects new fn mid-existing fn body

- **Symptom:** Batch `edit_file` with multi-line `new_string` containing `fn ` spliced a new function into an existing function's body instead of placing it between functions.
- **Root cause:** `guard_structural_rewrite` only checked `old_string` for definition keywords; single-line `old_string` + multi-line `new_string` with a new symbol bypassed the gate.
- **Fix (2026-05-09):** Guard also rejects multi-line `new_string` containing definition keywords. Single-line `new_string` with `fn` token (e.g. comment) still allowed.
- **Status:** Fixed.

### #9 — BUG-051: `edit_code insert-after` places code mid-function when symbol body is truncated

- **Symptom:** `edit_code(action="insert", position="after")` injected code mid-function body, splitting an open `assert!()` and breaking compilation.
- **Root cause (primary):** `editing_end_line` early-returned on `has_syntax_errors`, falling back to LSP `end_line` which reported the last statement line, not the closing `}`.
- **Root cause (residual):** When AST returned `None` for a top-level symbol with no parent, silent LSP fallback still landed the insert mid-body.
- **Fix (2026-05-02 + 2026-05-09):** AST trusted unconditionally when it finds the symbol. `editing_end_line_strict` returns `Option<u32>`; `None` for no-parent symbols → `RecoverableError` instead of corruption.
- **Status:** Mostly fixed. Parented under-extension residual remains (rare in practice).

### #10 — BUG-052: `RecoverableError` guidance absent from `Display` / `to_string()`

- **Symptom:** `err.to_string()` omitted attached hint/warning/must_follow text; tests asserting `contains("did you mean...")` failed.
- **Root cause:** `guidance` was serialized only into MCP JSON; `Display` only emitted `self.message`.
- **Fix (2026-05-09):** `Display` appends `" — <field_name>: <text>"` when `guidance` is `Some`. MCP JSON unchanged. Tests: `display_includes_hint_text`, `display_includes_warning_text`, `display_includes_must_follow_text`.
- **Status:** Fixed.

### #11 — BUG-054: `symbols(path)` returns silent empty `[]` during LSP cold-start

- **Symptom:** `symbols(path)` returns `{"symbols": []}` for files with known symbols shortly after session start. Resolves after ~30–60s.
- **Root cause (probable):** rust-analyzer returns `Ok([])` during initial indexing rather than `-32800 RequestCancelled`. `Ok([])` treated as valid empty result; tree-sitter fallback not invoked.
- **Workaround:** Retry after ~30–60s, or use `detail_level="full"`.
- **Fix:** Open. Proposed: in `list_overview` single-file branch, retry once + fall back to tree-sitter when LSP returns empty Vec for a non-empty file.
- **Status:** Open.

### #12 — BUG-055: `artifact(create)` leaves orphan file when DB insert fails

- **Symptom:** If `artifact::upsert` failed (e.g. `NOT NULL constraint failed`), the file remained on disk with no DB record, blocking retries with "path exists".
- **Root cause:** Disk write happened before both DB upserts in `crates/librarian-mcp/src/tools/create.rs`.
- **Fix (2026-05-09):** Both DB upserts run first; disk write is last. Test: `create_does_not_leave_orphan_file_when_upsert_fails`.
- **Status:** Fixed.

### #13 — BUG-056: `artifact(update, patch={params: ...})` silently drops `params`

- **Symptom:** `patch={params: {...}}` passed to `artifact(update)` was silently ignored; augmentation params unchanged. `commit_refresh=true` still fired, recording a stale refresh timestamp.
- **Root cause:** `UpdatePatch` struct had no `params` field; serde dropped unknown keys.
- **Workaround:** `artifact_augment(id, merge=true, params={...})` to update params, then `artifact(update, commit_refresh=true)` separately.
- **Fix:** `params` added to `UpdatePatch`, routed through `augmentation::merge_params`. Commit `e406218`. Both prompt surfaces updated.
- **Status:** Fixed.

### #14 — LIMIT-001: `call_graph direction=callees` has no tree-sitter fallback

- **Symptom:** `call_graph(direction="callees")` returns `RecoverableError` when LSP `callHierarchy` is unavailable.
- **Root cause / design:** `LspClientOps::references()` finds callers (refs *to* a symbol); finding callees requires parsing the symbol body and chasing each call via `goto_definition` — no tree-sitter path exists yet.
- **Workaround:** Activate a language server. `direction="callers"` has a full tree-sitter fallback.
- **Status:** By design. Revisit if a "callees via AST body walk" helper is added.
## History

### 2026-05-09 — Tracker bootstrapped

Created from the `audit_issues` archetype via `librarian(tracker_design)`.
Replaces the static `docs/issues/INDEX.md` shipped earlier on `experiments`
(commit b3b063b). Inaugural issue (#1) filed for the `edit_markdown` H1
footgun observed during this very bootstrap session.


### 2026-05-11 — Bulk migration from `docs/TODO-tool-misbehaviors.md`

Entries #2–#14 migrated from the deprecated `docs/TODO-tool-misbehaviors.md`.
That file retains only its deprecation banner. All active refs updated to point here.
