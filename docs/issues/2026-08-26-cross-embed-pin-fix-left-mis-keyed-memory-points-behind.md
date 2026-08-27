---
id: '162f24579375f3c8'
kind: bug
status: open
title: 'BUG: the workspace-pin cross-embed fix shipped without a back-fill, so memories written during the defect window are still keyed to the wrong project'
tags:
- memory
- semantic-store
- workspace-pin
- data-integrity
- backfill
opened: 2026-08-26
owner: marius
related:
- docs/issues/archive/2026-07-10-memory-cross-embed-ignores-workspace-pin.md
- docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
severity: medium
unverified: 'Only ONE of the three missing prompt-engineering points is traced to the pin defect: prompt-tdd-skill-eval-confounds, by created_at (~2026-07-04) falling inside the 2026-07-04..2026-07-13 window and by its point being the matching codescout orphan. Why `language-patterns` and `onboarding` have no point anywhere is NOT established -- cross_embed_memory is explicitly best-effort and non-fatal (it warns and the write still reports success), so a silent embed failure is a plausible cause but is inferred, not measured. Whether other umbrella projects hold mis-keyed points from the same window is NOT established: two projects were checked. `codescout migrate-memories --in-place` is quoted from index(action="verify")''s own hint and has NOT been run or verified this session.'
---

## Summary

`docs/issues/archive/2026-07-10-memory-cross-embed-ignores-workspace-pin.md` (severity
**high**, tagged `data-integrity`) was fixed in `0cefd1f3` on 2026-07-13 and archived. The
fix corrected three code sites and added a pinned-vs-default regression test. It did
**not** touch the data the defect had already written.

At least one mis-keyed point survives. `prompt-tdd-skill-eval-confounds` is a
**prompt-engineering** memory whose file lives at
`prompt-engineering/.codescout/memories/`, but whose vector point is keyed to
**codescout**. Its store `created_at` is `1783155391` (~2026-07-04) — nine days *before*
the fix, i.e. squarely inside the defect window.

The effect is that the memory is invisible to `recall` in the repo that owns it, and
returns as noise in a repo it has nothing to do with.

## Symptom (Effect)

`memory(action="list")` and `memory(action="recall")` disagree, silently, because they read
different substrates — `list` walks a directory, `recall` queries the vector store. Neither
reports the divergence, so each answers confidently in its own world.

- `memory(list, workspace=prompt-engineering)` → **10 topics**, including
  `prompt-tdd-skill-eval-confounds`.
- `memory(recall, workspace=prompt-engineering, limit=20)` → **7 points**, `has_more:
  false`, and the topic is absent.

The memory's content is exactly the kind that being unreachable hurts most: it records
three ways a prompt-tdd skill A/B silently measures base-model behaviour instead of the
skill. `CLAUDE.md` tells you to read that class of ledger *before running an eval*.

## Reproduction

```
memory(action="list",   workspace="/home/marius/work/claude/prompt-engineering")   → 10 topics
memory(action="recall", workspace="/home/marius/work/claude/prompt-engineering",
       query="prompt-tdd skill A/B craft traps install dir confound ablation",
       limit=20)                                                                   → 7 results
index(action="verify",  workspace="/home/marius/work/claude/prompt-engineering")
    → memories: {on_disk: 10, in_store: 7, missing_count: 3}
index(action="verify")            # active project = codescout
    → memories: {on_disk: 23, in_store: 25, orphan_count: 2,
                 orphan_sample: ["prompt-tdd-skill-eval-confounds", "zz-probe-delete-me"]}
```

The same memory appears as `missing` from one project and `orphan` from the other. That
pair *is* the bug.

## Environment

Qdrant-backed `memories` collection, `server-stack` profile, 2026-08-26. Both projects are
members of the `codescout-ecosystem` umbrella.

## Root cause

