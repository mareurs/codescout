# Phase 1 — Core / Server / Agent

**Date:** 2026-04-24
**Scope:** `src/lib.rs`, `src/main.rs`, `src/server.rs`, `src/workspace.rs`, `src/agent/`, `src/config/`
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Security (Ibex taxonomy)

### S1 — HIGH — Weak fallback auth token for HTTP transport
- **Location:** `src/server.rs:1006-1043` (closure inside `auth_token.unwrap_or_else(...)` in `run`); also `src/server.rs:771-781` (`generate_auth_token`).
- **Evidence:** Primary path reads `/dev/urandom` (OK). Fallback seeds `DefaultHasher` (SipHash-1-3, NOT a CSPRNG) with `process::id()` + `thread::current().id()` + `SystemTime::now()` — predictable. `b % 62` biases the alphabet. Separately, `pub fn generate_auth_token` builds a 32-hex token from nanosecond timestamp + PID; comment admits "NOT cryptographically secure" but is `pub`.
- **Exploit:** On a host where `/dev/urandom` is unreadable, bearer is recoverable in seconds (guess start time + PID range). Bearer guards `/mcp` → file read/write/shell exec.
- **Fix:** (a) Use `rand::rngs::OsRng` / `getrandom::getrandom` for both paths; abort startup on hard failure rather than producing a weak token. (b) Delete or `#[deprecated]` `generate_auth_token`. (c) Use `subtle::ConstantTimeEq` for bearer compare at `:1064`.
- **Confidence:** high (fallback + `generate_auth_token`); medium (timing).

### S2 — MEDIUM — `normalize_path` does not clamp to `base`
- **Location:** `src/workspace.rs:402-417`
- **Evidence:** Pops on `ParentDir` without enforcing `base` floor. `normalize_path("/home/u/project", "../../../etc")` → `/etc`. Used today only for matching, but `pub`-visible — next caller wiring it to a sink gets path traversal.
- **Fix:** Assert `result.starts_with(base)` post-loop; on violation return `Option<PathBuf>` and propagate rejection. Document as "trusted-input only" if kept permissive.
- **Confidence:** medium (footgun, not exploitable today).

### S3 — MEDIUM — `resolve_project_for_path` no canonicalize before prefix match
- **Location:** `src/workspace.rs:203-225`; caller `Workspace::resolve_root` at `:310-329`.
- **Evidence:** Lexical `starts_with` only; no canonicalize, no symlink resolve, no `..` normalization. `/home/u/project/../../etc/passwd` lexically matches.
- **Exploit:** Tool receives `read_file(path="subproj/../../../etc/passwd")` from MCP client. Workspace lexically resolves to sub-project and returns its root. If downstream sink trusts "workspace resolved → path is in project" without re-checking, project boundary bypassed. Sink behavior is out of Phase 1 scope.
- **Fix:** Lexically normalize `..` before prefix match in `resolve_project_for_path`; require downstream to re-validate via `path_security`. Better: canonicalize when path exists; reject `..` escapes for non-existent paths.
- **Confidence:** medium — depends on Phase 2 (tools) sink behavior.

### S4 — LOW — Verbose anyhow errors flow to MCP responses
- **Location:** `src/server.rs:763` (`route_tool_error` non-recoverable arm); `:233-234` (`McpError::internal_error`).
- **Evidence:** Non-`RecoverableError` errors stringified into MCP response, including absolute paths from `with_context` chains.
- **Exploit:** Over HTTP transport, attacker who reaches a tool learns absolute paths via error oracles.
- **Fix:** Either (a) accept as design + document HTTP = trusted operator, or (b) sanitize fatal errors in `route_tool_error`, log details server-side via `tracing::error!`.
- **Confidence:** medium that it's IDOR-class info disclosure; low that it matters given bearer model.

