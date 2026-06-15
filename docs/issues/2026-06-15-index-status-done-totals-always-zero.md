---
status: fixed
opened: 2026-06-15
closed: 2026-06-15
severity: low
owner: marius
related: []
tags: [semantic-index, misleading-output, ux]
kind: bug
---

# BUG: `index(status)` `done` state reports `total_files: 0, total_chunks: 0` despite the real totals being present

## Summary
After a project-scope build completes, `index(action="status")` returns an
`indexing` block with `status: "done"` whose `total_files` and `total_chunks`
are always `0` — even though the **same response** carries the correct totals
at the top level (`file_count`, `chunk_count`). Sibling of the just-fixed
`Running{0,0}` bug (`2026-06-15-index-status-zero-progress-during-healthy-build.md`),
but a distinct mechanism: there the counts genuinely didn't exist yet; here
they exist and are simply not wired into the `done` summary.

## Symptom (Effect)
```json
{
  "indexed": true,
  "file_count": 1185,
  "chunk_count": 31286,
  "indexing": {
    "status": "done",
    "files_indexed": 15,
    "files_deleted": 17,
    "detail": "elapsed_ms=10714",
    "total_files": 0,        //  ← contradicts top-level file_count: 1185
    "total_chunks": 0        //  ← contradicts top-level chunk_count: 31286
  }
}
```
Observed **3×** this session (one incremental build → `files_indexed: 31`, one
force reindex → `files_indexed: 36411`, one incremental → `files_indexed: 15`);
`total_files`/`total_chunks` were `0` in every case.

## Reproduction
1. `git rev-parse HEAD` → `cf308600` (branch `experiments`).
2. `index(action="build")`, wait for completion.
3. `index(action="status")` → the `indexing.done` block shows
   `total_files: 0, total_chunks: 0` while top-level `file_count`/`chunk_count`
   are correct.

## Environment
codescout MCP server (release binary), Linux, project `codescout`, branch
`experiments` @ `cf308600`. Qdrant-backed semantic index (`code_chunks`).

## Root cause
Task #91 was left half-wired. Two halves, both in `src/tools/semantic/index.rs`:

1. **Producer hardcodes 0.** The sync-task completion path (`IndexProject::call`,
   `src/tools/semantic/index.rs:323-332`) builds `IndexingState::Done { … total_files: 0,
   total_chunks: 0 }` with an explicit comment: *"Total counts now live in Qdrant —
   IndexStatus re-route (task #91) will scroll the collection for these. For now leave
   0 to avoid a sqlite round-trip…"*. So the producer deliberately defers totals to the
   reporter.
2. **Reporter never picked them up.** `IndexStatus/call`
   (`src/tools/semantic/index.rs:470-484`) destructures the `Done` variant and emits
   `total_files`/`total_chunks` **verbatim** — i.e. the `0` placeholders — instead of
   sourcing them from the Qdrant scroll it already performed (the top-level
   `file_count`/`chunk_count`, computed earlier in the same function). The planned
   "IndexStatus re-route" never landed.

The `Done` variant's `total_files`/`total_chunks` fields (`src/agent/mod.rs:33-34`) are
otherwise inert: the only other consumer, `src/agent/mod.rs:1154`, maps `Done { .. }`
→ `"indexed"` and ignores them, and `src/mcp_resources/project_summary.rs:373` only sets
them in a test fixture. So nothing depends on the `0` placeholder — it is pure
misleading output.

## Evidence
### Producer comment (placeholder rationale)
`src/tools/semantic/index.rs:323-332` — the `total_files: 0, total_chunks: 0` block with
the task-#91 deferral comment.
### Reporter echoing the placeholder
`src/tools/semantic/index.rs:470-484` — the `IndexingState::Done` arm copies
`total_files`/`total_chunks` straight from the variant into the JSON.
### Live observations (2026-06-15, 3 datapoints)
All three builds this session returned `indexing.done.total_files = 0` and
`total_chunks = 0` while top-level `file_count`/`chunk_count` were correct.
### Documented intent
`docs/manual/src/tools/semantic-search.md:166-167` documents the status output's
`total_files`/`total_chunks` as carrying real totals — so the fix is to populate
them, not remove them.

## Hypotheses tried
1. **Hypothesis:** the producer could populate the totals directly. **Test:** read
   the `Done` construction + comment. **Verdict:** rejected as the fix site — the
   producer deliberately avoids the count query (the comment's "avoid a sqlite
   round-trip"); the reporter already holds the Qdrant counts. Reporter-side is the
   intended + cheap fix.

## Fix

Implemented **reporter-side** on `experiments`, fulfilling task #91's intent (not yet on `master`; held for testing). Added `pub(crate) fn resolve_done_total(result, key, fallback) -> u64` (`src/tools/semantic/index.rs`) that prefers the top-level Qdrant count (`result["file_count"]` / `result["chunk_count"]`) and falls back to the variant's placeholder only when Qdrant didn't supply one (offline / not indexed). `IndexStatus/call`'s `IndexingState::Done` arm now resolves `total_files`/`total_chunks` through it instead of echoing the `0` placeholders.

The producer's hardcoded-0 in `IndexProject::call` is intentionally left as-is — by design the totals are sourced at report time from the Qdrant scroll the reporter already performs, avoiding the redundant count query the original comment flagged.

**SHA:** experiments-side, this commit (master-side SHA pending — not cherry-picked until testing completes).
## Tests added

`resolve_done_total_prefers_qdrant_count_over_placeholder` in `src/tools/semantic/tests.rs` — asserts (a) when `result` carries `file_count`/`chunk_count`, the helper returns the Qdrant value rather than the `0` placeholder; (b) when the key is absent (Qdrant offline), it returns the fallback. `cargo test --lib semantic` → 45 passed, 0 failed; `cargo clippy --lib --tests -- -D warnings` → clean. The full path (real Qdrant counts flowing into the `done` block) is verified live, since the test harness has no populated Qdrant — hence the helper extraction, which makes the preference logic unit-testable in isolation.
## Workarounds
Read the **top-level** `file_count` / `chunk_count` for the real totals; ignore
`indexing.done.total_files` / `total_chunks` until the fix lands.

## Resume

N/A — fixed reporter-side + unit test. If the `total_files`/`total_chunks` fields on `IndexingState::Done` are ever fully retired (the producer comment's "step 8"), drop them from the variant + the reporter + `resolve_done_total` together. Archive to `docs/issues/archive/` only after the fix ships to `master` (currently held).
## References
- Sibling (fixed): `docs/issues/2026-06-15-index-status-zero-progress-during-healthy-build.md`.
- Producer placeholder: `src/tools/semantic/index.rs:323-332` (task #91 deferral comment).
- Reporter: `src/tools/semantic/index.rs:470-484`.
- `Done` variant: `src/agent/mod.rs:33-34`.
- Documented shape: `docs/manual/src/tools/semantic-search.md:166-167`.
