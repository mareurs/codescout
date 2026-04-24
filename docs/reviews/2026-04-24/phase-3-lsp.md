# Phase 3 — LSP Integration

**Date:** 2026-04-24
**Scope:** `src/lsp/`, `src/lsp/mux/`, `src/lsp/servers/`
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Security (Ibex)

### S1 — LOW — LSP server spawn relies on `$PATH` lookup (bare binary names)
- **Location:** `src/lsp/servers/mod.rs:14-135` (`default_config`); `src/platform/unix.rs:80-82` (`lsp_binary_name` no-op); `src/lsp/client.rs:223` (`Command::new(&config.command)`).
- **Evidence:** All entries unqualified (`"rust-analyzer"`, `"pyright-langserver"`, `"kotlin-lsp"`, etc). `Command::new` does `$PATH` lookup; first directory wins.
- **Exploit:** Polluted `$PATH` (project-local `node_modules/.bin`, `.envrc` prepend, malicious `~/.local/bin`) → attacker-controlled LSP binary executes silently on first file open of matching language.
- **Fix:** Resolve via `which::which()` once, stash canonical path; refuse if resolved binary lives in writable-by-non-root dir. At minimum log resolved absolute path on first spawn.
- **Confidence:** medium. Standard editor footgun; flagging for documented choice.

### S2 — LOW — `path_to_uri` falls back to `current_dir()`; LSP can be pointed at paths outside workspace
- **Location:** `src/lsp/client.rs:49-60` (`path_to_uri`) + every caller (`did_open`, `did_change`, `goto_definition`, `references`, etc.).
- **Evidence:** No assertion that resolved abs path is under `self.workspace_root`. `did_open` then `read_to_string`s and ships content to LSP via `textDocument/didOpen`. LSP child can read any file the codescout process can read.
- **Cross-check Phase 2 F4 confirmed:** LSP layer does NOT re-validate. It trusts the tool layer. F4 + S2 = amplifier.
- **Fix:** In `did_open`/`did_change`/`*_position`, canonicalize and assert `path.starts_with(&self.workspace_root)`; on violation `RecoverableError`. Cheap defense-in-depth.
- **Confidence:** medium.

### S3 — LOW (DOS) — `read_message` allows up to 100 MiB body per LSP message; multiplied by mux clients
- **Location:** `src/lsp/transport.rs:35` (`MAX_MESSAGE_SIZE = 100 * 1024 * 1024`); allocated as single `vec![0u8; length]` at `:42`.
- **Evidence:** Per-message cap, no per-process budget. N clients × 100 MiB potential.
- **Fix:** Lower per-message cap to ~16 MiB OR per-connection inflight allocation budget.
- **Confidence:** low.

### S4 — MEDIUM-LOW — World-readable mux socket / lock files in `/tmp`
- **Location:** `src/lsp/mux/mod.rs:19-33` (`socket_path_for_workspace` / `lock_path_for_workspace`); `src/lsp/mux/process.rs:73-81, :212`; `src/lsp/manager.rs:417-422`.
- **Evidence:** Socket + lock files in `std::env::temp_dir()` (typically `/tmp`, sticky+world-readable). No explicit `mode(0o600)`. Socket name from `DefaultHasher` (random-seeded) of workspace path — other users can `ls /tmp/codescout-*-mux-*.sock` and connect.
- **Exploit (multi-user host):** User B connects to user A's socket → mux forwards to LSP server running as user A → reads user A's source via `documentSymbol`/`hover`. Lock file also writable by anyone.
- **Fix:**
  1. Bind socket inside per-user `XDG_RUNTIME_DIR` (`/run/user/$UID`, mode `0700`), or create `0700` subdir under temp first.
  2. Set socket permissions to `0600` after bind.
  3. `OpenOptions::mode(0o600)` on lock file.
  4. (Lower priority) replace `DefaultHasher` with stable hash for debug reproducibility.
- **Confidence:** medium-high for multi-user; n/a single-user laptop.

### S5 — LOW — `socket_path.exists()` → `remove_file` → `bind` TOCTOU under flock contention
- **Location:** `src/lsp/mux/process.rs:209-213`.
- **Evidence:** `remove_file` removes ANY file at path (incl. attacker-placed symlink). flock taken first mitigates in-process; combined with S4 fix becomes non-issue.
- **Confidence:** low.

---

## Critical (non-security)

