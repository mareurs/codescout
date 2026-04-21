# Kotlin-LSP Concurrent Instance Failures

**Filed:** 2026-03-24
**Status:** Investigating
**Severity:** High — silently degrades all Kotlin LSP operations in multi-instance setups

## Symptom

When two codescout MCP server instances target the same Kotlin project, the second
instance's `list_symbols` and `find_symbol` calls fail with:

```
LSP error (code -32800): cancelled
```

The first instance works fine. The second instance spawns 40+ kotlin-lsp processes
in ~80 seconds, none of which can serve requests. Eventually the tool times out
after 60 seconds.

## Root Cause

Two codescout instances (`cdbc` at 11:44 and `ce25` at 12:06) both activated the
same backend-kotlin project. Instance `cdbc` successfully started kotlin-lsp and
held it alive. Instance `ce25` then tried to start its own kotlin-lsp — but
kotlin-lsp (JetBrains) uses a file-based workspace index (likely MVStore/H2) that
enforces single-writer access. Every kotlin-lsp spawned by `ce25` either:

1. Failed to acquire the workspace lock and crashed after initialization
2. Started in a degraded mode, returned -32800 to all requests, then exited

Evidence from diagnostic logs:
- `cdbc`: kotlin-lsp initialized in 7987ms, stayed alive through 12:22+
- `ce25`: 40+ "Starting LSP server: kotlin-lsp" lines, ZERO "LSP initialized"
  for kotlin, while typescript/javascript/python all initialized in <400ms

## Why It Was Hard to Diagnose

### 1. Diagnostic log level too restrictive
The diagnostic log (`--diagnostic`) uses `EnvFilter::new("info")` (src/logging.rs:146).
LSP reader errors are logged at `warn!` and process exits at `debug!` — both invisible.
We couldn't see kotlin-lsp crashing because the crash signal never made it to the log.

**Location:** `src/logging.rs:146`

### 2. -32800 errors masked as ok=true
`route_tool_error` (src/server.rs:295) returns `CallToolResult::success(...)` for -32800
to avoid aborting sibling parallel calls. This is correct for the MCP protocol, but the
diagnostic log's `ok` field derives from `is_error` — so -32800 shows as `ok=true`.

**Location:** `src/server.rs:198` — `let ok = call_result.is_error.map_or(true, |e| !e);`

### 3. No startup circuit-breaker
Each `find_symbol` retry (3 retries × 300ms backoff) triggers `get_or_start` which
spawns a fresh kotlin-lsp on every call since the previous one died. No backoff or
limit on repeated startup failures for the same language.

## Potential Solutions

### Fix 1: Bump diagnostic log to WARN level (quick win)
Change `src/logging.rs:146` from `EnvFilter::new("info")` to include warn-level LSP
messages. This makes crashes visible immediately.

**Effort:** Trivial (one line)
**Impact:** High — turns invisible failures into visible ones

### Fix 2: Add recoverable/degraded signal to diagnostic logs
Add a `recoverable=true` field or use a distinct log level for -32800 errors so they
don't appear as successful calls when grepping diagnostics.

**Effort:** Small
**Impact:** Medium — improves debuggability

### Fix 3: Startup circuit-breaker
Track per-language startup failure count + timestamps. After N failures in T seconds,
stop retrying and return a clear error: "kotlin-lsp failed to start N times in Ts —
another process may hold the workspace lock. Check for other codescout instances or
editors targeting this project."

**Effort:** Medium
**Impact:** High — prevents 40+ zombie processes and gives actionable error

### Fix 4: Per-instance workspace storage directory

kotlin-lsp supports `--system-path PATH` for its caches and indexes. If codescout
passes a unique directory per server instance, multiple instances can coexist.

**Implementation:**

1. In `src/lsp/servers/mod.rs`, for the kotlin config, generate a unique system path:
   ```rust
   let system_dir = std::env::temp_dir()
       .join(format!("codescout-kotlin-lsp-{}", std::process::id()));
   // ...
   args: vec![
       "--stdio".into(),
       format!("--system-path={}", system_dir.display()),
   ],
   ```
   Using `std::process::id()` guarantees uniqueness per codescout process.

