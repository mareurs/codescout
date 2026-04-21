# MCP Integration Ideas — April 2026

Landscape captured after a deep read of the Claude Code source at
`/home/marius/work/claude/claude-code` (commit of 2026-04-13). Items #1–#3 are
being implemented under
[`docs/plans/2026-04-13-mcp-token-budget-design.md`](../plans/2026-04-13-mcp-token-budget-design.md).
This file tracks the rest — explore if/when they become relevant.

## How these were sourced

Each item is grounded in a concrete Claude Code source finding. Reality-check
the finding before building — the CC codebase moves fast and the cited lines
may have drifted.

---

## #4 — Subagent-aware project state

**CC finding:** subagents share the parent's MCP connection and state
(`src/entrypoints/sdk/`, `AppState` context). No isolated MCP context per
subagent.

**Codescout implication:** `activate_project` in a subagent clobbers the
parent's active project. Iron Law 4 ("always restore the active project") is a
discipline rule defending a sharp structural edge. It will eventually get
violated.

**Options**
- **A.** Per-connection session state keyed by an MCP init parameter (requires
  clients to opt in; unclear whether CC subagents expose a distinct client ID).
- **B.** `activate_project(scope: "caller")` flag that the companion plugin's
  `SubagentStart` hook auto-injects. Cheapest. Still leaves the manual case.
- **C.** Kill global active-project entirely. Every tool takes `project:
  Option<PathBuf>`. Most invasive, most correct. Would eliminate the iron law
  outright.

**Trigger to revisit:** next time someone reports a subagent-corrupted active
project, or before we encourage heavy multi-agent workflows.

---

## #5 — Lifecycle resilience / kill the `/mcp` restart dance

**CC finding:** no auto-reconnect on binary change; users must manually
`/mcp` after a `cargo build --release`. `MCPConnectionManager.tsx:37` has a TODO
to simplify the client context. Hung `callTool()` promises after `onerror`
(`client.ts:1340`) are a known fragility.

**Codescout implication:** every codescout dev cycle pays a
rebuild → `/mcp` → re-onboard tax. BUG-021 (parallel write crashes from rmcp
0.1.5 cancellation race) is adjacent — it also kills the server and forces a
manual reconnect.

**Options**
- **A.** `codescout serve` supervisor process: watches `target/release/codescout`,
  execs the new binary while keeping stdio open.
- **B.** PID file + graceful restart: the old binary, on receiving SIGUSR1 from
  a hook in the build script, drains in-flight requests and re-execs.
- **C.** Skip the supervisor; add explicit cancellation checkpoints inside
  spawned tool tasks so BUG-021 stops producing phantom responses even on the
  current rmcp. Smaller, lower-risk, partial fix.

**Trigger to revisit:** if we add a feature that requires frequent rebuilds
during a single session (e.g. live-configurable prompt templates).

---

## #6 — Server-side write serialization ✅ shipped (experiments, 2026-04-17)

**CC finding:** Claude Code dispatches tool calls concurrently when both sides
are marked `isConcurrencySafe`
(`src/services/tools/StreamingToolExecutor.ts:127–160`). Our write tools are
*not* concurrency-safe in practice — see BUG-021 and the memory entry "never
dispatch parallel write tool calls."

**Codescout implication:** we currently defend against parallel writes via a
rule. A server-side per-project write mutex would enforce the invariant and let
us delete the rule.

**Sketch**
- `ActiveProject` gets an `Arc<Mutex<()>>` write lock.
- `create_file`, `edit_file`, `replace_symbol`, `insert_code`, `remove_symbol`,
  `rename_symbol` each acquire it around the mutation.
- Reads stay lock-free.
- Document that parallel writes now queue FIFO rather than race.

**Trigger to revisit:** whenever we want to enable agent-parallelism confidently
(multi-file refactors, dispatched subagents editing different files at once).

**Shipped:** Two-layer locking — in-process `tokio::sync::Mutex` + cross-process
`fs4` flock on `.codescout/write.lock`. Contention returns `RecoverableError`
(`isError: false`). Timeout configurable via `security.write_lock_timeout_secs`
(default 5s). See `src/agent/write_guard.rs`.

---
## #7 — Usage-driven tool pruning

**CC finding:** every tool description is re-sent every turn. `CLAUDE.md` already
treats `usage.db` as ground truth.

**Idea:** periodic `codescout_doctor` that reports tools with <N calls over M
sessions, flags them for the next prompt-surface review. Could be a resource
(`doctor://tool-usage`) or a dedicated tool.

**Prerequisite:** feature #1 (resources) should ship first; this rides on it.

**Trigger to revisit:** after bundle (a) ships and we want to quantify what the
token-diet actually bought us.

**Delivered 2026-04-15:** `doctor://tool-usage` MCP resource in
`src/mcp_resources/tool_usage.rs`. Returns JSON with per-tool call counts,
error/overflow rates, p50/p99 latencies, prune candidates (tools with
1 ≤ calls < 5 in 30d window) and unused tools (registered but zero calls).
Always registered in `build_resource_registry`; gracefully returns zeros when
`usage.db` is absent. Experimental doc: `docs/manual/src/experimental/tool-usage-doctor.md`.
Verified end-to-end via HTTP transport smoke test.

