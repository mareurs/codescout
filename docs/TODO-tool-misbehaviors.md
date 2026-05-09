# Tool Misbehaviours — Living Log [DEPRECATED 2026-05-09]

> **Going forward, all new tool quirks and misbehaviors are tracked as bug
> files in `docs/issues/<date>-<slug>.md` using `docs/issues/_TEMPLATE.md`.**
> Do not add new `BUG-XXX` entries below — open a bug file instead.
> Existing entries stay here for historical reference; they will be
> migrated in a future bulk pass.

**Reader:** developers (including Claude) working on codescout's own MCP tools.

**Purpose:** catch unexpected behaviour from codescout's tools (`edit_file`, `replace_symbol`, `find_symbol`, `semantic_search`, `edit_markdown`, `run_command`, …) *before* it gets normalised as "just how the tool works". Log it while the context is fresh; fix it or let it inform future work.

**Scope:** bugs, silent failures, misleading errors, corrupt output. Not feature requests — those go to GitHub issues.

## Before starting any task

Skim the "Mitigated quirks" section below so you know which sharp edges still exist. If you hit a new one, add an entry **before continuing**.

## Adding an entry

Use the template at the bottom. Keep it one entry per observation, even if you think it's a duplicate — historians decide that later. Mention commits and tests where possible.

## Known design limitations (not bugs)

### LIMIT-001 — `call_graph direction=callees` requires LSP callHierarchy; no tree-sitter fallback

