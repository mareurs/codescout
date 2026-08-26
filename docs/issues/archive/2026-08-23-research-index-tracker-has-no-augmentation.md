---
kind: bug
status: fixed
title: Research Index tracker promises a [LIVE] index it has no augmentation to render
tags:
- librarian
- trackers
- docs-drift
closed: 2026-08-26
opened: 2026-08-23
owner: marius
related: []
severity: medium
unverified: 'The index is catalog-only state — augmentation has no on-disk form, so a fresh clone or a rebuilt catalog starts without it, and no automated gate asserts its existence. Mitigated by construction rather than by test: every entry is a projection of its file''s own frontmatter, so the whole array is regenerable from disk by the procedure in the augmentation prompt. Separately NOT established, and now moot: which catalog the 2026-08-23 session read.'
---

# BUG: Research Index tracker documents a [LIVE] table and a params refresh, but has no augmentation

## Summary

**Refuted 2026-08-26 — nothing was lost.** This file escalated a single-tracker
defect to "every augmentation in the codescout catalog is absent" on the strength
of one query returning zero rows. The catalog's own columns disprove that. The
original, narrow bug stands; the escalation does not. See § Root cause.

**Still true:** `docs/research/README.md` (`5086e3c7c0b9d83c`) carries no
augmentation, while its body tells the reader that "the [LIVE] table above is
rendered automatically from each file's frontmatter by the librarian augmentation
refresh — do not edit it by hand". That projection exists only in
`librarian(action="context")` output — nothing ever writes it to disk — and
without an augmentation it is not produced there either. The instruction refers
to something no surface shows.

**Not true:** the repo-wide claim. 21 codescout trackers are augmented, and
`docs/trackers/tool-usage-patterns.md` (`f2ecdd76a6189efb`) — the id CLAUDE.md
hard-codes for the T-N ledger — carries its `observations` collection intact,
created 2026-07-05 and never interrupted.
## Symptom (Effect)

`artifact(action="get", id="5086e3c7c0b9d83c", full=true)` returns:

```
"augmentation": null,
```

while the body of the same artifact states:

```
This folder catalogs every research artifact in the codescout repo. The
[LIVE] table above is rendered automatically from each file's frontmatter
by the librarian augmentation refresh — do not edit it by hand.
```

No index table exists on disk either (expected — `render_template` projects into
`librarian(action="context")` only, never to the file), so the catalog is the only
place the table could ever have appeared, and it cannot.

## Reproduction

Measured 2026-08-23 at HEAD `e97d89bc1725cae0adb0838bc9bde025383d7469`, branch
`experiments`:

```
workspace(action="activate", path="/home/marius/work/claude/codescout")
artifact(action="get", id="5086e3c7c0b9d83c", full=true)
```

Read the `augmentation` field → `null`. Read the body → claims a [LIVE] table.

## Environment

codescout MCP, project `codescout`, branch `experiments`, Linux. Catalog is the
local SQLite catalog (not in git — see CLAUDE.md § *Tool Usage Patterns*).

## Root cause

Two causes were conflated. Separated:

**1. The repo-wide loss did not happen.** Established 2026-08-26 by querying the
catalog directly:

| Evidence | Reading |
|---|---|
| `f2ecdd76a6189efb` augmentation `created_at` | `2026-07-05T06:51:44Z` — seven weeks before the measurement |
| `augmentation::upsert` | stamps `updated_at` on conflict and never `created_at`, so the row cannot have been re-inserted later wearing an old date |
| all 21 codescout rows | `created_at` spans 2026-06-13 … 2026-08-17; **none** on 08-22/23/24, which a restore would have stamped |
| `~/.sync-backups/…/20260712/catalog.db` | 53 augmentations against 70 today — monotonic growth, no wipe |
| `worktree_registration` | zero codescout rows, ever — the overlay-shadow explanation is out |
| this file's own § Fix | "Not yet planned" — no restore was ever performed, so nothing re-created these rows |

The 2026-08-23 readings were false negatives. **Which catalog that session opened
is not established**, and likely never will be: the path is `env.db`, falling
back to `dirs::data_local_dir()/librarian/catalog.db`, so it moves with
`$XDG_DATA_HOME`/`$HOME`. A catalog holding *some* augmentations but no codescout
rows reproduces E-4 exactly — count 0 with a populated scope block.

**2. The tool made that mistake easy to make.** This part is fixed.
`find(augmented=true)` returned zero without saying which world the zero
described: the empty-catalog path discarded the scope block outright
(`scope: null`), and the populated path never reported how many augmentations the
catalog actually held. Both now do — see § Fix.

The durability fact the previous root cause cited remains correct and worth
knowing: augmentation lives only in the catalog DB, has no on-disk form, and
`reindex` cannot rebuild it. It simply is not what happened here.
## Evidence

### E-1 — catalog state

`artifact(action="get", id="5086e3c7c0b9d83c", full=true)`, 2026-08-23:
`"augmentation": null`, `"time_scope": "dated_snapshot"`, `"status": "active"`,
`created_at` 2026-05-08, `updated_at` 2026-08-17.

**Valid:** dated 2026-08-23

### E-2 — the body's own claim

Body § *How to save a research*, step 4 prescribes `artifact_refresh(action="gather")`
followed by `patch={params: {...}}` with `commit_refresh=true` — both of which
presuppose an augmentation that E-1 shows is absent.

### E-3 — existing entries do not carry the frontmatter the table would read

