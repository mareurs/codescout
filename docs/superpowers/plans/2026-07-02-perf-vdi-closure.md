# Perf Sprint + VDI Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut codescout's measured latency hot spots (activation 5.8s avg, LSP cold-start spikes to 21.9s), close WIN-5, lock Windows support in CI, and validate the shipped lite stack — per `docs/superpowers/specs/2026-07-02-perf-vdi-closure-design.md`.

**Architecture:** Instrument-first: a tiny `PhaseTimer` emits structured timing events on the activation and retrieval hot paths, then three targeted fixes land behind that telemetry — a session cache for the per-activation vector-store probe, a `spawn_blocking`+timeout wrapper around the LSP process spawn (WIN-5), and a bounded "first-call budget" that serves tree-sitter output with a warming hint instead of blocking on LSP cold start. Windows closure is CI (MinGW+wine job), tracker repair, benchmark, and a user-run VDI pass.

**Tech Stack:** Rust (tokio, tracing, async-trait), GitHub Actions, existing benchmark harness (`scripts/run-tc-benchmark.sh`), librarian MCP tools.

## Global Constraints

- All work on branch `experiments`; never commit to `master` directly (cherry-pick later per `docs/RELEASE.md`).
- Pre-commit gate on EVERY task: `cargo fmt` && `cargo clippy -- -D warnings` && `cargo test`. All three must pass. No exceptions.
- Implementer subagents MUST use codescout MCP tools (`symbols`, `read_file`, `edit_code`, `edit_file`, `run_command`, `read_markdown`/`edit_markdown`); native `Bash`/`Edit`/`Write` on source files are hard-denied by the companion plugin.
- Error handling: `RecoverableError` for expected input-driven failures (`isError: false`); `anyhow::bail!` for genuine failures. "LSP warming" is a NORMAL result with a marker field, never an error.
- Timing budgets (exact values, from the spec): first index probe `500ms`; LSP first-call budget `2s`; LSP spawn timeout `10s`.
- Response-schema additions limited to: `"lsp": "warming"` + `"hint"` on symbols output. No tool renames (prompt-surface tests guard this).
- Commit style: conventional commits (`feat`/`fix`/`docs`/`chore`/`test`), imperative subject ≤72 chars, ending with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Line/symbol references below were verified 2026-07-02 on `experiments` (ad67db9a). If a file drifted, re-locate with `symbols`/`grep` and keep the shape.

---

### Task 1: PhaseTimer + activation instrumentation

**Files:**
- Create: `src/perf.rs`
- Modify: `src/lib.rs` (register module)
- Modify: `src/tools/config/mod.rs` (`ActivateProject::call` lines ~120-203, `build_activation_response` lines ~424-591)
- Test: inline `#[cfg(test)]` in `src/perf.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `crate::perf::PhaseTimer` — `PhaseTimer::start(op: &'static str) -> Self`, `fn lap(&mut self, name: &'static str)`, `fn finish(self)` (emits one `tracing::info!` event, target `codescout::perf`). Tasks 2 and 3 rely on these exact names.

- [ ] **Step 1: Write the failing test** — create `src/perf.rs` with only the test module:

```rust
//! Lightweight wall-clock phase timing for hot paths (activation, retrieval).
//!
//! Emits ONE structured `tracing` event per timed operation under the
//! `codescout::perf` target, e.g.:
//! `INFO codescout::perf: op="activate_project" phases=[("agent_activate", 412), ...] total_ms=812`
//!
//! Permanent instrumentation: the numbers that justified the activation/LSP
//! perf fixes are also the regression alarm that protects them.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lap_records_each_phase_in_order() {
        let mut t = PhaseTimer::start("test_op");
        t.lap("a");
        std::thread::sleep(std::time::Duration::from_millis(5));
        t.lap("b");
        assert_eq!(t.phases.len(), 2);
        assert_eq!(t.phases[0].0, "a");
        assert_eq!(t.phases[1].0, "b");
        assert!(t.phases[1].1 >= 5, "second lap must include the sleep");
        t.finish(); // must not panic
    }
}
```

Register in `src/lib.rs` alongside the existing `pub mod` list: `pub mod perf;`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib perf:: 2>&1 | tail -20`
Expected: COMPILE ERROR — `cannot find struct PhaseTimer`.

- [ ] **Step 3: Write minimal implementation** — add above the test module in `src/perf.rs`:

```rust
use std::time::Instant;

pub struct PhaseTimer {
    op: &'static str,
    t0: Instant,
    last: Instant,
    phases: Vec<(&'static str, u128)>,
}

impl PhaseTimer {
    pub fn start(op: &'static str) -> Self {
        let now = Instant::now();
        Self { op, t0: now, last: now, phases: Vec::new() }
    }

    /// Record the time since the previous lap (or start) under `name`.
    pub fn lap(&mut self, name: &'static str) {
        let now = Instant::now();
        self.phases.push((name, now.duration_since(self.last).as_millis()));
        self.last = now;
    }

    /// Emit the single summary event. Consumes the timer.
    pub fn finish(self) {
        tracing::info!(
            target: "codescout::perf",
            op = self.op,
            phases = ?self.phases,
            total_ms = self.t0.elapsed().as_millis() as u64,
        );
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib perf:: 2>&1 | tail -5`
Expected: `test perf::tests::lap_records_each_phase_in_order ... ok`

