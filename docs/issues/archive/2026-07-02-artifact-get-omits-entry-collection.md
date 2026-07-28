---
status: fixed
opened: 2026-07-02
closed: 2026-07-04
severity: low
owner: marius
related: []
tags: [librarian, tool-surface]
kind: bug
---

# BUG: artifact(get) does not echo entry_collection for augmented artifacts

## Summary
`artifact(action="get")` on an augmented artifact returns the `augmentation` object
(prompt + params) but omits the `entry_collection` field, so a reviewer/agent cannot
confirm the field's value by direct read — it must be inferred from `entry_filter`
behavior. Read-side serialization gap only; writes are unaffected.

## Symptom (Effect)
During the Task 8 review of the windows-tracker re-augmentation (2026-07-02), the
reviewer grepped the full 219-line `artifact(get, id="52451519052d207c")` augmentation
payload for `entry|collection`:

```
zero hits — no entry_collection key anywhere in the get response
```

Yet `artifact(get, id="52451519052d207c", entry_filter={"status":{"eq":"open"}})`
returned exactly the expected singleton row (WIN-18), proving `entry_collection="issues"`
IS set server-side.

## Reproduction
1. `artifact_augment(id=<any>, prompt="...", params={rows:[...]}, entry_collection="rows")`
2. `artifact(action="get", id=<same>)` → inspect `augmentation` — no `entry_collection` key.
3. `artifact(action="get", id=<same>, entry_filter={...})` → works, proving the field is set.

## Environment
codescout `experiments` @ 6f30b6dd, MCP live server, Linux.

## Root cause
Unknown — likely the `get` action's augmentation serializer selects prompt/params only.
Candidate: the augmentation row → JSON projection in `src/librarian/` (get.rs or the
augmentation model's Serialize impl) omits the column.

## Evidence
See Symptom — Task 8 reviewer's independent catalog queries,
`.superpowers/sdd/task-8-report.md` + review transcript (2026-07-02).

## Hypotheses tried
1. **Hypothesis:** field set but not serialized on read. **Test:** entry_filter
   functional probe (above). **Verdict:** confirmed (functionally) — write path fine,
   read path omits. Code-level confirmation pending.

## Fix

Fixed 2026-07-04 (experiments). The `artifact(get)` augmentation projection in
`src/librarian/tools/get.rs` now serializes the full augmentation row —
`entry_collection`, `append_mode`, `history_cap`, `render_template`, `params_schema`
(plus the new `refreshed_at_commit`) — not just prompt/params. Landed alongside the
finding-7 provenance keys, which touch the same projection.
## Tests added

`get_includes_entry_collection_in_augmentation` (`src/librarian/tools/get.rs`) —
augments an artifact with `entry_collection="rows"`, `append_mode=true`,
`history_cap=10`, then asserts all three appear in the `artifact(get)` `augmentation`
object.
## Workarounds
Infer via `entry_filter` probe: a correct singleton result proves the collection is wired.

## Resume
Locate the augmentation→JSON projection used by `artifact(action="get")` in
`src/librarian/` (start: `grep "augmentation" src/librarian/catalog/get.rs` or
`symbols(path="src/librarian")`); check which fields it serializes; add the missing ones
+ a round-trip test (augment with entry_collection → get → assert field present).

## References
- `.superpowers/sdd/task-8-report.md` (this repo, scratch)
- docs/trackers/perf-windows-session-log.md (work stream)