---

## #8 — MCP prompts as slash commands

**CC finding:** MCP servers can expose `prompts/list` + `prompts/get`. Claude
Code auto-registers them as `/mcp__<server>__<prompt-name>` slash commands
(verified via the commands infrastructure in `src/commands.ts`).

**Idea:** codescout exposes curated workflows as prompts:

| Slash command                        | Does |
|--------------------------------------|------|
| `/mcp__codescout__tour`              | Guided repo walk — list_dir + top symbols + memory overview |
| `/mcp__codescout__whats-new`         | git log since last `activate_project` + memory delta |
| `/mcp__codescout__onboard-here`      | Re-run onboarding against the current directory |
| `/mcp__codescout__explain-this-file` | hover + find_references on a path the user just mentioned |

**Benefit:** discoverability without more SessionStart hook prose.

**Trigger to revisit:** after the companion plugin's SessionStart hook starts
feeling bloated, or when we want to package an opinionated workflow.

---

## #9 — Custom permission prompts

**CC finding:** `capabilities.experimental['claude/permission/custom-prompt']`
is checked by CC's permission path (the `useCanUseTool` family in the tool
executor).

**Idea:** codescout declares the capability and injects custom approval dialogs
for tools where silent execution is a footgun:
- `index_project` on repos > some size threshold: "embed 500MB / 12k files?"
- `rename_symbol` across a large blast radius: show the file count first.
- `run_command` with `acknowledge_risk: true` on a novel dangerous pattern.

**Caveat:** this is an experimental CC surface — stability of the capability
flag is not guaranteed across CC versions.

**Trigger to revisit:** if we get reports of accidental large-scale operations,
or when CC promotes the capability out of experimental.

---

## After bundle (a) ships — follow-up observations (2026-04-13)

These came out of the code-quality reviews during implementation of the
token-budget bundle. None blocked the PR; each is worth a pass if/when the
surrounding code is touched again.

### T6 — `tools/list_changed` broadcast is keyed to the tool name string

`CodeScoutServer::call_tool` detects `req.name == "activate_project"` to decide
when to broadcast. If that tool is ever renamed without updating the literal,
the broadcast silently stops firing. Low urgency but a real rename hazard.

**Safer alternatives:**
- Expose `const NAME: &str = "activate_project"` on the tool struct and match
  against it.
- Have `call_tool_inner` return a `ToolOutcome { result, caps_may_have_changed }`
  signal that the outer handler consumes.

**Fixed:** Added `ActivateProject::NAME: &'static str = "activate_project"` const in `src/tools/config.rs`. `name()` returns `Self::NAME`; server.rs uses `crate::tools::config::ActivateProject::NAME` in the broadcast guard.

### T6 — `has_embeddings` is a `cfg!` check, not a runtime probe

Always `true` in default builds because both `local-embed` and `remote-embed`
features are on. If a user builds with the feature but has no model wired,
`semantic_search` / `index_project` / `index_status` appear in the tool list
and fail at call time with a `RecoverableError`. Acceptable UX today; graduate
to a runtime `Agent::embedding_configured()` probe when such a method exists.

### T6 — no integration test through `ServerHandler::list_tools`

Current tests exercise `Availability::is_available` and the filter closure
directly. A test that goes through the rmcp handler boundary would close the
gap, but requires constructing a `RequestContext<RoleServer>` which is awkward.
Defer until rmcp exposes a cleaner test harness.

### T7 — `AgentSummarySource::index_status` is always `"unknown"`

`Agent` currently has no cheap freshness probe; SQLite-DB-path inspection was
judged too coupled for T7. Add an `IndexingState`-backed probe when that state
machine stabilizes.

**Fixed:** Added `Agent::index_status_label()` in `src/agent/mod.rs` — reads
`self.indexing` (a `std::sync::Mutex<IndexingState>`) synchronously and maps
`Idle → "idle"`, `Running → "indexing"`, `Done → "indexed"`, `Failed → "failed"`.
`AgentSummarySource::snapshot()` now calls it instead of hardcoding `"unknown"`.
Four variant tests added to `src/mcp_resources/project_summary.rs`.

### T7 (post-smoke) — `language` probe picks the wrong primary for mixed repos

Smoke-reading `project://summary` on the codescout repo itself returned
`language: "bash"` despite it being a Rust project. The probe likely walks the
root and picks the first file with a known extension (the repo has
`gitpretty-apply.sh` near the top). This is pre-existing behaviour of
`Agent::active_language()`, surfaced — not introduced — by the new resource.
Fix by weighting by file count or by consulting `Cargo.toml` / `package.json`
first when present.

**Already fixed:** `detect_primary_language()` in `src/mcp_resources/project_summary.rs` checks manifest files (Cargo.toml → rust, package.json → ts/js, etc.) before falling back to `configured.first()`. `snapshot()` passes project-specific languages from `p.config.project.languages`, not the workspace-aggregated list. Verified with unit tests.