- [ ] **Step 5: Wire laps into `ActivateProject::call`** (full-activation arm, `src/tools/config/mod.rs`). After `let had_home = ...` insert `let mut timer = crate::perf::PhaseTimer::start("activate_project");`, then lap after each awaited phase:

```rust
ctx.agent.activate(root.clone(), read_only).await?;
timer.lap("agent_activate");
```

```rust
let concurrent_warning = ctx.agent.note_activation(&root).await;
timer.lap("note_activation");
let auto_registered = crate::library::auto_register::auto_register_deps(&root, ctx).await;
timer.lap("auto_register_deps");
let mut resp = build_activation_response(ctx, scenario, &auto_registered).await?;
timer.lap("build_response");
timer.finish();
```

(`prewarm_lsp_background` needs no lap — it is already fire-and-forget.)

- [ ] **Step 6: Wire laps into `build_activation_response`**: `let mut timer = crate::perf::PhaseTimer::start("activation_response");` at fn top, then `timer.lap("project_snapshot")` after the `with_project_at` block, `timer.lap("check_has_index")` after `let has_index = ...`, `timer.lap("workspace_summary")` after `let workspace = ...`, `timer.lap("probe_project_hints")` after `let hints = ...`, and `timer.finish();` immediately before `Ok(result)`.

- [ ] **Step 7: Full gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test --lib 2>&1 | tail -3`
Expected: clippy clean; all lib tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/perf.rs src/lib.rs src/tools/config/mod.rs
git commit -m "feat(perf): PhaseTimer + activation phase instrumentation"
```

- [ ] **Step 9 (verification, after next MCP reload):** `cargo build --release --features server-stack`, reconnect `/mcp`, activate a project, then `grep "codescout::perf" .codescout/debug.log | tail -3` — expect one `activate_project` and one `activation_response` event with per-phase ms.

---

### Task 2: Session cache for the per-activation index-status probe

**Files:**
- Modify: `src/tools/config/mod.rs` (add cache + `check_has_index_cached`; switch the one call site in `build_activation_response`)
- Test: `src/tools/config/tests.rs`

**Interfaces:**
- Consumes: existing `check_has_index(project_id: &str, project_root: &Path) -> bool` (async, `src/tools/config/mod.rs:409-421`) — unchanged, stays the probe primitive.
- Produces: `check_has_index_cached(project_id: &str, project_root: &Path) -> bool` (async, module-private) + module-private `index_status_get/put`. No public API change; `index.status` response field semantics become "last-known" (one-activation staleness).

- [ ] **Step 1: Write the failing test** — append to `src/tools/config/tests.rs` (three-query sandwich per the conventions memory; `#[serial]` because `check_has_index` reads retrieval env):

```rust
#[tokio::test]
#[serial_test::serial]
async fn index_status_cache_serves_stale_then_refreshes() {
    // Unique key so the process-global cache can't collide across tests.
    let pid = format!("cache-sandwich-{}", std::process::id());
    let root = std::env::temp_dir();

    // 1. Baseline: no entry -> bounded live probe (stack offline in tests => false), cached.
    assert!(!super::check_has_index_cached(&pid, &root).await);
    assert_eq!(super::index_status_get(&pid), Some(false));

    // 2. Assert-STALE: seed true; the cached value must be returned even though
    //    a live probe would say false. Regression step — fails if the cache is
    //    ever bypassed for an eager re-probe.
    super::index_status_put(&pid, true);
    assert!(super::check_has_index_cached(&pid, &root).await);

    // 3. Invalidate -> fresh: remove the entry; next call re-probes (false).
    super::index_status_remove(&pid);
    assert!(!super::check_has_index_cached(&pid, &root).await);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib index_status_cache_serves_stale 2>&1 | tail -10`
Expected: COMPILE ERROR — `check_has_index_cached` / `index_status_get` not found.

- [ ] **Step 3: Implement** — add to `src/tools/config/mod.rs` directly below `check_has_index`:

```rust
/// Session-scoped last-known index status per project id. Avoids a vector-store
/// round-trip on every activation: `index.status` is a hint field where
/// one-activation staleness is acceptable, a per-activation network probe is not.
static INDEX_STATUS_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, bool>>,
> = std::sync::OnceLock::new();

/// A slow or hung vector store must not stall the first activation either.
const FIRST_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

fn index_status_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, bool>> {
    INDEX_STATUS_CACHE.get_or_init(Default::default)
}

fn index_status_get(project_id: &str) -> Option<bool> {
    index_status_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(project_id)
        .copied()
}

fn index_status_put(project_id: &str, has_index: bool) {
    index_status_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(project_id.to_string(), has_index);
}

#[cfg(test)]
fn index_status_remove(project_id: &str) {
    index_status_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(project_id);
}

/// Cached wrapper: cache hit returns immediately and refreshes in a detached
/// task; first-per-session probe is bounded by `FIRST_PROBE_TIMEOUT`
/// (timeout => `false`, corrected by the background refresh on the next
/// activation).
async fn check_has_index_cached(project_id: &str, project_root: &std::path::Path) -> bool {
    if let Some(cached) = index_status_get(project_id) {
        let pid = project_id.to_string();
        let root = project_root.to_path_buf();
        tokio::spawn(async move {
            let fresh = check_has_index(&pid, &root).await;
            index_status_put(&pid, fresh);
        });
        return cached;
    }
    let fresh = tokio::time::timeout(FIRST_PROBE_TIMEOUT, check_has_index(project_id, project_root))
        .await
        .unwrap_or(false);
    index_status_put(project_id, fresh);
    fresh
}
```

