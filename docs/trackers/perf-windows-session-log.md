# Session Log — Perf + Windows Work Stream

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-02 | med | plan-prose | fixed-verified | WIN-26 zombie-open: lite stack Phases 0-4 shipped to master but tracker said "Phases 1-3 designed" |
| F-2 | 2026-07-02 | med | librarian-artifact | fixed-verified | windows tracker augmentation missing; body cites nonexistent artifact id 42dfdfc8b1522192 |
| F-3 | 2026-07-02 | high | release-pipeline | open | CI on experiments red since 2026-06-22+ across 8 jobs (pre-existing rot, exposed by Task 7 push) |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-02 | med | Measured dev-loop rebuild before/after toggling `lto = "thin"` -> `false` in `[profile.release]`; gated on >=15% speedup | Without measuring, thin-LTO's ~6s/rebuild tax (33% of single-file-touch wall time) would have stayed invisible, assumed as "standard release hygiene" | validated |
---

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — WIN-26 zombie-open: lite stack fully shipped but tracker said "Phases 1-3 designed"

**Observed:** 2026-07-02, perf+Windows brainstorm (recon pass before proposing approaches).

**When:** Summarizing open Windows work to scope the brainstorm; about to propose "finish WIN-26 Phases 1-3" as a design approach.

**Expected:** Tracker row WIN-26 (`docs/trackers/windows-platform-support.md`): status `open`, "Phase 0 shipped 825c0c52; Phases 1-3 designed in the plan."

**Got:** `docs/plans/2026-06-16-two-stack-retrieval-lite.md` marks Phases 0-4 ALL DONE (`0ff972f7`, `b96c8ae4`, `93ef0d43`, `9d40d36b`, `5c1ecfa8`); `git branch --contains 5c1ecfa8` → **master**; `src/retrieval/sqlite_code_store.rs` + `src/memory/sqlite_semantic_store.rs` exist. The lite stack is shipped and the lean build is the default.

