---
id: '73158c500ff6b293'
kind: bug
status: open
title: SDD ledger directory and this work-stream's catalog rows both vanished between sessions
tags:
- librarian
- catalog
- sdd
- data-loss
closed: null
opened: 2026-08-25
owner: marius
related:
- docs/issues/2026-08-23-research-index-tracker-has-no-augmentation.md
- docs/issues/archive/2026-05-17-reindex-cascade-delete-data-loss.md
severity: high
unverified: Root cause undetermined — the loss was detected after the fact, with no session running to observe it.
---

# BUG: SDD ledger directory and this work-stream's catalog rows both vanished between sessions

## Summary

Between 2026-08-24 17:13 (commit `85bd01e1`) and 2026-08-25 08:00, two things
belonging to the hidden-information-eval work stream disappeared: the git-ignored SDD
ledger directory `.superpowers/sdd/2026-08-23-hidden-information-eval/` was deleted from
disk, and the librarian catalog rows for that stream's plan and spec were dropped even
though both files were still on disk. Nothing in git was lost. The ledger was
reconstructed from the session transcript (see **Workarounds**); the catalog was repaired
by `librarian(action="reindex")`.

## Symptom (Effect)

Catalog side — the plan artifact's id, read from the file's own committed frontmatter,
no longer resolved:

```
artifact(action="get", id="89c2984ca7c074a0")
→ unknown artifact id '89c2984ca7c074a0'. If this id came from an earlier call, an
  artifact(action="move") since then will have re-keyed it (id = sha256(abs_path))…
```

A path query found nothing either, at the widest scope and with archived rows included:

```
artifact(action="find", filter={"rel_path": {"contains": "hidden-information"}},
         include_archived=true, scope="umbrella")
→ {"count": 0, "items": []}
```

The `find` response's own hints named the real shape of the problem:

```
"unindexed_files": 12,
"unindexed_hint": "12 file(s) under this scope are not in the catalog and cannot match
                   any filter; run librarian(action=\"reindex\") to include them"
```

Filesystem side — the ledger named by the plan's `ledger:` frontmatter key was gone:

```
read_markdown(".superpowers/sdd/2026-08-23-hidden-information-eval/progress.md")
→ file not found
```

## Reproduction

Not yet reproducible — the loss was noticed on resuming after a compaction, with no
session running to observe the deletion. Best leads are under **Hypotheses tried**.

`git rev-parse HEAD` at detection: `047dd433`.

## Environment

- codescout `experiments`, main checkout `/home/marius/work/claude/codescout`
  (5 linked worktrees registered, none active).
- Catalog: `/home/marius/.local/share/librarian/catalog.db` (machine-local, not in git).
- Profile `~/.claude-sdd`. Two sessions touched this repo in the window: the one that
  wrote `85bd01e1` (17:13:03) and one that wrote `047dd433` (17:39:45).

## Root cause

Unknown — see **Hypotheses tried**. Not measured; no session observed the deletion.

## Evidence

### Reindex re-minted the same ids, so the paths never moved

```
librarian(action="reindex", scope="repo")
→ {"added": 12, "updated": 42, "removed": 0, "unchanged": 1065, "embedded": 54}
```

After it, both rows resolved at their original ids — `89c2984ca7c074a0` (plan, still
`status: active`) and `556cc34167321863` (spec). Since `id = sha256(abs_path)`, identical
ids prove the files never moved; the rows were simply absent.

### The row loss was wider than this work stream, and cost event history

`docs/trackers/prompt-surface-measurement-session-log.md` came back with
`created_at == updated_at == 1787634828271` — the reindex timestamp — meaning its row was
**re-created**, not updated, so its events and links are gone. Its `extra` survived intact
(`entry_high_water_F: 9`, `entry_high_water_W: 7`, `entry_prefix: [F, W]`) because those
live in committed frontmatter. This is the design working as `get_guide("tracker-conventions")`
describes: *"the counter has to travel with the repo too."*

### It was not a blanket clean of ignored files

Six sibling directories under `.superpowers/sdd/` survived with their original mtimes
(2026-08-06 through 2026-08-20), and `.superpowers/sdd/.gitignore` is `*`, so a
`git clean -fdx` would have taken all of them. Only the 2026-08-23 directory went.

### Augmentations were NOT lost this time