`head -12` of `docs/research/2026-07-03-mcp-guidance-findings.md` and
`docs/research/2026-07-02-lite-vs-hybrid-benchmark.md` (2026-08-23): neither file
opens with the five-key YAML block that § C-7 declares mandatory. So even with an
augmentation attached, a gather pass would find nothing to render for those entries.
This suggests the augmentation may never have been attached, rather than lost.


### E-4 — REFUTED: a false negative, not a repo-wide loss

Kept as recorded, because the reasoning is the lesson. Measured 2026-08-23,
branch `experiments`, HEAD `6307a06a`:

```
artifact(action="find", kind="tracker", augmented=true, limit=20)
  → {"count": 0, "items": [], "scope": {"applied": "repo"}}

artifact(action="get", id="f2ecdd76a6189efb")
  → "augmentation": null      (docs/trackers/tool-usage-patterns.md)
```

Both readings were wrong about the world. `f2ecdd76a6189efb`'s augmentation row
was created 2026-07-05 and is present today with its `observations` collection
intact. A `get` by primary key is not scope-filtered, so a session that saw
`null` for it was not reading this catalog.

The inference — one query returning zero, generalised to "every augmentation in
the repo is gone" — is the failure this entry now documents. A zero is evidence
about the query first and the world second, and the check that would have settled
it (`SELECT created_at FROM artifact_augmentation`) cost one command.
## Hypotheses tried

1. **Hypothesis:** the augmentation was attached at index creation (2026-05-08) and
   lost in a later catalog rebuild.
   **Test:** not yet run. The catalog is not in git, so there is no history to diff.
   **Verdict:** deferred.
2. **Hypothesis:** the augmentation was never attached — the index was authored from
   its design spec describing the intended end state, and the
   `artifact_augment` call was never made.
   **Test:** not yet run. E-3 is weak supporting evidence (no entry file carries the
   frontmatter the template would consume), as is the spec reference in the body's
   § History.
   **Verdict:** deferred, currently the likelier of the two.

## Fix

**Shipped** — `a77a39a0` on `experiments`, patch-id `087f3d1dd43afad04cf0df97a85c511c8129b4cd`.
`artifact(action="find", augmented=true)` no longer returns an uninterpretable
zero, in either of its two shapes:

- *Zero because the catalog holds no augmentations at all.* The short-circuit no
  longer skips the pipeline. It stays — `compile()` and `eval()` both reject an
  empty `in` list, so "match nothing" is not expressible as a filter — but forces
  the rows empty instead, so the scope block, `build_hints` and `catalog.total`
  all survive, alongside a hint saying the zero is catalog-wide.
- *Zero because they are elsewhere.* The response now carries
  `augmented_in_catalog`, the count that separates "excluded by this query" from
  "destroyed". This is the shape the 2026-08-23 session actually hit.

Two regression tests in `src/librarian/tools/find.rs`, each verified red against
the old shapes before the fix.

**The original, narrow bug — also fixed, 2026-08-26.** `docs/research/README.md`
(`5086e3c7c0b9d83c`) now carries an augmentation, so the `[LIVE]` index its body
promised actually renders.

The backfill was re-costed first, and the earlier estimate in this file was
wrong in a way worth recording: it said "10 of 16 files already carry the
frontmatter", taken from `grep -l '^title:'`, which matches anywhere in a file
— one non-compliant file quotes a YAML block in its body. The true split was
**9 compliant, 6 not**. Measuring the predicate, not just running it, is the
same lesson as § Root cause.

What shipped:

- Five-key C-7 frontmatter backfilled onto the 6 files that lacked it. All 15
  entry files now verify as `title, date, topic, summary, status`, in order.
  The two `*-brief.md` files are indexed as `topic: research-brief`, because
  C-1 counts a *finding* as research and a brief is a request for one — the
  index says so rather than relocating files, which § How to save a research
  step 5 reserves for an explicit user request.
- An augmentation with `entry_collection: "entries"`, a `params_schema`
  (six required keys, `status` enum, `summary` maxLength 200), and a
  `render_template` that renders `superseded` entries last per C-6.
- The body now states where the index lives, because the absence of a table in
  this file is what was misread as loss in the first place.

Verified on the live server: `librarian(action="context", anchor_id=…)` renders
the 15-row `[LIVE]` table, and `artifact(get, entry_filter={"topic": {"eq":
"retrieval-quality"}})` returns `entry_total: 3`.
## Tests added

None — bug is filed, not fixed.

## Workarounds

Save research files by writing them directly with the § C-7 frontmatter (the
procedure's steps 1–3), and skip step 4. The body already states that a file saved
without the refresh is still saved correctly. Discovery still works via
`artifact(action="find", …)` after a `librarian(action="reindex")`.

## Resume

Run `artifact_event(action="list", artifact_id="5086e3c7c0b9d83c")` and look for an
`artifact_augment` event dated near 2026-05-08. Its presence confirms hypothesis 1
(lost in rebuild); its absence, with other events from that date present, confirms
hypothesis 2 (never attached). That single command separates them.

## References

- `docs/research/README.md` — the affected tracker (artifact `5086e3c7c0b9d83c`)
- `docs/superpowers/specs/2026-05-08-researcher-tracker-design.md` — the spec the
  index body cites as its origin
- CLAUDE.md § *Tool Usage Patterns* — the augmented-artifact pattern and the
  "catalog is not in git" caveat
- `docs/architecture/augmented-artifacts.md` — body / params / render_template deep-dive
- Noticed while saving `docs/research/2026-08-23-opus5-harness-minimalism.md`
