# Tool Misbehaviors — Observed in the Wild

This is a living document. **Update it whenever you observe unexpected, wrong, or dangerous
behavior from codescout's own tools while working on the codebase.** Each entry should
capture: what you did, what you expected, what actually happened, and a reproduction hint.

---

## Prompt for future sessions

> Before starting any task on this codebase, re-read this file. While you work, watch for
> unexpected tool behavior: wrong edits, corrupt output, silent failures, misleading errors.
> When you find something, **add an entry here before continuing** — even a one-liner is
> enough to capture it while it's fresh. The goal is to build a corpus of real failure cases
> to drive test and UX improvements.

---

### BUG-035 — `edit_markdown`: `compute_section_end` ignores fenced code blocks, splitting sections mid-fence

- **Observed:** 2026-04-03
- **Tool:** `edit_markdown` (replace, remove, insert_after)
- **What happened:** Replacing `## Reading the log` whose body contained a bash code block with `# or tail the most recent:` (a shell comment). `compute_section_end` treated it as a level-1 heading, truncating the section boundary mid-code-block. Subsequent `remove` on the phantom heading swallowed the rest of the file. Subsequent heading lookups failed because the orphaned closing ` ``` ` toggled `parse_all_headings`'s `in_code_block` state, hiding all headings below.
- **Root cause:** `compute_section_end` called `heading_level()` per-line with no fenced-code-block tracking, while `parse_all_headings` (used for heading lookup) correctly tracked `in_code_block`. Inconsistency between the two paths.
- **Fix:** Added `in_code_block` tracking to `compute_section_end`, matching `parse_all_headings` logic. Updated existing test (`heading_inside_code_block_edit`) whose assertion was wrong (passed for the wrong reason — same-level heading coincidence). Added two regression tests: `code_block_heading_different_level_does_not_split_section` and `insert_after_section_with_code_block_heading`.
- **Status:** ✅ Fixed (commit `0701f72`)

## Observed Bugs

### BUG-041 — `insert_code` / `replace_symbol`: stale LSP positions after large edits cause "symbol not found"

**Date:** 2026-04-16

**What I did:** During I7 refactor, inserted ~600 lines of helpers at line 1645 via `insert_code`. Then called `replace_symbol("run_command_inner")` and `insert_code(before: "run_command_inner")`.

**Expected:** Symbol found, edit applied.

**What happened:** Both calls failed — "symbol expected in lines X–Y but not found in file content." The LSP-cached `start_line` for `run_command_inner` was 6 lines off (stale pre-insertion position).

**Root cause:** LSP does not re-index synchronously after write. The `-3` line tolerance in range validation is insufficient when the stale offset is larger or when the fn signature spans multiple lines (fn keyword above cached `start_line`).

**Workaround:** `/mcp` reconnect forces LSP re-index. Always verify with `find_symbol(include_body: false)` before any write following a large insertion — if `start_line` doesn't match actual fn keyword line, reconnect first.

**Fix direction:** Widen pre-validation tolerance, or force `did_change` notification after writes to flush stale positions.

**Fix (2026-04-18):** `replace_symbol`, `insert_code`, and `remove_symbol` now route symbol lookup through a new `fetch_validated_symbol` helper. It runs up to 3 attempts: fetch `documentSymbol`, validate range + position. On staleness, fires a fresh `did_change`, sleeps with linear backoff (50 / 100 ms), and retries. If all retries exhaust, surfaces the existing `RecoverableError` so the user still sees a clear signal (can `/mcp` restart). Test infra: `MockLspClient::with_symbols_sequence` stages a queue of responses; `did_change` advances the queue. Regression tests: `replace_symbol_retries_on_stale_lsp_positions_until_fresh` + `replace_symbol_surfaces_stale_error_after_max_retries`.

### BUG-042 — `replace_symbol`: body-only `new_body` silently drops the function signature

**Date:** 2026-04-16

**What I did:** Called `replace_symbol("run_command_inner", <body-only code without fn signature>)` — intended to replace just the function body.

**Expected:** Error or the signature preserved.

**What happened:** `replace_symbol` replaces the ENTIRE symbol (attributes + signature + body). Passing body-only code dropped the function definition, leaving orphaned statements at module scope. No error was returned.

**Root cause:** The tool description says `new_body` must include the full declaration, but no validation enforced this. The old name `replace_symbol_body` reinforced the (wrong) mental model that it replaces just the body.

**Fix (2026-04-16):** Post-write AST check (pre_count vs post_count). If the symbol existed before the write (tree-sitter indexed it) but disappears after, the file is restored and a `RecoverableError` is returned. Test: `replace_symbol_rejects_body_only_new_body_and_restores_file`.

**Scope (original 2026-04-16 fix):** Only caught symbols tree-sitter indexes at the flat level (top-level Rust fns; Rust `impl` methods happen to land flat because `extract_rust_symbols` merges them into the parent). Methods kept in `children` (Java/Kotlin/Python/TypeScript classes) had `pre_count == 0` so the check was skipped for them.

**Fix extended (2026-04-18):** Count is now recursive over `SymbolInfo.children` and matches by full `name_path` (e.g. `Foo/target`) instead of short name. Covers nested methods in every language the AST parser emits hierarchically. Also eliminates short-name ambiguity (two `impl` blocks each with `fn new` used to leave a false-negative gap). Regression test: `replace_symbol_rejects_body_only_for_nested_method`.

### BUG-043 — `edit_markdown`: `replace` on a heading whose section extends to EOF wipes entire file tail

**Date:** 2026-04-16
**Tool:** `edit_markdown`
**Action:** `replace`
**Severity:** High — silent data loss

**What happened:**
Called `edit_markdown(path, heading="## File Map", action="replace", content=<new content>)` on a plan file. The `## File Map` section was the only level-2 heading; all subsequent content was level-3 (`###` task headings). The tool computed the section as running from `## File Map` to end-of-file and replaced everything — 860 lines of tasks wiped, leaving only the 20-line header + new File Map content.

**Expected:** Replace only the body of the `## File Map` section (between its heading and the next `###` heading).

