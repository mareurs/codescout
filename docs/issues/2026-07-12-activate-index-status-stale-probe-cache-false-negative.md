---
status: open
opened: 2026-07-12
closed:
severity: low
owner: marius
related: []
tags: [activation, index-status, qdrant, cache, false-negative, ux]
kind: bug
---

# BUG: workspace(activate) reports index.status="not_indexed" for a fully queryable index — 500ms first-probe timeout poisons a session-lived cache

## Summary
The `index` field in the `workspace(action="activate")` response can report
`{"status":"not_indexed"}` while the index is in fact fully built and
queryable. It stems from a 500 ms first-probe timeout that caches a `false`
result for the rest of the session, whereas `index(action="status")` and
`workspace(action="status")` query Qdrant live and correctly report the index
as present. The authoritative tools are right; only the activation envelope is
misleading.

## Symptom (Effect)
Observed live 2026-07-12 on `experiments` @ `82d0183d`, three times in one
session (codescout first-activation, codescout return-home, backend-kotlin
switch). Each `workspace(action="activate")` returned:

```json
"index": {
  "status": "not_indexed",
  "hint": "Run index(action='build') to enable semantic_search."
}
```

…while an immediately-following `index(action="status")` on the same active
project returned the index as healthy:

```json
{ "indexed": true, "queryable": true, "project_id": "codescout",
  "collection": "code_chunks", "file_count": 1298, "chunk_count": 35546,
  "git_sync": { "status": "up_to_date", "last_indexed_commit": "8df2f430",
                "head_commit": "8df2f430" } }
```