Switch the call site in `build_activation_response`:
`let has_index = check_has_index(&project_name, &project_root_path).await;`
→ `let has_index = check_has_index_cached(&project_name, &project_root_path).await;`

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib index_status_cache_serves_stale 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 5: Full gate** — `cargo fmt && cargo clippy -- -D warnings && cargo test --lib 2>&1 | tail -3`

- [ ] **Step 6: Commit**

```bash
git add src/tools/config/mod.rs src/tools/config/tests.rs
git commit -m "feat(perf): cache index-status probe; bound first probe to 500ms"
```

---

### Task 3: Retrieval phase instrumentation (measure-only)

**Files:**
- Modify: `src/retrieval/search.rs` (`RetrievalClient::search_in`, starts line ~58)

**Interfaces:**
- Consumes: `crate::perf::PhaseTimer` (Task 1).
- Produces: `codescout::perf` events with `op="semantic_search"`, phases `embed` / `vector_query` / `rerank` (or `rerank_degraded`). No behavior change.

- [ ] **Step 1: Rewrite `search_in` with laps** (telemetry-only change; the three early-return paths each need `timer.finish()`):

```rust
    /// Core helper: embed → query (hybrid or dense-only) → optional rerank.
    async fn search_in(
        &self,
        collection: &str,
        project_id: &str,
        query: &str,
        opts: SearchOpts,
    ) -> Result<Vec<Hit>> {
        let mut timer = crate::perf::PhaseTimer::start("semantic_search");
        let q = self.embedder.embed(query).await?;
        timer.lap("embed");
        let candidates = self
            .code_store
            .query(
                collection,
                project_id,
                &q.dense,
                &q.sparse,
                opts.overfetch,
                self.config.bm25_boost,
                self.config.disable_sparse,
                &opts.exclude_languages,
            )
            .await?;
        timer.lap("vector_query");

        // Lite stack has no reranker server — skip the rerank step entirely.
        if !opts.rerank || self.lite || candidates.is_empty() {
            timer.finish();
            return Ok(candidates.into_iter().take(opts.limit).collect());
        }

        let texts: Vec<String> = candidates.iter().map(|h| h.content.clone()).collect();
        match self.reranker.rerank(query, &texts).await {
            Ok(scores) => {
                timer.lap("rerank");
                timer.finish();
                let mut zipped: Vec<(Hit, f32)> = candidates.into_iter().zip(scores).collect();
                zipped.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                Ok(zipped
                    .into_iter()
                    .take(opts.limit)
                    .map(|(mut h, s)| {
                        h.rerank_score = Some(s);
                        h
                    })
                    .collect())
            }
            Err(e) => {
                timer.lap("rerank_degraded");
                timer.finish();
                tracing::warn!("reranker degraded: {e}");
                Ok(candidates.into_iter().take(opts.limit).collect())
            }
        }
    }
```

- [ ] **Step 2: Full gate** — `cargo fmt && cargo clippy -- -D warnings && cargo test --lib 2>&1 | tail -3`
Expected: no new failures (telemetry-only; existing retrieval tests cover behavior).

- [ ] **Step 3: Commit**

```bash
git add src/retrieval/search.rs
git commit -m "feat(perf): instrument semantic_search embed/query/rerank phases"
```

---

### Task 4: Bounded LSP process spawn (WIN-5 core)

**Files:**
- Modify: `src/lsp/client.rs` (`LspClient::start`, spawn site lines ~364-380)

**Interfaces:**
- Consumes: nothing new.
- Produces: no signature change — `LspClient::start(config) -> Result<Self>` behavior hardened: the OS spawn syscall runs on a blocking thread with a `10s` timeout.

**Why (context for the implementer):** `tokio::process::Command::spawn()` calls the OS process-creation syscall *synchronously on the calling thread*. On Windows under EDR (CrowdStrike injects into process creation) `CreateProcessW` can hang indefinitely, pinning a tokio worker — this is WIN-5 in `docs/trackers/windows-platform-support.md`. `spawn_blocking` isolates the syscall; `timeout` unpins the caller. If the blocked thread eventually returns, the orphaned `Child` is dropped and `kill_on_drop(true)` (already set) reaps it. There is **no portable failing test for a hung spawn syscall** — regression coverage is the existing `start()`-exercising tests (rust-analyzer-gated), the wine gate (Task 7), and ultimately the VDI (Task 11). Do not invent a mock for this.

- [ ] **Step 1: Replace the spawn block.** Current code:

```rust
        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to start LSP server: {}", config.command))?;
```

New code (add `const SPAWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);` above `impl`-level or directly above `start`):

