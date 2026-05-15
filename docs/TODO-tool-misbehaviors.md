# Tool Misbehaviours — Living Log

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

**Depth-≥2 corollary (observed 2026-05-15 via nav-eval round 2/3):** Because the
tree-sitter fallback for callees returns an empty edge list (rather than tracing
calls in the source), BFS at `max_depth ≥ 2` only yields edges from the seed
node. Non-seed nodes hit `prepareCallHierarchy = None` (rust-analyzer does not
serve call-hierarchy at a function's *definition* position for fixtures whose
manifest isn't a workspace member), the resolver returns `RecoverableError`,
and (since round-3 fix in `src/tools/symbol/call_graph/mod.rs::one_hop`) BFS
silently skips that hop and continues with whatever else is queued. The
nav-eval case `C-11` (`a → b → c → a` cycle, depth=5) is the canonical
regression watchdog for this gap — it stays `SILENT_WRONG` until a real
tree-sitter fallback for callees ships.

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
- **Probable cause:** the structural-edit guard fires only when `old_string` spans multiple lines containing `fn`. A single-line `old_string` with `fn ` in `new_string` slips through the gate.
- **Workaround:** for any new-function injection, use a multi-line `old_string` with explicit anchor lines on both sides, or use `insert_code`/`replace_symbol` instead of `edit_file`.
- **Fix:** `guard_structural_rewrite` now scans `new_string` for def-keywords when `old_string` is single-line but `new_string` is multi-line. Single-line→single-line literal substitutions remain unaffected.
- **Regression:** `src/tools/edit_file/tests.rs::batch_edit_rejects_single_line_old_with_def_keyword_in_new_string` (plus existing `batch_edit_blocks_structural_rewrite` and the fixture `tests/fixtures/edit-eval-rust/src/bug050_repro.rs` at commit `43b7cb45`).
- **Status:** Fixed (2026-05-15)
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
- **Probable cause:** two possibilities (root cause unconfirmed):
  1. `edit_code` uses the body's last *displayed* line as the insertion anchor rather than the LSP-authoritative `end_line`. Body display is capped at a max-length (same truncation visible in the `symbols` output), so insertion lands at the display cutoff, not the real end.
  2. LSP itself reported a wrong `end_line` that coincides with the display truncation point — both the display and the insertion used the same wrong value.
- **Workaround:** before any `insert after`, verify the symbol's `end_line` equals its real closing `}` by reading surrounding lines with `read_file(path, start_line=end_line-2, end_line=end_line+2)`. If the body display is truncated (ends mid-expression), do NOT use `insert after` — use `run_command` with a Python script for safe text replacement instead.
- **Fix:** investigate whether `edit_code` anchor comes from display length or LSP; add a guard that refuses `insert after` when the symbol's body appears truncated.
- **Root cause (confirmed 2026-05-02):** `editing_end_line` in `src/symbol/edit.rs` had an early return when `has_syntax_errors` was true, falling back to LSP's `end_line`. During mid-session editing (prior edits leave broken syntax), LSP frequently reports the last *statement* line rather than the closing `}`. Insertion used this short value as anchor, landing inside the function.
- **Fix applied (2026-05-02):** Removed the syntax-error early return. AST is run unconditionally and trusted when it finds the symbol (same as on a clean file). `max(ast, lsp)` was considered but rejected — it regresses BUG-029 when syntax errors coexist with LSP over-extension. Two regression tests added in `src/tools/symbol/tests.rs`: `editing_end_line_with_syntax_errors_uses_ast_not_lsp_fallback` and `editing_end_line_syntax_errors_do_not_regress_lsp_overextend`.
- **Residual (documented, not fixed):** When the file has syntax errors AND tree-sitter's error recovery fails to find the function boundary (`find_ast_end_line_in` returns `None`), the function still falls back to `sym.end_line` — the original failure path. In this worst-case scenario (badly broken parse tree), the bug can still manifest.
- **Status:** Partially fixed

### BUG-052 — `RecoverableError` guidance/hint not included in `Display` / `to_string()`

- **Observed:** 2026-05-02
- **Component:** `src/tools/mod.rs` — `impl std::fmt::Display for RecoverableError`
- **Severity:** Low (test footgun; no runtime data loss — MCP JSON output is correct)
- **What I did:** wrote a test asserting `result.unwrap_err().to_string().contains("did you mean ...")` where `"did you mean ..."` was set as the `hint` in `RecoverableError::with_hint(message, hint)`.
- **Expected:** `to_string()` to include both the message and the hint text, as the LLM sees both.
- **What happened:** test failed — `to_string()` emits only `self.message`; the `guidance` field (Hint/Warning/MustFollow) is serialized only in the MCP JSON response body, invisible to `Display`.
- **Reproducing call:**
  ```rust
  let e = RecoverableError::with_hint("symbol not found: Foo/bar", "Did you mean 'Baz/bar'?");
  assert!(e.to_string().contains("Did you mean")); // FAILS
  ```
- **Fix applied (same session):** moved suggestions into the `message` string itself (`"symbol not found: X — did you mean 'Y'?"`), kept the `hint` for static usage guidance. Tests now check `err_str.contains("did you mean")` against the message.
- **Broader implication:** any code that checks `RecoverableError` content via `anyhow::Error::to_string()` or `Display` only sees the message. Tests for hint/warning/must_follow content must either downcast (`err.downcast_ref::<RecoverableError>()` + `.hint()`) or put the asserted text in the message instead.
- **Status:** Fixed by convention (2026-05-02) — no `Display` change made; document pattern for future test authors.

### BUG-053 — `semantic_search` MCP server panics on UTF-8 multi-byte char near byte 47 of result preview

**Symptom:** Calling `semantic_search` via MCP returns `RpcError` / "MCP server closed stdout"; subprocess exits via SIGABRT mid-tool-call; subsequent calls fail with broken pipe.

**Trigger:** A returned chunk's first line contains a non-ASCII char (`→`, `—`, smart quotes, accented letters) crossing byte index 47.

**Root cause:** `src/tools/semantic/semantic_search.rs:491` did `&first_line[..47]` — a byte-index slice that panics if byte 47 is mid-UTF-8 sequence.

**Fix (2026-05-07):** Use `is_char_boundary` to floor the slice end to the nearest valid char boundary; also count chars (not bytes) for the >50 threshold.

**Lessons:** Any tool that builds string previews via byte-slice `&s[..N]` is a panic waiting to happen on UTF-8. Audit other `[..N]` usages in formatter code paths.## Archive

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
### BUG-054 — `edit_code action="replace"` on trait-method body appended stray `}`

- **Observed:** 2026-05-15
- **Tool:** `edit_code` (action=replace, symbol=`impl Tool for ReadMarkdown/format_compact`)
- **Severity:** Medium (compile-broken file after edit; not data loss)
- **What I did:** Replaced the body of `format_compact` inside `impl Tool for ReadMarkdown` in `src/tools/markdown/read_markdown.rs`. New body was multi-branch (CONTENT/MAP) with internal `return` statements.
- **Expected:** Method body swapped, surrounding `impl` block braces preserved.
- **What happened:** Replacement produced one extra `}` at the end — the impl-block closing brace was included in the symbol range and got re-emitted by the new body, leaving an unbalanced file.
- **Probable cause:** symbol range computation for methods inside an `impl` block may include the trailing closer when the body ends near the impl boundary. Related to BUG-030 / BUG-032 (range over-capture) but on `edit_code` not `replace_symbol`.
- **Workaround:** sanity-check brace balance after `edit_code action=replace` on method bodies; fix via `edit_file` for the single stray brace.
- **Fix:** open
- **Status:** Open

### BUG-055 — `edit_code action="replace"` strips preceding doc comment when `new_body` omits it

- **Surfaced by:** edit_code eval R-08 (`tests/fixtures/edit-eval-rust/src/replace_doc_adj.rs`), rounds 1–6.
- **Symptom:** `edit_code(action="replace", body="pub fn documented() -> &'static str {\n    \"after\"\n}")` against
  ```rust
  /// Doc that lives immediately above the target with no blank line.
  pub fn documented() -> &'static str { "before" }
  ```
  removes the `///` doc comment. Post-state has only the new body, no doc.
- **Root cause:** `editing_start_line` (BUG-031 fix) walks back past `///`/`#[...]` decorators above the keyword line so that a `new_body` containing the doc-comment+signature replaces them cleanly (no duplication). But when the LLM passes a `new_body` that intentionally omits decorators (e.g. only changing the body), the walk-back drops the original doc comment — it's inside the replace range but absent from the new body.
- **Fix (2026-05-15):** In `EditCode::do_replace` (`src/tools/symbol/edit_code.rs`), after computing the walk-back `start`, inspect `new_body`'s first non-empty line. If it does NOT start with a decorator (`///`/`//!`/`//`/`#[`/`/**`/`/*`/`@`), narrow `start` forward past any decorator lines (with multi-line `#[...]` bracket tracking) inside the captured range. Result: doc comments / attributes that exist above the symbol but are absent from the new body are preserved; the BUG-031 duplication-prevention path still fires when `new_body` does lead with decorators.
- **Regression sentinel:** `tests/symbol_lsp.rs::replace_symbol_preserves_doc_when_new_body_has_no_doc_comment` (mock-LSP unit) + edit_code eval R-08 (end-to-end via live rust-analyzer).

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
