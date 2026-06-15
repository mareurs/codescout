---
status: open
opened: 2026-06-15
closed:
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
Not yet implemented (status `open`). Options, smallest first:

- **(a) Annotate, don't stream (cheap, recommended now):** in `IndexStatus/call`,
  when `IndexingState::Running { done: 0, total: 0, .. }`, add a
  `note:"per-file progress not streamed for project-scope builds — watch chunk_count
  for liveness"` and/or surface the live Qdrant `chunk_count` inside the `indexing`
  block so a climbing number is visible at a glance. Kills the misdiagnosis without
  the streaming rewrite.
- **(b) Stream real progress (correct, larger):** wire `sync_project` to increment
  `IndexingState` per file/batch via the already-moved `progress` token, so
  `done`/`total` reflect reality. This is the "tracked separately" deeper wiring the
  comment refers to.
- **(c) Doc-only fallback:** document in `get_guide` / server instructions that
  `0/0` during a build is healthy and `chunk_count` growth is the liveness signal.

Recommendation: (a) now, (b) when the streaming wiring lands.

## Tests added
N/A — not yet fixed. **Caution for whoever fixes it:** the existing test
`index_status_shows_running_progress()` (`src/tools/semantic/tests.rs:140-173`)
hand-sets a *non-zero* `Running` state and asserts it surfaces — so it stays green
while the real project-scope path emits `0/0`. A regression test for THIS bug must
drive the actual `sync_project` path (or assert the new `0/0` annotation from fix
(a)), not a hand-set state — otherwise it's false coverage.

## Workarounds
During a build, ignore `indexing.done`/`indexing.total`. Verify liveness by watching
the Qdrant `code_chunks` point count for the project grow (the top-level
`chunk_count` climbs once chunks land), or simply wait for `indexing.status` to flip
to `done`. **Do NOT cancel a build because `done/total` reads `0/0`** — that is the
healthy in-progress shape, not a stall.

## Resume
Implement fix (a): edit the `IndexingState::Running` match arm in `IndexStatus/call`
(`src/tools/semantic/index.rs`, within 380-495) to add the liveness `note` and inline
`chunk_count` when `done==0 && total==0`. Add a regression test that drives the
project-scope status path and asserts the annotation is present — the existing
`index_status_shows_running_progress` does NOT cover this (it hand-sets a non-zero
state). Run `cargo test --lib semantic`.

## References
- `src/tools/semantic/index.rs:288-291` — producer comment ("stays at Running{done:0,total:0}").
- `src/tools/semantic/index.rs:380-495` — `IndexStatus/call` reporter.
- `src/tools/semantic/tests.rs:140-173` — `index_status_shows_running_progress` (does not cover this path).
- Session origin: 2026-06-14 semantic-index rebuild misdiagnosis (cancelled two healthy builds reading `0/0` as a stall).
