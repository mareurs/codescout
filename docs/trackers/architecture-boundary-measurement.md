---
kind: tracker
status: active
title: Architecture Boundary Measurement
tags:
- architecture
- measurement
- refactoring
- service-boundaries
topic: architecture-boundary-measurement
---

# Architecture Boundary Measurement

## Purpose

Measure the current coupling before deciding whether codescout should be split into
code-intelligence, execution, and knowledge services.

This is a measurement tracker, not an approved refactor. No architectural boundary is
settled, and no code should move until the baseline and decision criteria below are complete.

## Current hypothesis, not a decision

Keep one MCP server and one public tool surface while testing three internal ownership domains:

1. **Code intelligence** - LSP, tree-sitter, symbols, references, call graph, library navigation,
   and code-oriented semantic search.
2. **Agent execution** - workspace resolution, file operations, path security, write locking,
   shell execution, and change notification.
3. **Knowledge system** - markdown memory, librarian artifacts, links, trackers, provenance,
   freshness, and knowledge-oriented retrieval.

Treat retrieval and workspace identity as candidate shared substrates until measurements show
whether either belongs exclusively to one domain. Cross-domain operations such as `edit_code`,
`index`, `onboarding`, and workspace activation are candidate application workflows rather than
forced members of one domain.

## Frozen source populations

Use these exact path populations for the first baseline. Report unclassified Rust files instead of
silently assigning them.

### Intelligence

- `src/lsp/**`
- `src/ast/**`
- `src/tools/symbol/**`
- `src/tools/library.rs`
- `src/library/**`

### Execution

- `src/tools/read_file.rs`
- `src/tools/tree.rs`
- `src/tools/grep.rs`
- `src/tools/create_file.rs`
- `src/tools/edit_file/**`
- `src/tools/run_command/**`
- `src/tools/approve_write.rs`
- `src/util/path_security.rs`
- `src/agent/write_guard.rs`

### Knowledge

- `src/librarian/**`
- `src/memory/**`
- `src/tools/memory/**`

### Retrieval candidate substrate

- `src/retrieval/**`
- `src/embed/**`
- `src/tools/semantic/**`
- `crates/codescout-embed/src/**`

### Orchestration candidate substrate

- `src/server.rs`
- `src/agent/**`, excluding `src/agent/write_guard.rs`
- `src/tools/core/**`
- `src/tools/config/**`
- `src/prompts/**`

## Measurements owed

### 1. Static dependency edges

**Unit:** one unique `(production source file, target top-level module)` edge. Separate `use`
imports from concrete calls where possible. Exclude test-only modules and generated code.

Report:

- directed edge counts between every population;
- the raw edge list, so the totals can be independently recounted;
- concrete cross-domain references that bypass a trait or service interface;
- population-level cycles;
- unclassified files and feature-gated blind spots.

**Positive control:** `src/server.rs` must be observed importing or constructing registered tools.
If the instrument cannot find that edge, stop.

### 2. Runtime and context reach-through

Use the live registered tool list as the denominator. Do not copy a tool count from documentation.

Report:

- each `ToolContext` field and the distinct registered tools whose production call path reads it;
- each tool's capability set;
- tools touching 0, 1, 2, 3, and 4+ capabilities;
- direct `ctx.agent` reach-through sites, grouped by called Agent method or field;
- cross-domain workflow tools and their actual call paths;
- the separate librarian adapter context, rather than pretending it is the same type.

**Positive control:** verify one known pair, such as the workspace tool reading Agent state. Direct
field access and indirect dependence are different measurements and must not be merged.

### 3. Historical co-change

Freeze one shared window with:

```text
git rev-list --max-count=500 HEAD
```

For every SHA in that set, classify changed tracked `.rs` paths using the frozen populations.
The unit is one commit touching at least one Rust file in a population.

Report:

- per-population commit counts;
- every pair's intersection and union;
- Jaccard similarity;
- both conditional directions, `P(B|A)` and `P(A|B)`;
- commits touching three or more populations;
- one manually inspected multi-population commit as a positive control.

