# Tool Misbehaviours — Living Log

**Reader:** developers (including Claude) working on codescout's own MCP tools.

**Purpose:** catch unexpected behaviour from codescout's tools (`edit_file`, `replace_symbol`, `find_symbol`, `semantic_search`, `edit_markdown`, `run_command`, …) *before* it gets normalised as "just how the tool works". Log it while the context is fresh; fix it or let it inform future work.

**Scope:** bugs, silent failures, misleading errors, corrupt output. Not feature requests — those go to GitHub issues.

## Before starting any task

Skim the "Mitigated quirks" section below so you know which sharp edges still exist. If you hit a new one, add an entry **before continuing**.

## Adding an entry

Use the template at the bottom. Keep it one entry per observation, even if you think it's a duplicate — historians decide that later. Mention commits and tests where possible.

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

## Open

### BUG-048 — `find_symbol` hangs 60s during LSP cold-start indexing

**Date:** 2026-04-24
**Severity:** High (core navigation tool unusable after `/mcp` reconnect on large projects)
**Status:** Fixed (2026-04-24) — `workspace/symbol` now bypasses the cold-start retry budget in `src/lsp/client.rs`. Unit test: `workspace_symbol_skips_cold_start_retry_budget`.

**Repro during initial investigation:**

```
find_symbol(name="open_repo")                       # timed out after 60s
find_symbol(name="open_repo", substring_matching=false)  # returned in <1s
```

**Red herring:** `substring_matching` is NOT a `find_symbol` parameter. The schema accepts only `query`/`symbol`/`name`/`path`/`kind`/`include_body`/`depth`/`detail_level`/`offset`/`limit`/`scope`. Unknown kwargs are silently dropped by MCP, so both calls executed the identical code path. The second call returning instantly was rust-analyzer finishing indexing in the background between calls, not the extra kwarg.

**Actual root cause:**

1. After `/mcp` reconnect on a 587-file Rust project, rust-analyzer started reindexing.
2. `workspace/symbol` requests during indexing return `-32800 RequestCancelled` — the response only becomes answerable once the *whole* project is indexed (minutes).
3. `LspClient::request` (`src/lsp/client.rs`) gives every idempotent method a cold-start retry budget of **10 × 3 s linear backoff + 30 s per-attempt timeout** — far more than the 60 s MCP tool timeout.
4. Per-file LSP ops (`textDocument/documentSymbol`, `hover`, `definition`, `references`) answer lazily per file, so `list_symbols`/`hover`/`goto_definition`/`find_references` stayed fast while `find_symbol` blocked.
5. Tree-sitter fallback in `find_symbol` runs only after `workspace_symbols` returns, so the retry loop had to drain before it could kick in.

**Fix:** new helper `uses_cold_start_retry_budget(method)` in `src/lsp/client.rs` returns `false` for `workspace/symbol`. That method now uses the warm retry budget (3 × 300 ms ≈ 1.2 s) even inside the cold-start window, so `find_symbol` fails over to the tree-sitter walker within ~1 s instead of hanging for 60 s. `workspace/symbol` remains in `is_idempotent_lsp_method` so the warm-path retry still engages.

**Mitigation for users on older builds:** pin `path` to a file or directory — that takes the per-file `document_symbols` path, which is unaffected.
### BUG-049 — `find_symbol` hangs ~90s when kotlin-lsp hits "Multiple editing sessions"

**Date:** 2026-04-24
**Severity:** High (core navigation tool unusable on mixed-language projects whenever another editor/agent is holding the kotlin-lsp workspace lock)
**Status:** Fixed (2026-04-24)

**Repro:** From a second codescout instance (or while IntelliJ / another editor is open on the same Kotlin project):

```
find_symbol(query="some_name", include_body=true, limit=1)   # run from backend-kotlin
```

Hangs until the MCP client gives up (~60 s from the agent's view; server-side the task keeps running for another ~30 s before SIGINT).

**Observed in logs (`1e88` instance, `diagnostic-1e88.log` lines 3065-3090):**

```
17:00:50.940  WARN  Mux startup failed for kotlin, falling back to direct LSP
17:00:50.941  INFO  Starting LSP server: kotlin-lsp
17:00:55.548  INFO  LSP initialize cancelled, retrying 1/5: backend-kotlin
17:00:55.550  WARN  lsp_stderr: com.jetbrains.lsp.implementation.LspException:
                    Multiple editing sessions for one workspace are not supported yet
…heartbeats, no tool_done…
17:02:27.432  user-cancel received
```

**Root cause (two compounding issues):**

1. **`find_symbol` fast path waits for every language.** Its `JoinSet` fan-out (`src/tools/symbol/find_symbol.rs`) spawns one `get_or_start + workspace_symbols` task per detected language and awaits **all** of them via `join_next`. backend-kotlin contains Rust/JS/Bash/Python/Kotlin source, so one pathological LSP (kotlin) blocks the whole tool call.
2. **`initialize()`'s fatal-stderr check only fired on `-32800`.** The original check in `LspClient::initialize` looked for `Multiple editing sessions` only inside the `Err(e) if … "-32800"` arm. In practice the first attempt got -32800, the stderr exception arrived ~2 ms *after* that arm dispatched the retry, and subsequent attempts hit a closed pipe / timeout (not -32800), so the fatal pattern was never checked again. Combined with `MAX_INIT_RETRIES=5 × INIT_RETRY_DELAY_MS=3000 ms` linear backoff and a 300 s per-attempt budget for JVM servers, one doomed kotlin-lsp could burn the entire MCP ceiling on its own.

**Fix (this commit):**

- **`src/tools/symbol/find_symbol.rs` — per-language hard budget.** Each JoinSet task is wrapped in `tokio::time::timeout(PER_LANG_BUDGET = 8 s, …)`. On timeout that language yields an empty result and the tree-sitter fallback still runs if every language produces nothing. Any future pathological LSP state is time-boxed instead of taking the whole tool down.
- **`src/lsp/client.rs` — `detect_fatal_stderr` + `fatal_stderr_hint`.** Extracted a pure helper that scans the buffered stderr for `Multiple editing sessions`. `initialize()` now calls it before **every** attempt and on **every** error arm (not only `-32800`), so the common race where kotlin-lsp crashes mid-init and the next send hits a closed pipe is now detected and fast-failed.
- Tests: `detect_fatal_stderr_flags_kotlin_multi_session`, `detect_fatal_stderr_ignores_benign_lines`.

**Workaround on older builds:** close the other editor / stop the other codescout instance before calling `find_symbol` from a Kotlin project; or pin `path=` to a non-Kotlin file so the per-file `document_symbols` path is used instead.

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
