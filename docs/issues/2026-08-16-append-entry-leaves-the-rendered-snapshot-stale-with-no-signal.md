---
kind: bug
status: mitigated
tags:
- librarian
- durability
- silent-drift
- trackers
- git
closed: null
opened: 2026-08-16
owner: marius
related: []
severity: high
---

# BUG: `append_entry` writes catalog-only state, so a tracker's committed snapshot silently drifts from its live rows

## Summary

Augmented-tracker rows live in `params`, and `params` live in the librarian
catalog under `~/.local/share/librarian/catalog.db` — machine-local and
git-ignored. `artifact(action="append_entry")` writes there and reports success.
It does **not** touch the markdown file, so the rendered snapshot committed to
git goes stale the moment anyone appends, and nothing anywhere says so:
`git status` stays clean, the tool returns a row id, and the file on disk still
looks like a complete queue.

`e86e153d` fixed a *symptom* of this two hours earlier — it discovered 5,039
bytes of BL queue that "existed on this machine and nowhere else" and wrote a
rendered snapshot into the body. The snapshot went stale on the next
`append_entry`, which is this bug.

## Symptom (Effect)

Measured 2026-08-16, roughly two hours after the snapshot was created:

```
artifact(action="append_entry", id="9a892c2a5976e296",
         entry_collection="tasks", id_prefix="BL", entry={...})
-> {"id": "BL-25", "artifact_id": "9a892c2a5976e296"}     # x4, all success
```

Then:

```
$ grep -c "BL-2[5-8]" docs/trackers/open-issue-work-queue.md
0
$ grep -o 'BL-[0-9]*' docs/trackers/open-issue-work-queue.md | sort -t- -k2 -n | tail -1
BL-24
$ git status --short docs/trackers/open-issue-work-queue.md
                                                    # (empty — file unmodified)
```

Four rows accepted, four rows absent from git, clean working tree.


### Corpus-wide audit, 2026-08-16 — how much already drifted

> **Scope correction (same day).** The table below is **machine-wide, not
> codescout**. The audit read `~/.local/share/librarian/catalog.db` directly, and
> the catalog holds every artifact from every repo on this host with no project
> scoping — the same property `get_guide("tracker-conventions")` and the
> tracker-hygiene skill warn about for `librarian(action="doctor")`. Of the 28
> augmented trackers, only **11 are codescout's**; the corpus spans 7 repos.
> Per repo: codescout 11 (7 in-sync, 1 drifted, 3 prose-only), backend-kotlin 6
> (1 drifted), MRV-poc 5, ie-pal-engine 2, researcher 2, claude-plugins 1,
> lang-pal-engine 1. `innovaplan-export-tracker.md` is
> `mirela/backend-kotlin`'s and is not codescout's to reconcile.

Every augmented tracker with an `entry_collection`, comparing params ids against
the ids its body line-anchors. Buckets partition the corpus, so they reconcile:

| bucket | n |
|---|---|
| no entries / unparseable params | 8 |
| mixed id prefixes (not reasoned about) | 2 |
| prose-only — body anchors nothing, by design | 5 |
| keeps a snapshot, in sync | 10 |
| **keeps a snapshot, DRIFTED** | **3** |
| total | 28 |

| missing | rows | tracker |
|---|---|---|
| **54** | 68 | `provenance-subsystem.md` — 79% of its rows exist only in the catalog |
| 9 | 23 | `prompt-hamsa-audit-log.md` — A-15 … A-23 |
| 3 | 6 | `innovaplan-export-tracker.md` |

**The consequence worth naming.** `grep 'A-2[0-3]'` on `prompt-hamsa-audit-log.md`
returns **zero matches** — not a heading, not a table row, not even prose. Yet:

- the machine's `CLAUDE.md` cites **A-21** as the measurement behind the Conclude
  Last iron rule (*"13.3% → 73.3% verify-before-assert under planted-belief
  traps; ledger A-21"*), and
- **A-22** is cited by R-90 in `docs/trackers/reconnaissance-patterns.md`.

Both citations currently resolve only against
`~/.local/share/librarian/catalog.db` — machine-local, git-ignored, and one of
three profiles on this host. The evidence for a standing iron rule is in no repo.

That is what inverted the sequencing: options 1 and 2 prevent only NEW drift, so
the `doctor` check shipped first because it is the only one that surfaces what
already happened.
## Reproduction

1. `artifact(action="append_entry", …)` against any augmented tracker whose body
   carries a rendered table.
2. `git status` — clean.
3. `grep` the new row id in the tracker file — absent.

## Environment

codescout `experiments` at `bb11bba3`. Catalog at
`~/.local/share/librarian/catalog.db` (git-ignored, per
`src/prompts/guides/librarian-runtime.md` § Where catalog state lives).

## Root cause

The two stores have no reconciliation step and no drift signal.

1. **Params are catalog-only by design.** `librarian-runtime.md` states it
   plainly: augmentation "has no on-disk representation" and the DB is "machine-
   local and git-ignored". That design is deliberate and not itself the bug.
2. **The remedy for durability is a hand-written snapshot.** `e86e153d` added a
   rendered table to the body so the rows exist in git. Hand-written means
   hand-maintained.
3. **`append_entry` does not know the snapshot exists.** It writes params and
   returns. No re-render, no `field_patch` on the body, no warning that the
   artifact declares a `render_template` whose output is now behind.

So durability depends on every future caller remembering to re-render, with
nothing to remind them and nothing to detect the omission. The failure is silent
in both directions a check would normally catch: the tool says success, and git
says clean.

measured 2026-08-16: four `append_entry` calls returned ids BL-25..BL-28; `grep`
for those ids in the tracker file returned 0; `git status` on the file returned
empty.

## Evidence

### The same defect, two hours apart, found twice

`e86e153d`'s own message closes with: *"Worth knowing when creating any augmented
tracker: writing a good body does not make its live state durable, and the file
does not look wrong."* That is this bug, observed from the other end — and the
snapshot it created was already stale by the time these four rows were added.

### Why the blast radius is larger than one tracker

`e86e153d` also records that recovering the rows after a merge-patch accident
was only possible *because* the rendered snapshot happened to have been written
minutes earlier — "without it the rows would have been gone with no copy
anywhere." So the snapshot is not cosmetic; it is the only backup of catalog
state. A stale snapshot is a stale backup.


### Third instance, same day — `update_entry`, and a worse sub-shape

2026-08-16, ~5h after this file was opened, on `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`
(`ac8fbe339e66ade3`). Two `update_entry` calls flipped GF-5 and GF-8:

```
artifact(update_entry, entry_id="GF-8", fields={...})
-> {"changed_fields":["title","status","target"], "entries_total":8}   # success
git commit --only <tracker>
-> "no changes added to commit"
```

Three things this adds to the record:

**1. It is not only `append_entry`.** `update_entry` has the identical shape — a success
envelope naming the fields it changed, and no write to the file. The Fix and Resume sections
below already name `UpdateEntryOutcome`; this is the field instance confirming it.

**2. A worse sub-shape: the snapshot that never existed.** This bug is written about a
snapshot going *stale*. Here the tracker was created with `augment={render_template,
entry_collection, params:{findings:[8 rows]}}` and a hand-written body that contained **no
rendered table at all**. `artifact(action="create")` accepted it and reported success. So the
failure was not drift — it was **absence**: eight findings existed in the catalog and zero in
git, and every reader on another machine would have seen a tracker with no findings. The
tracker was born broken, and nothing at create time said so.

**3. The documented workaround has a hole, and it is exactly this case.** § Workarounds says
*"after every `append_entry` / `update_entry` on a tracker **with a rendered table**, edit the
table in the same turn."* When there is no rendered table, the precondition is false, the
workaround silently does not apply — and the outcome is worse than the case it guards. A
workaround that is conditional on the artifact already being half-correct cannot catch the
artifact that was never correct.

**Detection differed too, and was slower.** The first two instances were caught by
`grep <new-id> <file>` returning 0. This one surfaced as `git commit` reporting *"no changes
added to commit"* — which reads as *"my edit did not apply"*, not as *"my edit went to a
database outside version control"*. The later and more misleading signal is the one an author
hits when they were not already suspicious.

**Repaired by** giving the body a real `## Findings index` table plus a note stating it is
hand-synced and why (`e3dd375f`).
## Hypotheses tried

1. **Hypothesis** — `reindex` reconciles the body from params. **Test** — ran
   `librarian(action="reindex")` twice during this session. **Verdict** —
   rejected; reindex reads files into the catalog, not the reverse, and the
   tracker file remained unmodified.

## Fix

**Options 1 and 3 shipped 2026-08-16 in `99aaf83f`, in the reverse of the
sequence this file proposed, and option 1 was implemented differently than it
was specified.** Both changes came out of measuring the filed plan.

### Option 1's gate was wrong — `render_template` is the opposite signal

As filed: *"when the artifact declares a `render_template`, include a field in
the response naming the body as stale."* Two measurements killed that:

- `src/librarian/tools/render.rs:1-3` — `render_template` feeds
  **`librarian_context`**, and its stated purpose is *"to project `params` into
  a markdown table/snippet **so the artifact body can stay prose-only**"*. It is
  a declaration that the body does NOT carry the rows.
- **26 of 28** augmented trackers on this machine declare one. The flag would
  have fired on nearly every append forever — the `removed_attributes` noise
  failure.

**The correct gate was already written**, inside `body_max_index`'s regex: *if a
body line-anchors at least one `PREFIX-N`, that tracker demonstrably keeps a
snapshot.* Self-configuring, no new field, and silent for the 5 of 28 trackers
that are prose-only by design. Generalized to `body_claimed_indices` (set rather
than max); `body_max_index` had no remaining callers and was removed, its four
tests now asserting on the whole set.

### `append_entry` already had half this check

It was **already** reading the file from disk and computing `body_max_index`, to
warn when the BODY runs ahead of params
(`docs/issues/archive/2026-07-20-append-entry-id-drift-params-vs-body.md`). The
mirror direction — params ahead of body, i.e. THIS bug — simply had no branch.
Same read, same parse, **zero new I/O**. Response now carries
`snapshot_missing` + `snapshot_hint`.

The newly assigned id is included deliberately: at that moment the body does not
carry it, and naming it is the reminder to write the row while the caller still
has the context.

### `update_entry` — why the third instance had nothing to notice it

`abs_path` / `read_to_string` appeared **only** in `append_entry`; `update_entry`
never read the body at all. And its sub-shape is the one no id comparison can
catch — a patched row is usually *present* in the body, showing its previous
values. It now does one read after commit (advisory; it must never fail the
mutation) and distinguishes **stale** (row rendered, values behind) from
**absent** (row in no repo), which need different remedies.

### Option 4 (create-time) — still open

Unchanged. An artifact born with `render_template` + `entry_collection` and a
body with no rendered rows still starts with its entries invisible to git. The
new gate deliberately stays silent there, because at that moment it cannot
distinguish that case from a prose-only tracker.

### Option 2 (re-render on write) — still open, and still the real fix

Overlaps BL-30; scope them together.
## Tests added

Eleven, in `99aaf83f`.

`doctor` (4): reports rows that reached params but never the body; silent for a
prose-only tracker; silent when the body carries every row; a **prose mention**
is not accepted as a rendered row (or the check under-reports exactly the drift
it exists to find).

`append_entry` (2): names the missing rows — including one that was already
adrift plus the id just assigned, and NOT the one the body renders; silent for a
prose-only tracker.

`update_entry` (3): a rendered row whose values changed says the committed table
now disagrees; an unrendered row says it is absent entirely; a prose-only
tracker says nothing.

`body_claimed_indices` (4, converted from `body_max_index`'s): the full set from
headings and index rows; prose mentions ignored; prefix boundaries respected;
empty when the body claims nothing — the last is load-bearing, since empty is
how a prose-only tracker is recognised.

**Every `seed` helper sets `render_template: Some(..)` (doctor) or `None`
(append/update) deliberately**, so no test can pass by accident if the gate were
silently re-keyed to that field.

**Mutation-verified**, both prose-only gates independently: disabling either
makes every prose-only tracker report a false missing/absent row on every write
— which is precisely the noise the `render_template` gate would have shipped.
## Workarounds

After every `append_entry` / `update_entry` on a tracker with a rendered table,
edit the table in the same turn and commit both. Verify with
`grep <new-id> <tracker-file>` — the tool's success envelope does not imply the
row is in git, and `git status` staying clean is the expected appearance of the
bug, not evidence against it.

## Resume

**Mitigated, not fixed.** New drift is now visible at the moment it is created,
and pre-existing drift is discoverable via `librarian(action="doctor")` — but
params and body still do not reconcile automatically, and the 3 drifted trackers
above are still drifted.

Remaining, in order:

1. **Reconcile the drifted trackers.** `prompt-hamsa-audit-log.md` was the
   urgent one (A-21 backs an iron rule in `CLAUDE.md`) and is **done** —
   `6ff00eee`, 11 rows → 23, regenerated mechanically from params. That pass
   also caught A-2 rendering `pending measurement` while params said MEASURED /
   HELD / CLOSED: the `update_entry` sub-shape, live, weeks stale.
   **codescout's remaining drift is one tracker**: `provenance-subsystem.md`
   (54 of 68 rows). A judgement call, not mechanical — a 54-row body rewrite,
   and whether a provenance log belongs in git at all is the maintainer's
   decision. (`innovaplan-export-tracker.md`, 3 of 6, belongs to
   `mirela/backend-kotlin` — a different repo, listed only because the audit
   query was machine-wide.)
2. **Option 4** — create-time, for artifacts born with entries invisible to git.
3. **Option 2** — re-render on write. The real fix; overlaps BL-30, scope together.

Do not re-derive the gate. `render_template` is **not** the signal (it means the
opposite), and `append_entry` already reads the body — the machinery is present
and `body_claimed_indices` is the shared entry point.
## References

- `src/prompts/guides/librarian-runtime.md` § Where catalog state lives — the
  catalog-only durability class, stated as design
- `src/prompts/guides/librarian.md` § Augmentation Lifecycle
- commit `e86e153d` — the first discovery of this, from the other end
- `docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md`
  — the adjacent write-safety defect; the snapshot is what made recovery possible there
- `docs/trackers/open-issue-work-queue.md` — the tracker this was measured on