Stamp the exact `HEAD`, branch, command time, and whether tracked Rust files differ from `HEAD`.

### 4. Feature and dependency weight

Without building, run `cargo tree -e normal --prefix none --format '{p}'` for:

- `--no-default-features`;
- default features;
- `--no-default-features --features server-stack`.

Count unique exact package lines after sorting. Positive controls:

- librarian-only dependencies such as `serde_yml`, `pulldown-cmark`, and `jsonschema` are absent
  from lean and present in default;
- `qdrant-client` is absent from default and present in server-stack.

Record installed, debug, and release binary sizes if present, but state explicitly that file size
without build identity is not evidence about current `HEAD`.

### 5. Baseline verdict

Do not turn a large number into an automatic extraction recommendation. For each proposed boundary,
answer separately:

- Does it reduce source dependency edges?
- Does it reduce ambient `ToolContext` or Agent reach-through?
- Does history show independent change?
- Does it remove meaningful optional dependency or binary weight?
- Does it isolate a measured failure mode?
- What cross-domain workflow becomes more complicated?

The first implementation candidate remains an internal boundary with one MCP server. A crate or
process split requires evidence beyond directory size.

## Rejected evidence - do not reuse

Three exploratory subagent reports from 2026-09-10 were rejected and carry **no baseline value**:

1. A static-edge report returned `~16` edges, `4` concerns, and `0` cycles without stamping `HEAD`
   or providing a recountable edge population. Approximation is not a measurement.
2. A context report used generic search despite being instructed to use codescout, mixed direct and
   indirect `workspace_override` use, and contained internally inconsistent tool/capability totals.
3. A history report did not execute its proposed commands. It returned expectations as results and
   claimed a `HEAD`/timestamp combination incompatible with the current September 2026 history.

These reports are useful only as examples of failure modes. Re-run every number from scratch.

## Measurement discipline

Apply `docs/PROBES.md` literally:

- name what each predicate counts;
- stamp the tree and instant;
- include a known-positive control;
- treat truncated output as a floor;
- when two measurements disagree, rerun instead of explaining the disagreement;
- distinguish direct references, indirect capability dependence, historical co-change, and runtime
  failure coupling rather than collapsing them into one score.

No single "coupling score" is planned. The baseline is a table of independently interpretable
measurements.

## Resume protocol

Read **Status** and **Bounded baseline and verdict — 2026-09-13** first. Confirm home workspace and fresh git status before doing new work. Do not restart broad measurement merely because shared HEAD moved: preserve the named frozen snapshot and version any changed source-population definition. Design approval is next; runtime implementation is not yet authorized.
## Status

**Phase:** bounded architecture review complete. **Slice 1 approved and implemented 2026-09-15**; slices 2–4 remain unauthorized. Read **Bounded baseline and verdict — 2026-09-13** below, then **Slice 1 — approved and implemented** at the end of this section.

**Verification:** 14 probe regression tests and self-test pass. Four applied mutation candidates were detected, zero survived. Second full gate completed with FMT_EXIT=0, CLIPPY_EXIT=0, LEAN_EXIT=0, DEFAULT_EXIT=0, in the required order; logs are under `.codescout/measurements/architecture-boundary/2026-09-13/gate2-*.log`. The earlier formatter refusal/default attribution failure remain in the first-run logs. These gates do not establish server-stack coverage.

**Source bound:** frozen snapshot 23adef79023ebc43ea30d8d8e50a2175bacab5aa, not current shared HEAD; corrected measurement commands retained their exit-2 HEAD-change warning. No numeric finding is a whole-project/compiler-resolution claim.

**Next action:** slice 1 is done pending commit — see below. Next is whether slice 2 (generalize semantic results) is authorized. Missing-symbol latency repetition and unresolved action paths remain explicitly scoped follow-ups. The measurement work stream — probe, regression tests, this tracker and its seven bug files — is committed in `d3a2c24f`.

### Slice 1 — approved and implemented

