---
id: '19f44bead56b56cc'
kind: bug
status: open
title: 'BUG: expects_augmentation is a boolean, so a fresh clone knows an augmentation is missing but nothing records what it was — CLAUDE.md''s documented entry_filter workflow breaks on every new machine with no recovery path'
owners:
- marius
tags:
- librarian
- augmentation
- cross-machine
- claude-md-drift
---

# BUG: `expects_augmentation` records existence, not shape

## Summary

Augmentation is the one artifact state with no on-disk form. Frontmatter can say
`expects_augmentation: true` — that an augmentation *should* exist — but nothing
in the repo records **what it should be**. So on every fresh clone, 22 trackers
report `augmentation_declared_but_absent`, the workflows CLAUDE.md documents for
them fail, and the only repair is hand-reconstruction that no check can verify.

The declaration correctly turns a silent absence into a loud one. It stops one
step short of making the absence *fixable*.

## Symptom (Effect)

On a machine that pulled the repo but never built the catalog:

```
artifact(action="get", id="f2ecdd76a6189efb",
         entry_filter={"status": {"eq": "open"}})

→ {"ok": false,
   "error": "entry_filter set but this artifact is not augmented — declare
             entry_collection on its augmentation, or retrofit it
             (docs/conventions/retrofitting-trackers-for-filtering.md)"}
```

That is the exact call `CLAUDE.md` § *Tool Usage Patterns* documents for browsing
T-N rows. `librarian(action="doctor")` reports the same state for 21 sibling
trackers.

## Reproduction

Deterministic on any machine whose `catalog.db` predates or never saw these
artifacts — i.e. every fresh clone:

```
git clone <repo> && cd repo
librarian(action="reindex")
librarian(action="doctor")     # → augmentation_declared_but_absent × 22
artifact(action="get", id="f2ecdd76a6189efb", entry_filter={"status":{"eq":"open"}})
```

Measured 2026-08-28 on a laptop after pulling 437 commits from a desktop stream:
**22 hits**, including the three trackers CLAUDE.md gives explicit append/query
recipes for — `tool-usage-patterns` (`f2ecdd76a6189efb`), `open-issue-work-queue`
(`9a892c2a5976e296`), `prompt-hamsa-audit-log` (`59ebeebb6ed05c89`).

## Environment

Branch `experiments` @ `14aab5ff`, linux, codescout 0.15.0, catalog at
`~/.local/share/librarian/catalog.db` (machine-local, gitignored, machine-global).

## Root cause

Measured 2026-08-28 — this is the benign cause, not a data-destroyer:

1. Augmentation rows live only in `catalog.db`. That file is gitignored and
   machine-local, so they do not travel. **Confirmed** by `doctor` firing on a
   catalog that never held them, with bodies intact in git.
2. `librarian(action="reindex")` preserves augmentation **keyed by id** rather
   than regenerating it, so it reports healthy after a loss and repairs nothing.
   (Stated in `get_guide("tracker-conventions")` § *Declaring an augmentation*;
   consistent with the 99-added / 67-updated / 0-augmentation-restored result of
   the reindex run today.)
3. `expects_augmentation` is a **boolean**. It is sufficient to raise the alarm
   and insufficient to answer it.

Note this is a *different* cause from the 2026-07 instance of the same symptom,
which was the v6 migration cascade-deleting child rows under
`PRAGMA foreign_keys = ON`
(`docs/issues/archive/2026-07-05-v6-migration-cascade-deletes-child-rows.md`, fixed).
That one was destruction; this one is absence. Do not conflate them — the fixed
migration bug does not cover this.

## Evidence

### 1. The declaration fires correctly, and offers nothing

`librarian(action="doctor")` `by_check` on this machine:

```
"augmentation_declared_but_absent": 22
```

Every other `doctor` check with a repair has a `fix=` option
(`prune_missing`, `reseat_worktree`, `rehome`, `repair_frontmatter_id`,
`mint_slugs`). This one has none — correctly, because the data to repair with
does not exist anywhere.

### 2. Reconstruction is possible but unverifiable