```rust
        // WIN-5: the spawn syscall is synchronous (CreateProcessW on Windows) and
        // can hang under EDR. Run it on a blocking thread, bounded. On timeout the
        // detached task keeps running; when the syscall eventually returns, the
        // Child is dropped and kill_on_drop reaps it.
        let command_name = config.command.clone();
        let mut child = match tokio::time::timeout(
            SPAWN_TIMEOUT,
            tokio::task::spawn_blocking(move || cmd.spawn()),
        )
        .await
        {
            Err(_elapsed) => anyhow::bail!(
                "LSP spawn for '{}' exceeded {}s — OS-level process creation hung \
                 (known cause: EDR interference on Windows; see WIN-5)",
                command_name,
                SPAWN_TIMEOUT.as_secs()
            ),
            Ok(Err(join_err)) => {
                return Err(anyhow::anyhow!("LSP spawn task panicked: {join_err}"))
            }
            Ok(Ok(spawn_res)) => spawn_res
                .with_context(|| format!("Failed to start LSP server: {}", command_name))?,
        };
```

Note: `cmd` is moved into the closure — every use of `config.command` after this point must use `command_name`. The existing `tracing::debug!(... binary = %config.command ...)` a few lines below still works (`config` is not moved, only `cmd`).

- [ ] **Step 2: Full gate + LSP-gated tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test --lib lsp:: 2>&1 | tail -5`
Expected: all pass (rust-analyzer-gated tests exercise the new spawn path for real).

- [ ] **Step 3: windows-gnu cross-check**

Run: `scripts/build-windows.sh check`
Expected: clean type-check for `x86_64-pc-windows-gnu`.

- [ ] **Step 4: Commit**

```bash
git add src/lsp/client.rs
git commit -m "fix(lsp): bound process spawn with spawn_blocking + 10s timeout (WIN-5)"
```

---

### Task 5: `client_within_budget` — bounded LSP acquisition with detached warm-up

**Files:**
- Modify: `src/lsp/mod.rs` (new pub helper + const; place near `prewarm_lsp_background`)
- Test: inline `#[cfg(test)]` module in `src/lsp/mod.rs`

**Interfaces:**
- Consumes: `LspProvider::{is_ready, get_or_start}` (`src/lsp/ops.rs:77-105`; `is_ready` is the existing non-blocking readiness probe, default `false`); `MockLspClient::new()` + `LspClientOps` (`src/lsp/mock.rs`) for tests.
- Produces: `crate::lsp::LSP_FIRST_CALL_BUDGET: Duration` (= 2s) and
  `pub async fn client_within_budget(lsp: Arc<dyn LspProvider>, language: &str, root: &Path, mux_override: Option<bool>, budget: Duration) -> Option<Arc<dyn LspClientOps>>`.
  Task 6 consumes both, exactly as named.

- [ ] **Step 1: Write the failing test** — in `src/lsp/mod.rs`:

```rust
#[cfg(test)]
mod budget_tests {
    use super::*;
    use crate::lsp::mock::MockLspClient;
    use crate::lsp::ops::{LspClientOps, LspProvider};
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// LspProvider whose cold start takes `delay`; `is_ready` flips true once
    /// a start has completed (mirrors LspManager's cache-hit fast path).
    struct SlowStart {
        client: Arc<MockLspClient>,
        delay: Duration,
        ready: AtomicBool,
    }

    #[async_trait::async_trait]
    impl LspProvider for SlowStart {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn LspClientOps>> {
            if !self.ready.load(Ordering::SeqCst) {
                tokio::time::sleep(self.delay).await;
                self.ready.store(true, Ordering::SeqCst);
            }
            Ok(self.client.clone())
        }
        async fn notify_file_changed(&self, _path: &Path) {}
        async fn shutdown_all(&self) {}
        async fn is_ready(&self, _language: &str, _workspace_root: &Path) -> bool {
            self.ready.load(Ordering::SeqCst)
        }
    }

    #[tokio::test]
    async fn cold_start_over_budget_returns_none_but_keeps_warming() {
        let lsp: Arc<dyn LspProvider> = Arc::new(SlowStart {
            client: Arc::new(MockLspClient::new()),
            delay: Duration::from_millis(200),
            ready: AtomicBool::new(false),
        });
        // Budget (50ms) < cold start (200ms): must yield None, not block 200ms.
        let t0 = std::time::Instant::now();
        let got =
            client_within_budget(lsp.clone(), "rust", Path::new("/tmp"), None, Duration::from_millis(50))
                .await;
        assert!(got.is_none());
        assert!(t0.elapsed() < Duration::from_millis(150), "must not wait out the cold start");

        // The DETACHED warm-up must finish on its own: after the delay elapses,
        // the provider is ready and the next call succeeds immediately.
        tokio::time::sleep(Duration::from_millis(250)).await;
        let got =
            client_within_budget(lsp, "rust", Path::new("/tmp"), None, Duration::from_millis(50))
                .await;
        assert!(got.is_some(), "second call after warm-up must hit the ready fast path");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib budget_tests 2>&1 | tail -10`
Expected: COMPILE ERROR — `client_within_budget` not found.

- [ ] **Step 3: Implement** — in `src/lsp/mod.rs`:

