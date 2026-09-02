---
kind: spec
status: draft
title: Cross-Machine Catalog Integration — recovery and a durable mechanism
owners:
  - marius
tags:
  - librarian
  - catalog
  - augmentation
  - cross-machine
  - durability
topic: cross-machine catalog integration
---

# Cross-Machine Catalog Integration

**Date:** 2026-08-31
**Status:** draft — design approved in conversation, not yet planned
**Supersedes nothing.** Extends the sidecar mechanism shipped 2026-08-30 (BL-50,
`c2039a16`, `e799f29d`) and the resume procedure in
`docs/conventions/cross-machine-catalog-resume.md`.

## Problem

codescout splits state across two stores with opposite durability. Markdown bodies and
frontmatter travel with `git pull`. The semantic index, `cites` edges, augmentation rows,
events and observations live in `~/.local/share/librarian/catalog.db`, which is
machine-local and gitignored.

As of 2026-08-30 augmentation **shape** travels, via committed sidecars under
`docs/augmentations/`. Augmentation **params** — the entry rows themselves — do not, and
neither do events or observations.

Two consequences, both measured:

1. **Loss is silent and gets recorded as permanent.** `reindex` preserves augmentation
   keyed by id rather than regenerating it, so it reports success and repairs nothing;
   `artifact(get)` returns `augmentation: null` without comment.
2. **Divergence has no recorded common ancestor**, so establishing which side is stale
   costs forensics. On 2026-08-31 that took most of a session, and the first heuristic
   tried — "longer field is newer" — was **wrong on five of nine** artifacts.

## Evidence base

All figures measured 2026-08-31 on this repo at `2f434fba`, desktop and laptop compared
directly over SSH.

**The harm on record is loss, and false conclusions about loss — not merge conflicts.**

| claim on record | reality |
|---|---|
| `CM-1`: five trackers have "no provenance anywhere… restoration is not possible, only invention" | All five hold live augmentations on the desktop. `structural-debt-refactor` alone: 25,455 bytes of params across 11 `items`. |
| `CM-2`: "provenance-subsystem is missing 38 rows, permanently" | All 38 present on the desktop. The enumerated ids match exactly, 38 of 38. |
| `CM-3`: `PV-9` / `PV-11` titles "AUTHORED… no canonical title survived the catalog loss" | Both canonical titles present on the desktop. `PV-9`'s authored title asserts a different claim than the canonical one. |

There is **no** instance on record of a genuine concurrent-edit conflict requiring
adjudication. Every incident is one machine concluding data was lost while another machine
held it.

One case comes close and is named rather than hidden: `tool-usage-patterns` `T-30` holds
`legitimate, shape-mismatched` on the desktop and `legitimate` on the laptop — the only
same-id / different-non-null-value pair found anywhere. It is explicable as a schema
normalisation (the compound string is what the enum rejects) rather than two independent
edits, so it does not overturn the claim above. It is the closest thing to a counterexample
that exists, and a later reader should weigh it as such.

**Direction cannot currently be inferred.** `sidecar_shape_drift` says so in its own
message — *"THIS CHECK CANNOT TELL WHICH — mtime does not discriminate, because a git
checkout stamps the file with checkout time whatever its shape's age."* That is correct
and deliberate (`03b86cd7`, "report sidecar shape drift, without guessing its direction").
The direction was established empirically instead: the laptop's catalog matches the
committed sidecars byte-exactly on every field, so the sidecars are a faithful projection
of the authoritative host.

**Churn is not uniform across rows.** `provenance-subsystem`'s 68 rows are
`settled: 43, open: 17, carried: 4, descoped: 2, killed: 1, superseded: 1` — **47 terminal,
21 live**. This is the observation the mechanism rests on.

## Section 1 — Recovery

One-time, no new code. Recovers what three `CM-N` entries declare unrecoverable.

### 1.1 Archive companion (the durability fix)

Create `docs/trackers/archive/provenance-subsystem-recovered-entries.md`:

- frontmatter `kind: tracker`, `status: archived`
- one `#### PV-N — <title>` section per recovered row, carrying
  `type · status · priority`, the full `detail`, `evidence`, `gated_on`
- header records provenance: recovered VERBATIM from the desktop `catalog.db` on
  2026-08-31, naming the backup file
  (`~/.local/share/librarian/catalog.db.bak-20260831-preintegration`) and `CM-2`

`status: archived` is deliberate. Where two artifacts define one token the sole **active**
one wins, so a live stub added later automatically takes precedence and no ambiguous token
is ever created.

Approximate size: 38 rows, ~45 KB, dominated by `detail` (64,844 bytes across all 68 rows).

### 1.2 No stubs in the live body

`provenance-subsystem.md`'s own § *Defining sections for cited entries* states a measured
policy: promote only entries other files actually cite, because "mass-promotion would
contradict this tracker's own 'narrative only when a row is insufficient' rule, and it is
**safe** to leave them precisely because prefix `PV` is now defined — `link_scan` reports a
citation of an undefined `PV-N` as *dangling*, so future breakage is visible rather than
silent."

Measured against the 38: **22 appear cited from outside, but 0 carry a load-bearing content
citation.** The 22 decompose as

- 3 bookkeeping files that enumerate these ids *as lost*
  (`cross-machine-catalog-resume.md`, `resume-cross-machine-catalog-restore.md`,
  `2026-08-19-entry-without-definition-asserts-omission-without-checking-citations.md`)
- a citation-count table (`prompt-surface-compaction-session-log.md:1154`)
- a doc-comment example of citation syntax (`src/librarian/tools/doctor.rs:2329`) — the
  exact class named by
  `docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md`
- a test fixture using `PV-3` / `PV-9` as arbitrary tokens in a synthetic `led.md`
  (`src/librarian/tools/doctor.rs:10617`)

So the tracker's policy holds unmodified. Add no stubs.

### 1.3 Params restored to both catalogs

The desktop holds all 68 rows; the laptop holds 30. Push the 38 to the laptop so
`entry_filter` returns the same answer on both hosts.

**Blocker:** `PV-48`'s status `superseded` is not in the laptop's enum
`["settled","open","blocked","descoped","carried","killed"]`. Widen the enum to include
`superseded`. Precedent: `2a8decc5 docs(trackers): widen the queue's status enum to the
vocabulary its bodies use` — the same move for the same reason. The widening writes
through to the sidecar, so it travels.

Three further trackers are blocked by the same coupling and are fixed the same way. The
laptop's newer `params_schema` rejects this desktop's older rows:

| tracker | violation | scope | resolution |
|---|---|---|---|
| `research/README` | `"path" is a required property` | all 15 entries | Counts match 15/15 — copy the laptop's params, which are strictly better |
| `fable-tuning-findings` | `"title" is a required property` | all 18 entries | Counts match 18/18 — copy the laptop's params |
| `tool-usage-patterns` | `verdict: "legitimate, shape-mismatched"` not in enum | 1 row | **Field-level union required — see below. Do not copy either side wholesale.** |
| `provenance-subsystem` | `status: "superseded"` not in enum | 1 row | Desktop is the superset here — widen the enum instead |

**`tool-usage-patterns` is not a copy, it is a merge.** Verified id-by-id across both hosts
on 2026-08-31: the laptop's **id set** is a superset (it adds `T-31`, `T-32`; the desktop
has no id the laptop lacks), but its **field content is not**. Ten rows carry a `verdict`
on the desktop and none on the laptop — `T-005`, `T-008`, `T-011`, `T-012`, `T-19`,
`T-20`, `T-22` (`wrong-tool`) and `T-17`, `T-18`, `T-21` (`legitimate`). Copying the
laptop's params wholesale would erase all ten.

The resolution is a field-level union: take `T-31` and `T-32` from the laptop, keep the
desktop's ten verdicts, and accept the laptop's `T-30` value (`legitimate`) over the
desktop's `legitimate, shape-mismatched` — the latter is the compound string the enum
rejects, and the laptop's is its normalised form.