### S5 — LOW — Config: size cap exists but no element-count limits
- **Location:** `src/config/global.rs:69-75`; `src/config/project.rs:~340`.
- **Evidence:** Both files cap raw bytes at 1 MiB. But `SecuritySection.extra_write_roots`, `shell_dangerous_patterns`, `IgnoredPathsSection.patterns` have no element-count limits. A 1 MiB TOML can encode tens of thousands of regex patterns evaluated on every shell tool call.
- **Fix:** Add `max_len` invariants on the lists in deserialize, or assert post-deserialize. Skip if DOS out of scope.
- **Confidence:** low (DOS-class).

### S6 — QUESTION — `EMBED_API_KEY` / `api_key` handling
- **Location:** `src/config/project.rs:73` (field) + `:43` (`derive(Debug)`).
- **Evidence:** `api_key: Option<String>` in plain string. Struct derives `Debug` and `Serialize`. Risk: `tracing::debug!(?config)` somewhere out of scope leaks key; or any diagnostic dump of `ProjectConfig` over the wire.
- **Question:** Is `ProjectConfig` ever serialized to a tool response, log line, or MCP resource? If yes, mask `api_key` (consider `zeroize`).
- **Confidence:** medium — needs Phase 2/8 confirmation.

---

## Critical (non-security)

### C1 — `std::Mutex::lock().unwrap()` in async hot path
- **Location:** `src/server.rs:629-635` (`last_broadcast_caps.lock().unwrap()` inside `async fn call_tool`); `:359` (`instructions.write().unwrap()`).
- **Evidence:** Std `Mutex`/`RwLock` poison panic propagates. Held briefly today (no `.await` while held), but invites breakage.
- **Fix:** `parking_lot` (poison-free) or `tokio::sync::Mutex` since call sites are async; or `expect()` with documented invariant.
- **Confidence:** medium.

### C2 — Cross-process write lock unbounded across in-process queue
- **Location:** `src/agent/write_guard.rs:48-65`; default timeout `src/config/project.rs:209`.
- **Evidence:** `async_mutex.lock_owned().await` taken before `spawn_blocking` polling `try_lock_exclusive` (up to 60s). Per-call wait ceiling is `60s × queue_depth` — no overall deadline.
- **Fix:** Wrap async-mutex acquire in `tokio::time::timeout(timeout, ...)` for total-wait bound, or document the queue semantics.
- **Confidence:** high on behavior; medium on impact.

---

## Important

### I1 — `activate()` rebuilds `ActiveProject` mid-write → write-lock desync
- **Location:** `src/agent/mod.rs:146-232` (`Agent::new`) vs `:234-324` (`activate`).
- **Evidence:** `activate()` creates a fresh `ActiveProject` with new `dirty_files`, `write_lock`, `file_lock`. In-flight tool holding the prior `tokio::Mutex<()>` / `Arc<File>` does not serialize against the new writer. Two concurrent writers possible against the same project root.
- **Fix:** Reuse existing `write_lock` / `file_lock` / `dirty_files` when re-activating the same root; only create new ones for a different root. Or serialize all `activate()` behind the workspace's existing write lock.
- **Confidence:** medium — depends on whether `activate_project` is reachable concurrently with tool calls (with rmcp per-request task spawning, yes).

### I2 — `discover_projects` blocking I/O + unbounded manifest parse on async path
- **Location:** `src/workspace.rs:42-46` (`max_depth(Some(max_depth + 1))`); `:90-110` (`package.json` parse).
- **Evidence:** Every `package.json` is `read_to_string` + `serde_json::from_str` during workspace construction (every `Agent::new` and `activate()`). No size limit. Synchronous I/O on Tokio runtime.
- **Fix:** (a) Cap manifest read size (~1 MiB matches existing precedent); (b) wrap `discover_projects` in `spawn_blocking`.
- **Confidence:** high.