**Actual:** Section end computed as EOF because no subsequent `##` or `#` heading exists. All `###` task headings treated as children of the section, not as siblings.

**Workaround:**
- Use `action="edit"` with `old_string`/`new_string` for targeted in-section edits instead of `replace`.
- Or ensure plan/spec files have a closing `##` sentinel heading (e.g. `## End`) so sections don't extend to EOF.
- `create_file` for full rewrites when structural damage occurs.

**Related:** BUG-035 (`compute_section_end` ignores fenced code blocks) — same `compute_section_end` function responsible for both.

**Fix (2026-04-18):** `EditMarkdown::call` now runs a pre-flight guard on `action="replace"`. If the target section contains any deeper-level sub-headings, the call is rejected with a `RecoverableError` naming every would-be-wiped heading and the `include_subsections: true` opt-in. `perform_section_edit` semantics unchanged — the guard lives at the tool layer, so the underlying CommonMark behaviour (level-3 is child of level-2) is preserved for explicit opt-in. New schema field: `include_subsections`. Regression tests: `find_consumed_subsections_*` + `bug043_edit_markdown_replace_*`.### BUG-021 — `edit_file`: parallel calls cause partial state + MCP server "crash"

**Date:** 2026-03-03
**Severity:** High — leaves files in inconsistent partial state; server exit requires `/mcp` restart
**Status:** ✅ Crash fixed (rmcp 1.2.0); ⚠ partial-state remains by design

**What happened:**
Dispatched two `edit_file` calls in the same parallel response (targeting two different source
files: `src/embed/local.rs` and `src/config/project.rs`). The Claude Code permission system
handles each call independently: the first call was approved and returned `"ok"` (edit applied
to `local.rs`); the second call was rejected by the user and returned an error. This left the
two files in an inconsistent state — one edited, one not. Immediately after, the codescout
MCP server crashed and became unavailable, requiring a manual `/mcp` reconnect.

**Reproduction hint:**
1. Dispatch two `edit_file` tool calls in a single parallel response to different source files.
2. Approve the first permission prompt, reject (or let timeout) the second.
3. Observe: first file edited, second file unchanged — inconsistent partial state.
4. codescout MCP server crashes; subsequent tool calls fail until `/mcp` restart.

**Root cause (investigated 2026-03-03 — two separate issues):**

**Issue A — Partial state: inherent to independent parallel writes.**
When two `edit_file` calls target different files, they run as independent `tokio::spawn` tasks
inside rmcp's `serve_inner`. There is no transaction semantics across them. If one is denied
(permission dialog) while the other succeeds, the files are left in a partially-applied state.
This is NOT a bug in our code — it's the correct behavior for two independent operations. The
fix is operational: never dispatch parallel write tool calls.

**Issue B — "Crash" is actually Claude Code closing the stdio pipe (rmcp cancellation race).**
Static analysis of the full code path confirms there are NO panic paths in our production code
that could crash the server:
- All `lock().unwrap()` calls in the hot path (`open_files`, `OutputBuffer`) have trivial
  critical sections (HashSet ops only) — mutex cannot be poisoned by normal use.
- `call_tool_inner` routes ALL errors through `route_tool_error`; no unhandled panics.
- rmcp 0.1.5 spawns each request as `tokio::spawn` with the JoinHandle **dropped** — task
  panics are absorbed by the detached task and never propagate to the serve loop.
- The serve loop in `serve_inner` has no `unwrap()`/`expect()` in its event handler.

The "crash" is the server process exiting cleanly after the **stdio pipe closes**. This maps to
`service.waiting()` returning `QuitReason::Closed` → error propagates via `?` in `run()`.

**Why does Claude Code close the pipe?** Most likely a cancellation race in rmcp 0.1.5:
When Claude Code denies a parallel call, it may send a `notifications/cancelled` for the
in-flight request. rmcp cancels the `CancellationToken` but the spawned task has **no check**
for `context.ct.is_cancelled()` — it runs to completion and sends a response back through
`sink_proxy_tx`. The main loop then writes that response to stdout. Claude Code receives an
unexpected response for an already-cancelled request ID, which may cause it to close the
connection (a Claude Code MCP client bug, not ours).

**Fix:**
- **Issue B — RESOLVED (rmcp 1.2.0):** rmcp 1.2.0 architecturally fixed the cancellation
  race. Spawned tasks now write to an internal `mpsc` channel, not directly to stdout. The
  main event loop gates all transport writes, preventing cancelled responses from reaching
  Claude Code. The server no longer crashes on denied parallel calls.
- **Issue A — Partial state remains by design:** Two independent parallel writes still have
  no transaction semantics. If one is denied, files end up half-applied. The operational
  guidance still applies: never dispatch parallel write tool calls.
- **Defence-in-depth** (applied): `[profile.release] panic = "abort"` in Cargo.toml ensures
  any future panic kills the process cleanly rather than leaving a zombie server.

---

### BUG-044 — `replace_symbol` with nested method path replaces entire outer `impl` block

**Date:** 2026-04-20
**Status:** ✅ Fixed (2026-04-20)

**What I did:** While refactoring `LeafOp::parse` → `FromStr` impl in `crates/librarian-mcp/src/filter.rs`, called `replace_symbol(symbol="impl LeafOp/parse", new_body=<FromStr impl block>)`. The intent was to replace only the `parse` method inside `impl LeafOp { ... }`, keeping the sibling `sql` method intact.

**Expected:** Just the `parse` fn inside the `impl LeafOp` block gets replaced; `sql` method preserved.

**What happened:** The tool interpreted the path as targeting the entire outer `impl LeafOp` block and wrote the new content in its place. The `sql` method was dropped. A duplicate fragment was also left. `cargo build` failed with "cannot find method `sql`".

**Recovery:** Used `replace_symbol(symbol="impl LeafOp", new_body=<only sql method>)` to normalize the block back to a single method, then `insert_code(before="impl LeafOp", new_code=<FromStr impl>)` to prepend the `FromStr` impl as a sibling impl block. Two writes + one build retry.

**Root cause (confirmed):** Asymmetric parent clamp. The fix for BUG-034 clamped the *start* of a child's edit range to the parent container's body, but the *end* was unclamped. When the LSP (or AST-fallback) range for the target method overshot into a sibling, `replace_symbol` silently ate the sibling.

