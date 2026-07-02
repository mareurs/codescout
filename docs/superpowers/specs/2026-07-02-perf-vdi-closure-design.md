# Design — Perf Sprint + VDI Closure

**Date:** 2026-07-02 · **Status:** approved · **Owner:** marius
**Work stream:** perf-windows (`docs/trackers/perf-windows-session-log.md`)
**Approach:** A of three proposed — one work stream, two tracks + build loop, evidence first.

## Problem

Two felt pains, plus one latent gap:

1. **Tool-call latency** — felt on both Linux and the EDR-locked VDI.
2. **Release-build dev loop** — `cargo rb` (release + `server-stack`) is the drag
   on this Linux machine between MCP reloads.
3. **Windows/VDI support is nearly complete but unlocked-in** — 26 WIN-N issues
   mostly fixed, the daemon-free lite retrieval stack shipped to master
   (Phases 0–4, `5c1ecfa8` et al.), but CI never builds the windows-gnu ABI the
   VDI actually runs, the dense-only quality delta is unmeasured, and the lite
   stack is (per repo records) unvalidated on the real VDI.

## Evidence

usage.db, last 30 days on the Linux dev box (`tool_calls` table):

| Tool | Calls | Avg | Worst | >2s |
|---|---|---|---|---|
| `workspace` (activation) | 165 | **5,847ms** | 16.8s | 137 (83%) |
| `references` | 53 | 1,923ms | 12.5s | 11 |
| `semantic_search` | 38 | 1,329ms | 3.5s | 3 |
| `symbols` | 1,317 | 531ms | **21.9s** | 100 |
| `grep` / `read_*` / `edit_*` | ~4,200 | ≤7ms | — | 0 |

Activation path scout (`src/tools/config/mod.rs::ActivateProject::call` +
`build_activation_response`): the response **serially awaits**
`Agent::activate` (incl. `discover_projects` tree walk in `spawn_blocking`),
`auto_register_deps`, and `check_has_index` — the last constructs a fresh
`RetrievalClient` and makes a network round-trip to Qdrant **on every
activation** to fill one `index.status` field. LSP prewarm is already
correctly backgrounded.

LSP cold start explains the `symbols`/`references` spikes: first call on a
cold server blocks on LSP spawn/init (up to 21.9s observed). WIN-5 (deferred)
is the same defect class under EDR, where `spawn()` is sync `CreateProcessW`
and can hang unboundedly.

Build loop: sccache + mold already configured; `cargo rb` compiles
`--features server-stack` (qdrant-client/tonic/prost) at `opt-level 3`,
thin-LTO, `codegen-units 16`, strip.

**Corrected fact (recon F-1):** WIN-26's lite stack is DONE — Phases 0–4 on
master (`0ff972f7`, `b96c8ae4`, `93ef0d43`, `9d40d36b`, `5c1ecfa8`). This
design deliberately contains **no retrieval re-implementation**.

## Design

### Track 1 — Tool-call latency

**T1.1 Activation (largest measured win).**
Instrument first: per-phase wall-clock timings (resource load,
`discover_projects`, `auto_register_deps`, `check_has_index`,
`probe_project_hints`) to the debug log. Then, guided by the split:

- **Cache `check_has_index`.** Per-project last-known index status held in
  session state; activation returns the cached value immediately and refreshes
  in the background. First-ever probe bounded by a short timeout (~500ms).
  Staleness window = one activation; acceptable for a hint field.
- **Background `auto_register_deps`** (only if timings blame it), same
  fire-and-forget shape as LSP prewarm. `auto_registered_libs` becomes a
  best-effort field — documented, prompt-surface tests run.
- **Cache `discover_projects`** keyed on root + discover settings, invalidated
  by top-level dir mtimes; or tighten default depth/excludes if the walk is
  the cost.

**T1.2 Bounded LSP spawn + first-call budget (this IS WIN-5).**
Wrap LSP process spawn in `spawn_blocking` with a timeout (closes the EDR
unbounded-hang class on the VDI). On tool calls that would block on a cold
LSP: wait up to a small budget (~2s), then serve the tree-sitter answer with a
`"lsp": "warming"` hint — the same graceful degradation `symbols` already has
when no LSP exists. Warming hint is a normal result, not an error;
`RecoverableError` shape only where a result is impossible.

**T1.3 `semantic_search` — measure only.**
Instrument the embed / vector-search / rerank split. Act only if one phase
dominates; 38 calls/month makes this lowest priority.