### C1 — Timed-out LSP requests don't send `$/cancelRequest` → server keeps computing
- **Location:** `src/lsp/client.rs:556-566` (timeout branch).
- **Evidence:** On timeout, pending sender removed and bail. No `$/cancelRequest` notification sent. Server continues processing — for kotlin-lsp during Gradle import this is real CPU waste.
- **Fix:** Send `$/cancelRequest` notification with the id before bailing. Standard LSP pattern.
- **Confidence:** high.

### C2 — Cancellation leaves LSP child running and `pending` populated when `request()` future is dropped
- **Location:** `src/lsp/client.rs:458-503` (`request` retry); `:506-568` (`request_with_timeout`).
- **Evidence:** Three gaps when caller is cancelled mid-await:
  1. `pending.insert(id, tx)` before `timeout(rx)` → drop between insert/await leaks entry until LSP responds.
  2. No `$/cancelRequest` sent (same as C1).
  3. LSP child NOT killed on per-tool cancel. `Child` handle owned by spawned reader task that exits only on EOF. Drop-of-`LspClient` SIGTERM never fires (held in `Arc` inside `LspManager.clients`).
- **Cross-check Phase 1 I4:** confirmed in different shape. The cancelled request itself doesn't park on `pending()`, but the LSP child it kicked off keeps running. Per-request cancellation has zero effect on LSP-side resources.
- **Fix:** Send `$/cancelRequest` on drop. Use `scopeguard` / `RemoveOnDrop` for `pending` entry. Killing LSP child per-cancel is over-aggressive (shared pool); cancel-request notification is the right granularity.
- **Confidence:** high.

### C3 — `request()` retries non-idempotent methods on `-32800`
- **Location:** `src/lsp/client.rs:478-501`.
- **Evidence:** Up to 10 retries on RequestCancelled. `rename` (`:1025-1052`) goes through `request("textDocument/rename", ...)`. If server cancels but applied edit before cancel, retry double-applies.
- **Fix:** Allowlist idempotent methods for retry (`documentSymbol`, `references`, `hover`, `definition`, `workspace/symbol`). For `rename`, surface cancel as `RecoverableError`.
- **Confidence:** medium.

---

## Important

