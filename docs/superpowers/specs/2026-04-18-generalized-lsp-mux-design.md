# Generalized LSP Mux — Design

**Status:** Design — awaiting implementation plan.
**Date:** 2026-04-18.
**Branch:** work lands on `experiments`, graduates per-language to `master`.

## 1. Goals

Extend the existing LSP multiplexer (today, Kotlin-only) to cover `rust`,
`java`, `python`, `typescript`/`javascript`, and `go`. Each muxed language
gets a single LSP server process per `(language, workspace-root)` pair,
shared across all codescout instances.

**Primary wins:**

1. **Read coherence.** Instance A writes + `didChange`s, instance B's
   queries see the post-write state immediately. Eliminates the
   "stale hover/goto after parallel edit" class of bug.
2. **Memory sharing.** One `rust-analyzer` per workspace (2–4 GB),
   not one per codescout instance. One `jdtls` JVM, not two.
3. **Re-index amortization.** Cargo indexing, `tsconfig` resolution,
   venv scan, etc. happen once per workspace.

## 2. Non-goals

- **Cross-client coherence** (VS Code + codescout both editing). Mux uses
  first-client init result (`MuxState::cached_init_result`); heterogeneous
  init negotiation is out of scope.
- **New mux protocol features.** Tagging, fan-out, idle shutdown stay as-is.
- **Workspace-root detection improvements** (rust-analyzer Cargo workspace
  walking, pyright venv resolution). Only addressed per language *if* mux
  breaks them.
- **Non-Unix platforms.** Mux is `#[cfg(unix)]`. Windows stays direct-process.

## 3. Drivers and priorities

The three drivers above (coherence + memory + re-index) argue for
heaviest-resource languages first. Ordering:

**Rust → Java → Python → TypeScript/JavaScript → Go.**

Rationale: Rust first proves the non-JVM path on low-hanging fruit (native
binary, fast init, no JVM). Java second because `jdtls` is JVM-on-disk state
just like `kotlin-lsp` — the Kotlin precedent transfers most directly, and
Java is a heavy-resource target. Python, TS/JS, Go are similar difficulty;
order can be decided by demand.

## 4. Existing infrastructure (do not touch)

The mux is complete and working for Kotlin. No changes to:

- `src/lsp/mux/` — `process.rs` (event loop, client/server fan-out),
  `protocol.rs` (`ClientTag` tagging, `tag_request_id`/`untag_response_id`),
  `mod.rs` (`socket_path_for_workspace`, `lock_path_for_workspace`).
- `LspManager::get_or_start_via_mux` — file-lock arbitration, `ready`
  handshake, 5-retry connect.
- `LspServerConfig.mux: bool` dispatch in `LspManager::get_or_start`.
- `LspClient::connect` socket transport.
- `MuxState::cached_init_result` — first-client init. Homogeneous clients
  only; acceptable given non-goal 1.

## 5. Shared infrastructure changes

These land *before* any language is flipped, as a single infra PR.

### 5.1 Per-language `idle_timeout_secs` on `LspServerConfig`

Today the mux spawn in `LspManager::get_or_start_via_mux` passes a hardcoded
`--idle-timeout 300`. Replace with a per-language value.

```rust
pub struct LspServerConfig {
    // existing fields...
    /// Seconds the mux process waits with no connected clients before
    /// exiting. Only used when `mux == true`. `None` = mux default (300s).
    pub idle_timeout_secs: Option<u64>,
}
```

Spawn site in `get_or_start_via_mux` picks up `config.idle_timeout_secs`
(or 300 fallback).

### 5.2 Per-language opt-out via project config

Add an `[lsp.<lang>]` table to `ProjectConfig` with a single `mux: Option<bool>`
field. `Some(false)` forces the direct-process path even when
`servers::default_config` returns `mux: true`. `None` (default) = use the
built-in default.

```toml
# .codescout/project.toml — opt-out example
[lsp.rust]
mux = false
```

Precedence (evaluated in `LspManager::get_or_start` after
`default_config` returns a config):

1. If project config sets `[lsp.<lang>] mux = ...`, use it.
2. Otherwise, use `default_config(lang).mux`.