The historical code defect is established and fixed — `cross_embed_memory` resolved
`project_id` via `active_project()` rather than the `workspace=` pin, so a cross-embed
issued under a pin landed the point under the session-default project. That is stated
verbatim in the docstring of the regression test
`cross_embed_memory_stores_under_pinned_project_not_session_default`
(`src/tools/memory/tests.rs`), and the three sites now use
`ctx.agent.with_project_at(ctx.workspace_override.as_deref(), ...)`.

**What is unfixed is the data.** The archived bug's `## Fix` lists the three sites and the
test; its `## Resume` says "re-check whether any other best-effort cross-store helper
resolves project_id unpinned" — another *code* sweep. Neither mentions the points already
written. A regression test guards new writes; nothing repairs old ones, and nothing
schedules a repair either, so the record reads as fully closed.

This is the same shape as `bug-fix-session-log:F-69` on the citation side: the corrective
event and the cleanup event are separate, and only the first has an owner.

## Evidence

- Store point `67f92aba-f155-59e2-b902-52665a594afa`, title
  `prompt-tdd-skill-eval-confounds`, `created_at 1783155391` (~2026-07-04), returned by
  `recall` **in codescout**.
- File present at `prompt-engineering/.codescout/memories/prompt-tdd-skill-eval-confounds.md`
  (27 lines) with an anchor sidecar naming `src/prompt_tdd/adapters/claude_code.py` — a
  prompt-engineering path that does not exist in codescout.
- No git history for that slug under `codescout/.codescout/memories/`, which *is* tracked
  (positive control: `.codescout/memories/gotchas.md` returns commits). So the file was
  never in codescout; only the point ever was.
- Fix commit `0cefd1f3` 2026-07-13, `fix(memory): pin project_id in
  cross-embed/anchor/delete paths`.

**Ruling out a similarity floor:** the absence from `recall` is not a ranking artifact.
Two independent queries — one built from the memory's own vocabulary, one aimed at
`onboarding` — each returned the identical 7 points with `has_more: false`, ranking down
to **0.09** similarity. There is no threshold hiding them.

## Fix

Not yet applied. Two halves, and they must go in this order:

1. **Restore the losing side first.** `index(action="verify")`'s own hint prescribes
   `codescout migrate-memories --in-place`, which reads the memories from disk
   server-side and writes their points. Run it for `prompt-engineering`; it repairs all
   three missing points in one pass.
2. **Then drop the mis-keyed point** from codescout with
   `memory(action="forget", id="67f92aba-f155-59e2-b902-52665a594afa")`. Only after step 1,
   because until then that point is the only embedded copy of the content anywhere.

Reversing the order risks deleting the sole point before its replacement exists.

## Tests added

None yet. A regression test for this is awkward and worth naming rather than skipping: the
code path is already correct and pinned by
`cross_embed_memory_stores_under_pinned_project_not_session_default`. What is missing is
not a behaviour but a **reconciliation** — nothing compares memory files on disk against
points in the store as a routine gate. `index(action="verify")` already computes exactly
that comparison; it is simply not run anywhere automatically.

## Workarounds

Read the memory by topic. `memory(action="read", topic=..., workspace=...)` walks the
directory and is unaffected — it is only `recall` that cannot see it.

## Resume

1. Run `codescout migrate-memories --in-place` for prompt-engineering; re-run
   `index(action="verify", workspace=...)` and confirm `missing_count: 0`.
2. `memory(action="forget", id="67f92aba-...")` on codescout; confirm `orphan_count` drops.
3. Separately drop `zz-probe-delete-me` — unrelated to this bug, a self-named probe left by
   the investigation of `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md`,
   which is fixed and archived.
4. Decide whether the defect window (2026-07-04 .. 2026-07-13) touched other projects. Two
   were checked; the umbrella has more.

## References

- `docs/issues/archive/2026-07-10-memory-cross-embed-ignores-workspace-pin.md` — the fixed
  code defect this is the data residue of
- `docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md` —
  same family, different axis: that one is disk-vs-disk (two surfaces reading two
  *directories*); this one is disk-vs-store
- `docs/issues/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md` — the
  other live source of junk in the same collection
- `docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md` —
  `index(action="status")` reports `ok`/`up_to_date` for a store `verify` calls incomplete