**Approved 2026-09-15** by the operator, with one amendment to the design as proposed: the background call **returns at spawn time** rather than after the 5s warm-up. That amendment turned out to resolve a tension rather than create one — `format_run_command` renders an absent `exit_code` as "running", which is a claim that could be false when emitted after a wait and is true by construction when emitted at spawn.

Shipped in one change, because each part makes the previous one non-vacuous:

- `BufferInner.background_jobs` is `HashMap<String, BackgroundJob>`, not `HashMap<String, PathBuf>`. A path cannot answer "did it finish, and how".
- A supervisor task owns the `Child` and records `JobState::Exited { code }` / `Failed`. `drop(child)` gave the process to tokio's orphan reaper and discarded the status; there is no other channel it exists on.
- The 5s warm-up window and `BackgroundKillGuard` are gone; the call returns immediately.
- Job state reaches the caller through the **response envelope** (`OutputBuffer::job_states_in` → a `jobs` array). It cannot travel the `@bg_` channel: that resolves by textual substitution to a filename, which is why a `tail @bg_x` returns the reader's exit code and never the job's. A job record with no read path would have been `cluster/declared-not-wired`.
- Eviction consults liveness and never unlinks a running job's log.

**Verification.** Gate green on all four lanes (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`), own test names read out of the default lane rather than the total. **Three mutations, each KILLED** in an isolated worktree: eviction predicate → FIFO (2 tests red), `status.code()` → `Some(0)` (red, `last seen: "exited 0"`), `job_states_in(command)` → `job_states_in("")` (red). Mutating the production path, not test inputs.

**Committed 2026-09-15** in `f098069a` — patch-id `8658a129d49444e33a6fce955a2f8b498bb743d1`. Archived with it: `b9935bb5470a799c` (background command loses terminal status) and `52f03908f97a948a` (eviction unlinks a running job's log), both carrying a killed mutation; and `d44b0a9aa38738f5` (the raw-pid kill guard), archived on the weaker basis that its remedy was deletion, so no regression test is possible — its **Tests added** section names what would re-introduce it. Scouting friction recorded as `architecture-boundary-session-log:F-2`; an instrument limitation found on the way as `a0dd1c43aeef41de`.
## Bounded baseline and verdict — 2026-09-13

**Status:** review evidence collected; recommendations below are proposals, not approved runtime changes.
**Valid:** dated 2026-09-13
**Rests on:** the frozen-source reports and live workflow record below; production-function reproductions and regression tests; architecture-boundary-session-log:F-1 and architecture-boundary-session-log:W-1.

### Evidence identity and acceptance bounds

Use `.codescout/measurements/architecture-boundary/2026-09-13/baseline-validated.json` for the numbers in this section: source `23adef79023ebc43ea30d8d8e50a2175bacab5aa`, branch experiments, measured **09:21:05–09:23:28 +03:00**. SHA-256: `aa9984c6f5b1dad485df3c6a326a89356a29f63b5f2c764d3062f5b8008b88a0`.

This is a **bounded frozen snapshot, not a clean current-tree certification**. The command exited 2 because shared HEAD moved to `e0b8d2359f9aec32d5f1cdbc18a49530f8725c1c` after collection. The report was written before that guard fired. Inspection of `frozen_source_tree` and `main` confirms static/context source and Cargo metadata were materialized from git archive at the starting HEAD, and history was explicitly queried at that SHA. The guard's failure is retained; it is not silently relabeled a passing run.

A second corrected run, `baseline-final.json`, also exited 2 when HEAD moved: source `02e61230b6708d8879185f6469ec02202bfdaacb`, ending HEAD `96574bfacee39c6e45168df3fd2eefb6d6847db6`, 09:24:51–09:27:03 +03:00. It independently recorded the same static edge total; its different history window is **not mixed into** this section. SHA-256: `9b6edf653fc70510dc109ab5007a1151cf73f2a7b0474149de61ba7da1d63797`.

Reject the static totals in `baseline-initial.json` and `baseline-actions.json`: external imports were falsely made local and identifiers containing “as” were mangled. Both production defects were reproduced, given failing regressions, corrected, and rerun. The change from 379 to 315 edge rows is an **instrument correction**, not an architectural improvement. See [invented imports](../issues/2026-09-13-architecture-probe-invents-internal-imports.md) and [mangled identifiers](../issues/2026-09-13-architecture-probe-mangles-as-identifiers.md).

The registry in the report came from a newly started installed release executable at `target/release/codescout`; its recorded size/mtime are not proof of source identity. The separate workflow checks used the attached MCP, whose status reported `git_sha=408709ea`, dirty build, deleted executable, PID 2178312. Neither runtime is silently equated to the frozen source.

### Filed defects

Seven bugs were filed from this work stream. The probe implementation and all
seven files are committed in `d3a2c24f`. Six are architecture-probe defects; the
seventh is a codescout defect this work surfaced, listed here because nothing
else found it — not because it is an instrument problem.

Architecture-probe defects:

- [Longer cycles omitted](../issues/2026-09-13-architecture-probe-omits-longer-cycles.md) — `a4d7380dc4643d88`.
- [Multiline context omitted](../issues/2026-09-13-architecture-probe-misses-multiline-context.md) — `d648b0a40c1c0bf8`.
- [Registration control accepts non-tools](../issues/2026-09-13-architecture-probe-registration-control-accepts-nontools.md) — `dd4a2785969615c9`.
- [Test-field masking consumes production structure](../issues/2026-09-13-architecture-probe-test-field-masking.md) — `83b1a79f605582b3`.
- [Invented internal imports](../issues/2026-09-13-architecture-probe-invents-internal-imports.md) — `0094594dea9a52b7`.
- [Mangled alias-like identifiers](../issues/2026-09-13-architecture-probe-mangles-as-identifiers.md) — `b6b0361595f8b3f5`.

Not an instrument defect:

- [Background command loses terminal status](../issues/archive/2026-09-13-background-command-loses-terminal-status.md) — `b9935bb5470a799c`. A `run_command` defect, surfaced by the live workflow checks below. **Fixed and archived 2026-09-15** in `f098069a`.

The last two probe defects are the pair that invalidated the earlier static
totals, and both carry `cluster/addressing-without-an-escape-hatch` (`IC-6`) —
which is the reason the 379 → 315 change is an instrument correction and not an
architectural improvement. Do not archive any of these as fixed until the
project's fix-evidence requirements are met: gate green plus a regression test,
with the fix SHA **and** its stable patch-id recorded.

Rejected artifact, retained deliberately: `baseline-initial.json` (1,356,751
bytes, SHA-256
`b578889215d60ec471d4ca08ebc8de51b786976457307abbd7ec1a1739948df3`) **predates
the action-row additions** — its context object has no `action_rows`, so it is
not merely superseded on edge totals; it cannot answer an action question at
all. Every measurement artifact named in this section is tracked under
`.codescout/measurements/architecture-boundary/2026-09-13/`.
### Static dependency observations

**Unit:** unique (classified production source file, classified target module) lexical edge. The raw list, its independently deduplicated file/module keys, and the matrix sum each recount to **315**, of which **144** cross populations.

| Source → target | Intelligence | Execution | Knowledge | Retrieval | Orchestration |
|---|---:|---:|---:|---:|---:|
| Intelligence | 36 | 1 | 0 | 1 | 25 |
| Execution | 6 | 10 | 0 | 0 | 25 |
| Knowledge | 7 | 1 | 80 | 10 | 18 |
| Retrieval | 3 | 0 | 1 | 23 | 14 |
| Orchestration | 9 | 12 | 7 | 4 | 22 |

These are not whole-project edges or compiler-resolved calls. The declared source populations contain **202 files**, leave **110** non-test Rust paths unclassified, and record **636 unresolved internal-target reference sites**. Bare roots are now left unresolved by the normalizer rather than promoted to fabricated local paths; they are not included in that explicit-internal-target count.

Crucially, unclassified paths include `src/tools/output_buffer.rs`, `src/tools/output.rs`, `src/tools/progress.rs`, `src/usage/mod.rs`, `src/fs/mod.rs`, `src/symbol/edit.rs` and `src/util/librarian_sync.rs`. This is a valid first population contract but **not a complete harness architecture map**. Do not change the population and compare totals as though the denominator stayed fixed.

The population graph contains **32 simple directed cycles**, spanning all candidate populations collectively. That is a graph description, not 32 independent design defects. A concrete reference is not automatically an interface bypass: the frozen adapter accepts `Arc<dyn crate::lsp::LspProvider>`, and returns `Arc<dyn crate::tools::Tool>`; those are existing interfaces worth preserving. The adapter's source lines 28/171/175 were checked at the frozen SHA.

The import control observes **16 source construction sites**, not the live tool denominator. Other inspected edges include `src/librarian/artifact_store.rs:20` importing `crate::retrieval::qdrant::QdrantWrap`. Several upward “orchestration” references are shared `Tool` / `ToolContext` / `RecoverableError` contracts, not domain workflows. Moving those contracts can improve dependency direction without adding a remote service.

### Runtime surface and action distinctions

The fresh registry advertised **21 distinct tools**. The textual direct-read scan sees `agent` in **15** registered tools' entry bodies; helper expansion reaches **20**. These are distinct measures, and the expanded one is a heuristic footprint, not an observed execution trace.

Helper-expanded capability buckets contain: 0 → none; 1 → get_guide; 2 → approve_write, library, onboarding, tree; 3 → call_graph, create_file, edit_code, grep, index, memory, references, semantic_search, symbol_at, symbols; 4+ → doc, edit_file, librarian, read_file, run_command, workspace.

The action inventory has **54 advertised enum actions**, plus **13 no-enum tool rows**, making **67 rows**. Of the enum actions, **49** have a textual branch and **5** remain unresolved: all advertised edit_file actions. A textual branch alone does not imply its transitive capabilities are resolved: edit_code's branches have no direct context reads because they delegate, not because editing is context-free.

Useful distinctions in the separate librarian context:

| doc action | Direct handler fields observed | Shared dispatch-prefix fields |
|---|---|---|
| get | catalog, current_project | catalog |
| delete | artifact_store, catalog, current_project | catalog |
| find | artifact_store, catalog, current_project, embedding, workspace | catalog |

This demonstrates why “doc depends on everything” is too coarse for designing an internal boundary. It does **not** prove every invocation of find uses embedding or every helper has been followed. Preserve unknown routes explicitly.

### Historical co-change observations

The fixed window has **500 distinct SHAs**. Parent-row inspection found **5 merge commits**; the probe's diff-tree command does not expand merge-parent diffs. This baseline therefore describes the emitted tracked-Rust path changes in that window, including inline test changes, not every semantic change introduced by every merge.

Per-population commit membership, independently recounted from the raw rows: intelligence **23**, execution **22**, knowledge **58**, retrieval **4**, orchestration **80**. **9** rows touch three or more populations.

Percentages below are rounded to one decimal place. A and B follow the first two columns.

| A | B | Intersection | Union | Jaccard | P(B given A) | P(A given B) |
|---|---|---:|---:|---:|---:|---:|
| Intelligence | Execution | 5 | 40 | 12.5% | 21.7% | 22.7% |
| Intelligence | Knowledge | 4 | 77 | 5.2% | 17.4% | 6.9% |
| Intelligence | Retrieval | 3 | 24 | 12.5% | 13.0% | 75.0% |
| Intelligence | Orchestration | 9 | 94 | 9.6% | 39.1% | 11.3% |
| Execution | Knowledge | 6 | 74 | 8.1% | 27.3% | 10.3% |
| Execution | Retrieval | 2 | 24 | 8.3% | 9.1% | 50.0% |
| Execution | Orchestration | 11 | 91 | 12.1% | 50.0% | 13.8% |
| Knowledge | Retrieval | 3 | 59 | 5.1% | 5.2% | 75.0% |
| Knowledge | Orchestration | 19 | 119 | 16.0% | 32.8% | 23.8% |
| Retrieval | Orchestration | 3 | 81 | 3.7% | 75.0% | 3.8% |

All pair intersections and unions were recomputed from raw memberships and matched. These modest overlaps do not show that the proposed domains must always ship together, but do not establish deployment independence either. Retrieval's denominator is only four commits; avoid a strong independence claim from it.

**Manual control now actually inspected:** `f909a160c3693e12505955f1f9f5323c134695b2`, the edit_file content/body parameter change. Its librarian change passes the level-specific key into a shared markdown helper; its execution change modifies edit_file's schema/alias; its server changes are tests and a surface-budget ratchet. Thus the three-population membership is correct at the file unit, while reading it as a three-domain production change would be wrong. The probe's field name `positive_control_inspected_commit` alone never proved this inspection.

### Feature and dependency observations

**Unit:** unique exact non-empty printed cargo-tree package line, including its printed decorations; not distinct crates, build time, RAM or attributable binary bytes.

| Lane | Features | Printed package lines |
|---|---|---:|
| Lean | no default features | 226 |
| Default | defaults | 339 |
| Server-stack only | no defaults + server-stack | 324 |
| Deployed | defaults + server-stack,local-embed | 512 |

All recorded dependency controls passed: serde_yml, pulldown-cmark and jsonschema absent from lean/present in default; qdrant-client absent from default/present in server-stack. The deployed lane retains defaults and matches the inspected cargo rb feature selection. No binary-size reduction is predicted from these line counts.

### Live workflow characterization

Raw calls, returned content and timings: `.codescout/measurements/architecture-boundary/2026-09-13/workflow-contracts.json`, SHA-256 `07d77644966f4f07f5ddf57075193f1d4a7397e57056bc4235d696a5596a7d68`. **14 recorded calls on one attached MCP instance**, not a model-evaluation sample.

- In a disposable Rust fixture, symbols returned answer() = 41 and cargo test failed with actual 41 / expected 42. Missing-target edit returned a recovery hint; valid replacement returned success; cargo test then passed its one test. No project source was edited for this demonstration.
- The missing-target call took **134,088 ms**; the following successful edit took **156 ms**. This is an observed outlier, **not** a measured steady-state latency or proven cold-LSP cause. Repetition with separated startup/miss costs is still owed before diagnosing it.
- Background `exit 7` returned “Process running” after **5,183 ms**, without a terminal code. Reading its empty log returned the **reader's** exit code 0. An explicit-marker control exposed FIXTURE_EXIT=7 after **5,166 ms**, but the response still said running. Source inspection of `spawn_background_command` confirms Child is dropped and the message is unconditional. Filed as [background terminal status](../issues/archive/2026-09-13-background-command-loses-terminal-status.md).
- A 300-line command stayed inline; a 3,000-line command produced a buffer handle. A targeted follow-up returned exactly lines 1500, 1501, 1502. The initial buffered text-content blocks measured **246 UTF-8 bytes**, the follow-up **54 bytes**. This characterizes successful disclosure, not a token-cost saving against a counterfactual model run.
- Pinned Cargo.toml reads returned harness-contract-fixture for the temporary root and codescout for home. This verifies those pinned calls. No process-wide activation race was deliberately induced in the shared MCP.

Timings include awaited transport/tool overhead and exclude model thinking time. No token estimates, latency percentiles, before/after agent-efficiency claims, cancellation tests or ambiguous-target tests are asserted.

### Proposed decisions and sequence

**Decision:** retain one MCP server; strengthen internal execution/result/request boundaries before considering domain extraction.
**Context:** the measured job-status failure requires a semantic lifecycle contract, not another deployment. The static graph also mixes service logic with shared tool/error interfaces; current populations omit important harness code.
**Alternatives considered:** domain microservices add remote identity, partial failures and lifecycle coordination without demonstrated independent deployment needs; file-size-driven splitting changes organization without proving a workflow gain; more prose reminders retain the agent's bookkeeping burden.
**Consequences:** easier to preserve current tooling, pins and LSP/retrieval seams while improving composability; harder to enforce ownership until contracts and migration adapters are explicit.
**Change scenarios absorbed:** waiting on failed jobs, recovering after compaction, rendering the same semantic result for different clients, and operating on pinned projects while other requests run.
**Revisit-when:** independently deployable workloads, measured resource isolation requirements, or a calibrated broader graph show a crate/process split paying for its added workflow complexity.
**Confidence:** high against a process split now; medium on exact migration order.

Recommended implementation slices, **not yet authorized**:

1. **Job lifecycle as the first vertical slice of a typed result contract.** Retain job state and terminal outcome separately from output buffers; provide status/result/cancel operations or equivalent explicit continuations. Change scenario: start a failing build, compact, and recover its actual result without shell markers. Alternative: status prose/log conventions are smaller but cannot reliably distinguish silent completion. Cost: ownership, cancellation and retention rules must be designed and tested. Confidence high on the need, medium on API shape.
2. **Generalize semantic results before further renderer growth.** `src/usage/mod.rs::classify_content_result` currently parses the first rendered content block as JSON to infer error/overflow. Separate outcome, completeness, provenance, notices and follow-up arguments from rendering. Change scenario: text, JSON and buffered projections preserve identical semantics and telemetry. Alternative: splitting a large file alone leaves this coupling intact. Cost: compatibility adapter and incremental tool-family migration. Confidence high on boundary, medium on migration cost.
3. **Resolve request context once.** Build on existing workspace pins rather than pretending they are missing. A scoped request object should make project identity and available operations explicit and avoid repeated ambient resolution. Change scenario: tool work remains tied to the chosen project across async steps. Cost: threading the resolved context through helpers and keeping transport concerns out of domains. Confidence medium; audit the unresolved helper paths before selecting the initial migration.
4. **Prepared edit and recoverable catalog/file mutations.** Preserve existing write locks, path guards, syntax checks and notifications; consolidate their commit/recovery mechanics across actual sibling implementations. Change scenarios: semantic edits and text edits invalidate the same state; a catalog failure after a file change is recoverable. Cost: file/SQLite ordering, rollback/reconciliation semantics and concurrency tests. Confidence medium; no crash-injection or shared-edit parity experiment was performed here.

Against the original domain questions: intelligence/execution interfaces can reduce imports on named edit/navigation workflows, but no reduction has been implemented or measured; knowledge already has separate context and optional dependency gates, so start with recoverable operations rather than process extraction; retrieval has too little history here to justify an ownership decision. None of the proposals claims measured binary savings. Cross-domain editing/indexing/activation remains explicit orchestration, not something to force into one owner.

### Verification and remaining work

Probe: **14 regression tests pass**, self-test passes. Final checked worktree SHA-256: probe `c51c8a6eb63ca550fd12d63ef521b92e9c3b38117a525cf08697a2613701d50d`; tests `eb51fc6402ea7f94d29fc0579190756c352ff0b02c75c8220bdc735dec4b6e38`. Four isolated applied mutations were detected (zero survivors among those four); the two import fixes additionally have observed pre-fix failures. This does not establish exhaustive Rust lexical correctness.

The first full gate had formatter refusal, clippy/lean success and a default-lane failure in a peer-modified attribution test. A second full gate is recorded in `gate2-*.log`; terminal results are recorded in the current Status section when available. Do not credit the first run as green or these lanes as server-stack coverage.

No runtime refactor was implemented, and nothing has been archived. The work stream is committed in `d3a2c24f`; the six architecture-probe defects it fixes remain `investigating` rather than archived, because archiving requires this project's fix evidence — gate green plus a regression test, with the fix SHA and its stable patch-id recorded on each file.

Next useful work is **design approval for the job/result slice**, not more broad scans. Before implementation, settle job lifetime across reconnects/compaction, cancellation ownership, retention and backward-compatible rendering; add deterministic failed/success/cancelled job checks. Separate follow-ups: repeat the missing-symbol latency check; resolve edit_file/edit_code action footprints; widen the frozen population only as a separately versioned measurement; characterize merges/renames before stronger historical claims.