- **Observed:** 2026-05-01 (during Task 6 implementation)
- **Component:** `src/tools/symbol/call_edges/resolver.rs` — `resolve_via_ts`
- **Severity:** Low (expected limitation, clearly communicated to caller)
- **What happens:** when `prepare_call_hierarchy` returns `None` (language server not running or language not supported) and the caller requests `direction=callees`, `resolve_one_hop` returns a `RecoverableError` instead of edges.
- **Why:** `LspClientOps::references()` finds all locations that *reference* a symbol — i.e., calls *to* it. To find *callees* (calls made *from* the symbol's body) we would need to parse the symbol's body and enumerate every call expression inside it. That requires knowing the symbol's byte range in the file, which is only available via LSP document symbols or a full AST walk. Without LSP we have no reliable way to bound the symbol body.
- **Workaround:** activate a language server for the file. `direction=callers` has a full tree-sitter fallback.
- **Status:** By design. Revisit if a "find callees via AST body walk" helper is added in a future task.

## Mitigated quirks (live caveats)

These are fixed in the happy path but still have edge cases worth knowing about. Full write-ups in `docs/archive/bug-reports/2026-03-to-2026-04-tool-misbehaviors.md`.

### BUG-030 — `replace_symbol` on `mod tests` can eat an adjacent function body

- **Mitigation (2026-03-20):** `validate_symbol_position` guard detects stale LSP positions and surfaces a `RecoverableError`. Happy path works.
- **Still watch for:** stale LSP positions on large files mid-edit — if `replace_symbol` ever reports "symbol not found" after a big write, `/mcp` reconnect re-indexes.

### BUG-032 — `remove_symbol` can leave orphaned `impl` block code after enum removal

- **Mitigation (2026-03-20):** same `validate_symbol_position` guard catches the stale-position case.
- **Still watch for:** adjacent/nested `impl Trait for Type` next to inherent `impl Type` — range computation may still grab the wrong brace set (also noted on BUG-037). Workaround: `create_file` for those cases.

### BUG-021 — partial state after parallel `edit_file` calls (by design)

- **Crash fixed** by rmcp 1.2.0 cancellation-race fix.
- **Still applies:** never dispatch parallel write tool calls. Two independent writes have no transaction semantics; if one is denied by the permission dialog and the other succeeds, files end up half-applied.

### BUG-048 — `find_symbol` hangs 60s during LSP cold-start indexing

- **Mitigation (2026-04-24):** `workspace/symbol` bypasses the cold-start retry budget in `src/lsp/client.rs` via `uses_cold_start_retry_budget`; `find_symbol` falls over to tree-sitter in ~1 s. Test: `workspace_symbol_skips_cold_start_retry_budget`.
- **Still watch for:** `/mcp` reconnects on large projects (rust-analyzer reindex). Per-file paths (`list_symbols`, `hover`, `goto_definition`, `references`) remain unaffected.

### BUG-049 — `find_symbol` hangs ~90s when kotlin-lsp hits "Multiple editing sessions"

- **Mitigation (2026-04-24):** per-language 8 s hard budget in `src/tools/symbol/find_symbol.rs` JoinSet; `detect_fatal_stderr` in `src/lsp/client.rs` fast-fails kotlin-lsp's multi-session error on every init attempt. Tests: `detect_fatal_stderr_flags_kotlin_multi_session`, `detect_fatal_stderr_ignores_benign_lines`.
- **Still watch for:** another editor/agent holding the kotlin-lsp workspace lock — first call still pays up to ~8 s before falling back. Pin `path=` to a non-Kotlin file to skip kotlin-lsp entirely.

### BUG-050 — `edit_file` batch can silently inject mid-function when `new_string` contains `fn `

- **Observed:** 2026-04-29
- **Tool:** `mcp__codescout__edit_file` (batch mode with two edits)
- **Severity:** High (silent corruption of source)
- **What I did:** Batch `edit_file` against `crates/librarian-mcp/src/catalog/events.rs`. Second edit's `new_string` contained the literal `fn ` (declaring a new private helper function). The `old_string` for that second edit was a single-line match.
- **Expected:** either the edit applies cleanly between existing functions, or the multi-line-fn guard rejects it.
- **What happened:** the tool accepted the edit (no error) but injected the replacement text **mid-function**, splicing into the body of an existing fn (`insert`) and corrupting it. Caller noticed via subsequent `cargo build` failure; recovered by re-issuing the edit with surrounding context.
- **Root cause (confirmed 2026-05-09):** `guard_structural_rewrite` returned `Ok(())` on the very first line when `old_string` lacked a newline, and `find_def_keyword` was only ever called against `old_string`. A single-line `old_string` paired with a multi-line `new_string` that introduced a new symbol slipped through the gate entirely — the new function got spliced into whatever surrounded the anchor match.
- **Fix applied (2026-05-09):** `guard_structural_rewrite` now also rejects edits where a multi-line `new_string` contains a definition keyword for the file's language — covers both "rewriting an existing symbol" (old check) and "introducing a new symbol" (new check). Single-line `new_string` containing a `fn` token (e.g. comment edits) remains allowed. Tests: `batch_edit_blocks_new_symbol_introduction_via_new_string`, `single_edit_blocks_new_symbol_introduction_via_new_string`, `singleline_new_string_with_fn_token_still_allowed` in `src/tools/edit_file/tests.rs`.
- **Status:** Fixed
### BUG-051 — `edit_code insert after`: code injected mid-function body when symbol body is truncated in display

- **Observed:** 2026-05-02
- **Tool:** `mcp__codescout__edit_code` (`action="insert"`, `position="after"`)
- **Severity:** High (silent source corruption — compiles to wrong code or doesn't compile)
- **Exact call:**
  ```json
  {
    "symbol": "find_unique_symbol_by_name_path_errors_on_ambiguous_name",
    "path": "src/tools/symbol/tests.rs",
    "action": "insert",
    "position": "after",
    "body": "\n#[test]\nfn find_unique_symbol_by_name_path_suggests_leaf_matches() { ... }"
  }
  ```
- **Reproducing commit:** `629b7eae` (experiments branch) — file `src/tools/symbol/tests.rs`, function `find_unique_symbol_by_name_path_errors_on_ambiguous_name` was ~82 lines long (lines 1081–1162+ in that snapshot).
- **Preceding `symbols` call that revealed the truncation:**
  ```json
  {
    "name": "find_unique_symbol_by_name_path_errors_on_ambiguous_name",
    "path": "src/tools/symbol/tests.rs",
    "include_body": true
  }
  ```
  Returned `end_line: 1162` and a body that was cut off mid-`assert!()` at that same line — the function's closing `);` and `}` were NOT shown.
- **Expected:** new test function inserted after the closing `}` of the target function.
- **What happened:** code inserted at line 1162 — mid-body, splitting an open `assert!(err_str.contains("nonexistent"),` — corrupting both the old function and making the file fail to compile.
- **Root cause (confirmed 2026-05-02):** `editing_end_line` in `src/symbol/edit.rs` had an early return when `has_syntax_errors` was true, falling back to LSP's `end_line`. During mid-session editing (prior edits leave broken syntax), LSP frequently reports the last *statement* line rather than the closing `}`. Insertion used this short value as anchor, landing inside the function.
- **Fix applied (2026-05-02):** Removed the syntax-error early return. AST is run unconditionally and trusted when it finds the symbol (same as on a clean file). `max(ast, lsp)` was considered but rejected — it regresses BUG-029 when syntax errors coexist with LSP over-extension. Two regression tests added in `src/tools/symbol/tests.rs`: `editing_end_line_with_syntax_errors_uses_ast_not_lsp_fallback` and `editing_end_line_syntax_errors_do_not_regress_lsp_overextend`.
- **Residual closed (2026-05-09):** Even after the AST-trust fix, when AST extraction itself succeeded but `find_ast_end_line_in` returned `None` (severely broken parse, ambiguous match, etc.) `editing_end_line` silently fell back to LSP's `end_line`. For top-level symbols with no parent in the symbol tree, the parent-clamp safety net in `do_insert` couldn't recover. Added `editing_end_line_strict` (returns `Option<u32>`, `None` on any AST resolution failure) and wired it into `do_insert`'s "after" branch: when AST cannot pinpoint the end AND the symbol has no parent, the call now returns a `RecoverableError` with actionable guidance instead of corrupting the source. When a parent exists, the existing lenient path runs and the parent clamp keeps the result bounded — preserving the BUG-029/036 recovery path. Tests added: `editing_end_line_strict_returns_none_when_ast_cannot_find_symbol` and `editing_end_line_strict_returns_some_when_ast_finds_symbol` in `src/tools/symbol/tests.rs`; `insert_code_after_refuses_when_ast_fails_and_no_parent_clamp` in `tests/symbol_lsp.rs`.
- **Smaller residual (documented, not fixed):** When the file has syntax errors AND the symbol has a parent AND LSP under-extends `end_line` into the symbol's body, both the AST-trust fix and the strict guard still allow the lenient `editing_end_line` fallback (since the parent clamp can't detect under-extension). The parent clamp protects against over-extension only. This last sliver is rare in practice — most under-extension cases are caught by the 2026-05-02 AST-trust fix.
- **Status:** Mostly fixed (top-level symbol path: Fixed; parented under-extension path: residual)
### BUG-052 — `RecoverableError` guidance/hint not included in `Display` / `to_string()`

- **Observed:** 2026-05-02
- **Component:** `src/tools/core/types.rs` — `impl std::fmt::Display for RecoverableError`
- **Severity:** Low (test footgun; no runtime data loss — MCP JSON output is correct)
- **What I did:** wrote a test asserting `result.unwrap_err().to_string().contains("did you mean ...")` where `"did you mean ..."` was set as the `hint` in `RecoverableError::with_hint(message, hint)`.
- **Expected:** `to_string()` to include both the message and the hint text, as the LLM sees both.
- **What happened:** test failed — `to_string()` emitted only `self.message`; the `guidance` field (Hint/Warning/MustFollow) was serialized only in the MCP JSON response body, invisible to `Display`.
- **Initial workaround (2026-05-02):** moved suggestions into the `message` string itself (`"symbol not found: X — did you mean 'Y'?"`), kept the `hint` for static usage guidance.
- **Proper fix (2026-05-09):** `Display` now appends attached guidance as `" — <field_name>: <text>"` when `guidance` is `Some(_)`, surfacing hint/warning/must_follow content in `to_string()`. The MCP JSON output is unchanged (it uses serde, not Display) so no double-rendering. Audit confirmed no existing test asserts exact-equality on `to_string()` for `RecoverableError`; only the canary test `recoverable_error_display_shows_message` did, and it has been updated to assert the new contract. Tests added: `display_includes_hint_text`, `display_includes_warning_text`, `display_includes_must_follow_text`, `display_no_guidance_just_message` in `src/tools/core/tests.rs`.
- **Status:** Fixed
### BUG-055 — `artifact(create)` leaves orphan file on disk when DB insert fails

**When:** `artifact(create)` is called but the `upsert` fails (e.g. `NOT NULL constraint failed: artifact.repo` during v6 pre-migration state).

**Got:** File written to disk at the target path, no DB record created. Subsequent `artifact(create)` calls for the same path fail with `"path exists"` even though the artifact is not in the DB.

**Root cause (confirmed 2026-05-09):** `crates/librarian-mcp/src/tools/create.rs::call` wrote the file via `std::fs::write` *before* `artifact::upsert` (and the optional `augmentation::upsert`). Any DB error after the disk write left the file orphaned and blocked retry on the `if full.exists()` gate.

**Fix applied (2026-05-09):** Reordered `call` so the disk write is the last side effect — content is computed and `file_sha256` derived from in-memory bytes, both `artifact::upsert` and `augmentation::upsert` run first, and only after both succeed does `std::fs::write(&full, &content)` happen. A DB error now leaves the disk untouched, so retry isn't blocked. The remaining (much rarer) failure mode — DB rows committed but the file write fails — is benign because `upsert` is idempotent: a retry rewrites the row and writes the file. Test: `create_does_not_leave_orphan_file_when_upsert_fails` in `crates/librarian-mcp/src/tools/create.rs::tests` installs a `BEFORE INSERT` trigger that always raises, then asserts no file remains after the call returns Err.

**Status:** Fixed
### BUG-056 — `artifact(update, patch={params: ...})` silently drops `params`

**When:** Caller follows the documented refresh pattern and passes `patch={params: {entries: [...]}}` to `artifact(update)`.

**Got:** `params` is silently ignored — `UpdatePatch` struct has no `params` field, so serde drops it. The augmentation params remain unchanged. `commit_refresh=true` still fires, recording a refresh with stale params.

**Probable cause:** `params` belongs to the `artifact_augmentation` table, not `artifact`. The `update` tool only patches the artifact row.

**Workaround:** Use `artifact_augment(id, merge=true, params={...})` to update params, then call `artifact(update, commit_refresh=true)` separately to record the refresh timestamp.

**Fixed:** `params` added to `UpdatePatch`, routed through `augmentation::merge_params`. Commit `e406218` on `experiments`. Both prompt surfaces updated.
## Archive

Fixed / superseded entries: `docs/archive/bug-reports/`.


### BUG-047 — `ResilientStdin`: spinning `Poll::Pending` floods log files to 268 GB

**Date:** 2026-04-22
**Severity:** Critical (disk exhaustion)
**Status:** Fixed (2026-04-22)

**What happened:**
The codescout server for the `mirela/deployment` project hit a busy-loop on stdin
`EAGAIN` and logged every iteration at `WARN` level with no rate-limiting or size
cap. Two log files (`.codescout/debug.log` and `.codescout/diagnostic-*.log`)
grew to **268 GB each** before the disk was exhausted.

**Root cause — a regression introduced by BUG-033's fix:**

The `ResilientStdin` wrapper (added on 2026-03-21 to absorb transient `EAGAIN` so
rmcp would not close the stdio transport) had two compounding flaws:

1. **Spinning `Poll::Pending` (CPU busy-loop).** When `tokio::io::Stdin::poll_read`
   returned `Ready(Err(WouldBlock))`, the waker was *not* registered with epoll
   (the inner returned `Ready`, not `Pending`). To avoid a task hang, the code
   called `cx.waker().wake_by_ref()` before returning `Pending`. `wake_by_ref()`
   immediately re-schedules the task — an external event never fires the waker,
   the task itself does. The task re-polls, gets EAGAIN again, wakes itself again,
   at tokio scheduler rate. `Poll::Pending` requires the waker be called by an
   external event (I/O readiness, timer, channel), not by the pending future
   itself. This is the canonical "spinning pending" async anti-pattern.

2. **`WARN`-level log inside the spin.** A `tracing::warn!` fired on every poll,
   so two non-blocking tracing writers (debug.log at DEBUG, diagnostic-*.log at
   INFO) each received millions of lines per second.

3. **No size-based log rotation.** `rotate_logs()` in `src/logging.rs` ran only at
   startup and capped by count (3 backups), not size. Runtime floods had no gate.

**Fix (2026-04-22):**

- **`src/server.rs` — `ResilientStdin` truthful `Pending`.** On EAGAIN, arm a 1ms
  `tokio::time::Sleep` and poll it; this registers the waker via tokio's timer
  reactor so the task resumes after the delay, not immediately. Struct gains
  `backoff: Option<Pin<Box<Sleep>>>`. `WARN` → `TRACE`.
- **`src/logging.rs` — `SizeRotatingFile` defense-in-depth.** New `Write` wrapper
  caps each log at 50 MiB with 3 numbered backups, rotating on the non-blocking
  log-writer thread. Both `debug.log` and `diagnostic-*.log` routed through it.
  Guards against any future runtime log-flooding bug, not only this one.
- Tests: `size_rotating_file_rotates_on_cap_exceeded`,
  `size_rotating_file_caps_total_growth_at_keep_plus_one`,
  `size_rotating_file_single_write_under_cap_does_not_rotate`,
  `numbered_appends_suffix`, plus `resilient_stdin_absorbs_would_block` updated
  to mirror the new backoff state machine.

**Upstream still pending:** `rmcp::transport::AsyncRwTransport::receive()` converts
*any* IO error to `None`. It should distinguish transient (`WouldBlock`,
`Interrupted`) from fatal (`BrokenPipe`, EOF). File an issue with rmcp so this
wall is no longer needed.

**Reproduction hint:**
Observed in practice: Claude Code's Node.js runtime briefly sets the stdin pipe
`O_NONBLOCK` under I/O pressure. If that state persists past a single poll cycle,
the spin begins.

**Observed at:** `mirela/deployment` project, 2026-04-22.
### BUG-054 — `symbols(path)` returns silent empty `[]` during LSP cold-start indexing

- **Observed:** 2026-05-07
- **Tool:** `symbols` (path-only / `list_overview` dispatch)
- **Severity:** Medium (silent — agent thinks file has no symbols, abandons it)
- **What I did:** Called `symbols("src/tools/mod.rs")`, `symbols("src/agent/mod.rs")`, `symbols("src/tools/output.rs")` shortly after session start.
- **Expected:** Module declarations and other top-level symbols returned.
- **What happened:** All three returned `{"file": "<path>", "symbols": []}`. Calling the same tool with `detail_level="full"` returned the full symbol set. Re-running the compact call ~1 minute later also returned the full set. The empty result was non-deterministic and tied to LSP cold-start.
- **Probable cause:** rust-analyzer responds to `textDocument/documentSymbol` with `Ok([])` (success, empty list) during initial indexing rather than `-32800 RequestCancelled`. `is_idempotent_lsp_method` would have triggered the cold-start retry budget on a `RequestCancelled`, but `Ok([])` is treated as a valid empty result and propagated to the caller. Tree-sitter fallback (which would have populated module decls) is not invoked because the LSP call did not error.
- **Workaround:** Retry the call after ~30–60s; or pass `detail_level="full"` (no different code path on this — the bug appears to require LSP warmup, and by the time the second call runs LSP is warm).
- **Fix:** Open. Fix-idea: in `list_overview`'s single-file branch, when `client.document_symbols(...)` returns an empty Vec for a file with non-empty source AND tree-sitter detects the language, retry once after a short delay; if still empty, fall over to tree-sitter symbol extraction OR surface a "LSP returned no symbols — may still be indexing; retry" hint instead of an empty array.
- **Status:** Open

## Template for new entries

```markdown
### BUG-XXX — <tool>: <one-line symptom>

- **Observed:** YYYY-MM-DD
- **Tool:** `tool_name`
- **Severity:** Low / Medium / High (Low = cosmetic; High = data loss or crash)
- **What I did:** minimal repro, with actual args.
- **Expected:** …
- **What happened:** …
- **Probable cause / root cause:** (leave blank if unknown — investigation can be a separate entry)
- **Workaround:** …
- **Fix:** commit or test name, or "open"
- **Status:** Open / Fixed (YYYY-MM-DD, commit `abcdef0`) / Mitigated
```