2. Optionally clean up the directory on LSP shutdown (or let the OS handle `/tmp` cleanup).

3. Consider using a content-addressed path (hash of workspace root) so that the same
   codescout instance restarting reuses its warm index:
   ```rust
   let hash = crate::util::short_hash(&workspace_root);
   let system_dir = std::env::temp_dir()
       .join(format!("codescout-{}-kotlin-lsp", hash));
   ```
   But this reintroduces contention if two instances share a workspace. Using PID is safer.

**Trade-off:** PID-based dirs mean cold indexes on every restart. Hash-based dirs
mean warm indexes but potential contention (need a "try-lock" approach). For now,
PID is the safe choice — kotlin-lsp indexes fast enough (~8s cold start).

**Effort:** Small (10 lines in servers/mod.rs)
**Impact:** Very high — eliminates root cause
### Fix 5: LSP instance sharing across codescout instances
Instead of each codescout instance spawning its own kotlin-lsp, share a single
kotlin-lsp process via a local socket or IPC. This is a larger architectural change.

**Effort:** Large
**Impact:** Very high but complex

## Research: kotlin-lsp Concurrent Access

### Confirmed Facts

1. **`--system-path PATH`** is the CLI flag for kotlin-lsp caches and indexes
   - Confirmed via `kotlin-lsp --help` and bytecode inspection of `KotlinLspServerRunConfig`
   - The `systemPath` field is nullable — when null, kotlin-lsp calls `createTempDirectory`
   - IntelliJ properties set: `idea.config.path`, `idea.system.path`, `idea.home.path`

2. **GitHub Issue #108** ("Multiple LSP instances not supported") was closed Dec 10, 2025
   - Fix: `--system-path` flag enables workspace isolation
   - v261 release: "Indices are now stored in a dedicated folder and are properly shared
     between multiple projects and concurrent LS instances"

3. **Our installed version supports `--system-path`** — verified via `--help` output

4. **Without `--system-path`**, kotlin-lsp is supposed to create a temp dir per instance.
   But our evidence shows two instances conflicting without `--system-path`. Possible
   explanations:
   - The temp dir is deterministic (based on workspace path hash), not random
   - There's a shared resource beyond the system-path (e.g., Gradle daemon, build cache)
   - The IntelliJ platform has a secondary lock mechanism outside system-path

5. **Instance `cdbc` initialized kotlin-lsp in 7987ms** — normal for a cold JVM start
   - Instance `ce25` never got "LSP initialized" for kotlin despite 40+ spawn attempts
   - Other languages (typescript 83ms, javascript 86ms, python 377ms) initialized fine in `ce25`

### VS Code / Neovim Precedent

**JetBrains kotlin-lsp vs fwcd kotlin-language-server — two different servers:**
- JetBrains: storage configured ONLY via `--system-path` CLI arg (no `initializationOptions`)
- fwcd (community): uses `initializationOptions.storagePath` JSON key
- Many Neovim config snippets reference `storagePath` — that's for fwcd, not JetBrains

**How editors handle it:**
- VS Code Kotlin extension: passes `context.storageUri.fsPath` as `--system-path` at launch
- Neovim kotlin.nvim: `cmd = { "kotlin-lsp", "--stdio", "--system-path", vim.fn.stdpath("cache") .. "/kotlin-lsp" }`
- Emacs lsp-kotlin: `lsp-kotlin-workspace-cache-dir` (wraps the CLI arg)

**Without `--system-path`:** kotlin-lsp creates a temp dir that is deleted on exit.
Yet our two instances still conflicted — possibly through Gradle daemon sharing,
IntelliJ platform singleton detection, or a non-obvious shared resource. Passing
explicit `--system-path` per instance eliminates this regardless of the exact
sub-cause.
### Our Current Config (src/lsp/servers/mod.rs)

```rust
"kotlin" => Some(LspServerConfig {
    command: crate::platform::lsp_binary_name("kotlin-lsp"),
    args: vec!["--stdio".into()],
    workspace_root: root,
    init_timeout: jvm_timeout,  // 300s
}),
```

