---
id: 388290ad0f86fe03
kind: bug
status: fixed
title: 'BUG: the librarian guard is blind to any artifact whose frontmatter omits `id:` — 26 of 66 tracker/bug files are unprotected, including the most-damaged ledger'
tags:
- librarian
- guard
- trackers
- data-integrity
---

## Summary

`is_librarian_artifact` (`src/util/librarian_guard.rs:115-128`) decides whether a file
is librarian-managed by reading the file's **own text** for an `id:` line in frontmatter.
If no `id:` line is present, it returns `false` and the file is treated as an ordinary
markdown document — `edit_markdown`, `read_markdown`, and `edit_file` all proceed.

But the catalog does **not** need the file to carry an id: `id = sha256(abs_path)`, so a
tracker can be fully registered, queryable via `artifact(find/get)`, and still have no
`id:` in its frontmatter. Those artifacts are completely unguarded.

## Evidence

Measured 2026-08-17 across `docs/trackers/*.md` + `docs/issues/*.md`:

- **66 files checked, 26 have no `id:` line (39%)** — 13 trackers, 13 bug files.
- Unguarded trackers include `skill-frictions.md` and `codescout-usage-hookify.md`,
  both of which CLAUDE.md explicitly instructs agents to append to, and
  `archive-cadence-policy.md`, a live convention surface.

Demonstrated live in the same session:

```
edit_markdown("docs/trackers/reconnaissance-patterns.md", ...)   -> ACCEPTED  (no id: line)
edit_markdown("docs/trackers/tracker-hygiene-log.md",    ...)   -> REFUSED   (id: '7e49…')
```

Both are `kind: tracker`, `status: active`, `augmentation: null`, both registered in the
catalog. The only difference is whether the *file* happens to restate its own id.

## Why this matters more than the count suggests

The single most structurally damaged ledger in this repo is
`docs/trackers/reconnaissance-patterns.md`, and it is exactly the file the guard cannot
see. Today's sweep found in it: 13 body entries with no index row, 9 ids allocated
twice, 39 of 57 entries with no disposition field, and ~30 of the 39 sampled dangling
citation tokens in the whole project. An unguarded artifact accumulates hand-edits in
arbitrary shapes, because nothing routes its writers through the one surface that could
impose a shape. The correlation is causal, not coincidental.

## The repair path explicitly declines to fix it

`librarian(action="doctor", fix=repair_frontmatter_id)` exists and sweeps
`frontmatter_id_mismatch` files — but its own contract states that **"a file with NO
frontmatter id is left alone rather than stamped."** So the one tool positioned to close
this gap is documented to skip precisely this class.

## Relationship to the quoting bug

`docs/issues/archive/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md`
(status `fixed`) reported "15 of 27 trackers are unprotected" because the predicate
mishandled YAML quoting. That fix works — `id: '7e49…'` is now correctly guarded, as
demonstrated above. This is a **distinct and larger class**: not a mis-parsed id, but no
id at all. The earlier fix was validated against the corpus that produced it (files that
*had* ids, quoted) and did not reach the adjacent case (files with none). Kin R-97.

## Fix options

1. **Key the guard on the catalog, not the file text.** A path lookup against the
   catalog is authoritative and cannot drift. Most correct; cost is catalog access on a
   hot edit path.
2. **Widen the text predicate** to also treat `kind: tracker` / `kind: bug` frontmatter
   as managed, id or not. Cheap, no catalog access, no new drift surface. Catches
   everything under `docs/trackers/` and `docs/issues/` and nothing else.
3. **Backfill `id:` into the 26 files.** Cheapest to reason about, but it re-creates the
   very drift surface `frontmatter_id_mismatch` exists to police — every future move
   re-breaks it.

Recommended: **2 now, 1 as the principled endpoint.** 3 alone treats the symptom and
adds maintenance.

## Reproduction

```
grep -L '^id:' docs/trackers/*.md          # the unguarded set
edit_markdown("docs/trackers/skill-frictions.md", heading="...", action="edit", ...)
# succeeds; no librarian_guard error
```

## Status

`open` — but **substantially reframed 2026-08-17, hours after filing.** The
defect as reported is not one; the instinct behind it is sound. Both halves
matter, so neither is deleted.

### Retracted: "26 of 66 unprotected" is not a defect

`src/util/librarian_guard.rs` pins the predicate on purpose. The test
`a_catalogued_but_unaugmented_file_stays_directly_editable` states the rationale:
guarding by catalog **membership** would refuse `docs/RELEASE.md`,
`CONTRIBUTING.md` and every ADR, because all of them are catalog rows. It then
names `docs/trackers/skill-frictions.md` — a catalog row with no frontmatter id —
as a file CLAUDE.md documents `edit_markdown` for.