**Fix (2026-04-20, spec `docs/superpowers/specs/2026-04-20-impl-block-symbol-cluster-fix.md`):**
1. Symmetric `clamp_range_to_parent` applied to both start and end in `replace_symbol` and `remove_symbol` (symbol.rs).
2. Pre/post-write AST `name_path` set diff. Any sibling symbol that existed pre-write but not post-write (excluding the intentionally-edited target) triggers a rollback with a `RecoverableError` that names the dropped siblings.
3. Parent clamp also applied to `insert_code` (position `before`/`after`) so insertion points cannot escape the parent container body.
4. Helpers: `clamp_range_to_parent`, `collect_all_name_paths`, `find_ast_name_path`. Six pure-logic unit tests + one mock-LSP integration test (`replace_symbol_rolls_back_when_sibling_method_would_be_dropped`) + one for `insert_code` (`insert_code_after_clamps_to_parent_body_end`).

**Related (now covered by the same fix):** BUG-030, BUG-034, BUG-037. The cluster root cause was the same asymmetric clamp + missing sibling-drop guard.
### BUG-026 — `read_file`: large ranged read on `@file_*` buffer ref silently wraps in `@tool_*`, breaking line navigation

**Date:** 2026-03-15
**Severity:** High — sub-range reads on large buffer refs return empty content, silently
**Status:** ✅ Fixed (2026-03-15)

**What happened:**
`read_file("@file_X", start_line=N, end_line=M)` where the extracted slice exceeds
`TOOL_OUTPUT_BUFFER_THRESHOLD` (≈10 KB) would return `{"output_id": "@tool_Y", "summary":
"511 lines...", "hint": "..."}`. Then reading `@tool_Y` with any `start_line > 4` returned
`{"content": "", "total_lines": 4}`.

**Root cause:**
Two-layer failure:
1. `read_file`'s buffer-ref path (`@file_*`/`@cmd_*`) returned the extracted content inline
   via `call()` — `json!({ "content": content, "total_lines": 511 })`.
2. `call_content()` (default `Tool` trait impl) serialized this and, because the JSON string
   exceeded the threshold, stored it as `@tool_*` via `store_tool()`.
3. Reading a `@tool_*` with `start_line`: the buffer content is the pretty-printed JSON
   `{"content": "line1\nline2\n...", "total_lines": 511}` — but `serde_json::to_string_pretty`
   keeps string values as single-line JSON with `\n` escapes, so the whole JSON is only 4
   lines. `total_lines = 4`, and any `start_line > 4` hits out-of-range → empty string.

The same root cause was fixed for the real-file explicit-range path in `BUG-025`, but the
buffer-ref path was missed.

**Reproduction:**
1. Read any file > ~10 KB: `read_file("path/to/large.md")` → `@file_X`
2. Range-read a large slice: `read_file("@file_X", start_line=1, end_line=300)` → `@tool_Y`
3. `read_file("@tool_Y", start_line=70, end_line=100)` → `{"content": "", "total_lines": 4}`

**Fix (src/tools/file.rs):**
In the buffer-ref line-range path, check `exceeds_inline_limit` after extracting lines. If
exceeded, call `output_buffer.store_file()` and return `{"file_id": "@file_Z", "total_lines":
N}` — a small JSON that `call_content()` won't re-buffer as `@tool_*`. Regression test:
`read_file_buffer_ref_large_range_buffers_as_file_ref`.

---

### BUG-033 — MCP server killed by transient `EAGAIN` on stdin

**Date:** 2026-03-21
**Severity:** High
**Status:** Fixed (workaround)

**What happened:**
During a session on the `deployment` project, two back-to-back `run_command` calls
(`vercel logs | head`, `vercel inspect`) timed out (30s, 15s). After the second
timeout+killpg cycle, the codescout MCP server exited cleanly with
`QuitReason::Closed`. The diagnostic log showed:

```
ERROR rmcp::transport::async_rw: Error reading from stream: io error Resource temporarily unavailable (os error 11)
INFO  rmcp::service: input stream terminated
```

**Root cause:**
rmcp's `AsyncRwTransport::receive()` converts *any* IO error (including transient
`EAGAIN`/`WouldBlock`) to `None`, which the service loop interprets as "input stream
closed" → server exits. `EAGAIN` appeared on stdin likely because Claude Code's
Node.js runtime temporarily set the pipe to non-blocking mode under I/O pressure.

**Fix applied:**
`ResilientStdin` wrapper in `src/server.rs` intercepts `WouldBlock` at the
`AsyncRead` layer and converts it to `Poll::Pending` (correct async semantic).
Test: `resilient_stdin_absorbs_would_block`.

**Upstream:** Consider filing an issue with rmcp — `receive()` should distinguish
transient errors (`WouldBlock`, `Interrupted`) from fatal ones (`BrokenPipe`, EOF).

**Reproduction hint:**
Run two long-running commands via `run_command` that both time out in quick
succession. The killpg + I/O pressure can trigger `EAGAIN` on stdin.

### BUG-034 — `replace_symbol`: replacing first test in `mod tests` eats the module header

**Date:** 2026-03-31
**Severity:** Medium — produces broken code, manifests as compile error (caught immediately)
**Status:** Fixed (2026-03-31)

**What happened:**
During Task 4 of the unified-embedding-config implementation, called
`replace_symbol` on the first function inside `#[cfg(test)]\nmod tests { ... }` in
`src/embed/mod.rs`. The tool matched the symbol body BUT also consumed the
`#[cfg(test)]\nmod tests {` module header — the replacement output omitted it,
leaving subsequent test functions floating outside the module. This produced an
`unexpected token` compile error.

**Root cause:**
Stale LSP data after a prior edit caused `range_start_line` for the child function
to point to the parent module's `#[cfg(test)]` attribute (line 0) instead of the
child's own `#[test]` attribute. `editing_start_line` trusted this value and returned
line 0, so `lines[..0]` was empty — everything above the replacement body was lost.