`docs/issues/archive/2026-07-02-tool-usage-patterns-augmentation-lost.md` restored
this same artifact on 2026-07-05 by rebuilding params from body prose, and records
the schema it used (`{id, tool, verdict, session, summary, prompt_gap}`,
`entry_collection: "observations"`). **That bug file is currently the only on-disk
record of any tracker's augmentation shape, and it exists by accident** — it was
written to document an incident, not to serve as a schema store. The other 21
trackers have no equivalent.

### 3. The bodies survive, so this is shape-loss, not content-loss

`### T-N` × 30, `BL-N` × 44, `A-N` × 44 headings are all present in git. What is
gone is the *structure over* them.

## Hypotheses tried

1. **Hypothesis:** `reindex` would rebuild augmentation from frontmatter.
   **Test:** ran `librarian(action="reindex")` (99 added / 67 updated / 165
   embedded), then re-ran `doctor`.
   **Verdict:** rejected — all 22 still reported. Matches the documented
   preserve-by-id behaviour.

2. **Hypothesis:** the `entry_filter` failure is the known `artifact(get)`
   projection bug that omits `entry_collection` from a *present* augmentation.
   **Test:** `artifact(get)` shows `"augmentation": null` outright.
   **Verdict:** rejected — the row is absent, not mis-projected. Same distinction
   the 2026-07-02 bug drew.

## Fix

Not started. **Proposal, not a decision** — the split below is the load-bearing
part and deserves review before anything is built:

- **Shape is schema and SHOULD travel.** `entry_collection`, `params_schema`, and
  `render_template` are small, static, and authored once. Carrying them in
  frontmatter (or a sidecar the artifact names) would let `reindex` re-attach them
  automatically, turn `augmentation_declared_but_absent` into a repairable check
  with a real `fix=`, and make a reconstruction *checkable* — rebuilt params could
  be validated against the travelling `params_schema` instead of trusted.
- **`params` are data and CANNOT travel.** They are live state, they churn, and
  committing them would recreate the params-vs-body drift class the project spent
  BL-29/BL-40/BL-42 closing. Rows stay catalog-only; that part of a resume stays
  manual by design.
- **`prompt` is the open question.** It is authored prose, so it *could* travel —
  but it is also the `[LIVE]` standing instruction, and putting it in the body
  risks it being read as content rather than instruction. Needs a call.

Smallest useful version: let `expects_augmentation` take a **map** instead of a
bool, holding `entry_collection` + `params_schema`. Backward compatible — `true`
keeps today's meaning.

## Tests added

None yet. The regression test that matters is the one the 2026-07 fix did not
have: **build a catalog from scratch against a repo whose artifacts declare an
augmentation shape, and assert the shape is re-attached.** Note the trap named in
`docs/trackers/bug-ledger-resume-2026-08-28.md` — *a test that constructs the state
production derives cannot tell you the derivation runs* — so the test must drive
`reindex` against a genuinely empty catalog, not seed an augmentation row and
assert it is still there.

## Workarounds

Follow `docs/conventions/cross-machine-catalog-resume.md` § 7 — tiered manual
restore. Tier A (`prompt` + `params` + `entry_collection`) only for trackers with
a documented `entry_filter` workflow; Tier B (`prompt` only) for the rest.

Do **not** call `artifact_augment(merge=true)` on an unaugmented artifact — it
errors (`augment::tests::merge_true_without_existing_augmentation_errors`).

## Resume

Decide the three-way split under **Fix** with Marius — specifically whether
`prompt` travels. That decision gates everything else; the schema half is
mechanical once it is made.

## References

- `docs/conventions/cross-machine-catalog-resume.md` — the process this defect makes necessary
- `docs/conventions/retrofitting-trackers-for-filtering.md` — Tier-A mechanics
- `docs/issues/archive/2026-07-02-tool-usage-patterns-augmentation-lost.md` — same symptom, different cause; currently the only on-disk augmentation-shape record
- `docs/issues/archive/2026-07-05-v6-migration-cascade-deletes-child-rows.md` — the destruction variant, fixed
- `get_guide("tracker-conventions")` § *Declaring an augmentation*
- `docs/trackers/bug-ledger-resume-2026-08-28.md` — the cross-machine handoff