`skill-frictions.md` is cited **in this bug's own Evidence section** as proof of
the gap. It is in fact the pinned example of a file that must stay editable.
Fix options 2 and 3 above would break that contract; treat them as withdrawn.

And the recommendation cost something before it was checked: acting on it, I
stamped `id:` into `reconnaissance-patterns.md`, which silently disabled
TAXONOMY.md's documented `edit_markdown` append path for R-N. Confirmed by probe,
reverted in `bb9a94d7`.

### What survives, and it is the larger finding

**Neither predicate matches the damage.** Everything measured on 2026-08-16/17 —
9 twice-allocated ids, 13 orphaned bodies, 48 entries with no defining heading
(~30 of 39 sampled dangling citation tokens), 39 of 57 entries with no
disposition — happened in an **unaugmented** file, and none of it is params/body
desync, which is the only thing augmentation-keying protects. So:

- **augmentation** is orthogonal to the failure mode that actually occurs;
- **membership** is too broad, exactly as the pinned test argues.

The missing concept is a **ledger**: an artifact with an id namespace and entry
invariants. That set is not hypothetical — `docs/TAXONOMY.md` enumerates it (ten
numeric prefixes) and wires it to nothing, which is also why that table drifted
into prescribing an array-replacing call for two of its own slots (`9943164e`).

### The shape the fix should take

1. **Declare** `entry_prefix` on the artifact. Orthogonal to `entry_collection`,
   which is a *storage* fact.
2. **Allocate** through the server for any artifact with one — prototyped and
   mutation-verified in `540c29c3` (`allocate_entry_id`), including a two-thread
   test on a ledger with no params collection.
3. **Guard structurally, not per-file.** `edit_markdown` is heading-addressed,
   so the guard can decide mechanically: a target heading matching
   `^<PREFIX>-\d+`, or the declared index heading, routes through the allocator;
   any other heading stays directly editable. That protects entry structure
   without making a typo fix in a 2,800-line tracker a ceremony — which is the
   real objection to the whole-file guard this bug originally proposed.

#### What shipped, 2026-08-18 — and how the three pieces actually stood

Plan: `docs/superpowers/plans/2026-08-18-ledger-aware-librarian-guard.md`. Reconnaissance
before drafting it found this section's three pieces in three different states.

**Pieces 1 and 2 were already shipped.** `ENTRY_PREFIX_KEY` and `entry_high_water_<PREFIX>`
exist in `src/librarian/catalog/augmentation.rs`, and `allocate_entry_id` is wired to the
MCP surface at `src/librarian/tools/append_entry.rs:91` on the prose path
(`entry_collection` omitted) — hardened since by three follow-up fixes (worktree collision,
heading-level reporting, `frontmatter_max` diagnostics). This file's own text said "NOT yet
wired"; that was true at `540c29c3` and stale by the time it was read.

**Piece 3's goal shipped; its proposed mechanism was cut.**

Shipped, commit `f4db4e9c` (**experiments**): `declared_entry_prefixes` in
`src/util/librarian_guard.rs`, plus a third arm in `guard_with_oracle`'s union — stamped
`id:` OR augmented OR **declares `entry_prefix`**. Hand-parses the frontmatter because
`librarian` is a Cargo feature and that module compiles under `--no-default-features`;
`cargo check --no-default-features` is in the gate.

