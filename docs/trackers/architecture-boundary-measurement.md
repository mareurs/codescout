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

## Verified starting state

Verified 2026-09-10 and 2026-09-11:

- codescout is a single Rust MCP server coordinating LSP, tree-sitter, retrieval, filesystem and
  shell operations, project memory, and the librarian catalog.
- VS Code successfully starts `/home/marius/.cargo/bin/codescout`; the MCP log reported
  `Connection state: Running` and `Discovered 18 tools` on 2026-09-11 at 08:54 local time.
- The Copilot conversation used for the initial investigation retained the tool registry captured
  before the MCP restart. `tool_search` returned zero codescout tools even while VS Code's MCP log
  showed 18. A new Copilot chat is required for the measurements below.
- `docs/PROBES.md` contains no existing architectural-coupling probe. This work needs a new
  instrument rather than adapting a neighboring metric and silently changing its unit.
- The top-level docs disagree about the tool count and retrieval shape. Counts and backend claims
  must therefore come from the live registry and current source, not from prose.

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

Read **Status** and **Bounded baseline and verdict — 2026-09-13** first. The compaction checkpoint is historical context; its pending test/measurement checklist has been superseded by this review. Confirm home workspace and fresh git status before doing new work. Do not restart broad measurement merely because shared HEAD moved: preserve the named frozen snapshot and version any changed source-population definition. Design approval is next; runtime implementation is not yet authorized.
## Status

**Phase:** bounded architecture review complete; recommendations await design approval. Read **Bounded baseline and verdict — 2026-09-13** below, which supersedes the earlier compaction checklist.

**Verification:** 14 probe regression tests and self-test pass. Four applied mutation candidates were detected, zero survived. Second full gate completed with FMT_EXIT=0, CLIPPY_EXIT=0, LEAN_EXIT=0, DEFAULT_EXIT=0, in the required order; logs are under `.codescout/measurements/architecture-boundary/2026-09-13/gate2-*.log`. The earlier formatter refusal/default attribution failure remain in the first-run logs. These gates do not establish server-stack coverage.

**Source bound:** frozen snapshot 23adef79023ebc43ea30d8d8e50a2175bacab5aa, not current shared HEAD; corrected measurement commands retained their exit-2 HEAD-change warning. No numeric finding is a whole-project/compiler-resolution claim.

**Next action:** discuss and approve the job/result vertical slice before runtime implementation. Missing-symbol latency repetition and unresolved action paths remain explicitly scoped follow-ups. No runtime refactor, commit or push was performed.
## Compaction checkpoint — 2026-09-13

**Status:** paused at the user's request to preserve context; measurement work remains in progress.
**Valid:** dated 2026-09-13
**Rests on:** the preserved review and raw measurement artifacts below, the production probe, its regression suite, and the linked reproduction records.

### Scope and decisions retained

The user requested an architecture review for a natural, efficient agent harness, then a review of this measurement tracker, then approved improving the probe and collecting the evidence. This is approval for measurement work, not an approved runtime refactor or service split. Keep one MCP surface while evaluating internal boundaries. No commit or push has been made by this work stream.

The original tracker is a good measurement contract: frozen populations, independently interpretable dimensions, positive controls, and no extraction decision based on size alone. Its fresh-Copilot-chat blocker is historical, not current: codescout calls work in this Codex session and this tracker is cataloged as `c61d542269b5c6de`. The added requirements are action-level distinctions, the actual deployed feature lane, and workflow evidence. Do not infer agent friction from error frequency alone.

### Architecture findings to carry forward

These are qualitative review findings and refactoring candidates, not conclusions derived from an accepted numeric baseline. The preserved original review contains the detailed source/live evidence and tradeoffs.

| Candidate | Weak spot identified in review | Direction to evaluate |
|---|---|---|
| Result contract | Outcome, payload, rendering, buffering, notices and follow-up instructions are entangled. An agent needs reliable distinctions between failure, partial results and successful completion. | Typed result envelope carrying outcome, provenance, completeness, notices and continuations; text/JSON/buffering as projections. |
| Request identity | Tools repeatedly resolve ambient Agent/project/workspace state. Activation and workspace overrides make it difficult to reason about which project a multi-step request uses. | Resolve caller/project/workspace once per request and pass explicit scoped capabilities. Preserve existing pins and guards. |
| Long-running work | Background commands expose detached log handles; indexing has a separate abort mechanism. A log location is not a job lifecycle contract. | Explicit job handles with running/completed/failed/cancelled state, terminal result and cancellation semantics. |
| Edits | File and semantic editing share preparation, locking, validation and invalidation concerns but express them through different paths. | Shared prepared-edit/commit/invalidation mechanics beneath the public tools; preserve syntax checks, path security and write locks. |
| Librarian mutations | Filesystem and SQLite catalog changes need coordinated recovery; successful work on one side must not silently look like complete success. | Recoverable mutation boundary with explicit partial-failure/reconciliation behavior. |