This is the one place in the whole integration where a naive row-level copy loses data in
either direction, which is why it is called out rather than folded into the table.


### 1.3a Schema and params are mutually gating — migrate them atomically

Measured 2026-08-31 while executing § 1.3, and it invalidated that section's prescribed
order. Recorded here because it will recur on the next cross-machine schema change and
otherwise survives only in one task report.

**Neither order works one field-group at a time.** § 1.3 said params first, shape second,
reasoning that a new schema rejects old rows — true, and measured. The reverse is equally
true and was not measured: the *stored* schema rejects the new rows.

| artifact | stored schema (stale) | sidecar schema (current) |
|---|---|---|
| `docs/research/README.md` | `required: [file, …]` **plus `additionalProperties: false`** | `required: […, path]` |
| `docs/trackers/fable-tuning-findings.md` | `required: [… claim …]` | `required: [… title …]` |

The laptop's migrated `research/README` rows fail the stored schema on **two** counts —
missing `file`, and `path` disallowed by `additionalProperties: false`. Old-schema/old-rows
and new-schema/new-rows both validate, so the incompatibility is precisely the swapped
field. A params-only write is refused; a schema-only write is refused; the migration has no
one-field-group path.

**The escape is a single atomic call, and it is a designed affordance rather than a
bypass.** `validate_merged_against_schema` (`src/librarian/tools/augment.rs:36-52`, called
at `:370`) validates merged params against **the schema the call itself supplies** when one
is present — comment-tagged `F-5`. So one `artifact_augment` / `artifact-augment --merge`
carrying `params` *and* `params_schema` *and* the remaining shape fields is fully validated
against the target schema.

Pass the remaining shape fields in that same call, not because the schema needs them, but
because the write-through republishes the whole row
(`docs/issues/archive/2026-08-31-artifact-augment-write-through-republishes-the-whole-row.md`,
**fixed 2026-08-31 at `6ae7d39a`**). The three-call alternative — permissive schema, then
params, then the real schema — touches shape fields twice and doubles exposure to that
defect. Atomic is both the only working path and the safer one.

> **The exposure argument is narrower since the fix, and the conclusion is unchanged.** A
> merge call no longer republishes a shape field it did not name over a sidecar that
> disagrees — it refuses and reports. So the three-call alternative's extra shape writes now
> fail loudly rather than silently overwriting, which is better but still worse than not
> making them. The deadlock argument above is independent of that defect and stands on its
> own: atomic remains the only order that validates.

**Generalisation for unit 3.** Any design that projects schema and rows through separate
write paths inherits this: a projection restored field-by-field can deadlock against its
own live data. Terminal-row projection (§ 2.1) must therefore restore a row's shape and
its rows in one validated write, or define a documented permissive intermediate state.
### 1.4 Correct the two authored titles

`PV-9`'s committed title materially diverges from the canonical one and is replaced:

- committed (AUTHORED): *"M6 stale-drift: specs rarely change after code derives from
  them, at any horizon that matters"*
- canonical: *"DONE — M6 measured: spec churn is same-session, not long-horizon drift"*

These are different claims — the first asserts a result at any horizon, the second
localises the churn to a single session. Replace the title and relabel the provenance note
`RECOVERED-VERBATIM`.

`PV-11`'s authored title matches its canonical in substance (differing only in case and a
`RESOLVED —` prefix). Keep the text; drop the `AUTHORED` caveat.

**Correction 2026-08-31 (fix round 1) — this judgement was wrong.** A substance match
does not license a `RECOVERED-VERBATIM` label; verbatim means exact, byte for byte.
`PV-11`'s title is installed from the canonical instead, not kept and re-labelled:
*"RESOLVED — `unrecorded` dominates only at whole-repo scope, NOT at working-diff scope"*.
This is the same defect class the task exists to fix — a near-miss captioned as
recovered/verbatim.

### 1.5 Correct the CM entries

- `CM-2` — status `open` → `fixed`, citing the recovery. Retire "permanently"; its own
  `Next:` line named this recovery path and conditioned it on the desktop catalog
  existing, which it does.