No `--system-path` passed. Fix: add `--system-path=<unique-dir>` to args.
## Implementation Priority

| # | Fix | Effort | Impact | Status |
|---|-----|--------|--------|--------|
| 1 | **Diagnostic log: promote LSP crash signals to WARN** | Trivial | High | Done |
| 2 | **Per-instance `--system-path` for kotlin-lsp** | Small | Very High | Done |
| 3 | **LSP startup circuit-breaker** (N failures → stop + clear error) | Medium | High | Done |
| 4 | **WARN log for -32800 recoverable errors** | Small | Medium | Done |
| 5 | **Retry -32800 during LSP initialize handshake** | Small | Very High | Done |
| 6 | **Clean up `/tmp/codescout-*-kotlin-lsp` dirs on shutdown** | Small | Low | Roadmap |

Fix 5 was the critical missing piece — kotlin-lsp returns -32800 during `initialize`
when its JVM hasn't finished bootstrapping, and our `initialize()` had zero retries.
Now retries 5× with 3s backoff (~30s window), covering kotlin-lsp's 10-15s cold start.

Fix 6 is low priority: dirs are PID-scoped, tiny (4KB), and in `/tmp` (OS cleans on
reboot). Accumulation is negligible. Track in roadmap, don't block on it.

## Follow-up: Orphaned kotlin-lsp Processes from Subagents

**Status:** Open

When subagents spawn their own codescout processes (each with its own kotlin-lsp),
the kotlin-lsp process may outlive the subagent's Claude Code session. The codescout
process stays alive (MCP stdio pipe is still connected to something), and its
kotlin-lsp child holds the workspace session lock — blocking all other kotlin-lsp
instances for that project.

**Observed:** After our parallel test, PID 1269847 (kotlin-lsp, 2.2GB RSS) was still
running 25 minutes after its parent subagent finished. It held the "Multiple editing
sessions" lock, causing the other Claude Code instance's kotlin-lsp to fail.

**Root cause:** `kill_on_drop` on the child process only fires when the codescout
process itself exits. If codescout's MCP connection is held open (e.g., by a Claude
Code process that hasn't fully cleaned up), kotlin-lsp lives indefinitely.

**Potential fixes:**
- Add idle detection to LspManager: if no tool calls for N minutes, shut down
  kotlin-lsp proactively (the 2h TTL override for kotlin is too generous)
- Investigate why subagent codescout processes linger after subagent completion

## Follow-up: Intra-instance Cold Start Contention

**Status:** Open — less severe than the original inter-instance issue

### Symptom

When multiple parallel tool calls arrive during kotlin-lsp's ~8s cold start within
a single codescout instance, the first caller triggers `get_or_start` which spawns
and initializes kotlin-lsp. The barrier (`starting` watch channel) correctly serializes
concurrent callers — they wait for init to complete. But immediately after initialization,
kotlin-lsp is still running background indexing (Gradle import, symbol resolution).
Requests sent during this window get -32800 (RequestCancelled), and the retry loop
(3 × 300ms backoff = ~1.2s total) is too short for the indexing phase which can take
1-5 minutes on large projects.

### Evidence (2026-03-24 parallel test)

3 Explore agents fired simultaneously against backend-kotlin:
- Agent 2: all 3 queries succeeded (hit warm kotlin-lsp)
- Agent 1: 1/3 succeeded, circuit-breaker tripped after 7 failures in 5s
- Agent 3: 1/3 succeeded (tree-sitter fallback), 1 wrong symbol name, 1 unsupported op

The circuit-breaker correctly prevented zombie process spawning, but legitimate
queries were blocked for 60s until the breaker window expired.

### Potential Fixes

**Option A: Post-init readiness probe**
After `initialize` succeeds, send a lightweight probe request (e.g.,
`workspace/symbol` with query `"__readiness_check__"`) in a retry loop with
exponential backoff (1s, 2s, 4s...) up to 60s. Only mark the server as "ready"
and return from `get_or_start` once the probe succeeds. This delays all callers
equally but guarantees the first real request works.