Do not replace working LSP and retrieval interfaces merely for tidiness. The first candidate is an internal seam; a crate/process split requires additional evidence. Priority among the candidates remains provisional until the baseline and representative workflows have been evaluated.

### Work already implemented, but not fully gated

Work-stream files:

- `scripts/architecture-boundary-probe.py`: existing untracked probe, modified in place; preserve pre-existing work.
- `tests/test_architecture_boundary_probe.py`: new regression tests.
- This tracker and the issue records below.

Probe changes implemented:

- Enumerate simple directed population cycles, including length-three cycles without reciprocal edges; canonicalize rotations.
- Require the server registration positive control to construct a name imported from `crate::tools`, rather than accepting an unrelated `Arc::new`.
- Recognize multiline context field reads and Agent aliases; separate direct field reads from helper-expanded footprints.
- Preserve production constructor/function delimiters when masking test-only struct fields and initializer fields.
- Add the deployed dependency lane `--features server-stack,local-embed` with defaults retained, alongside the original lanes. This follows the inspected `cargo rb` alias.
- Add advertised tool/action rows: direct branch reads, shared dispatch-prefix reads, separate librarian context, resolvable delegates and explicit unresolved routes.

The initial regression run observed failures before the corresponding fixes (including a missing direct-field result and missing action helpers). The first whole-repository run then failed on `Agent::new` because test-field masking consumed a production delimiter; a focused regression reproduced that before the fix. Latest checkpoint verification: `python3 tests/test_architecture_boundary_probe.py` exited 0, **12 tests ran and passed**. This is evidence for those fixtures, not an exhaustive Rust parser or runtime dependency graph.

Still pending: current self-test rerun, applied mutation checks, full repository gate, final whole-repository measurement with the action additions, and baseline interpretation. Earlier review-phase gate results do not validate these newer probe changes.

### Reproduced probe defects and records

All remain unarchived; implementations above exist in the worktree, but full verification and a durable fix commit are not yet recorded. Some issue Fix sections still describe the pre-implementation state; reconcile them during resume rather than interpreting that prose as evidence that no work happened.

- [Longer cycles omitted](../issues/2026-09-13-architecture-probe-omits-longer-cycles.md) — catalog `a4d7380dc4643d88`.
- [Multiline context omitted](../issues/2026-09-13-architecture-probe-misses-multiline-context.md) — `d648b0a40c1c0bf8`.
- [Registration control accepts non-tools](../issues/2026-09-13-architecture-probe-registration-control-accepts-nontools.md) — `dd4a2785969615c9`.
- [Test-field masking consumes production structure](../issues/2026-09-13-architecture-probe-test-field-masking.md) — `83b1a79f605582b3`.

Do not archive as fixed until the required verification and regression evidence are present; record fix SHA and stable patch-id when there is an authorized fix commit.

### Preserved artifacts and exact measurement identity

Local project copies are under `.codescout/measurements/architecture-boundary/2026-09-13/`. They survive conversation compaction and are accessible to other profiles using this checkout; they are **local artifacts, not committed backups**. Source and destination SHA-256 values were compared and match.

- `baseline-initial.json` — 1,356,751 bytes; SHA-256 `b578889215d60ec471d4ca08ebc8de51b786976457307abbd7ec1a1739948df3`.
- `codescout-agent-harness-architecture-review-2026-09-12.md` — 21,369 bytes; SHA-256 `f94937b2dfc9141f7dbca846a1eaa1430294e3e64411a2f0a76f05901a71e9cc`.

The preliminary JSON records:

- Branch `experiments`; frozen source HEAD `23adef79023ebc43ea30d8d8e50a2175bacab5aa`.
- Start `2026-09-13T09:04:55+03:00`; finish `2026-09-13T09:07:07+03:00`.
- Static/context source and Cargo measurements use tracked bytes from `git archive` at that HEAD; live tool registry uses the installed executable, not a verified build of that HEAD.
- `measurement_stable: true` and `worktree_changed_during_measurement: true`: frozen HEAD stayed the same while the shared worktree changed. The recorded worktree digests are not the frozen archive's digest.
- Dirty tracked Rust path recorded at both endpoints: `src/tools/run_command/attribution.rs`.
- **This file predates action-row additions**; its context object has no `action_rows`. Do not present it as the final action-aware baseline.

Checkpoint worktree SHA-256 values (not the version that necessarily generated the preliminary JSON):

- Probe: `e04ee21ef4a026e636fa3a423318603e165842e1673c5e12a355388b610e24d7`.
- Tests: `bc6e5b659b4cb87a5fab05a7b4125e0f622d44c290d1405a919a0dee5fd07b97`.

Original temporary locations, useful only if investigating provenance — **fenced deliberately, not for looks: these are `/tmp` paths that do not survive a reboot, and `audit_doc_refs` rates an unresolvable backticked path-shaped token `high`, which reds CI. A fenced block caps the severity (`code_block`); ordinary backticks get `policy_default`, which is `high`.**