**Probable cause:** Fix-then-forget: phases landed under `feat(...)` commits naming the plan, not the tracker row; no gate flips WIN-N rows (same root cause as CLAUDE.md's verify-open cadence note / W-7 of the bug-fix stream).

**Workaround:** Flipped the row open→fixed via `artifact(update, body_edits)` + History entry, same session.

**Severity:** med — already caused one wrong "what's open" report to the user this session; unrepaired, the brainstorm would have produced a spec re-implementing shipped code (a wasted design cycle at minimum).

**Status:** fixed-verified

**Fix idea / Pointer:** Row flipped 2026-07-02 (this session). Residual: the plan header still says `Status: draft` and its "Quality tradeoff" benchmark is unrun — left for the owner to decide whether the plan flips to done before a lite-quality benchmark.

---

## F-2 — windows-platform-support tracker: augmentation missing, body cites nonexistent artifact id

**Observed:** 2026-07-02, same recon pass, while attempting the documented WIN-26 row flip.

**When:** Following the tracker's "How to append" protocol (artifact_augment merge + table re-sync).

**Expected:** Tracker is an augmented artifact; `artifact_augment(id="42dfdfc8b1522192", merge=true, params={issues:[...]})` maintains the WIN-N rows; `entry_filter` queries work.

**Got:** Catalog artifact `52451519052d207c` has `augmentation: null`; `artifact(get, id="42dfdfc8b1522192")` → null (id not in catalog). The documented maintenance protocol and the advertised `entry_filter` queries are impossible — the rendered markdown table is the only real surface.

**Probable cause:** Tracker file recreated or re-cataloged after the original artifact (`42dfdfc8b1522192`) was created; the replacement never got re-augmented and the body comments kept the dead id.

**Workaround:** Maintained the row via `artifact(update, patch={body_edits:[...]})` directly.

**Severity:** med — silent: an agent following the in-file instructions either errors or creates a divergent fresh augmentation; `entry_filter` consumers get empty results that read as "no open issues".

**Status:** fixed-verified

**Fix idea / Pointer:** Re-augment `52451519052d207c` with `issues` params rebuilt from the 26-row table (params_path route — payload >9KB), set `entry_collection="issues"`, fix both in-body id references. Candidate task for the perf-windows plan.

---
## F-3 — CI on experiments has been red since 2026-06-22+ across 8 jobs (pre-existing rot, exposed by Task 7 push)

**Observed:** 2026-07-02, first CI run after pushing Tasks 1-8 (run 28582988236).

**When:** Watching the run that was meant to gate the new windows-gnu job.

**Expected:** Only the new windows-gnu job at risk; rest of the matrix green (local full gate was 2983/0/43).

**Got:** 9 of 15 jobs failed — but the previous run (28039317667, e559c8a8, 2026-06-23, BEFORE this session's work) shows the IDENTICAL failure set minus windows-gnu: Tool Docs Sync, Audit Doc Refs, Test windows-latest/default, and local-embed + no-features configs on all 3 OSes. Only ubuntu/macos default + fmt/clippy/MSRV were green.

**Probable cause:** Fix-then-forget at the CI-matrix level: the non-default feature configs and doc-sync gates rotted while local development gates only exercise default features on Linux.

**Workaround:** Triage scoped to the new windows-gnu job only (20 wine test failures, investigation ongoing); the 8 pre-existing red jobs are explicitly OUT of the perf-windows plan's scope.

**Severity:** high — CI has provided no gate signal on experiments for 9+ days; any regression in non-default configs lands silently.

**Status:** open

**Fix idea / Pointer:** Needs its own triage stream (bug files per failing cluster). Not this plan.

---
## W-1 — Dropping thin-LTO from `[profile.release]` cuts dev-loop rebuild time ~33%

**Observed:** 2026-07-02, Task 9 (build-loop baseline + one lever, thin-LTO)

**Pattern:** Measured the actual live dev loop — `touch src/lib.rs && cargo build --release --features server-stack` (this rebuilds the running MCP binary, `target/release/codescout`) — before and after a single lever change (`lto = "thin"` -> `lto = false`), and gated the keep/revert decision on an empirical >=15% mean speedup rather than assuming LTO's cost was negligible for a fast-iterating local dev loop.

**Counterfactual:** Without measuring, thin-LTO would have stayed in `[profile.release]` on the general assumption that LTO is default release-build hygiene. Every `cargo rb` iteration during active development would keep paying ~18.3s per single-file touch instead of ~12.3s — a ~6s tax per rebuild, compounding across dozens of iterations per session. Evidence: the `cargo build --timings` breakdown (pre-lever) showed the final `codescout "bin"` unit — the codegen+link step where thin-LTO's cross-CGU work happens — consumed 16.22s of the 18.0s total wall time (start offset 1.77s), i.e. LTO dominates the single-crate rebuild; the already-cached dependency graph is not where the time goes.

**Confirming data points:**
- Baseline (`lto = "thin"`), `touch src/lib.rs`, 2 runs: 18.42s, 18.14s -> mean 18.32s (1.5% spread). A third same-profile run under `--timings` gave 17.99s (Total time in `cargo-timing.html`: 18.0s), consistent with the baseline pair.
- Post-lever (`lto = false`), `touch src/lib.rs`, 2 steady-state runs: 12.11s, 12.36s -> mean 12.28s (2.0% spread). Build #3 (first post-change build, no touch needed — the profile-fingerprint change alone forced a full dependency recompile) was 110.06s and was correctly excluded as the expected one-time outlier per protocol.
- Gate math: (18.32 - 12.28) / 18.32 = 32.98% faster, well above the 15% keep threshold.
- sccache hit rate stayed low throughout (7 hits / 1147 misses, 0.61% cumulative across the whole session including builds, clippy, and test compiles) — confirming the touched `codescout` crate itself, not sccache-cacheable dependency compilation, is what the lever affects.
- Full pre-commit gate stayed green after the change: `cargo fmt --check` clean, `cargo clippy --release --features server-stack -- -D warnings` clean (45.88s), bare `cargo test` 2983 passed / 0 failed / 43 ignored (matches the project's known-good fingerprint exactly).

**Impact:** med — single lever, ~6s saved per manual dev-loop rebuild on this machine; compounds with the number of `cargo rb` iterations per session.

**Promote-when:** If a future audit finds the un-LTO'd binary's runtime latency (query/search hot paths) measurably regresses in a way that matters for local MCP usage, revisit — thin-LTO could be reintroduced behind a separate pre-ship-only build alias, keeping the fast lever as the default dev-loop profile.

**Status:** validated

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