**Fix applied:**
Parent-boundary guard in `replace_symbol` and `remove_symbol`: after computing
`editing_start_line`, if the symbol is nested (has `/` in `name_path`), find the
parent symbol and clamp `start` to `max(start, parent.start_line + 1)`. The parent's
body starts on the line after its keyword — no child edit should extend above that.

New helper: `find_parent_symbol(symbols, child_name_path)` in `src/tools/symbol.rs`.

**Test:** `replace_symbol_child_in_mod_tests_preserves_module_header` in `tests/symbol_lsp.rs`.

---
### BUG-040 — `edit_file` / `replace_symbol` / `insert_code`: strips Unix exec bit on shell scripts

- **What happened**: editing a `.sh` file via `edit_file` silently dropped the `+x` mode. Hook scripts became non-executable after routine edits, causing Claude Code hook infrastructure to break at next session start.
- **Expected**: write tools preserve the target file's mode (perms, setuid, etc.). A content-level edit should never change file permissions.
- **Observed**: after `edit_file` on `pre-tool-use.sh` with mode `0755`, resulting file had mode `0644`. Same for any write path going through `atomic_write`.
- **Root cause**: `util::fs::atomic_write` writes a sibling `.tmp` file (default umask → `0644`) then `rename`s over the target. `rename` replaces the inode, so the original mode is lost.
- **Fix (2026-04-16)**: `atomic_write` now copies the original file's Unix mode onto the `.tmp` file before `rename`. Regression test `util::fs::tests::atomic_write_preserves_exec_bit` locks in 0755 survival.
- **Scope**: fix is at the `atomic_write` layer, so `edit_file`, `replace_symbol`, `insert_code`, `remove_symbol`, `rename_symbol`, `edit_markdown`, and any other write going through it are all covered.
- **Status**: fixed.

### BUG-036 — `insert_code` with `position: "before"` and module-symbol anchor lands inside an earlier function

**Date:** 2026-04-13 (during T7 of the MCP token-budget plan)
**File:** `src/mcp_resources/project_summary.rs`
**Tool:** `insert_code`

**What happened:** Implementer called `insert_code` with `position: "before"` targeting the `tests` module symbol to add a new `AgentSummarySource` struct just above `#[cfg(test)] mod tests`. The insertion landed mid-body inside the preceding `ProjectSummaryProvider::read` method, corrupting the file. Recovered by rewriting the whole file via `run_command(cat > ...)`.

**Probable cause:** The symbol anchor for a `#[cfg(test)] mod tests` block resolved to a line inside an earlier function rather than the line of the `mod` keyword. Likely the anchor points at the last non-whitespace token before the module attribute, which on a tightly packed file is the closing `}` of the previous method — and the "before" calculation landed one line further up, inside the method body.

**Workaround:** When inserting above a `#[cfg(test)] mod tests` block, prefer appending at end-of-file via explicit line number, or use `create_file` to rewrite the whole file, or insert after the symbol immediately preceding the test module instead of before the test module. Related to BUG-029 (`position: "after"` landing inside function body).
### BUG-037 — `replace_symbol` on `impl Trait for Type` blocks drops outer `#[async_trait]` attribute and leaves stray closing braces

**Date:** 2026-04-13 (during `lsp_ready` / language-detection fix, commit `363591b`)
**Files:** `src/lsp/manager.rs`, `src/mcp_resources/project_summary.rs`
**Tool:** `replace_symbol`

**What happened:** Two separate `replace_symbol` invocations on `impl SomeTrait for SomeType` blocks:
1. `impl crate::lsp::ops::LspProvider for LspManager` — the outer `#[async_trait::async_trait]` attribute above `impl` was silently dropped from the replacement, producing a compile error ("`async fn` is not permitted in traits").
2. `impl SummarySource for AgentSummarySource` — same attribute loss, plus the tool appeared to include part of an enclosing `impl AgentSummarySource { ... }` block in the replaced range, leaving orphaned closing braces after the new impl body.

In both cases the implementer recovered by rewriting the damaged region with a shell one-liner.

**Probable cause:** The LSP symbol range for an `impl` item starts at the `impl` keyword, excluding preceding attributes. `replace_symbol` uses that range verbatim, so outer `#[attribute]` lines above `impl` get dropped from the "old" selection but the corresponding attributes are also not re-emitted because they're not part of the `new_body` the caller supplies either. For nested impls (same `Type` with both inherent `impl AgentSummarySource` and trait `impl SummarySource for AgentSummarySource` adjacent to each other), the range may additionally grab the wrong brace set.

**Root cause (confirmed):** `editing_start_line`'s BUG-031 walk-back checked `lines[r-1]` directly. When that line was `#[async_trait]`, it matched `above.starts_with("#[")` and triggered `find_insert_before_line`, which walked the entire attribute into the deletion range. The LLM's `new_body` (from `find_symbol`, which starts at `impl`) did not include the attribute → attribute silently dropped.

**Fix (this session):** Changed the BUG-031 walk-back trigger in `editing_start_line` to skip over consecutive Rust `#[...]` lines first, then check for doc comments above them. If no doc comments are found above the attribute block, the LSP's `range_start_line` is returned unchanged — the attribute stays in the file. If doc comments ARE found above, walk-back still fires (BUG-031 behaviour preserved). Two regression tests added: `editing_start_line_does_not_walk_back_to_outer_attribute_on_impl_block` and `editing_start_line_walks_back_when_docs_exist_above_attribute_on_impl`.

**Status:** ✅ Fixed

**Remaining limitation:** Adjacent/nested `impl` blocks on the same type (e.g. inherent `impl Type` next to `impl Trait for Type`) can still cause range confusion — `replace_symbol` may grab the wrong brace set. Workaround: use `create_file` for those cases.

### BUG-038 — `activate_project`: switching back to home project after indexing a foreign project crashes the server

**Date observed:** 2026-04-14
**Tool:** `index_project` (background task)
**Severity:** Critical — kills the MCP server, requires `/mcp` reconnect
**Regression:** Introduced by `bbd93dd` (progress notifications). Only on `experiments` branch.

