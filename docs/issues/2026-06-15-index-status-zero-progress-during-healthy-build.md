---
status: fixed
opened: 2026-06-15
closed: 2026-06-15
severity: medium
owner: marius
related: []
tags: [semantic-index, misleading-output, ux]
kind: bug
---

# BUG: `index(action="status")` reports `indexing.done:0/total:0` during a healthy project-scope build, reading as "stalled"

## Summary
During an in-flight project-scope `index(action="build")`, the status response's
`indexing` block sits at `{status:"running", done:0, total:0, eta_secs:null}` for
the *entire* build — by design, because `sync_project` does not stream per-file
progress. A reader (human or LLM) sees `0/0` that never moves and concludes the
build is hung. The real progress signal — the Qdrant `chunk_count`, which climbs
steadily — is present in the same response but easy to overlook.

## Symptom (Effect)
`index(action="status")` polled during a running build returns (shape):

```json
{
  "indexed": false,
  "project_id": "...",
  "message": "No chunks indexed ... Run index(action='build').",
  "indexing": { "status": "running", "done": 0, "total": 0, "eta_secs": null }
}
```

`done`/`total` flatline at `0` for the whole build; only on completion does
`indexing` flip to `{status:"done", total_files, total_chunks}`. There is no
field or hint distinguishing "0/0 because nothing started" from "0/0 because
progress isn't wired for this path."

## Reproduction
1. `git rev-parse HEAD` → `84d5dd6729e60298579ac2017260fc07ad226392` (branch `experiments`).
2. With the retrieval stack up, trigger a full rebuild: `index(action="build")`
   (or do something that orphans the index first, e.g. a project rename, so the
   build has real work).
3. Poll `index(action="status")` repeatedly while it runs.
4. Observe `indexing.done` and `indexing.total` stay at `0` the whole time;
   meanwhile the Qdrant point count for the `code_chunks` collection (and the
   top-level `chunk_count` once chunks land) climbs steadily.

## Environment
codescout MCP server (release binary), Linux, project `codescout`, branch
`experiments` @ `84d5dd67`. Qdrant-backed semantic index (`code_chunks`
collection); project-scope `sync_project` path.

## Root cause
Two halves, both in `src/tools/semantic/index.rs`:

1. **Producer never moves the counter.** The project-scope build (`IndexProject/call`)
   hands `sync_project` an opt-in `progress` token but never increments the agent's
   `IndexingState`. The comment at `src/tools/semantic/index.rs:288-291` states it
   verbatim: *"sync_project does not yet stream incremental progress; that deeper
   wiring is tracked separately. … IndexingState stays at Running{done:0, total:0}
   until completion sets Done/Failed."*
2. **Reporter echoes it unannotated.** `IndexStatus/call`
   (`src/tools/semantic/index.rs:380-495`) faithfully copies
   `IndexingState::Running { done, total, eta_secs }` into `result["indexing"]`.
   Given a producer that never moves `done`/`total`, the reporter emits a
   truthful-but-useless `0/0` with nothing marking it as the expected in-progress
   shape rather than a stall.

Compounding it: the Qdrant arm (`project_index_stats`) is evaluated in the *same*
function, but early in a build it can still return `(0, 0)`, so the response can
simultaneously read `indexed:false` AND `indexing.status:"running", 0/0` —
reinforcing the "nothing is happening" misread. The reliable liveness signal
(`chunk_count` climbing) only becomes visible once the first chunks land.

## Evidence
### Producer comment (state machine, build side)
`src/tools/semantic/index.rs:288-291`:
```
// sync_project does not yet stream incremental progress; that deeper
// wiring is tracked separately. `progress` (opt-in via progressToken)
// is moved in so future increments can report safely. IndexingState
// stays at Running{done:0, total:0} until completion sets Done/Failed.
```
### Reporter echoing it (status side)
`IndexStatus/call`, `src/tools/semantic/index.rs:380-495` — the
`IndexingState::Running { done, total, eta_secs }` match arm copies `done`/`total`
straight into `result["indexing"]` with no "progress not streamed" hint.
### The misdiagnosis it caused (2026-06-14 session)
Reading `done:0/total:0` as a hang, I cancelled **two healthy in-progress builds**
before verifying via Qdrant point-count growth (≈ +31,266 points over ~100s; the
build then completed in ~9.1 min, `last_indexed_commit` 84d5dd67). The status
field said "stuck"; the point count said "working." Only the point count was true.