```rust
/// Budget for a tool call that would otherwise block on an LSP cold start.
/// Callers fall back to tree-sitter output (marked `"lsp": "warming"`) when
/// the budget elapses; the start continues in a DETACHED task so the next
/// call hits the warm fast path.
pub const LSP_FIRST_CALL_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);

/// Bounded LSP acquisition: immediate when a live client exists; otherwise
/// start it on a detached task and wait at most `budget`. `None` means "not
/// ready yet — serve the AST fallback"; it is never an error.
pub async fn client_within_budget(
    lsp: std::sync::Arc<dyn crate::lsp::ops::LspProvider>,
    language: &str,
    root: &std::path::Path,
    mux_override: Option<bool>,
    budget: std::time::Duration,
) -> Option<std::sync::Arc<dyn crate::lsp::ops::LspClientOps>> {
    if lsp.is_ready(language, root).await {
        return lsp.get_or_start(language, root, mux_override).await.ok();
    }
    let lang = language.to_string();
    let root_buf = root.to_path_buf();
    let handle =
        tokio::spawn(async move { lsp.get_or_start(&lang, &root_buf, mux_override).await });
    match tokio::time::timeout(budget, handle).await {
        Ok(Ok(Ok(client))) => Some(client),
        _ => None, // start error, join error, or budget elapsed (task keeps warming)
    }
}
```