- `CM-1` — correct the claim that five named trackers have "no provenance anywhere". They
  have provenance on the desktop. Whether to restore them stays a separate decision on
  `CM-1`'s existing reasoning (a restored augmentation's `[LIVE]` block is read by every
  agent meeting the tracker cold, so restoring purely to clear a check has a real cost).
- `CM-3` — update the AUTHORED provenance notes to reflect 1.4.

### 1.6 Verification

- `doctor`'s `entry_without_definition` and `cited_prefix_with_no_definer` recorded before
  and after
- `grep -c` of the 38 ids in the committed archive companion equals 38
- `artifact(get, entry_filter={"id":{"eq":"PV-48"}})` returns a row on **both** hosts
- `librarian(action="link_scan")` back to `edges_missing[0], edges_stale[0]`
- `sidecar_shape_drift` reaches 0

## Section 2 — Durable mechanism

### 2.1 Chosen: terminal-row projection

Commit terminal rows; keep live rows catalog-only.

The standing objection to committing params is stated in
`get_guide("tracker-conventions")`: params "deliberately stay catalog-only: they are live
state that churns, and committing them recreates the params-vs-body drift class
BL-29/BL-40/BL-42 closed."

That objection is true of live rows and **false of terminal ones**. A row whose status is
`settled`, `killed`, `descoped` or `superseded` cannot drift, because nothing will change
it again. Splitting on churn preserves the objection's force exactly where it applies and
removes it where it does not.

Mechanically this reuses the sidecar pattern, which is already proven in this codebase:

- **write-through** when a row's status reaches a terminal value — the same hook
  `artifact_augment` already uses to keep a sidecar current on shape change
- **`reindex` restores** a projected terminal row when the catalog row is absent, and
  never overwrites a live one — repair, not sync, mirroring the existing rule
- **a `doctor` check** reports projection drift, reporting rather than guessing

Effect: the at-risk set shrinks from "every params row" to "rows someone is actively
working", which are by definition open in a live session.

**Costs, stated plainly.** A third representation needs a third drift check. Reopening a
terminal row must un-commit it from the projection — handled by the same write-through,
not new machinery. Neither cost is hypothetical and both should be planned for.

### 2.2 Chosen: publish provenance in the sidecar

Add `written_at_commit` and `written_by_host` to `AugmentationSidecar`
(`src/librarian/augmentation_sidecar.rs`), bumping `SCHEMA_VERSION`.

`artifact_augmentation.refreshed_at_commit` already exists as a column and is populated on
6 of 72 rows locally; the sidecar publishes no provenance field at all — its fields are
exactly `schema_version, prompt, entry_collection, params_schema, render_template,
append_mode, history_cap`.

With the commit recorded, `sidecar_shape_drift` can compare it against local state and git
ancestry and will often be able to name the stale side, rather than always declining. That
single field is most of what the 2026-08-31 forensics reconstructed by hand.

**This is independently valuable and should not be sequenced behind 2.1.**

### 2.3 Rejected: generalise `merge_worktree` to hosts

Record a `host_fork` base snapshot the way `worktree_fork` does, and add a `merge_host`
action folding the delta through the existing three-way + entry-id-renumber logic. It
would cover events and observations as well as params.

Rejected now, for three reasons: a worktree has a natural creation event to fork at and a
second clone has none; it requires a transport for the snapshot; and it solves a merge
problem with zero measured instances.

**Revisit when** a genuine concurrent-edit conflict is observed — two hosts both mutating
the same entry between syncs, where neither side is a superset. At that point this becomes
the right answer and the `merge_worktree` machinery is already built.