### I1 — Stale-position regression test pattern missing for mux path
- **Location:** `src/lsp/mux/coherence_rust.rs::two_agents_coherent_after_edit`.
- **Evidence:** CLAUDE.md mandates three-query sandwich. This test is two-query (only post-invalidation fresh state). Future mux change that silently caches won't be flagged.
- **Fix:** Add stale-assertion step before didChange. ~10 LOC.
- **Confidence:** medium (didn't read full test body).

### I2 — Many LSP error paths use `anyhow::bail!` where `RecoverableError` fits CLAUDE.md spec
- **Locations:**
  - `src/lsp/client.rs:512` `bail!("LSP server is not running")`
  - `src/lsp/client.rs:559-565` timeout `bail!`
  - `src/lsp/manager.rs:441` circuit-breaker open
  - `src/lsp/manager.rs:471` mux start fail
  - `src/lsp/manager.rs:475` mux ready timeout (120s)
  - `src/lsp/client.rs:677-684` Kotlin "Multiple editing sessions" — canonical recoverable case (memory `gotchas` calls it out)
- **Fix:** Lift these to `RecoverableError` so sibling tool calls survive.
- **Confidence:** high.

### I3 — `terminate_process` SIGTERM only, no SIGKILL escalation; matches existing "orphaned kotlin-lsp" issue doc
- **Location:** `src/lsp/client.rs:1144-1168` (Drop); `src/platform/unix.rs:63-70`.
- **Fix:** After SIGTERM, detached cleanup task waits 5s then SIGKILL if pid alive. Or hold `Child` on `LspClient` directly so `kill_on_drop` works (requires reader-task ownership refactor).
- **Confidence:** high — confirmed by `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.

### I4 — `idle_eviction_loop` interval doesn't account for per-language TTL
- **Location:** `src/lsp/manager.rs:921-930` (loop), `:20-25` (`ttl_for_language`), `:892` (per-language filter).
- **Evidence:** Loop ticks at `global_ttl/4 = 7.5min`, but kotlin TTL is 2h. Eviction eligibility fires correct per-language; doc gap only — `pending_reason="idle_evicted"` event lags by up to 7.5min after the 2h mark.
- **Fix:** Doc clarification, OR tick at `min(per-language ttls)/4`. Low impact.
- **Confidence:** medium.

### I5 — `did_change` `wrapping_add` on `i32` violates LSP monotonicity contract
- **Location:** `src/lsp/client.rs:1099-1107`.
- **Evidence:** Comment justifies with "wrap is harmless in practice (sessions never reach 2 billion edits)" — magnitude correct. But LSP spec says version must be strictly monotonic per document; rust-analyzer/kotlin-lsp have rejected non-monotonic in past releases. Mux already uses `i64` (`mux/protocol.rs:62`) — inconsistency.
- **Fix:** `LspClient.open_files: HashMap<PathBuf, i64>` to match mux. Or `saturating_add(1)`.
- **Confidence:** medium.

### I6 — `MockLspClient` lacks fault injection — tests can't catch C1/C2/C3 class
- **Location:** `src/lsp/mock.rs`.
- **Evidence:** No timeouts, no crashes mid-request, no malformed responses, no `-32800` retry, no cancellation. Returns immediately or `bail!("no symbols configured")`.
- **Fix:** `with_failure_after(n)`, `with_slow_response(d)`, `with_recoverable_error(method)`. Required for Phase 1 I4 cancellation test coverage.
- **Confidence:** high.

### I7 — Cold-start budget consumed during init handshake → first post-init request gets warm budget
- **Location:** `src/lsp/client.rs:464-477` (cold-start window from `started_at`); `src/lsp/servers/mod.rs:13` (`jvm_timeout`).
- **Evidence:** `started_at` set at construction (before init). 5-min cold window includes init time. Slow kotlin-lsp finishing init at 4 min leaves ~1 min residual; first `documentSymbol` after init falls under "warm" budget (3 retries × 300ms ≈ 1.2s) and immediately fails.
- **Fix:** Reset `started_at` when init completes successfully (`init_completed_at`). One-liner.
- **Confidence:** high.

---

## Minor (grouped)

- **M1** — `src/lsp/client.rs:215` reads id `as_i64()`; brittle if mux ever returns string ids. Mux untags before dispatch in current code; confirm shared dispatch path.
- **M2** — `src/lsp/client.rs:1062` `did_close` canonicalize fallback to raw path → deleted-file close leaves stale `open_files` entry.
- **M3** — `src/lsp/manager.rs:447` mux uses `current_exe()`; binary replaced mid-session → protocol mismatch possible. Doc or pin.
- **M4** — `src/lsp/transport.rs:31` strict `"Content-Length: "` (single space) prefix match. `splitn(2, ':')` + trim more robust.
- **M5** — `src/lsp/mux/process.rs:212` `bind` after `remove_file` (see S5).
- **M6** — `src/lsp/manager.rs:425` flock try_lock then drop; window before mux child re-acquires. Tight in practice.
- **M7** — `src/lsp/client.rs:299` reader task `child.wait()` after read loop; wedged child means we never read exit status (cosmetic).
- **M8** — `src/lsp/mux/process.rs:417-435` `handle_server_message` "no id, no method" branch silently discards. Add debug log.
- **M9** — `src/lsp/mux/protocol.rs:42-46` `untag_response_id` for ids `> i64::MAX` won't dispatch via `as_i64()` (M1). Theoretical.

---

## Cross-check answers (Phase 1+2)

- **Phase 2 F4** — confirmed. LSP layer does NOT re-validate. F4 + S2 = real amplifier.
- **Phase 1 I4** — confirmed in different shape (C2). No `$/cancelRequest`, LSP-side work continues.
- **LSP child lifecycle on cancel** — not killed (correct: shared pool); on shutdown SIGTERM works but no SIGKILL escalation (I3).

---

## Open questions

1. `config.process_id: Some(std::process::id())` in `initialize` (`client.rs:589`) — LSP spec says server should exit if PID dies. Verified that kotlin-lsp / rust-analyzer honor this? If yes, orphaned LSP from cancelled subagent should self-terminate; if I3 is real, they don't or poll too rarely.
2. `DefaultHasher` (random-seeded) for socket paths (`mux/mod.rs:13-17`) — deliberate (no cross-restart collisions) or oversight (debug-unfriendly)? Stable hash is one line.
3. C3 — should non-idempotent methods be excluded from `-32800` retry?
4. Mux's `cached_capabilities` (`mux/process.rs:35`) accumulates from `client/registerCapability` — ever cleared, or grows until exit? Not a practical leak; worth confirming.