The SessionStart banner also contradicted the activation field ("semantic_search
works now").

## Reproduction
1. Start the codescout MCP server cold (retrieval stack — Qdrant :6333/:6334 —
   not yet warm, e.g. just after boot, a sync, or an `/mcp` restart).
2. `workspace(action="activate", path="<a project with a built index>")` as the
   first activation of the session.
3. If the first Qdrant round-trip exceeds 500 ms, the response reports
   `index.status="not_indexed"`.
4. `index(action="status")` on the same project → `queryable: true` with real
   `file_count`/`chunk_count`. The two disagree.

Timing-dependent: on a warm stack the first probe returns < 500 ms and the bug
does not manifest. `git rev-parse HEAD` at observation: `82d0183d`.

## Environment
Linux, codescout MCP over stdio (Claude Code), branch `experiments`.
Retrieval stack: Qdrant + dense embedder up (per `.env.amd` / codescout
retrieval profile). Reproduced on both `codescout` and `backend-kotlin`
projects (independent `project_id`s, independent first-probe caches).

## Root cause
Activation and the status tools read index health through **two different
paths that share the same underlying query but not the same timeout/caching
policy**:

- `build_activation_response` (`src/tools/config/mod.rs:530-704`) computes the
  field via `check_has_index_cached(&project_name, &project_root_path)`
  (call ~`src/tools/config/mod.rs:577`) and maps `false → not_indexed`
  (~`:580-584`).
- `check_has_index_cached` (`src/tools/config/mod.rs:502-527`) is a
  stale-while-revalidate cache. **First probe per session** (cache miss) runs
  `tokio::time::timeout(FIRST_PROBE_TIMEOUT, check_has_index(..)).await
  .unwrap_or(false)` and then `index_status_put(project_id, false)` — i.e. **a
  timeout is cached as a definitive `false`.** `FIRST_PROBE_TIMEOUT` is
  **500 ms** (`src/tools/config/mod.rs:446`).
- `check_has_index` (`src/tools/config/mod.rs:424-436`) is the *same* query the
  authoritative tools use: `RetrievalClient::from_env()` →
  `project_index_stats(collection("code_chunks"), project_id)` → `chunks > 0`.
  So there is **no logic divergence** — a cold-stack first round-trip simply
  exceeds 500 ms (client construction + first scroll), the timeout fires, and
  `false` is cached.
- Every later activation takes the cache-HIT branch: returns the stale `false`
  immediately and only *then* spawns a detached refresh bounded by
  `BACKGROUND_REFRESH_TIMEOUT` = 30 s (`src/tools/config/mod.rs:480`). The
  poisoned `false` is corrected only once (a) that background refresh completes
  AND (b) a *subsequent* activation reads the now-`true` cache. A session with
  few activations can therefore report `not_indexed` for its entire life.

Contrast the correct path: `ProjectStatus::call`
(`src/tools/config/mod.rs:251-399`, the `workspace(action="status")` tool) and
`index(action="status")` issue the Qdrant `project_index_stats` query **live,
with no 500 ms deadline and no negative caching**, so they report
`up_to_date` / `queryable: true`.

## Evidence
- Live tool calls 2026-07-12 (see Symptom): activation `not_indexed` vs
  `index(status)` `queryable:true`, same project, seconds apart.
- Source, read directly this session:
  - `src/tools/config/mod.rs:502-527` (`check_has_index_cached`) — timeout →
    `unwrap_or(false)` → `index_status_put(pid, false)`.
  - `src/tools/config/mod.rs:446` — `FIRST_PROBE_TIMEOUT = 500ms`.
  - `src/tools/config/mod.rs:480` — `BACKGROUND_REFRESH_TIMEOUT = 30s`.
  - `src/tools/config/mod.rs:424-436` (`check_has_index`) — identical query to
    the authoritative path.

## Hypotheses tried
1. **Hypothesis:** `check_has_index` diverges logically from
   `project_index_stats` (returns false despite chunks). **Test:** read
   `check_has_index` body. **Verdict:** rejected — it calls the *same*
   `project_index_stats(..).map(|(chunks,_)| chunks > 0)`. See Evidence.
2. **Hypothesis:** name-vs-id key mismatch (`check_has_index_cached` is passed
   `project_name`, the param is named `project_id`). **Test:** compared to
   `index(status)` output. **Verdict:** deferred/unlikely — for the observed
   projects `project_name == project_id` ("codescout", "backend-kotlin"), so a
   mismatch cannot explain these cases, though it is worth hardening (projects
   whose name ≠ id would key the cache under the wrong string).
3. **Hypothesis (accepted):** 500 ms first-probe timeout on a cold retrieval
   stack caches `false`; stale-while-revalidate then serves it for the session.
   **Test:** trace `check_has_index_cached` + constants. **Verdict:** confirmed
   as the mechanism consistent with all three observations.

## Fix
Not started. Options (prefer 1+3 together):
1. **Do not cache timeouts.** On the first-probe timeout branch, return `false`
   for *this* response but skip `index_status_put` (or store a distinct
   "unknown/expired" sentinel), so the next activation re-probes instead of
   serving a poisoned negative.
2. **Widen `FIRST_PROBE_TIMEOUT`** (500 ms → ~2 s) to tolerate cold-stack
   latency. Cheap but still races on a very cold stack.
3. **Don't assert a false negative.** When the probe times out / the stack is
   unreachable, report `{"status":"unknown"}` (or omit the field) rather than
   `not_indexed` — the current wording actively tells the agent to run
   `index(action='build')` on an already-built index.
4. Optionally have activation fall back to the live `project_index_stats` path
   (as `ProjectStatus::call` does) when the cached value is negative.

## Tests added
N/A — not yet fixed. A regression test should assert that a first-probe timeout
does not persist a cached `false` across activations (inject a slow/offline
retrieval client, assert the second activation re-probes rather than returning
the cached negative). Note `src/tools/config/tests.rs` already fixes
`not_indexed` in several activation-format fixtures — those pin the *rendering*,
not the probe policy, and would need a timing-aware harness.

## Workarounds
Ignore `workspace(activate)`'s `index` field; treat `index(action="status")`
(or `workspace(action="status")`) as authoritative — both query Qdrant live and
report the true state. Re-activating later in the session usually self-corrects
once the background refresh lands.

## Resume
Implement Fix option 3 (+1) in `src/tools/config/mod.rs`: in
`check_has_index_cached` (`:502-527`), stop persisting the first-probe timeout
result as a definitive `false`; and in `build_activation_response` (`:~580-584`)
map an indeterminate probe to `status:"unknown"`/omitted rather than
`not_indexed`. Add a regression test with an offline/slow retrieval client
asserting activation #2 does not serve a cached false negative. Verify the
existing `src/tools/config/tests.rs` activation-format fixtures still pass or
update the ones that assert `not_indexed`.

## References
- `src/tools/config/mod.rs:530-704` (`build_activation_response`)
- `src/tools/config/mod.rs:502-527` (`check_has_index_cached`)
- `src/tools/config/mod.rs:424-436` (`check_has_index`)
- `src/tools/config/mod.rs:446` (`FIRST_PROBE_TIMEOUT`), `:480`
  (`BACKGROUND_REFRESH_TIMEOUT`)
- `src/tools/config/mod.rs:251-399` (`ProjectStatus::call` — correct live path)
- `src/tools/config/tests.rs` (activation-format fixtures asserting
  `not_indexed`)
- Related prior index-status bugs (both fixed):
  `docs/issues/2026-06-15-index-status-done-totals-always-zero.md`,
  `docs/issues/2026-06-15-index-status-zero-progress-during-healthy-build.md`
