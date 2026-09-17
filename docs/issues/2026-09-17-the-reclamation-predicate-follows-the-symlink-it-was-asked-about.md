---
id: '7b1458c4ede1a86f'
kind: bug
status: open
title: 'BUG: the reclamation predicate asks exists() about a symlink, so a row the walk deliberately skipped is never reclaimed'
tags:
- cluster/selector-narrower-than-its-population
---

# BUG: the reclamation predicate asks `exists()` about a symlink, so a row the walk deliberately skipped is never reclaimed

## Summary

`index_repo_sync`'s reclamation loop selects rows the walk did **not** see, then deletes only
those whose path fails `Path::exists()`. `exists()` **follows symlinks**, so a row for a
symlinked path whose target is still present survives forever. The walk stops updating it and
nothing ever removes it, and the report says `removed: 0` — which reads as *nothing to reclaim*
rather than *one row was not examined*.

Introduced as a live defect by `0c8ff65d` (the fix for `cdcad7a0257ec7c0`), which is the honest
framing: before that commit, *unseen* and *file deleted* were the same set, so `exists()` was a
harmless second opinion. That commit created a **second reason** for a row to be unseen —
deliberately skipped — and the predicate cannot tell the two apart.

## Symptom (Effect)

After `librarian(action="reindex")` on a binary containing `0c8ff65d`:

```
{"added":0,"updated":8,"removed":0,"unchanged":1712,"orphans_removed":0, ...}
```

`removed: 0`, and the duplicate row is still there:

```
doc(action="find", filter={"rel_path": {"eq": "AGENTS.md"}})
  69d34fcf89157188  memory  draft  "codescout"  AGENTS.md   updated_at 1789617305530
```

`updated_at` is **2026-09-17 06:55:05** — four minutes *before* `0c8ff65d` was committed at
06:59:16, and before the binary was built at 07:10:27. The row has not been touched since, and
44 chunk rows hang off it.

## Reproduction

With `AGENTS.md -> CLAUDE.md` in the project root, on a binary containing `0c8ff65d`:

```
librarian(action="reindex")                                     # removed: 0
doc(action="find", filter={"rel_path": {"eq": "AGENTS.md"}})    # row still present
```

## Environment

`experiments`, binary `5a682c9d` (1 commit behind HEAD `7f3fe1ba`, contains `0c8ff65d`),
catalog `~/.local/share/librarian/catalog.db`.

## Root cause

`src/librarian/indexer.rs`. The candidate query is already exactly right — rows under the root
that this walk did not see:

```sql
SELECT id, abs_path FROM artifact WHERE abs_path LIKE ?1 AND id NOT IN (<seen_ids>)
```

The deletion predicate then re-asks a question that query has already answered, and re-asks it
wrongly:

```rust
for (cand_id, cand_abs_path) in &candidates {
    if !std::path::Path::new(cand_abs_path).exists() {
```

`Path::exists()` traverses symlinks (it is `fs::metadata`, not `symlink_metadata`), so
`AGENTS.md` answers `true` on the strength of `CLAUDE.md` being present — a **different
document**, and the one whose row is the survivor of the pair.

**This is the same law at a second call site, fifteen lines from the first.** `0c8ff65d` fixed
`is_file()` in the candidate filter; this is `exists()` in the reclamation loop. Both follow
links; both answer about the *target* when the question is about the *link*. CLAUDE.md
§ *Testing Discipline* states the trap exactly: *"Mutate once per guarded SITE, not once per
feature — where a law is implemented at N call sites, one kill says nothing about the other
N−1."* `a_symlink_resolving_inside_the_root_is_not_indexed_as_a_second_artifact` kills two
mutations at site 1 and says nothing whatever about site 2.

## Evidence

### The live catalog could not discriminate, and that is worth recording

The reindex above does **not** prove `0c8ff65d` fired. `CLAUDE.md` has been unchanged since
2026-09-16, so both rows' stored hashes read `1569f479311f` — the live content. *"The walk
skipped `AGENTS.md`"* and *"the walk visited it and found it unchanged"* therefore predict
byte-identical catalog state, and `updated_at` moves under neither.

The evidence that site 1 works is the regression test and its two killed mutations, not this
reindex. Stated because the reindex is the observation a later reader will reach for first, and
it cannot carry that weight.

### Why `removed: 0` is the IC-18 shape rather than a mere miss

The population is *rows that should be reclaimed*. The selector is `!exists()`, which reaches
only the deleted-file members. A deliberately-skipped row is never examined, so there is no
count to report and nothing to mark — `removed: 0` is well-formed and reads as *not present*
rather than *not looked at*, which is this class's claim verbatim. The blind party is the reader
of the reindex report.

## Hypotheses tried

1. **Hypothesis:** `0c8ff65d` did not fire, so the walk still indexed `AGENTS.md`.
   **Test:** compare stored hash against live content; check `updated_at` against the fix and
   build times.
   **Verdict:** **inconclusive from the catalog, and deliberately left that way.** `updated_at`
   (06:55:05) predates both the fix and the build, which is consistent with the walk skipping it
   — but also with an unchanged-content visit. Not resolvable without changing `CLAUDE.md`,
   which is a shared file several sessions are editing.

2. **Hypothesis:** the candidate query is scoped so the row never becomes a candidate.
   **Test:** read the SQL.
   **Verdict:** rejected — `abs_path LIKE '<root>%' AND id NOT IN (seen_ids)` includes it by
   construction once the walk stops seeing it. The query is right; the predicate after it is not.

## Fix

Not fixed. The candidate query has already established the fact the loop needs, so the smallest
correct change is to stop re-deriving it — or, if a liveness check is still wanted as a
safeguard, use `symlink_metadata()`, which does not traverse.

**Do not fix this without re-running site 1's mutations too.** The two predicates now interact:
a reclamation that deletes every unseen row would delete rows site 1 skips, which is correct
here and would be catastrophic if site 1's predicate were ever widened. CLAUDE.md § *Testing
Discipline*: *"after adding or changing any bound, re-run the mutation set for EVERY bound."*

## Tests added

None yet — no fix chosen. Named rather than left blank: the existing symlink test covers site 1
only, and crediting it with coverage here is the false-coverage failure this repo treats as
worse than no test.

## Workarounds

Delete the row by hand (`DELETE FROM artifact WHERE id='69d34fcf89157188'`) — the chunk rows
cascade. Not recommended while several sessions share the catalog; the row is inert rather than
harmful, costing one duplicate `find` hit and one doubled `doctor` line.

## Resume

Decide between dropping the `exists()` re-check and switching it to `symlink_metadata()`. The
former is smaller and provably equivalent for the pre-`0c8ff65d` population; the latter keeps a
safeguard whose value is now unclear, since the query it guards is the authority. Either way,
re-run both mutations from `cdcad7a0257ec7c0`'s Tests section afterwards.

## References

- `docs/issues/2026-09-16-a-symlinked-instruction-file-is-cataloged-as-a-second-artifact.md` —
  site 1, fixed at `0c8ff65d`; its `unverified:` field named this branch before it was found
- `docs/issues/archive/2026-09-06-the-unpushed-ledger-guard-goes-silent-on-a-symlinked-path.md` —
  the same seam a third time, from the path-form side