**Effort:** Medium — changes `do_start` flow
**Risk:** Adds latency to first use; probe might itself return stale results

**Option B: Longer retry window for recently-started servers**
Track when kotlin-lsp was started. If `request()` gets -32800 within the first
N minutes of server life, use a longer retry window (e.g., 10 retries × 3s = 30s)
instead of the default 3 × 300ms. This is transparent to callers.

**Effort:** Small — changes `request()` retry logic
**Risk:** Slow failure reporting if the server is genuinely broken (not just indexing)

**Option C: Circuit-breaker grace period for cold starts**
Don't count failures toward the circuit-breaker within the first N seconds after
a successful `do_start`. This lets the retry logic handle transient -32800s during
indexing without tripping the breaker.

**Effort:** Small — changes circuit-breaker check
**Risk:** Could allow a burst of zombie processes if init succeeds but server
immediately crashes (unlikely)

**Recommended:** Option B + C together. B gives queries a fighting chance during
indexing; C prevents the breaker from blocking them prematurely.

## Timeline

| Time | Instance | Event |
|------|----------|-------|
| 11:44:24 | cdbc | Started, activated backend-kotlin |
| 11:51:52 | cdbc | kotlin-lsp initialized (7987ms) |
| 12:06:34 | ce25 | Started, activated backend-kotlin |
| 12:08:37 | ce25 | First kotlin-lsp start attempt (40+ follow) |
| 12:08:37 | ce25 | typescript/js/python initialized OK |
| 12:09:59 | ce25 | find_symbol times out (60001ms, ok=false) |
| 12:10:27 | ce25 | 3× list_symbols "succeed" (ok=true = masked -32800) |
| 12:22:24 | cdbc | kotlin still alive in heartbeat |

## Resolution (2026-04-16)

**Status: FIXED by LSP multiplexer — `src/lsp/mux/`.**

Commit `a4bf02f feat: kotlin-lsp multiplexer — share one JVM across codescout instances` implemented Fix 5 (LSP instance sharing across codescout instances). Architecture:

- **One mux process per (language, workspace)**: `socket_path_for_workspace(lang, root)` hashes the workspace root into a deterministic Unix socket path under `/tmp/codescout-mux-<hash>-<lang>.sock`. Both codescout instances resolve to the same socket.
- **File-lock arbitration**: `get_or_start_via_mux` (`src/lsp/manager.rs:335`) uses `fs2::FileExt::try_lock_exclusive` on a sibling `.lock` file. Whichever codescout wins the lock spawns the mux child (a detached `codescout mux` subprocess); the loser sees the lock held and connects to the existing socket.
- **Request multiplexing**: `ClientTag` tags each inbound request ID (`tag_request_id` / `untag_response_id` in `src/lsp/mux/protocol.rs`) so responses route back to the originating codescout instance.
- **Document fan-out**: `DocumentState` tracks open URIs per client so `didClose` from one client doesn't kick other clients' open documents.
- **Idle shutdown**: `--idle-timeout 300` — mux exits after 5 minutes with no connected clients. Releases the JVM + the MVStore workspace lock.

Net effect: two codescout instances on the same Kotlin project now share a single kotlin-lsp JVM, which is also the only process holding the MVStore workspace lock. The original `-32800`/`ok=true` mask, the 40+ failed start attempts, and the 60s find_symbol timeouts are all impossible in this topology.

### Tests
- 21 mux unit tests in `src/lsp/mux/` (protocol, tag/untag, document state, socket/lock path determinism).
- No dedicated integration test for "two concurrent codescout instances on same kotlin project" — the original repro. Could be added but requires orchestrating two subprocess instances; cost/benefit is weak given the architectural fix.

### Follow-ups still open
- **Intra-instance cold start contention** — FIXED 2026-04-16: Option B (cold-start retry patience in `LspClient::request`) + Option C (circuit-breaker grace period in `LspManager`). See commit `5a76d09`.
- **Orphaned subagent kotlin-lsp processes** — closed; kotlin TTL is 2h and memory is plentiful. Not a problem in practice.
