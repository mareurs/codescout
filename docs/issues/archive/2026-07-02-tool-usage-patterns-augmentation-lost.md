---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- librarian
- augmentation
- tracker
- claude-md-drift
topic: null
time_scope: null
closed: '2026-07-05'
opened: '2026-07-02'
owner: marius
related:
- f2ecdd76a6189efb
root_cause: v6-migration-drop-table-cascade
severity: high
---

# BUG: tool-usage-patterns tracker has lost its augmentation; CLAUDE.md's documented T-N append workflow is broken

## Summary
The Tool Usage Patterns tracker (`id: f2ecdd76a6189efb`,
`docs/trackers/tool-usage-patterns.md`) is documented in `CLAUDE.md` as an
augmented artifact whose `params` hold the structured T-N table and whose
`render_template` renders it at the `[LIVE]` header. The catalog now holds **no
augmentation** for it: `artifact(get)` returns `"augmentation": null`, and it is
absent from `artifact(find, augmented=true)`. The append workflow CLAUDE.md
prescribes — `artifact_augment(id=..., merge=true, params={observations:[...]})`
— will error, because `merge=true` requires an existing augmentation
(`augment::tests::merge_true_without_existing_augmentation_errors`).

## Symptom (Effect)
```
# artifact(action="get", id="f2ecdd76a6189efb", full=true)
"updated_at": 1780489457853,
"freshness": "unknown",
"latest_event": null,
"augmentation": null,        # <-- expected: {prompt, params, render_template, entry_collection, ...}

# artifact(action="find", kind="tracker", augmented=true)  → 3 items:
#   52451519052d207c  windows-platform-support
#   3366f6ae253097bd  headroom-llm-proxy-integration
#   cd886c414f6751b4  legibility-backlog
#   (f2ecdd76a6189efb tool-usage-patterns is NOT in the list)
```

## Reproduction
Minimal, at HEAD `78d9ef21` (branch `experiments`):
1. `artifact(action="get", id="f2ecdd76a6189efb", full=true)` → `augmentation` is `null`.
2. `artifact(action="find", kind="tracker", augmented=true)` → 3 trackers, this id absent.

Both are read-only; reproducible immediately.

## Environment
codescout MCP (live release), Linux, project `codescout`, branch `experiments`,
HEAD `78d9ef21`. Catalog DB is machine-local (not in repo).

## Root cause

