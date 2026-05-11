---
id: '1ed658441715df5a'
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
- **Root cause:** Stale LSP symbol positions after prior edits; range for `mod tests` extended into adjacent code. Same cluster as BUG-034 (`replace_symbol` adjacent overwrite) and BUG-037 (`insert_code` adjacent splice).
- **Fix (2026-03-20):** `validate_symbol_position` guard detects stale positions and returns `RecoverableError`. Mitigation only — caught the easy cases.
- **Fix (2026-04-21, commit `27848fc`):** Cluster closure via symmetric parent-clamp on both start AND end of the child's edit range in `replace_symbol`/`remove_symbol`/`insert_code` (`clamp_range_to_parent` helper), plus a pre/post-write AST `name_path` set diff in `replace_symbol`: any sibling that existed pre-write but not post-write triggers a rollback with a `RecoverableError` naming the dropped siblings. New helpers `collect_all_name_paths`, `find_ast_name_path`. Tests: 6 pure-logic clamp tests + 2 mock-LSP regressions (`replace_symbol` sibling drop, `insert_code` parent-body clamp). Design spec: `docs/superpowers/specs/2026-04-20-impl-block-symbol-cluster-fix.md`.
- **Status:** Fixed.
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
- **Root cause (residual claim):** When AST returned `None` for a symbol with a parent, the lenient fallback to LSP could still land the insert mid-body. The accompanying comment claimed the parent-clamp at `do_insert` provides a safety net.
- **Fix (2026-05-02 + 2026-05-09):** AST trusted unconditionally when it finds the symbol. `editing_end_line_strict` returns `Option<u32>`; `None` for no-parent symbols → `RecoverableError`.
- **Residual closure (2026-05-11 audit):** The parent-clamp reasoning is incorrect — it bounds against the *parent's* body, not the target sibling's. But the parented residual is no longer reachable in practice: `validate_symbol_position` (name must appear at start_line) and `validate_symbol_range` (rejects when AST shows a later end than LSP) catch every realistic under-extension *before* `do_insert` runs. Verified by attempting to construct a regression test for the parented path — every plausible repro is caught by an earlier guard, including the syntax-errors + duplicate-name-siblings + name_path-mismatch combinations.
- **Status:** Fixed. The flawed parent-clamp comment in `edit_code.rs::do_insert` is harmless given the layered guards; a future cleanup can collapse the parented/no-parent branches into a single refusal once someone wants to touch that code.
### #10 — BUG-052: `RecoverableError` guidance absent from `Display` / `to_string()`

- **Symptom:** `err.to_string()` omitted attached hint/warning/must_follow text; tests asserting `contains("did you mean...")` failed.
- **Root cause:** `guidance` was serialized only into MCP JSON; `Display` only emitted `self.message`.
- **Fix (2026-05-09):** `Display` appends `" — <field_name>: <text>"` when `guidance` is `Some`. MCP JSON unchanged. Tests: `display_includes_hint_text`, `display_includes_warning_text`, `display_includes_must_follow_text`.
- **Status:** Fixed.

### #11 — BUG-054: `symbols(path)` returns silent empty `[]` during LSP cold-start

- **Symptom:** `symbols(path)` returns `{"symbols": []}` for files with known symbols shortly after session start. Resolves after ~30–60s.
- **Root cause:** rust-analyzer (and similar LSPs) return `Ok([])` during initial indexing rather than `-32800 RequestCancelled`. `Ok([])` was treated as a valid empty result; tree-sitter fallback not invoked.
- **Fix (2026-05-08, commit `e885509`):** Single-file branch of `list_overview` falls back to `ast::extract_symbols` when LSP returns empty for a non-empty file with tree-sitter support. Tests: `symbols_overview_falls_back_to_treesitter_when_lsp_returns_empty`, `symbols_overview_returns_empty_for_empty_file_via_treesitter`.
- **Residual:** glob-branch (multi-file) iteration in `list_overview` still trusts LSP empty results silently. Lower priority — single-file is the dominant invocation. Track separately if it bites.
- **Status:** Fixed.
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

### 2026-05-11 — #11 BUG-054 closed (already shipped)

Audited tracker, found commit `e885509` (2026-05-08) had already landed the tree-sitter
fallback in `list_overview`'s single-file branch plus two regression tests. Row #11 was
stale `open` — flipped to `fixed`. Residual gap noted: glob branch still trusts LSP
empty results silently; not promoted to its own row (lower-priority, would need a
new tracker entry only if it bites in practice).

### 2026-05-11 — #9 BUG-051 parented residual closed (layered guards)

Audited the parented under-extension residual claimed in the original entry. Attempted to
construct a regression test exercising the path where `editing_end_line_strict` returns
`None`, the symbol has a parent, and LSP under-extends `end_line`. Every plausible repro is
caught by an earlier guard: `validate_symbol_position` rejects bogus names, and
`validate_symbol_range` (added 2026-03-02 in commit `6d6da19`) rejects whenever AST shows a
later end than LSP. The parent-clamp safety-net claim in `do_insert`'s comment is incorrect
reasoning (the clamp bounds against parent body, not the target sibling's body) but harmless
in practice. Row flipped to fixed.

### 2026-05-11 — #3 BUG-030 elevated to Fixed (cluster commit reference)

Audited remaining mitigated rows. Commit `27848fc` (2026-04-21) explicitly closes the
BUG-030/034/037 cluster with symmetric parent-clamp on both start AND end of nested edit
ranges, plus pre/post-write AST `name_path` set diff that rolls back when a sibling is
silently dropped. The bulk migration imported BUG-030 as `mitigated` based on the earlier
`validate_symbol_position` fix (2026-03-20) without crediting the cluster closure. Row
flipped to fixed, design spec linked via `path`. BUG-032 (#4), BUG-021 (#2), BUG-048 (#6),
and BUG-049 (#7) audited and confirmed correctly classified as mitigated — they sit at LSP
boundaries or architectural surfaces that cannot be closed further without bigger redesigns.