(When `default_config` returns `None`, we never reach the mux decision —
that language has no LSP support at all.)

**Not documented in `server_instructions.md`** unless/until a language
breaks in the wild. Keeps prompt-surface noise low.

### 5.3 Per-language `env` overrides (existing; flagged for use)

`LspServerConfig.env: Vec<(String, String)>` already exists. Kotlin uses it
for `GRADLE_USER_HOME`. Java is expected to need analogs; other languages
are not expected to need any. No new plumbing — each per-language section
below lists which env vars (if any) it sets.

## 6. Per-language sections

### 6.1 Rust (first)

- **Server:** `rust-analyzer`
- **Config:** `mux: true`, `idle_timeout_secs: Some(180)`,
  `env: []`, `init_timeout: None`.
- **Investigation:** rust-analyzer is multi-client by design (`$/progress`,
  cancellation). No on-disk workspace lock. Cargo `target/` contention is
  a `cargo check` concern, not an LSP one. 180s idle because 2–4 GB RSS
  shouldn't squat.
- **Risk:** low. Proves the non-JVM path.

### 6.2 Java

This is the highest-risk language. Investigation must complete before the
Java PR opens.

- **Server:** `jdtls`
- **Likely config:** `mux: true`, `idle_timeout_secs: Some(600)`,
  `init_timeout: Some(Duration::from_secs(300))` (unchanged). The `env`
  and `args` are deliberately unspecified here — they depend on the
  investigation below. Expected to mirror Kotlin's pattern (stable
  data-dir arg + redirected build-tool env vars).
- **Investigation — must resolve before flipping:**
  - `jdtls` creates a workspace state dir (default
    `~/.cache/jdtls/workspace/<hash>/`) containing an Eclipse
    `.metadata/.lock` file. Two `jdtls` instances on one workspace → metadata
    lock collision, same failure mode as `kotlin-lsp`'s MVStore lock.
  - **Likely need:** stable `--data <path>` arg (analog to Kotlin's
    `--system-path`), keyed by workspace hash. Place under
    `/tmp/codescout-mux-jdtls-<hash>/`.
  - **Possibly need:** `JAVA_TOOL_OPTIONS`, `GRADLE_USER_HOME`, and/or
    `MAVEN_REPO_LOCAL` redirection to a mux-owned path so concurrent
    resolves don't collide. Same pattern as Kotlin's `GRADLE_USER_HOME`.
  - **Ready handshake:** `jdtls` prints `ServiceReady` on stdout; the mux's
    existing `ready` line handshake applies unchanged.
- **Risk:** medium-high. If investigation reveals `jdtls` doesn't share
  state the way `kotlin-lsp` does, Java may need design beyond "flip the
  flag" — revise this spec before proceeding.

### 6.3 Python

- **Server:** `pyright-langserver --stdio`
- **Config:** `mux: true`, `idle_timeout_secs: Some(120)`, `env: []`.
- **Investigation:** pyright is stateless between client sessions; venv
  resolution runs per-workspace at init. No on-disk lock. In-memory venv
  cache benefits directly from mux (re-scan avoidance).
- **Risk:** low.

### 6.4 TypeScript / JavaScript

- **Server:** `typescript-language-server --stdio`
- **Config:** `mux: true`, `idle_timeout_secs: Some(120)`, `env: []`.
- **Investigation:** wraps `tsserver`. Re-reads `tsconfig.json` on
  project-add events; confirm during smoke test that
  `didChangeConfiguration` from two clients produces one merged view, not
  two conflicting views.
- **Risk:** low.

### 6.5 Go

- **Server:** `gopls`
- **Config:** `mux: true`, `idle_timeout_secs: Some(60)`, `env: []`.
- **Investigation:** native, small (<200 MB), designed multi-client. No
  on-disk lock. 60s idle is safe — startup is sub-second.
- **Risk:** low.

## 7. Testing

### 7.1 In-process two-`Agent` coherence harness

One test file per language, driving the known stale-doc bug from both
sides inside a single test binary:

```
tests/lsp_mux_coherence_<lang>.rs
```