(Adjust the `use` paths to match the module's existing imports — `LspProvider`/`LspClientOps` may already be re-exported at `crate::lsp::` level; prefer the shortest existing path.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib budget_tests 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 5: Full gate** — `cargo fmt && cargo clippy -- -D warnings && cargo test --lib 2>&1 | tail -3`

- [ ] **Step 6: Commit**

```bash
git add src/lsp/mod.rs
git commit -m "feat(lsp): client_within_budget — bounded acquisition, detached warm-up"
```

---

### Task 6: symbols overview serves tree-sitter with a warming hint during LSP cold start

**Files:**
- Modify: `src/tools/symbol/list_overview.rs` (two sites: glob-files loop ~line 240; single-file arm ~line 284-300)
- Test: existing symbols tests (must stay green) + one new test in the same file's test module

**Interfaces:**
- Consumes: `crate::lsp::{client_within_budget, LSP_FIRST_CALL_BUDGET}` (Task 5); existing `crate::ast::extract_symbols(path) -> Result<Vec<SymbolInfo>>` (already the BUG-054 fallback in this very function); existing `symbol_to_json` rendering pipeline (source-agnostic — operates on `Vec<SymbolInfo>` regardless of LSP or AST origin).
- Produces: symbols overview responses gain optional `"lsp": "warming"` + `"hint"` fields when served from tree-sitter during cold start. Scope note: `symbols` **search** mode (`src/tools/symbol/symbols.rs:386/466`) is deliberately NOT budgeted here — workspace/symbol search has no AST equivalent; revisit after Task 1 telemetry shows whether it matters.

- [ ] **Step 1: Glob-files loop.** Replace the `get_or_start` acquisition:

```rust
            let mux_override = ctx.agent.lsp_mux_override(lang).await;
            if let Ok(client) = ctx.lsp.get_or_start(lang, &root, mux_override).await {
```

with a budgeted acquisition + AST fallback (the inner LSP rendering block is unchanged; the `else` branch is new):

```rust
            let mux_override = ctx.agent.lsp_mux_override(lang).await;
            let budget_client = crate::lsp::client_within_budget(
                ctx.lsp.clone(),
                lang,
                &root,
                mux_override,
                crate::lsp::LSP_FIRST_CALL_BUDGET,
            )
            .await;
            if let Some(client) = budget_client {
                // ... existing LspTimer + document_symbols + entry push, unchanged ...
            } else if let Ok(symbols) = crate::ast::extract_symbols(file_path) {
                // LSP still warming: serve tree-sitter so the overview is not
                // blocked or silently missing files; mark the entry.
                let rel = file_path.strip_prefix(&root).unwrap_or(file_path);
                let source = if include_body {
                    std::fs::read_to_string(file_path).ok()
                } else {
                    None
                };
                let json_symbols: Vec<Value> = symbols
                    .iter()
                    .map(|s| symbol_to_json(s, include_body, source.as_deref(), depth, false))
                    .collect();
                let json_symbols = if lang == "bash" {
                    filter_variable_symbols(json_symbols)
                } else {
                    json_symbols
                };
                let mut entry = json!({
                    "file": rel.display().to_string(),
                    "symbols": json_symbols,
                    "lsp": "warming",
                });
                if include_docs {
                    entry["docstrings"] = json!(collect_docstrings(file_path));
                }
                result.push(entry);
            }
```

- [ ] **Step 2: Single-file arm.** The existing code acquires `(client, lang)` via `get_lsp_client(...).await?` then runs `retry_on_mux_disconnect` → BUG-054 empty-fallback → a source-agnostic rendering pipeline. Replace ONLY the acquisition + LSP call with a budgeted `match`; the rendering below stays untouched. Current code (lines ~284-311):

```rust
        let (client, lang) = get_lsp_client(
            &ctx.agent,
            &*ctx.lsp,
            &full_path,
            ctx.workspace_override.as_deref(),
        )
        .await?;
        let timer = LspTimer::start();
        // I-4: single-retry on transient LSP-mux disconnect (covers Kotlin LSP
        // eviction churn). Closure is idempotent — document_symbols is a pure
        // read of the LSP-side index.
        let symbols = retry_on_mux_disconnect(
            &ctx.agent,
            &*ctx.lsp,
            &full_path,
            ctx.workspace_override.as_deref(),
            client,
            lang.clone(),
            |c, l| {
                let p = full_path.clone();
                async move { c.document_symbols(&p, &l).await }
            },
        )
        .await?;
        timer.record(&*ctx.lsp, raw_lang, &root).await;
```

New code:

```rust
        let mux_override = ctx.agent.lsp_mux_override(raw_lang).await;
        let lang = crate::lsp::servers::lsp_language_id(raw_lang).to_string();
        let mut lsp_warming = false;
        let symbols = match crate::lsp::client_within_budget(
            ctx.lsp.clone(),
            raw_lang,
            &root,
            mux_override,
            crate::lsp::LSP_FIRST_CALL_BUDGET,
        )
        .await
        {
            Some(client) => {
                let timer = LspTimer::start();
                // I-4: single-retry on transient LSP-mux disconnect (covers Kotlin LSP
                // eviction churn). Closure is idempotent — document_symbols is a pure
                // read of the LSP-side index.
                let symbols = retry_on_mux_disconnect(
                    &ctx.agent,
                    &*ctx.lsp,
                    &full_path,
                    ctx.workspace_override.as_deref(),
                    client,
                    lang.clone(),
                    |c, l| {
                        let p = full_path.clone();
                        async move { c.document_symbols(&p, &l).await }
                    },
                )
                .await?;
                timer.record(&*ctx.lsp, raw_lang, &root).await;
                symbols
            }
            None => {
                // LSP cold / not configured: serve tree-sitter now; the detached
                // warm-up (if a server exists) makes the next call LSP-grade.
                lsp_warming = true;
                ast::extract_symbols(&full_path)?
            }
        };
```

Then attach the marker at BOTH result-construction sites at the end of the single-file arm (the overflow early-return and the final `Ok(result)`), immediately before each `return Ok(result);` / `Ok(result)`:

```rust
        if lsp_warming {
            result["lsp"] = json!("warming");
            result["hint"] = json!(
                "Language server is starting; symbols served from tree-sitter. \
                 Re-run shortly for LSP-grade detail."
            );
        }
```

The BUG-054 empty-symbols fallback block and everything below it (caps, docstrings, overflow) stay byte-identical — both code paths feed the same `Vec<SymbolInfo>`.

Behavior note (deliberate): languages with a tree-sitter grammar but NO configured LSP previously errored out of `get_lsp_client`; they now return AST symbols with the warming marker. Slightly optimistic hint, strictly more useful output — record it in the commit message.

- [ ] **Step 3: New test** — in the file's existing test module, using the `SlowStart` provider pattern from Task 5's test (re-declare locally if the type is test-private): call the symbols overview on a fixture `.rs` file with a 1ms budget, assert the response contains `"lsp": "warming"` and a non-empty `symbols` array. Pattern the ToolContext construction on the nearest existing test in the same module (do not invent a new harness).

- [ ] **Step 4: Run the symbols suite**

Run: `cargo test --lib symbol 2>&1 | tail -5`
Expected: all pass, including the new warming test.

- [ ] **Step 5: Full gate** — `cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -3` (full suite: the prompt-surface tests must confirm no tool-name drift).

- [ ] **Step 6: Commit**

```bash
git add src/tools/symbol/list_overview.rs
git commit -m "feat(symbols): serve tree-sitter with warming hint during LSP cold start"
```

---
---

### Task 7: gnu-ABI CI gate (MinGW cross + wine)

**Files:**
- Modify: `.github/workflows/ci.yml` (add one job)

**Interfaces:**
- Consumes: `scripts/build-windows.sh` (`build` / `check` / `test [FILTER]` subcommands; requires `x86_64-w64-mingw32-gcc`, rustup target `x86_64-pc-windows-gnu`, wine for `test`).
- Produces: CI job `windows-gnu` — the ABI actually shipped to the VDI, previously untested in CI.

- [ ] **Step 1: Add the job** to `.github/workflows/ci.yml` after the `test` job, matching the existing jobs' action versions:

```yaml
  windows-gnu:
    name: Windows-gnu cross (MinGW + wine)
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-gnu
      - name: Install MinGW + wine
        run: |
          sudo dpkg --add-architecture i386
          sudo apt-get update
          sudo apt-get install -y gcc-mingw-w64-x86-64 wine wine64
      - uses: mozilla-actions/sccache-action@v0.0.7
      - uses: Swatinem/rust-cache@v2
        with:
          key: windows-gnu-cross
      - name: Cross-build (default features)
        run: scripts/build-windows.sh build
      - name: Cross-test under wine (lib only)
        run: scripts/build-windows.sh test --lib
```

- [ ] **Step 2: Lint the workflow locally (best-effort)**

Run: `actionlint .github/workflows/ci.yml 2>&1 | head -5 || echo "actionlint not installed — rely on CI"`
Expected: no errors (or the tool is absent — CI validates on push).

- [ ] **Step 3: Commit and observe CI**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add windows-gnu cross-compile + wine test job"
git push origin experiments
```

Then watch the run: `gh run watch --exit-status` (or `gh run list --workflow=CI --limit 1`). Expected: `windows-gnu` job green. If wine flakes on specific tests in CI, narrow with a filter (`scripts/build-windows.sh test --lib win32`) and record the exclusion + reason in a follow-up commit — do NOT delete the job.

---

### Task 8: Repair the windows tracker's augmentation (recon F-2)

**Files:**
- Modify (via librarian, not direct edit): `docs/trackers/windows-platform-support.md` (artifact id `52451519052d207c`)
- Catalog-only: new augmentation row (lives in the librarian DB, not in git)

**Interfaces:**
- Consumes: MCP `artifact_augment` (with `params_path` — payload >9KB), `artifact(update, body_edits)`, `artifact(get, entry_filter)`.
- Produces: a working `entry_filter` surface over the 26 WIN-N rows; correct in-body ids.

- [ ] **Step 1: Extract the table to JSON** (scratchpad path; adjust to the session's scratchpad):

```bash
python3 - <<'EOF'
import json, re, pathlib
text = pathlib.Path("docs/trackers/windows-platform-support.md").read_text(encoding="utf-8")
rows = []
for line in text.splitlines():
    m = re.match(r"\|\s*(WIN-\d+)\s*\|([^|]+)\|([^|]+)\|(.+)\|([^|]+)\|([^|]+)\|\s*$", line)
    if m:
        rows.append({"id": m.group(1), "area": m.group(2).strip(), "status": m.group(3).strip(),
                     "summary": m.group(4).strip(), "ref": m.group(5).strip(), "since": m.group(6).strip()})
assert len(rows) == 26, f"expected 26 rows, got {len(rows)}"
out = pathlib.Path("/tmp/win-issues.json")  # use the session scratchpad dir
out.write_text(json.dumps({"issues": rows}, indent=1), encoding="utf-8")
print(len(rows), "->", out)
EOF
```

- [ ] **Step 2: Create the augmentation** (merge=false — first augmentation on this artifact):

```
artifact_augment(
  id="52451519052d207c",
  prompt="Windows-platform issue index. params.issues is the canonical WIN-N table: one object per issue {id, area, status, summary, ref, since}. To add/flip an issue: artifact_augment(merge=true, params={issues:[...full array...]}) — arrays replace wholesale, so always send the complete array (params_path for >9KB). After any params change, re-sync the '## Issue index' markdown table via artifact(update, body_edits). Never reuse or delete a WIN-N id.",
  params_path="/tmp/win-issues.json",
  entry_collection="issues",
)
```

- [ ] **Step 3: Fix the two dead-id references in the body:**

```
artifact(action="update", id="52451519052d207c", patch={body_edits: [
  {heading: "## Issue index", action: "edit", replace_all: true,
   old_string: "42dfdfc8b1522192", new_string: "52451519052d207c"},
  {heading: "## How to append", action: "edit",
   old_string: "artifact_augment(id=\"<id>\", merge=true,",
   new_string: "artifact_augment(id=\"52451519052d207c\", merge=true,"}
]})
```

(If the second `old_string` does not match exactly, `artifact(get, id="52451519052d207c", heading="## How to append")` first and adapt — the goal is: every in-body id reference resolves to `52451519052d207c`.)

- [ ] **Step 4: Verify the entry_filter surface works**

```
artifact(action="get", id="52451519052d207c", entry_filter={"status": {"eq": "open"}})
```

Expected: exactly one entry (WIN-18). Also check `{"status": {"eq": "deferred"}}` → WIN-5.

- [ ] **Step 5: Close F-2 in the session log** — `edit_markdown("docs/trackers/perf-windows-session-log.md", action="edit", heading="## F-2 — windows-platform-support tracker: augmentation missing, body cites nonexistent artifact id", old_string="**Status:** open", new_string="**Status:** fixed-verified")`, and update the F-2 row in the `## Index` table the same way.

- [ ] **Step 6: Commit**

```bash
git add docs/trackers/windows-platform-support.md docs/trackers/perf-windows-session-log.md
git commit -m "docs(trackers): restore windows tracker augmentation, fix dead ids (F-2)"
```

---

### Task 9: Build-loop baseline + one lever (thin-LTO), keep-or-revert gate

**Files:**
- Modify (conditionally kept): `Cargo.toml` (`[profile.release]`)
- Record results in: `docs/trackers/perf-windows-session-log.md` (a W-N or F-N entry with numbers)

**Interfaces:** none — build configuration only.

- [ ] **Step 1: Baseline the dev-loop rebuild** (warm sccache, representative single-file touch):

```bash
sccache --show-stats | head -12
touch src/lib.rs && time cargo build --release --features server-stack
cargo build --release --features server-stack --timings 2>&1 | tail -3
```

Record: wall time of the `time` run + the link phase from `target/cargo-timings/cargo-timing.html` (open it; the final `codescout` bar is dominated by codegen+link).

- [ ] **Step 2: Apply the lever** — in `Cargo.toml`, `[profile.release]`: change `lto = "thin"` → `lto = false` (leave `opt-level`, `codegen-units`, `strip`, `panic` untouched).

- [ ] **Step 3: Re-measure identically**

```bash
touch src/lib.rs && time cargo build --release --features server-stack
```

- [ ] **Step 4: Keep-or-revert gate.** Keep the change if the rebuild is ≥15% faster; otherwise `git checkout -- Cargo.toml`. Either way, append the numbers to `docs/trackers/perf-windows-session-log.md` (new W-N if kept, F-N `wontfix-false-alarm` if reverted) via `edit_markdown(action="insert_before", heading="## Template for new entries", ...)` + index row.

- [ ] **Step 5: If kept — full gate + runtime sanity** — `cargo fmt && cargo clippy -- -D warnings && cargo test --lib 2>&1 | tail -3`, then reconnect `/mcp` and run one `symbols` + one `semantic_search` call to confirm the un-LTO'd binary behaves.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml docs/trackers/perf-windows-session-log.md
git commit -m "chore(build): drop thin-LTO from release profile (dev-loop measurement attached)"
# or, if reverted:
git add docs/trackers/perf-windows-session-log.md
git commit -m "docs(trackers): record LTO lever measurement — reverted, <15% gain"
```

---

### Task 10: Lite-vs-hybrid retrieval quality benchmark

**Files:**
- Create: `docs/research/2026-07-NN-lite-vs-hybrid-benchmark.md` (NN = run date)

**Interfaces:**
- Consumes: `scripts/run-tc-benchmark.sh` (env: `CODESCOUT_BINARY`, `CODESCOUT_PROJECT_PATH`; needs the bench worktree at `.worktrees/bench` and the running server stack for the hybrid arm); `CODESCOUT_DISABLE_SPARSE=1`; `CODESCOUT_VECTOR_BACKEND=sqlite-vec` (lite arm).
- Produces: the quality-delta report the spec requires before committing the VDI to lite.

- [ ] **Step 1: Preconditions** — server stack up (Qdrant + embedder + reranker, `.env.amd` / llm-infra); bench worktree exists (`ls .worktrees/bench || git worktree add .worktrees/bench master`); release binary current (`cargo build --release --features server-stack`).

- [ ] **Step 2: Three arms** (scratchpad dir for outputs):

```bash
# A. Hybrid baseline (dense + sparse + rerank)
./scripts/run-tc-benchmark.sh > "$SCRATCH/hybrid.json"
# B. No-sparse (isolates the SPLADE contribution)
CODESCOUT_DISABLE_SPARSE=1 ./scripts/run-tc-benchmark.sh > "$SCRATCH/no-sparse.json"
# C. Lite (sqlite-vec backend => dense-only + no reranker). One-time reindex into
#    sqlite-vec first — the store starts empty; then run:
CODESCOUT_VECTOR_BACKEND=sqlite-vec ./scripts/run-tc-benchmark.sh > "$SCRATCH/lite.json"
```

For arm C's reindex: launch the binary once with `CODESCOUT_VECTOR_BACKEND=sqlite-vec` against the bench project and run `index(action="build")` to populate the sqlite store (the benchmark script drives the same binary; check `scripts/run-tc-benchmark.py --help` for whether it reindexes itself before inventing a manual step).

- [ ] **Step 3: Write the report** with per-arm retrieval metrics as emitted by the harness (it outputs JSON; summarize hit@k / rank metrics it reports — do not invent metrics), a short "what lite loses" paragraph, and a recommendation: is dense-only quality acceptable for the VDI, or does the remote endpoint need a stronger code-embedding model?

- [ ] **Step 4: Commit**

```bash
git add docs/research/2026-07-NN-lite-vs-hybrid-benchmark.md
git commit -m "docs(research): lite-vs-hybrid retrieval benchmark (WIN-26 quality gate)"
```

---

### Task 11: VDI validation pass (USER-RUN — Claude prepares, Marius executes on the VDI)

**Files:** none in this repo (results append to `docs/trackers/windows-platform-support.md` via Task 8's restored augmentation).

**Checklist to execute on the VDI** (from the EDR runbook `docs/manual/src/configuration/embeddings-edr-windows.md` + `.env.lite`):

- [ ] Pull `experiments` with Tasks 1–6 landed; build **default features** (lean lite — no `local-embed*`, no `server-stack`) natively (windows-gnu).
- [ ] Swap the live exe per the WIN-10 workflow (move aside, rebuild, `/mcp` reload).
- [ ] Configure remote embeddings: `[embeddings].url` → corporate OpenAI-compatible endpoint + `EMBED_API_KEY`; confirm HTTPS-or-loopback guard passes.
- [ ] Reindex (`index(action="build")`) — dimension change vs any old local index.
- [ ] Verify: `semantic_search` returns results (sqlite-vec, no Qdrant, no ONNX DLL on disk); memory `recall` works; `workspace(activate)` completes and the new `codescout::perf` log line shows the phase split on EDR hardware.
- [ ] Verify WIN-5: open a Rust file, first `symbols` call during LSP cold start returns within the budget (warming hint acceptable) instead of hanging.
- [ ] Record outcomes as WIN-N status updates via the restored augmentation (Task 8 prompt documents the flow); any new defect gets a `docs/issues/<date>-<slug>.md` + WIN-N row.

---

## Contingent follow-ups (decision gate, not tasks)

The spec's remaining T1.1 levers fire only if Task 1's telemetry blames them
(read `grep "codescout::perf" .codescout/debug.log` after a week of normal use):

- `auto_register_deps` phase > 500ms median → move it to fire-and-forget
  background (same shape as `prewarm_lsp_background`); `auto_registered_libs`
  becomes best-effort.
- `agent_activate` phase > 1s median (dominated by `discover_projects`) → cache
  the walk keyed on root + discover settings, invalidated by top-level dir
  mtimes.
- `symbols` search mode still spiking after Task 6 → extend the budget pattern
  to `src/tools/symbol/symbols.rs:386/466`.

If a gate fires, write a new task against this plan (same TDD shape as Task 2)
rather than improvising.
## Execution order & independence

Tasks 1→2→3 (instrumentation then cache; 3 needs 1), 4 independent, 5→6, 7 independent, 8 independent, 9 anytime, 10 after 1–6 land (benchmark against the improved binary), 11 last (needs 4/6 on the branch). Each task commits independently on `experiments`.