**Root cause:** MCP progress notifications (`notifications/progress` and `notifications/message`)
sent from `index_project`'s background `tokio::spawn` task crash the connection. Claude Code 2.1.105
closes the stdin pipe ~630ms after `index_project` returns — exactly when the first progress callback
fires from `build_index`'s file processing loop.

**Investigation trail (2026-04-14):**
1. Initially misattributed to `activate_project` — diagnostic logs showed the crash happens during `build_index`, not project switching
2. No panic (custom panic hook confirmed — `crash.log` never created)
3. Stdout pollution hypothesis disproved: `StdoutGuard` (fd 1 → /dev/null) didn't help; permanent fd 1 redirect with private MCP fd didn't help
4. 2s diagnostic sleep before `build_index` proved the crash tracks `build_index` execution, not the "started" response
5. Disabling all progress notifications (pre-spawn `p.report()` + `progress_cb` + completion reports) fixed the crash completely

**Fix (2026-04-14):** Disabled progress notifications in `index_project`'s background task.
The `ProgressReporter` calls are commented out with `// see BUG-038` references.
Progress should be re-enabled when Claude Code supports MCP `notifications/progress`
from background tasks (check client capabilities for progress support).

**Collateral improvements retained:**
- `LocalEmbedder::new()` moved to `spawn_blocking` (prevents async executor starvation)
- `show_download_progress = false` on `fastembed::InitOptions` (good hygiene)
- Synchronous panic hook in `logging.rs` (captures crashes that `non_blocking` tracing misses under `panic = "abort"`)

### BUG-039 — `run_command` buffer query: `stdout_shown=0` despite non-zero `stdout_total`

**Date observed:** 2026-04-14
**Tool:** `run_command` (buffer query mode)
**Severity:** Medium — makes buffer query results invisible, forcing workarounds

**Steps to reproduce:**
1. `run_command("grep -in 'error\\|warn' some-log-file.log")` → returns `@cmd_xxxx` buffer
2. `run_command("cat @cmd_xxxx")` → `{ stdout_shown: 0, stdout_total: 28, truncated: true }`
3. `run_command("sed -n '1,28p' @cmd_xxxx")` → same: `stdout_shown: 0, stdout_total: 28`

**Expected:** Buffer content displayed inline (well under the 100-line cap).
**Actual:** `stdout_shown` is always 0. Content is confirmed present (`stdout_total > 0`) but never rendered.

**Observed with:** grep output that included ANSI escape codes (log files with tracing ANSI color sequences). Plain-text buffer queries may be unaffected — not confirmed.

**Root cause (confirmed):** ANSI escape codes inflate `raw_stdout` byte length. With tracing-colored log output, each line can be 200-500 bytes of escape codes around a few dozen bytes of visible text. The byte budget for inline display is 9,700 bytes (`TOOL_OUTPUT_BUFFER_THRESHOLD - JSON_OVERHEAD`). When stored `buffer_stderr` is replayed (fetched from the buffer entry), it is only line-capped (20 lines) not byte-capped — so 20 ANSI-heavy stderr lines can consume the entire budget, leaving `stdout_byte_budget ≈ 0`. `truncate_lines_and_bytes` then drops even the first stdout line (any `line.len() > 0` exceeds a 0-byte budget), giving `stdout_shown=0`.

**Fix (this session):** Strip ANSI CSI sequences from `raw_stdout` and `raw_stderr` immediately after capturing them, but only for buffer-only commands. Added `strip_ansi_codes()` to `command_summary.rs`; applied in `run_command_inner` for the `buffer_only` branch. ANSI codes are opaque to LLMs and should not count toward byte budgets.

**Status:** ✅ Fixed

### BUG-040 — `edit_file` / `replace_symbol` / `insert_code`: strips Unix exec bit on shell scripts

- **What happened**: editing a `.sh` file via `edit_file` silently dropped the `+x` mode. Hook scripts became non-executable after routine edits, causing Claude Code hook infrastructure to break at next session start.
- **Expected**: write tools preserve the target file's mode (perms, setuid, etc.). A content-level edit should never change file permissions.
- **Observed**: after `edit_file` on `pre-tool-use.sh` with mode `0755`, resulting file had mode `0644`. Same for any write path going through `atomic_write`.
- **Root cause**: `util::fs::atomic_write` writes a sibling `.tmp` file (default umask → `0644`) then `rename`s over the target. `rename` replaces the inode, so the original mode is lost.
- **Fix (2026-04-16)**: `atomic_write` now copies the original file's Unix mode onto the `.tmp` file before `rename`. Regression test `util::fs::tests::atomic_write_preserves_exec_bit` locks in 0755 survival.
- **Scope**: fix is at the `atomic_write` layer, so `edit_file`, `replace_symbol`, `insert_code`, `remove_symbol`, `rename_symbol`, `edit_markdown`, and any other write going through it are all covered.
- **Status**: fixed.

## Template for new entries

```
### BUG-XXX — `<tool name>`: <one-line description>

**Date:** YYYY-MM-DD
**Severity:** Low / Medium / High
**Status:** Open

**What happened:**
<what you did, what you expected, what happened instead>

**Reproduction hint:**
<minimal steps or context to reproduce>

**Root cause hypothesis:**
<your best guess at why it happened>

**Fix ideas:**
<options for fixing it in the tool or in its UX>

---
```

### BUG-027 — `replace_symbol` / `remove_symbol`: Kotlin LSP `range.start` lands mid-docstring, leaving unclosed `/**`

**Date:** 2026-03-18
**Severity:** High — silently corrupts Kotlin source files; causes cascading "Unclosed comment" + "Unresolved reference" compile errors
**Status:** ✅ Fixed (2026-03-18)

**What happened:**
Called `replace_symbol("createSolver", ...)` on a Kotlin file where `createSolver` had a
multi-line KDoc with preamble text before `@param` tags:

```kotlin
// line 106: /**
// line 107:  * Create a configured Stage1 solver for a specific tier.
// line 108:  *
// line 109:  * @param tier ...        ← kotlin-language-server reports range.start HERE
// line 110:  * @param lessonCount ...
// ...
// line 113:  */
// line 114: fun createSolver(
```