Each test:

1. Create a `tempdir` and copy a minimal per-language fixture from
   `tests/fixtures/lsp-mux/<lang>/` (e.g. `Cargo.toml` + `src/lib.rs`
   with one known symbol for Rust).
2. Construct two `Agent` instances on the same workspace path:
   `let a = Agent::new(Some(dir.clone())).await?;`
   `let b = Agent::new(Some(dir)).await?;`
3. Both call `get_or_start(<lang>, workspace)`. First wins the file lock
   and spawns the mux; second connects to the existing socket.
4. Agent A writes new content via `edit_file` → `notify_file_changed`
   triggers A's `LspClient` to send `didChange`.
5. Agent B calls `find_symbol` (or language-equivalent) on the new
   symbol → must return the post-write result.
6. Assert symbol found with correct position, no stale-content error.

### 7.2 Shared harness code

Put the two-`Agent` setup in a `#[cfg(test)] pub(crate) mod test_support`
under `src/lsp/mux/mod.rs`. Every language test reuses it. Matches the
`CountingSink` pattern in `src/tools/progress.rs::test_support`.

### 7.3 Coverage

Covers: file-lock arbitration (A wins, B connects), `ClientTag` routing
under concurrent queries, `didChange` propagation from A invalidating B's
view, mux idle-shutdown does NOT fire during the test (idle ≥ test
duration).

Does NOT cover (intentionally, per Q5 decision):
- Real stdio MCP boundary between host and codescout.
- Mux process crash recovery (existing Kotlin unit tests cover this).
- Version skew between codescout binaries on the same workspace.

## 8. Rollout

### 8.1 Per-language PR shape

Each language ships as its own PR on `experiments`:

1. Flip `mux: true` + set `idle_timeout_secs` in `src/lsp/servers/mod.rs`.
2. Apply any language-specific `env`/`args` fixups (Kotlin-style) surfaced
   by investigation. Java is the main candidate.
3. Add the coherence integration test in `tests/lsp_mux_coherence_<lang>.rs`.
4. Smoke test: `cargo build --release` + `/mcp` restart + exercise tools
   on a real project of that language.
5. Add `docs/manual/src/experimental/mux-<lang>.md` per the experimental
   feature convention in `CLAUDE.md`.
6. Graduate to `master` via standard cherry-pick after a week on
   `experiments` with no reported regressions.

### 8.2 Infra PR ordering

The shared infrastructure changes (Section 5) land first, as a single PR
before the Rust PR opens. Order: infra → Rust → Java → Python →
TypeScript/JavaScript → Go.

### 8.3 Rollback

- **Broad regression post-merge on `master`:** flip `mux: false` in
  `default_config` for the affected language, ship a patch release.
  Users with the opt-out already set are unaffected.
- **Workspace-specific issue:** tell affected user to set
  `[lsp.<lang>] mux = false` in their `.codescout/project.toml` and
  `/mcp` restart. No binary change needed.

## 9. Per-language acceptance criteria

A language PR does not merge until:

- `cargo test lsp::` passes.
- `tests/lsp_mux_coherence_<lang>.rs` passes.
- `cargo build --release` + manual smoke on a real project of that
  language succeeds (open file, find symbol, hover, goto definition,
  at least one cross-file reference).
- `cargo fmt` + `cargo clippy -- -D warnings` clean.
- Experimental doc page in place.

## 10. Open questions / deferred

- **Java `jdtls` workspace-lock investigation.** Must resolve before the
  Java PR opens. If `jdtls` doesn't share workspace state the way
  `kotlin-lsp` does, revisit this spec before proceeding.
- **Per-language memory caps.** Not in scope. Mux idle-shutdown is the
  only resource bound.
- **Heterogeneous client support** (VS Code + codescout concurrent):
  explicit non-goal (§2).
- **Windows support:** explicit non-goal (§2).
- **Future:** if usage shows mux contention recoveries in `usage.db`,
  revisit per-file lock granularity or symbol-safe write queuing (see
  `docs/trackers/mcp-integration-ideas-2026-04.md` — "Explorations
  spun off from bundle (b)").