`docs/trackers/tool-usage-patterns.md` (`f2ecdd76a6189efb`) still carries its
augmentation, `entry_collection: "observations"`, and all 26 `T-N` rows. This
distinguishes the incident from `docs/issues/2026-08-23-research-index-tracker-has-no-augmentation.md`
(F-4) and from the archived `2026-05-17-reindex-cascade-delete-data-loss.md`.

## Hypotheses tried

1. **Hypothesis:** `git clean` removed the ignored ledger.
   **Test:** compare mtimes of all seven `.superpowers/sdd/*` directories.
   **Verdict:** rejected — six siblings survived, all equally ignored.

2. **Hypothesis:** the artifacts were moved, re-keying them.
   **Test:** reindex and compare the new ids to the ids in the files' frontmatter.
   **Verdict:** rejected — ids identical, so `abs_path` never changed.

3. **Hypothesis:** the catalog DB was swapped or repointed (`catalog_db_path`).
   **Test:** read the reindex counts.
   **Verdict:** rejected — `unchanged: 1065` means the same populated DB was in use;
   only 12 files were absent from it.

4. **Hypothesis:** `librarian(doctor, fix=prune_missing)` pruned rows under a root it
   judged dead.
   **Test:** not run. The files exist on disk, and `prune_missing` refuses a root that
   still exists — but the batch mode's dead-root derivation is the one place this could
   still bite. **Verdict:** deferred; this is the strongest remaining lead.

5. **Hypothesis:** the rows were never created — the plan and spec were written with
   `create_file` rather than `artifact(action="create")` and only ever appeared indexed.
   **Test:** the previous session ran `artifact(action="update", id="89c2984ca7c074a0",
   patch={...})` successfully, which requires a row. **Verdict:** rejected.

## Fix

None yet — the incident is recorded, not diagnosed. Recovery is documented below.

## Tests added

None. A regression test needs a reproduction first, and hypothesis 4 is the only
untested lead.

## Workarounds

**Catalog rows:** `librarian(action="reindex", scope="repo")` restores them at their
original ids, because `id = sha256(abs_path)`. Frontmatter-borne state (`status`,
`extra`, `entry_high_water_*`) survives; catalog-only state (events, links, and — per
F-4 — augmentation) does not.

**A deleted git-ignored working file:** replay it out of the session transcript. Every
`create_file` / `edit_markdown` / `edit_file` call is recorded there with its full
payload, so the file's whole write history is recoverable:

1. Scan `~/.claude*/projects/<project-slug>/*.jsonl` for `tool_use` blocks whose input
   names the path.
2. Pair each with its `tool_result` and **drop the ones that failed** — this ledger had
   two `edit_markdown` calls refused with *"File writes are disabled for this project"*
   that wrote nothing, and replaying them would have inserted content the original never
   held.
3. Apply the survivors in timestamp order.

That recovered 1,264 lines and all 24 `R-N` rulings here. Two caveats found the hard way:
a simulated `insert_before` will not reproduce the tool's exact whitespace, so any later
`edit_file` keyed to that text misses and must be reconciled by hand; and the
reconstruction is content-faithful but not byte-faithful, so it should say so in its own
header. The scripts are in this session's scratchpad (`recover_ledger.py`,
`rebuild_ledger.py`).

## Resume

Test hypothesis 4. Run `librarian(action="doctor")` read-only and read the
`abs_path_outside_managed_roots` and missing-file counts; then check whether batch
`prune_missing`'s dead-root derivation can select a root whose subtree is present, by
reading `derive_dead_roots` and `count_dead_root` (added by the catalog-hygiene plan,
`docs/superpowers/plans/2026-07-18-catalog-hygiene-prevention-cleanup.md` Task 5). If it
cannot, close this as `zombie` with a re-open trigger rather than leaving it open.

## References

- `docs/superpowers/plans/2026-08-23-hidden-information-eval.md` — the plan whose
  `ledger:` key names the deleted file.
- `docs/trackers/prompt-surface-measurement-session-log.md` — the work stream's session
  log; its row was re-created by the repair reindex.
- `docs/issues/2026-08-23-research-index-tracker-has-no-augmentation.md` — F-4,
  augmentation loss; related but distinct (augmentations survived here).
- `docs/issues/archive/2026-05-17-reindex-cascade-delete-data-loss.md` — the earlier
  reindex-driven data loss, fixed.