`replace_symbol` replaced from line 109 onward. Lines 106–108 (`/**`, description, blank `*`)
were left in the file. The new body also started with `/**`, producing two nested `/**`
openers with only one `*/` — an unclosed block comment. Kotlin compiler error:
`Syntax error: Unclosed comment at EOF`.

**Root cause:**
`kotlin-language-server` returns `DocumentSymbol.range.start` pointing to the first `@param`
tag line instead of the `/**` opener, when the KDoc has preamble text (description + blank
line) before its first `@` tag. Functions with short KDocs (no preamble, or only description,
no `@param`) are unaffected — their `range.start` correctly lands on `/**`.

codescout's `editing_start_line` (`src/tools/symbol.rs`) trusts `range_start_line`
(= `ds.range.start.line`) unconditionally. When it points mid-comment, `replace_symbol`
leaves the `/**` opener orphaned.

**Reproduction:**
1. Kotlin file with a function whose KDoc has preamble text before first `@param`:
   ```kotlin
   /**
    * Description paragraph.
    *
    * @param x ...
    */
   fun foo(x: Int) { ... }
   ```
2. Call `replace_symbol("foo", new_body_starting_with_/**/)`.
3. Observe: original `/**\n * Description paragraph.\n *\n` left in file; new `/**` appended.
4. Kotlin compiler reports "Unclosed comment".

**Fix (src/tools/symbol.rs — `editing_start_line`):**
When `range_start_line` is `Some(r)` and the line at `r` is inside a block comment
(starts with `*` after trimming), walk backward to find the `/**` opener:

```rust
fn editing_start_line(sym: &SymbolInfo, lines: &[&str]) -> usize {
    if let Some(r) = sym.range_start_line {
        let r = r as usize;
        // Kotlin LSP (and possibly others) may report range.start inside a /** */ block —
        // at the first @param line rather than the /** opener. Walk back to fix it.
        if r < lines.len() && lines[r].trim_start().starts_with('*') {
            for i in (0..r).rev() {
                if lines[i].trim_start().starts_with("/**") {
                    return i;
                }
            }
        }
        return r;
    }
    find_insert_before_line(lines, sym.start_line as usize)
}
```

Also add a Kotlin fixture test: function with multi-line KDoc + @params → assert
`body_start_line` == line of `/**`, not the `@param` line.

---

### BUG-028 — `create_file` / `edit_file`: does not notify LSP, leaving index stale

**Date:** 2026-03-18
**Severity:** Medium — after writing a file, `find_symbol` / `list_symbols` return stale results until LSP restarts
**Status:** ✅ Fixed (2026-03-18)

**What happened:**
After `create_file` rewrote a Kotlin fixture file with a new function added, subsequent
`list_symbols` and `find_symbol` calls did not return the new function. The Kotlin LSP was
still serving the pre-write symbol table. Only an `/mcp` reconnect (which kills and restarts
the LSP process) refreshed the index.

**Root cause:**
`create_file` (and `edit_file` for non-LSP-structural changes) writes directly to disk without
sending `textDocument/didChange` (or `didOpen` + `didChange`) to any running LSP client for
that file's language. The LSP only re-reads on the next `didOpen` — which only fires if the
file hasn't been opened before in the current session.

**Fix:**
After any write to a source file (`create_file`, `edit_file`), call
`ctx.lsp.notify_file_changed(&full_path).await` — the same `did_change` notification that
`replace_symbol` and `insert_code` already send. This ensures the LSP re-indexes the file
before the next `document_symbols` call.

Check: `create_file` and `edit_file` both already have access to `ctx.lsp` — just missing
the `notify_file_changed` call.

---

### BUG-029 — `insert_code`: `position: "after"` inserts inside function body instead of after it

**Date:** 2026-03-20
**Severity:** High — silently corrupts source files by splitting function bodies
**Status:** ✅ Fixed (2026-03-20)

**What happened:**
Called `insert_code("tests/write_produces_valid_framing", "src/lsp/transport.rs", code, "after")`
to add two new test functions after the last test in a `mod tests` block.

Expected: new functions inserted after the closing `}` of `write_produces_valid_framing`.

Actual: the new code was inserted **inside** `write_produces_valid_framing`'s body, splitting
the function in half. The original function's `let msg = json!(...)` ended up separated from
the rest of its body by the two new functions. Result:

```
    async fn write_produces_valid_framing() {
        let msg = json!({"test": true});

    #[tokio::test]                           // ← inserted HERE, inside the function
    async fn rejects_oversized_content_length() { ... }

    #[tokio::test]
    async fn accepts_normal_content_length() { ... }

        let mut buf = Vec::new();            // ← remainder of write_produces_valid_framing
        write_message(&mut buf, &msg).await.unwrap();
        ...
    }
```

Compiler warning: `cannot test inner items` (test functions defined inside another function).

**Reproduction:**
1. File with a `mod tests` block containing multiple `#[tokio::test]` async functions
2. Call `insert_code(name_path="tests/write_produces_valid_framing", path="src/lsp/transport.rs", code="<two test fns>", position="after")`
3. Observe: code lands inside the function body, not after it

**Root cause hypothesis:**
`insert_code` with `position: "after"` likely uses the LSP `DocumentSymbol.range.end` to find
the insertion point. For a function inside a `mod tests` block, the range may end at the last
line of the function body (before the closing `}`) rather than after it. Alternatively, the
insertion logic may be using `selection_range.end` (which points to the name) instead of
`range.end` (which should encompass the entire symbol including braces).

**Fix ideas:**
1. Verify that `insert_code` uses `range.end` (not `selection_range.end`) for the "after" position
2. After computing the insertion line, verify the line after it is outside the symbol's range
3. Add a test: `insert_code_after_places_code_after_closing_brace` with a multi-function mod block

**Fix (2026-03-20):**
- `editing_end_line` now trusts AST unconditionally when available (was: only capped downward).
  When LSP reports `end_line` too short (inside body, not at `}`), AST corrects it upward.
- Added `validate_symbol_position` guard to all mutation tools — detects when LSP returns
  stale positions that don't match file content.
- Tests: `editing_end_line_corrects_lsp_short_end_line_via_ast`,
  `editing_end_line_nested_fn_returns_closing_brace_line`