### I3 — `WRITE_TOOLS` hardcoded list; new write tool silently bypasses lock
- **Location:** `src/server.rs:45-76` (`WRITE_TOOLS` const + `MEMORY_WRITE_ACTIONS`).
- **Evidence:** Write-lock acquisition gated on string-name allowlist, not on a `Tool` trait property. New mutating tool not added to the list runs without cross-process write lock.
- **Fix:** Add `fn is_write(&self, input: &Value) -> bool { false }` default on `Tool` trait; override per write tool; have `acquire_write_guard_if_writing` ask the tool. Removes hardcoded list.
- **Confidence:** high on design; need Phase 2 scan for actual missed tool.

### I4 — Cancelled write tasks park via `pending()` while still holding write guard
- **Location:** `src/server.rs:266-278` and `:294-303`.
- **Evidence:** On client cancel, task `await`s `std::future::pending()` until rmcp drops it on connection close. `write_guard` is bound outside `tool_call_fut` and dropped after `race_against_cancel` returns; on cancel the parked task never reaches the drop, so the cross-process write guard stays held until disconnect.
- **Impact:** User pressing Escape on a write tool leaves the project's write lock held until the MCP connection tears down. Successive writes in the same session timeout.
- **Fix:** Drop the write guard explicitly before parking — restructure so the cancel arm drops the guard before `pending()`. Simplest: move `pending().await` to caller, after `drop(write_guard)`.
- **Confidence:** high. Real correctness bug.

### I5 — `git2::Repository::open` on every `list_tools`
- **Location:** `src/server.rs:387-395` (`current_capabilities`).
- **Evidence:** `git2::Repository::open(&root)` walks parent dirs every `current_capabilities` call (called from `list_tools` and after every `activate_project`). Performance smell.
- **Fix:** Cache; invalidate on activate.
- **Confidence:** medium.

### I6 — `Workspace::new` (line 271) — body unverified
- Initial focus selection from discovered project list. If a non-root focus is picked by accident, security boundary surprises elsewhere. Worth a separate look.

---

## Minor / grouped

- **`.unwrap()` in deep paths** (`server.rs:630`, `:359`; `mod.rs:474` "safe: found above") — annotate or `expect()` with precise invariant.
- **`AgentInner::with_project` shape** — both `&` and `&mut` accessors over `Arc<RwLock<…>>` plus `OwnedMutex` makes locking story fragile. Consider closure-style `with_project_mut` under explicit write half.
- **`memory_dir_for_project` defaults `is_root = true` on missing project ID** (`workspace.rs:373` `unwrap_or(true)`) — silent fallback masks bugs. Prefer `Result` or `unwrap_or(false)`.
- **Canonicalization inconsistency** — `Agent::new` canonicalizes startup project (`mod.rs:151`); `activate()` does not. `is_home` comparison at `mod.rs:262` then compares canonical home_root vs possibly-non-canonical activate target. Canonicalize in `activate` too, or document caller responsibility.
- **Doc-comment wrong on `project_explicitly_activated`** — `Agent::new` doc claims operator chose write target → flag = true. But `--project` omitted + CWD fallback (`server.rs:893-896`) still flips it. Affects activation banner.
- **`shutdown_signal()` body unread** — verify SIGTERM/SIGINT handling.
- **CLAUDE.md compliance broadly good in this layer.** `route_tool_error` correctly distinguishes `RecoverableError`. No echo on writes (writes go through tools). No prompt-surface violations here.
- **`run` does `std::fs::canonicalize(&p).unwrap_or(p)`** at `server.rs:895` — silently falling back on permission error defeats the purpose. Prefer fail-fast.

---

## Open questions (need Phase 2+ context)

1. Does any `tools/` sink re-validate paths against project root, or trust `Workspace::resolve_root`? — gates S3 severity.
2. Where does `EMBED_API_KEY` flow after deserialization? Is `ProjectConfig` ever rendered to wire? — gates S6.
3. Does the cancel test suite cover I4's write-guard-held-during-pending case?
4. Does rmcp synchronously drop `tool_call_fut` captures when cancel arm wins? If not, `kill_on_drop` semantics in the comment break.
5. `StreamableHttpService.clone()` at `server.rs:990` — confirm the `Agent` clone shares `Arc`s (so write lock is process-wide), not duplicates.