### Track 2 — VDI closure

- **WIN-5** — delivered by T1.2.
- **gnu-ABI CI gate** — `ubuntu-latest` job running the existing
  `scripts/build-windows.sh` (MinGW cross-compile; wine-executed tests).
  Locks all shipped WIN-N fixes against rot. wine validates logic, not EDR —
  the VDI remains the EDR-realism gate.
- **Lite-quality benchmark** — existing harness
  (`scripts/run-tc-benchmark.*`, `scripts/sweep-bm25-*.sh`,
  `CODESCOUT_DISABLE_SPARSE`): dense-only vs hybrid, quantify what the VDI
  loses without SPLADE + reranker; decides whether the remote endpoint needs a
  stronger code-embedding model (CodeRankEmbed-class).
- **VDI validation pass** — run the EDR runbook
  (`docs/manual/src/configuration/embeddings-edr-windows.md` + `.env.lite`)
  end-to-end on the actual VDI.
- **Tracker hygiene (recon F-2)** — re-augment
  `docs/trackers/windows-platform-support.md` (id `52451519052d207c`): rebuild
  `issues` params from the 26-row table via the `params_path` route (>9KB),
  set `entry_collection="issues"`, fix both in-body references to the dead id
  `42dfdfc8b1522192`.

### Track 3 — Build loop (Linux dev)

Measure before choosing: `cargo build --release --features server-stack
--timings` + `sccache --show-stats`. Candidate levers in likely-payoff order:
drop thin-LTO on the dev machine's release profile (link time; mold present),
raise `codegen-units`, or a separate `release-dev` profile. The profile-variant
option interacts with the `~/.cargo/bin/codescout → target/release/` symlink
convention (memory `gotchas`), so it is taken only if the timings justify
rewiring that. **One lever, chosen by data.**

## Acceptance criteria

- Warm re-activation p50 < 1s on the dev box; no activation network round-trip
  on the hot path.
- `symbols`/`references` first-call bounded by the LSP budget (~2s) with AST
  fallback + warming hint; no unbounded LSP spawn wait on either platform.
- gnu-ABI CI job green on PRs.
- Benchmark report (dense-only vs hybrid) committed under `docs/research/`.
- Windows tracker augmentation restored; `entry_filter` queries return rows.
- Build loop: measured baseline + one applied lever with before/after numbers.

## Error handling

- Cached `index.status` is stale-tolerant; background refresh failures are
  silent (field keeps last-known value) — same best-effort contract the Qdrant
  probe has today.
- LSP spawn timeout → warming-hint result path, not `isError: true`; genuine
  spawn failures keep existing error routing.
- Activation response field semantics changes (best-effort fields) documented
  and covered by `prompt_surfaces_reference_only_real_tools`.

## Testing

- Per-phase activation instrumentation stays in permanently (regression
  visibility).
- Index-status cache: three-query sandwich (baseline → assert-stale →
  invalidate → assert-fresh) per the conventions memory.
- Bounded LSP wait: mock-slow-spawn test on Linux; wine gate covers the
  Windows arm; env-dependent tests use `EnvGuard` + `#[serial]`.
- Pre-commit gate on every step: `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`. Work lands on `experiments`, cherry-picked to `master` per
  `docs/RELEASE.md`.

## Sequencing

1. Instrumentation: activation phase timings + build `--timings` baseline
   (pure evidence, no behavior change).
2. Activation fixes (T1.1) driven by the numbers.
3. Bounded LSP spawn / WIN-5 (T1.2).
4. gnu-ABI CI gate.
5. Lite-quality benchmark + VDI validation pass.
6. Tracker hygiene (F-2) + build-loop lever.

Each step ships independently.

## Non-goals

- Re-implementing retrieval (lite stack shipped — recon F-1).
- General Windows-consumer polish (installers, MSVC distribution).
- Optimizing already-fast tools (`grep`/`read_*`/`edit_*` at ≤7ms).
- `run_command` latency (dominated by the user's commands, not codescout).

## Open questions

- Has the lite stack been run on the real VDI since Phase 4 shipped?
  (Unanswered at design time; determines whether step 5 is validation or
  debugging. First action of step 5 is to find out.)
- LTO removal affects any binary built from the repo's release profile —
  acceptable for a personal tool, but revisit if binaries are ever
  distributed.