---

### BUG-030 — `replace_symbol`: replacing `mod tests` eats adjacent function body

**Date:** 2026-03-20
**Severity:** High — silently destroys neighboring function, producing compile errors
**Status:** ⚠ Mitigated (2026-03-20) — editing_start_line logic is correct; validate_symbol_position guard added

**What happened:**
After BUG-029 corrupted `src/lsp/transport.rs` (insert_code placed code inside
`write_produces_valid_framing`), attempted to fix by calling
`replace_symbol("tests", "src/lsp/transport.rs", <full corrected mod tests body>)`.

Expected: only the `mod tests` block (lines 56–118) replaced with the new body.

Actual: `replace_symbol` replaced lines 56–118, but the `write_message` function body
(lines 47–54) was **also consumed**. The replacement left `write_message` as an empty
function signature with no body:

```rust
pub async fn write_message<W: AsyncWriteExt + Unpin>(writer: &mut W, msg: &Value) -> Result<()> {
#[cfg(test)]
mod tests {
    // ... new body starts here, immediately after the opening brace
```

The `write_message` function's body (`let body = serde_json::to_string(msg)?; ...`) was
deleted entirely. Compiler error: `unclosed delimiter` because `write_message`'s `{` was
never closed.

**Reproduction:**
1. File: `src/lsp/transport.rs` with this structure:
   ```rust
   pub async fn write_message(...) -> Result<()> {
       let body = serde_json::to_string(msg)?;
       // ... 5 lines of body
   }

   #[cfg(test)]
   mod tests {
       // ... 60 lines of tests
   }
   ```
2. Call `replace_symbol("tests", "src/lsp/transport.rs", <new mod tests body>)`
3. Observe: `write_message` body deleted, `mod tests` now starts inside `write_message`'s braces

**Root cause hypothesis:**
The LSP `DocumentSymbol.range` for `mod tests` likely extends upward to include the blank
line and possibly the closing `}` of the preceding function. rust-analyzer may report
`range.start` for `mod tests` at the `#[cfg(test)]` attribute line, but the
`editing_start_line` logic in `src/tools/symbol.rs` may walk further back to include
preceding blank lines or comments, accidentally consuming the end of `write_message`.

Alternatively, the `range.start.line` for the `mod tests` block may point to line 47
(inside `write_message`) rather than line 56 (`#[cfg(test)]`), because rust-analyzer
sometimes includes leading whitespace/blank lines in the symbol range.

**Key debugging info:**
- File: `src/lsp/transport.rs`, Rust, rust-analyzer LSP
- Symbol: `mod tests` (module kind)
- The `write_message` function ends at line ~54, `#[cfg(test)]` starts at line 56
- `replace_symbol` replaced lines "56-118" per its response, but the actual effect was
  that lines 47-54 (write_message body) were also gone

**Fix ideas:**
1. In `replace_symbol`, after computing the replacement range, verify the start line is
   NOT inside another symbol's range (cross-reference with `document_symbols` results)
2. Add a sanity check: if the line at `range.start` contains code that doesn't look like
   the symbol being replaced (e.g., not `#[cfg(test)]` or `mod`), refuse the edit
3. Write a regression test: file with `fn foo() { ... }\n\n#[cfg(test)]\nmod tests { ... }`,
   replace `tests` → verify `foo` body is untouched

---

### BUG-031 — `replace_symbol`: duplicates doc comment and signature, leaving orphaned opening brace

**Date:** 2026-03-20
**Severity:** High — produces unparseable source (unclosed delimiter)
**Status:** ✅ Fixed (2026-03-20)

**What happened:**
Called `replace_symbol("is_source_path", "src/util/path_security.rs", <new body>)` to
update a small function with cached regex logic.

Expected: the old function (lines 571–577) replaced with the new body.

Actual: the old doc comment + signature (3 lines) were **left in place**, and the new
body (including its own doc comment + signature) was **appended after them**, producing:

```rust
/// Returns true if the path refers to a source code file (by extension).
/// Used to gate `edit_file` multi-line source edits.
pub fn is_source_path(path: &str) -> bool {   // ← OLD, now an unclosed brace
/// Returns true if the path refers to a source code file (by extension).
/// Used to gate `edit_file` multi-line source edits.
pub fn is_source_path(path: &str) -> bool {   // ← NEW body starts here
    static RE: std::sync::OnceLock<Option<Regex>> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(SOURCE_EXTENSIONS).ok())
        .as_ref()
        .is_some_and(|re| re.is_match(path))
}
```

Compiler error: `unclosed delimiter` at EOF (the old `{` on line 573 was never closed).

**Reproduction:**
1. File: `src/util/path_security.rs` with a short function:
   ```rust
   /// Returns true if the path refers to a source code file (by extension).
   /// Used to gate `edit_file` multi-line source edits.
   pub fn is_source_path(path: &str) -> bool {
       Regex::new(SOURCE_EXTENSIONS)
           .map(|re| re.is_match(path))
           .unwrap_or(false)
   }
   ```
2. Call `replace_symbol("is_source_path", "src/util/path_security.rs", <new body with doc comments>)`
   where `new_body` starts with `/// Returns true...`
3. Observe: old doc comment + signature left in place; new body appended below them

**Root cause hypothesis:**
`replace_symbol` computes the replacement range using `editing_start_line` (which should
include doc comments) and `range.end.line`. The issue may be that `editing_start_line` is
returning the **body** start line (the `{` line) rather than the doc comment start line.
So only the body (lines 573–577) is replaced, while the doc comments + signature
(lines 568–572) survive. The new body, which includes its own doc comments, is then
inserted starting at line 573, creating the duplication.

**Key debugging info:**
- File: `src/util/path_security.rs`, Rust, rust-analyzer LSP
- Symbol: `is_source_path` (function kind)
- `replace_symbol` response said `replaced_lines: "571-577"` — this matches the old body
  range but NOT the doc comment lines (568-570)
- The `new_body` parameter included doc comments starting with `///`
- The function is standalone (not inside an impl block), near the end of the file,
  just before `#[cfg(test)] mod tests`