Cut: the heading-scoped guard this section proposed. **Its premise was measured false.**
The argument for heading-scoping was that a whole-file guard makes "a typo fix in a
2,800-line tracker a ceremony". But `artifact(action="update", patch={body_edits:
[{heading, action: "edit", old_string, new_string}]})` is already a section-scoped text
swap, and it works on any catalog row — verified on `docs/trackers/skill-frictions.md`, a
row with **no `id:` and no augmentation**, which returned `old_string not found`, i.e.
reached the swap logic. Augmentation is not required. So the ergonomic gap the
heading-scoping existed to close does not exist, and building it would have cost a fourth
parameter on a public function threaded through three call sites, a regex, and four tests,
for a class of file with **zero current members**. What survives of it is hint text: the
ledger refusal now names both routes, `append_entry` for an entry and `body_edits` for
prose, with a test pinning that the entry route does *not* leak into the augmented arm
(where the file is a rendered snapshot and `append_entry` would be wrong).

**The hole was verified end-to-end, not inferred.** All five ledgers that exist today were
already guarded — `capability-proposals.md` and `tracker-hygiene-log.md` by a stamped
`id:`, `codescout-usage-frictions.md`, `codescout-usage-hookify.md` and
`reconnaissance-patterns.md` by augmentation (the last two confirmed by live `edit_markdown`
probe). So the protection was **accidental, not principled**. The reachable hole is a file
created the documented way: a scratch ledger with `entry_prefix: ZZ` /
`entry_high_water_ZZ: 3` / no `id:` / not augmented accepted a hand-written `## ZZ-4`
heading via `edit_markdown` and left the mark at 3. Compaction later lowers `body_max` back
to 3, and `allocate_entry_id`'s `next = max(body_max+1, reserved_max+1, frontmatter_max+1,
1)` reissues `ZZ-4` — re-arming
`docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`.

**A defect the plan itself introduced, caught in review.** Piece 3 necessarily creates a
second reader of `entry_prefix`, and the plan's first draft held the two in agreement with
a doc comment: *"accepts all three YAML forms `allocate_entry_id` honours"*. That is a
co-change contract enforced by prose — it proves someone knew and supplies no mechanism,
the shape that cost this project 48 needlessly-compiled crates
(`docs/adrs/2026-07-25-embedding-transport-boundary.md`). Commit `9ac00440`
(**experiments**) replaces the sentence with
`both_entry_prefix_readers_agree_on_every_yaml_form`: 11 YAML forms, both readers,
`assert_eq!`. The hand parser matched `serde_yml` on every form first run.
### One genuine inconsistency, low severity

The guard has two signals with different semantics: the `id:` line (a membership
proxy) and the augmented-artifact oracle (augmentation). The pinned test's doc
comment describes augmentation as *the* predicate, yet the `id:` path guards
files that are catalogued and unaugmented — `tracker-hygiene-log.md` is exactly
that, and is refused. Not harmful, and probably back-compat from before the
oracle existed, but it is what made this bug's premise look true on inspection.

### Tests added, 2026-08-18

All gate-green, all mutation-verified.

In `src/util/librarian_guard.rs` (commit `f4db4e9c`, **experiments**):

- `a_declared_ledger_is_guarded_with_no_id_and_no_augmentation` — the core arm.
  Mutation-verified: forcing `let ledger = false` drops it with `a declared ledger must be
  guarded: ()`.
- `every_yaml_form_of_entry_prefix_is_recognised` — scalar, quoted scalar, inline flow,
  block sequence. Protection must not depend on which writer last emitted the file — the
  same reasoning as BL-33's quoted-id fix.
- `entry_prefix_outside_frontmatter_declares_nothing` — a doc that *discusses*
  `entry_prefix` in prose owns no namespace; otherwise every convention doc in `docs/`
  becomes a ledger.
- `a_valueless_entry_prefix_declares_nothing` — bare key, empty string, empty flow list.
  Load-bearing rather than defensive: an empty prefix would make every numbered heading in
  the file read as an entry.
- `a_ledger_refusal_names_both_the_entry_route_and_the_prose_route` — the hint must carry
  `append_entry` **and** `body_edits`. A single-branch hint is the failure mode being
  avoided, not a smaller version of it.
- `the_ledger_hint_does_not_leak_into_the_augmented_or_stamped_arms` — an augmented file's
  params live in the catalog, so `append_entry` is the wrong route there; both other arms
  keep the generic hint.

In `src/librarian/catalog/augmentation.rs` (commit `9ac00440`, **experiments**):

- `both_entry_prefix_readers_agree_on_every_yaml_form` — 11 forms through both readers,
  asserted equal. Mutation-verified by blinding the guard to block sequences: it fails
  naming that form, `left ["F", "W"]` against `right []`, which states the silent hole as a
  diff — the allocator issuing `F-N` for a ledger the guard reads as not-a-ledger.

The pinned `a_catalogued_but_unaugmented_file_stays_directly_editable` is **unmodified and
green**. It was the contract this fix had to not break, and it is why Fix options 2 and 3
stay withdrawn.

Not added: any test of the heading-scoped guard, which was cut — see the shipped-notes
subsection above for the measurement that killed it.

### Resume

Fixed on **experiments** in `f4db4e9c` (guard arm + hint) and `9ac00440` (parity test).
`git rev-list --left-right --count master...experiments` had a `0` on the left at fix time,
so fast-forward is available and the `experiments` SHAs already *are* the master SHAs — no
cherry-pick, no second SHA to record.

One follow-up, filed separately rather than folded in:
`docs/issues/2026-08-18-tracker-conventions-guide-recommends-reverted-id-stamping.md`.
`get_guide("tracker-conventions")` § *Make the tracker guarded* still prescribes stamping
`id:` into frontmatter — the remedy this file retracted and `bb9a94d7` reverted. The guide
auto-injects on the first `artifact` call of every session, so the disproved advice sits on
a louder surface than the retraction. With this fix shipped, the correct advice is *declare
`entry_prefix`*.

## Fix provenance

- **SHA:** `9ac00440` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `4e15440ec4dfdcd95ce5b9d5c623e95b8a27792f` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 4e15440ec4df /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