**CONFIRMED (2026-07-05) — the v6 catalog schema migration cascade-deleted it.**
The earlier "delete+recreate" hypothesis is DISPROVEN: `created_at` is still the
original 2026-06-02 (a recreate would reset it), and the event log is *empty*
(a recreate wouldn't wipe pre-existing events — a cascade would).

Chain (all verified against code + an empirical SQLite repro):
1. `artifact_augmentation.artifact_id` and `events.artifact_id` are
   `REFERENCES artifact(id) ON DELETE CASCADE` (`src/librarian/catalog/schema.sql:117`, `:58`;
   same for `artifact_link`, `artifact_observation`, `event_edges`).
2. `Catalog::open_with_workspace` runs `PRAGMA foreign_keys = ON`
   (`src/librarian/catalog/mod.rs:199`) then calls
   `migrate_v6::drop_legacy_and_stamp` (`:203`).
3. `drop_legacy_and_stamp` rebuilds `artifact` via a table-copy that does
   `DROP TABLE artifact` + rename (to drop legacy `repo`/`rel_path`)
   (`src/librarian/catalog/migrate_v6.rs:141-216`). Under `foreign_keys=ON`,
   SQLite's `DROP TABLE` performs an implicit row-DELETE that **invokes FK
   actions** — firing `ON DELETE CASCADE` on augmentation/events/links/
   observations. The migration copies only `artifact` + `commits` forward, not
   the child tables, so those rows are gone; the artifact row is re-inserted
   with its original `created_at` (hence the row survives, augmentation/events
   do not).
4. Empirical proof (Python stdlib sqlite3, exact table-copy pattern):
   `before: aug=1 events=1` -> `after: artifact=1 aug=0 events=0`.

Why only this tracker (of the trackers): it was augmented ~2026-06-02, BEFORE
the v6 migration ran on this box. The 3 trackers that still have augmentation
(windows-platform-support, headroom-llm-proxy-integration, legibility-backlog)
were all augmented AFTER the migration; nobody re-augmented this one.

**This is a general data-loss defect, not tracker-specific** — the migration
wiped augmentation + events + links + observations for EVERY artifact present
when it ran, and it is still LATENT: any catalog that has not yet reached v6
will lose its augmentations/events the first time this codescout opens it. That
migration bug warrants its own issue + fix (wrap the table-copy in
`PRAGMA foreign_keys=OFF`/`ON` toggled OUTSIDE the transaction, since SQLite
ignores the pragma inside a transaction).
## Evidence
### 1. get returns null augmentation
Buffer `@tool_24879a21` from `artifact(get, id=f2ecdd76a6189efb, full=true)`:
top-level `"augmentation": null`; `preview.headings` still lists T-001…T-010
(body prose intact).

### 2. find augmented=true excludes it
`artifact(find, kind=tracker, augmented=true)` returns exactly 3 augmented
trackers (windows-platform-support, headroom-llm-proxy-integration,
legibility-backlog); tool-usage-patterns is not among them — corroborates that
the null in (1) is a real absence, not a get-projection omission.

## Hypotheses tried
1. **Hypothesis:** `augmentation: null` is the known get-projection bug
   (`artifact(get) does not echo entry_collection`).
   **Test:** cross-checked with `find(augmented=true)`.
   **Verdict:** rejected — that bug omits *fields* of a present augmentation; here
   the artifact is absent from the augmented set entirely, so the augmentation
   row itself is gone.
   **Evidence link:** Evidence #2.

## Fix

**Restored (2026-07-05).** Re-augmented `f2ecdd76a6189efb` via `artifact_augment`
(merge=false, fresh) reconstructing from the intact body prose:
- `prompt` — maintenance instructions + the reconstruction note.
- `params.observations` — all 10 rows (T-001..T-010) rebuilt from the body's
  `### T-NNN` sections: {id, tool, verdict, session, summary, prompt_gap}.
- `render_template` — MiniJinja table over `params.observations`.
- `entry_collection = "observations"`.

Verified: `artifact(get)` returns a non-null augmentation; `find(augmented=true)`
now lists it (4 augmented trackers); `entry_filter={"verdict":{"eq":"wrong-tool"}}`
returns the 6 matching rows (`entry_total: 10`) — proving `entry_collection` is
wired; `librarian(context)` renders the live table with no template error. The
CLAUDE.md `artifact_augment(merge=true)` T-N append workflow works again.

**Scope caveat:** augmentation lives only in the machine-local `catalog.db`, not
in the git-tracked `.md`, so this restore fixes THIS machine only. Other machines
(and any catalog restored from the .md alone) still show no augmentation and must
re-run the same `artifact_augment`. The event history remains unrecoverable (it
was cascade-deleted, never persisted to the file).

Root-cause prevention is the separate fix
(docs/issues/2026-07-05-v6-migration-cascade-deletes-child-rows.md, committed):
the v6 migration no longer cascade-deletes augmentations/events, so this class of
loss won't recur.
## Tests added
N/A — not yet fixed. A regression test would assert augmentation survives the
reindex/move path once the root cause is confirmed.

## Workarounds
To append a T-N entry today without the augmentation: edit the body prose
directly via `artifact(update, id=..., patch={body_edits:[...]})` (the analysis
section), and defer the structured params row until augmentation is restored. Do
NOT call `artifact_augment(merge=true)` — it will error.

## Resume
Confirm root cause: run `artifact_event(action="list",
artifact_id="f2ecdd76a6189efb")` and look for the last event that references the
augmentation (a `field_patch` on params, or a status/move event) to date the
loss and identify the triggering operation. If a params snapshot exists in event
history, use it to seed the restore; otherwise reconstruct from body prose. Then
decide restore-vs-document with Marius.

## References
- `CLAUDE.md` § "Tool Usage Patterns — docs/trackers/tool-usage-patterns.md"
  (documents the augmentation + append workflow now broken).
- `docs/issues/2026-07-02-artifact-get-omits-entry-collection.md` (distinct: field
  omission on a *present* augmentation).
- Session: prompt-hamsa / tracker-as-skill work stream, 2026-07-02.