**Fix ideas:**
1. Compare `replaced_lines` start with the first `///` doc comment line above the function
   — if they don't match, the range missed the doc comments
2. When `new_body` starts with `///` or `#[`, verify that `editing_start_line` walked back
   to include the existing doc comments/attributes
3. Regression test: function with `/// doc\npub fn foo() { old }` → `replace_symbol` with
   body starting with `/// doc\npub fn foo() { new }` → verify no duplication

**Fix (2026-03-20):**
- `editing_start_line` now walks back past `///` doc comments when `range_start_line` points
  to a non-decorator line (function keyword) and the line above is a doc comment/attribute.
  Only triggers when the LSP missed doc comments — trusts LSP when it already points to a
  decorator/attribute.
- Tests: `editing_start_line_walks_back_past_doc_comments_when_range_misses_them`,
  `editing_start_line_trusts_range_when_it_already_covers_docs`

---

### BUG-032 — `remove_symbol`: leaves orphaned `impl` block code after enum removal

**Date:** 2026-03-20
**Severity:** High — produces unparseable source
**Status:** ⚠ Mitigated (2026-03-20) — validate_symbol_position guard detects stale LSP positions

**What happened:**
Called `remove_symbol("SourceFilter", "src/embed/index.rs")` to remove an enum, followed by
`remove_symbol("impl SourceFilter", "src/embed/index.rs")` to remove its impl block.

Expected: both the enum (10 lines) and its impl block (10 lines) cleanly removed.

Actual: The first `remove_symbol` on the enum reported `removed_lines: "28-37"` and appeared
to succeed. The second `remove_symbol` on `impl SourceFilter` reported
`removed_lines: "39-48"` — but the actual file content showed the impl block body was
**still present** starting at line 29, now orphaned (no `impl SourceFilter {` header, no
closing `}`). The file had:

```rust
// line 28: (blank, after enum removal)

impl SourceFilter {                        // ← should have been removed
    /// Convert to the `Option<&str>` format used by the search functions.
    pub fn as_sql_filter(&self) -> Option<&'static str> {
        match self {
            SourceFilter::All => None,
            SourceFilter::SourceOnly => Some("source"),
            SourceFilter::NonSourceOnly => Some("non-source"),
        }
    }
}
        .join("embeddings")               // ← code from the NEXT function, now malformed
        .join("project.db")
```

The `remove_symbol` for the impl block removed incorrect lines, and the remaining code
from the next function (`project_db_path`) was left orphaned.

**Reproduction:**
1. File: `src/embed/index.rs` with:
   ```rust
   // line 28: doc comment
   pub enum SourceFilter { ... }      // lines 29-37

   impl SourceFilter { ... }          // lines 39-48

   /// Path to the embedding database
   pub fn project_db_path(...) { ... } // lines 50+
   ```
2. Call `remove_symbol("SourceFilter", "src/embed/index.rs")`
3. Call `remove_symbol("impl SourceFilter", "src/embed/index.rs")`
4. Observe: impl body still present, next function corrupted

**Root cause hypothesis:**
After the first `remove_symbol` deleted lines 28-37, the LSP did NOT receive a
`didChange` notification (or the notification was processed asynchronously). The second
`remove_symbol` call used **stale line numbers** from the pre-deletion state.
`removed_lines: "39-48"` was correct for the original file but wrong for the modified
file (where the impl block had shifted up by 10 lines to lines 29-38). The tool deleted
lines 39-48 of the **new** file, which contained the beginning of `project_db_path`.

This is a classic **stale index** problem: sequential symbol edits on the same file
require the LSP to re-index between operations, but `remove_symbol` may not wait for
the re-index to complete before returning line numbers for the second operation.

**Key debugging info:**
- File: `src/embed/index.rs`, Rust, rust-analyzer LSP
- Two sequential `remove_symbol` calls on the same file
- First call: `removed_lines: "28-37"` (10 lines deleted → all subsequent lines shift up by 10)
- Second call: `removed_lines: "39-48"` (these were the ORIGINAL line numbers, not adjusted)
- The impl block was at original lines 39-48, which after deletion would be at lines 29-38

**Fix ideas:**
1. After `remove_symbol` deletes lines, send `didChange` AND wait for the LSP to re-index
   (poll `document_symbols` until the removed symbol no longer appears) before returning
2. If two `remove_symbol` calls target the same file, the second should re-resolve the
   symbol position from the LSP rather than using cached line numbers
3. Consider a `remove_symbols` (plural) API that batches multiple removals on the same
   file, computing all ranges first, then applying deletions bottom-up (highest line
   numbers first) to avoid shift invalidation
4. Regression test: file with `enum Foo { ... }\nimpl Foo { ... }\nfn bar() { ... }` →
   remove Foo, then remove impl Foo → verify bar is untouched

---

### BUG-022 — Agent bypasses library tools, greps cargo registry directly

**Date:** 2026-03-16
**Severity:** Low — wasteful tokens, no data corruption
**Status:** ✅ Fixed — multi-ecosystem auto-registration implemented

**What happened:**
When exploring rmcp's elicitation API, the agent used raw `run_command("grep ...")` on
`/home/marius/.cargo/registry/src/index.crates.io-*/rmcp-1.1.0/src/` instead of using
codescout's library tools (`register_library` + `find_symbol(scope="lib:rmcp")`).

**Expected:** Agent should register rmcp as a library and use structured symbol navigation.

**Actual:** 3 raw grep commands returning unstructured text, wasting context tokens.

**Root cause:** rmcp was not pre-registered as a library (`list_libraries` showed only
`anyhow`). The agent defaulted to the familiar grep pattern rather than first registering
the dependency and then using structured tools. The server instructions mention library
auto-discovery via `goto_definition`, but that requires navigating to an rmcp symbol first —
a chicken-and-egg problem when you don't yet know the API surface.

**Fix options:**
1. Auto-register top-N dependencies from `Cargo.lock` during `onboarding` or `activate_project`
2. Add a hint to `search_pattern` when it detects results in a cargo registry path:
   "Consider `register_library` + `find_symbol(scope=...)` for structured navigation"
3. Add `register_library` suggestion to server instructions for the "Know nothing" row