```
/tmp/codescout-architecture-baseline-aTsJEG/baseline-initial.json
/tmp/codescout-agent-harness-architecture-review-2026-09-12.md
/tmp/codescout-boundary-cycle-VANLia/                     (cycle fixtures)
```

The durable copies are under `.codescout/measurements/architecture-boundary/2026-09-13/`, committed alongside this tracker — prefer those; the `/tmp` paths above are recorded only to tie the committed artifacts to the run that produced them. The portable cycle reproduction is also in its issue file.

### Instrument limitations that must remain visible

- Static scanning is lexical, not compiler resolution: macros, re-exports, unqualified references and conditional compilation require explicit bounds. A qualified concrete reference is not automatically proof of bypassing a trait boundary.
- Helper-expanded context footprints are heuristic, not exhaustive call graphs or observed runtime dependencies. Unique-name fallback can lack module identity; some method syntax remains unresolved.
- Action rows distinguish direct branch/shared-prefix observations and selected delegates; they do not yet prove complete transitive per-action dependence. Unknown routes must remain unknown, never zero.
- The history field named `positive_control_inspected_commit` is selected by the script; a human has **not yet inspected its diff**. It cannot be credited as a manual control. Review merge handling, renames and historical path classification before accepting ratios.
- Dependency weight counts unique exact printed package lines, not distinct crates, compile time, memory or binary contribution. Tree decorations can affect that unit.
- Installed binary version/size/mtime do not prove source identity. Preserve the runtime/source distinction.
- No end-to-end workflow measurements or model behavioral evaluations have been completed. Scripted round trips/bytes/latency are contract measurements, not spontaneous agent efficiency or actual token counts.

### Exact continuation checklist

1. Flush caches with `workspace(post_compact=true)`, confirm the home project, read this checkpoint, `docs/PROBES.md` and relevant architecture/friction memories. Inspect fresh git status: this is a shared, changing checkout with unrelated Rust, documentation and audit changes. Do not revert or format peers' files.
2. Re-read the current probe/test functions before editing. Re-run `python3 tests/test_architecture_boundary_probe.py` and `python3 scripts/architecture-boundary-probe.py self-test`. Apply candidate mutations in isolated copies and record observed kills/survivors; no such mutation pass has happened yet.
3. Run the current probe into a **new** artifact, retaining the preliminary one: `python3 scripts/architecture-boundary-probe.py all --repo /home/marius/work/claude/codescout --history-limit 500 --output <new-baseline.json>`. Capture terminal exit status, exact source HEAD, timestamps, probe hash and runtime identity. Ensure the result now contains action rows.
4. Independently recount raw edges, check known positive/negative references, inspect unclassified files and unresolved context/action routes. Manually inspect the selected co-change commit and validate history semantics; do not merely copy aggregate numbers.
5. Add bounded, reproducible live workflow checks in disposable fixtures: successful symbol edit and verification; missing/ambiguous target recovery; oversized-output follow-up; failed background command terminal status; project activation versus pinned operations. Record actual calls, outcome, elapsed time and UTF-8 output bytes separately. Do not mutate the user's source for a demo. If moving to model evals, first read the prompt-engineering operating guide.
6. Run the required repository gate for this work: `./scripts/fmt-mine.sh`, `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`, `cargo test --workspace --no-default-features`, then `cargo test --workspace`. Separate test lanes with `;`, not `&&`, and capture their individual exits so the default rebuild always runs. Respect formatter ownership refusals; do not substitute a workspace-wide formatter on this shared tree. Default/lean green is not coverage for server-stack; consult its CI lane.
7. Update the issue records with actual implementation/verification state. No commit/push is authorized by this checkpoint. Archive only when the project's fix evidence requirements are satisfied.
8. Publish a bounded observed baseline and revise candidate priorities in this tracker. Only then propose the first runtime refactor with its concrete workflow benefit, cost and regression controls.

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
- Background `exit 7` returned “Process running” after **5,183 ms**, without a terminal code. Reading its empty log returned the **reader's** exit code 0. An explicit-marker control exposed FIXTURE_EXIT=7 after **5,166 ms**, but the response still said running. Source inspection of `spawn_background_command` confirms Child is dropped and the message is unconditional. Filed as [background terminal status](../issues/2026-09-13-background-command-loses-terminal-status.md).
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

No runtime refactor, commit, push or archive operation was performed. Issue bodies are being reconciled with prototype implementation state; fix SHA/patch-id remain absent because no fix commit was made. Before committing this work, perform required peer coordination and final review of the exact diff.

Next useful work is **design approval for the job/result slice**, not more broad scans. Before implementation, settle job lifetime across reconnects/compaction, cancellation ownership, retention and backward-compatible rendering; add deterministic failed/success/cancelled job checks. Separate follow-ups: repeat the missing-symbol latency check; resolve edit_file/edit_code action footprints; widen the frozen population only as a separately versioned measurement; characterize merges/renames before stronger historical claims.