## Hypotheses tried
1. **Hypothesis:** the build was genuinely stalled. **Test:** queried the Qdrant
   collection point count twice ~100s apart during a `0/0` window. **Verdict:**
   rejected — count grew +31,266; build healthy. **Evidence:** "the misdiagnosis
   it caused" above.
2. **Hypothesis:** the symbol-search (LSP/AST) index was also orphaned by the
   rename. **Test:** `symbols(name="open_db")`. **Verdict:** rejected — symbol
   search worked; only the semantic index was affected by the project-id change.

## Fix

Implemented **option (a) — annotate, don't stream** on `experiments` (not yet on `master`; held for further testing per the 2026-06-15 decision). In `IndexStatus/call` (`src/tools/semantic/index.rs`, the `IndexingState::Running` arm, ~line 439): when `done == 0 && total == 0`, the `indexing` block now carries:

- a `note`: *"per-file progress is not streamed for project-scope builds — 0/0 is the healthy in-progress shape, not a stall; watch chunk_count for liveness"*, and
- a `chunks_so_far` field surfacing the live Qdrant `chunk_count` (0 when none have landed yet) right next to the misleading counter.

Non-zero progress states are unchanged. Deeper **option (b)** — real per-file streaming from `sync_project` — is intentionally NOT implemented; it remains a separate future enhancement (the producer comment at `index.rs:288-291` still holds) rather than a bug. The misleading-output *symptom* this file tracks is fully addressed by (a).

**SHA:** experiments-side, committed alongside this bug-file update (master-side SHA pending — not cherry-picked until testing completes). Live-MCP verification (rebuild + `/mcp` restart, observe the annotation in a real `index(status)` during a build) is pending the next restart; logic is verified by the regression test below.
## Tests added

`index_status_running_zero_zero_carries_liveness_note` in `src/tools/semantic/tests.rs` (inserted after `index_status_shows_running_progress`, ~line 174). It drives the real `0/0` project-scope path and asserts `indexing.note` contains `"0/0"` + `"liveness"` and `chunks_so_far` is present; it ALSO asserts the non-zero path does **not** carry the note — guarding against the annotation leaking to the wrong branch.

`cargo test --lib index_status` → 9 passed, 0 failed. `cargo clippy --lib --tests -- -D warnings` → clean. The pre-existing `index_status_shows_running_progress` (which hand-sets a *non-zero* state) is unaffected — confirming the false-coverage caution: that test never exercised the `0/0` path, which is why this bug slipped.
## Workarounds
During a build, ignore `indexing.done`/`indexing.total`. Verify liveness by watching
the Qdrant `code_chunks` point count for the project grow (the top-level
`chunk_count` climbs once chunks land), or simply wait for `indexing.status` to flip
to `done`. **Do NOT cancel a build because `done/total` reads `0/0`** — that is the
healthy in-progress shape, not a stall.

## Resume

N/A — fixed via annotation (option a), regression test added and passing. If real per-file streaming is later wanted, that is a separate enhancement on the producer side (`IndexProject::call` / `sync_project` per `index.rs:288-291`), not this bug. Archive to `docs/issues/archive/` only after the fix ships to `master` (currently held).
## References
- `src/tools/semantic/index.rs:288-291` — producer comment ("stays at Running{done:0,total:0}").
- `src/tools/semantic/index.rs:380-495` — `IndexStatus/call` reporter.
- `src/tools/semantic/tests.rs:140-173` — `index_status_shows_running_progress` (does not cover this path).
- Session origin: 2026-06-14 semantic-index rebuild misdiagnosis (cancelled two healthy builds reading `0/0` as a stall).