### T7 — MCP resource `Blob` path returns `internal_error`

No provider currently emits `ResourceBytes::Blob`. If one is added (e.g.,
rendered PNGs from a diagram tool), flip the path to `ResourceContents::blob`
and pull in `base64` as a dependency.

### T11 — LSP cold-start progress not wired

Skipped during T11 because plumbing `progress: Option<Arc<ProgressReporter>>`
through `LspProvider::get_or_start` requires touching the trait, `LspManager`,
`MockLspProvider`, `get_or_start_via_mux`, and every LSP-backed tool's call
site. High cost, modest UX gain given the three sites that did ship. Revisit
if we add a new LSP-heavy tool or start getting user complaints about silent
cold starts.

### T11 — `CountingSink` copy-pasted across test modules

`src/tools/progress.rs`, `src/tools/semantic.rs`, and `src/tools/workflow.rs`
each carry their own `CountingSink`. Consolidating into
`src/tools/progress::test_support` (a `#[cfg(test)] pub(crate) mod`) would DRY
this up at the cost of one `mod test_support;` declaration. Low urgency.

**Fixed:** Added `pub(crate) mod test_support` in `src/tools/progress.rs` with canonical `CountingSink`. Removed local copies from `semantic.rs` and `workflow.rs`; both now import `crate::tools::progress::test_support::CountingSink`.

## Explorations — spun off from bundle (b) design (2026-04-17)

These are the follow-up problems deferred from the cross-process write-serialization spec (`docs/superpowers/specs/2026-04-17-cross-process-write-serialization-design.md`). The spec ships the fail-fast file-lock version; these are the larger moves that would earn us the next level of correctness.

### Cross-instance LSP read coherence via generalized mux

**Problem:** after instance A writes and sends `textDocument/didChange` to its own LSP client, instance B's LSP client still holds pre-write document state. Subsequent `hover` / `goto_definition` / `find_references` from B returns stale data. The write lock prevents torn disk writes but does nothing for LSP split-brain.

**Approach:** extend the `LspServerConfig.mux: bool` pattern to every language. The mux process in `src/lsp/mux/process.rs` is already language-agnostic (generic JSON-RPC forwarding, tagged request IDs, client lifecycle, shared document state). The blockers are operational, not architectural:

- per-language init timeout tuning (jdtls needs 300s, rust-analyzer a few seconds)
- memory footprint when every LSP for every project is always resident
- per-language workspace-root quirks (rust-analyzer Cargo detection, pyright venv resolution)
- per-client capability negotiation (mux uses first-client init today — fine for homogeneous CC instances, fragile for mixed clients)

**Rollout plan:** per-language PRs. Rust first (cleanest, no JVM, fast init), then Python, then TS/JS. Kotlin already serves as reference. Each PR flips `mux: true`, adjusts timeouts, adds smoke tests.

**Estimate:** ~1 week spread across the rollout.

### Symbol-safe write queuing on contention

**Problem:** the fail-fast file lock returns `RecoverableError` on contention, which forces the LLM to retry. Retries usually work, but are visible to the user and burn latency. A scheduler could queue symbol-addressed writes (`replace_symbol`, `insert_code`, `remove_symbol`) and execute them against post-write state.

**Approach sketch:**
- Per-file work queue, not just a lock.
- Classify writes as *queue-safe* (symbol-addressed, no line numbers) or *must-fail* (raw `edit_file` with literal `old_string`, line-range-based writes).
- Queue-safe writes re-resolve their symbol target against the LSP's post-write view before executing.
- Must-fail writes return `RecoverableError` as today.

**Open questions:**
- Does re-resolution stay correct if the first write deleted the target symbol? (Expected: fail with clear error — "symbol X no longer exists after write.")
- How long is the queue allowed to grow? Bounded by the lock timeout, presumably.
- Does `edit_file` with `old_string` count as queue-safe if the literal still matches post-write? (Possibly — cheap to check.)

**Trigger to revisit:** if we see contention recoveries showing up in `usage.db` at non-trivial rates, or if we enable heavier multi-agent parallelism.

### Per-file lock granularity

**Problem:** the v1 lock is per-project. Writes to different files in the same project serialize unnecessarily. In practice this is rare — contention is the exception — but under a multi-agent workload it becomes a bottleneck.

**Approach:** `DashMap<PathBuf, Arc<FileLock>>` on `ActiveProject`, keyed by canonicalized path. Lock creation is lazy; eviction on `ActiveProject` drop.

**Trigger to revisit:** when we enable multi-agent parallelism or when usage.db shows contention concentrating on a small set of frequently-edited files.

## Things explicitly deprioritized

From the original brainstorm; not tracked for follow-up unless the landscape
changes:

- **More rewrites of `server_instructions.md`.** Three rounds already;
  diminishing returns. The token-diet work in bundle (a) is the current
  marginal win.
- **Status-line integration.** Neat but audience-limited to users who have
  configured CC's status line.
- **Sampling / elicitation APIs.** CC's support is uneven and changing; revisit
  when it stabilizes.