**The "zero measured instances" clause above is falsified. The Revisit-when trigger is NOT — keep the two apart.** `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` records a measured cross-host merge problem: two hosts independently allocated the same id in the `R` namespace, 147, for two *different* entries (desktop high-water 146, laptop 147 unpushed). That is an **allocator** collision — there is no shared entry, no common ancestor, and nothing for a three-way merge to reconcile. It therefore falsifies "solves a merge problem with zero measured instances", and it does **not** satisfy the trigger as worded, which requires two hosts mutating *the same* entry with neither side a superset. The rejection still stands on all three reasons, with the third narrowed from "no instances" to "no instance of the class `merge_host` would address". *(The first draft of this annotation led with "that condition fired the same day" — overstated, and caught by the fix wave's re-review. Its own closing sentence already conceded the distinction, which is what made the headline wrong rather than merely loose.)*

### 2.4 Rejected as the answer, adopted as a stopgap

Routine `sqlite3 .backup` of `catalog.db` to a synced location, plus a documented
one-writer-per-tracker convention.

This solves *loss* completely for near-zero cost, and one such backup was taken on
2026-08-31 before any integration work. It is **not** the answer, because it leaves the
adjudication cost entirely in place and the convention is unenforceable.

Adopt the backup half immediately as a stopgap covering the window before 2.1 ships. Use
`sqlite3 .backup`, never `cp` — the MCP server holds the database open in WAL mode and a
plain copy can tear mid-write.

### 2.5 Explicitly not building

- **Events and observations sync.** No measured harm.
- **Automatic conflict resolution.** The project's stance is report-don't-guess
  (`03b86cd7`), and this design does not weaken it.
- **Any daemon or background sync service.**


## Implementation sequencing

This spec covers three separable units. They should **not** become one implementation
plan — they differ in shape, risk and test surface.

1. **Recovery (Section 1)** — data only, no code. Reversible via the 2026-08-31 backup.
   Ship first and alone: it is time-sensitive, because the desktop's `catalog.db` is
   currently the sole copy of the recovered rows, and it needs no design work beyond this
   document. It has no tests to write; its verification is § 1.6.

2. **Sidecar provenance (§ 2.2)** — one struct field pair, a `SCHEMA_VERSION` bump, and a
   `sidecar_shape_drift` improvement that consumes it. Small, independently valuable, and
   dependent on neither 1 nor 3. The existing corpus test
   (`every_committed_sidecar_parses_and_carries_no_params`) pins the schema, so the bump
   has a ready-made regression surface.

3. **Terminal-row projection (§ 2.1)** — the actual feature. Blocked on settling the
   per-ledger "terminal" mapping listed under *Open decisions*. Sequence it after 2 so the
   projection carries provenance from its first write rather than acquiring it later; its
   test surface (write-through, reindex-restore-when-absent, the reopen path, the drift
   check) is larger than 1 and 2 combined.

Each unit gets its own plan and its own review.
## Open decisions deferred out of this spec

- Whether to restore the desktop-only augmentations at all. There are **14** such
  augmentation rows (present on the desktop, absent on the laptop); **13** of the 14
  declare `expects_augmentation: true` and so are reported by
  `augmentation_declared_but_absent` on the laptop, while `claim-decay` declares nothing
  and is therefore invisible to that check. `CM-1` argues against on
  grounds unaffected by this recovery: an augmentation's `[LIVE]` block is read by every
  agent meeting the tracker cold, so restoring one purely to clear a check converts a
  precise signal into a false all-clear. This spec corrects `CM-1`'s factual premise
  without overturning its judgement.
- Which statuses count as terminal, per ledger. The candidate set is
  `settled | killed | descoped | superseded | done-archived | fixed | wontfix`, but each
  ledger declares its own enum and the mapping must be per-ledger rather than global.

## References

- `docs/conventions/cross-machine-catalog-resume.md` — the resume procedure this extends
- `docs/trackers/resume-cross-machine-catalog-restore.md` — `CM-1`, `CM-2`, `CM-3`
- `docs/superpowers/specs/2026-07-17-worktree-overlay-design.md` — the delta-merge
  precedent named in 2.3
- `src/librarian/augmentation_sidecar.rs` — the shape sidecar this extends
- `src/librarian/tools/doctor.rs` — `sidecar_shape_drift`, `params_behind_body`,
  `params_status_drift`
- `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` — the class the
  "longer field is newer" heuristic would have fallen into
