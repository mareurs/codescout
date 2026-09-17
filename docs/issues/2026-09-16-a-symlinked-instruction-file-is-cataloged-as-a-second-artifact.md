---
id: cdcad7a0257ec7c0
kind: bug
status: fixed
title: 'BUG: a symlink inside the project is cataloged as a second artifact, so one document has two ids and 84 chunks'
tags:
- cluster/unclassified
closed: 2026-09-17
unverified: 'RESOLVED 2026-09-17. Was: not established end-to-end that an already-minted duplicate row disappears. It does NOT disappear -- reindex on a binary containing 0c8ff65d reports removed: 0 and the row survives. Cause is a SECOND defect at a different call site (reclamation''s Path::exists() follows symlinks), filed as 7b1458c4ede1a86f. This fix stops the minting, which is what it claimed; nothing about it is outstanding.'
---

# BUG: a symlink inside the project is cataloged as a second artifact, so one document has two ids and 84 chunks

## Summary

`AGENTS.md` in this checkout is a **symlink to `CLAUDE.md`**. The indexer walks it as an
ordinary file, so the catalog holds **two artifact rows and 84 chunk rows for one physical
document**. Nothing errors. `doc(action="find")` returns both, and `doctor` reports every
per-artifact finding for that file twice.

The setup is not exotic: codescout's own manual prescribes `AGENTS.md` as the instruction
file for Codex, Cursor and generic harnesses (`docs/manual/src/concepts/after-onboarding.md:77-79`),
and a symlink is the obvious way to keep it in step with `CLAUDE.md`.

## Symptom (Effect)

Two rows, one file:

```
doc(action="find", filter={"or":[{"rel_path":{"eq":"AGENTS.md"}},{"rel_path":{"eq":"CLAUDE.md"}}]})

0b9c1365555fc686  memory  draft  "codescout"  CLAUDE.md   updated_at 1789583814167
69d34fcf89157188  memory  draft  "codescout"  AGENTS.md   updated_at 1789583813760
```

Same title, same kind, 407 ms apart — one indexer walk, both rows.

`doctor` then double-reports, and the giveaway is that **both violations carry the identical
hash pair**, which two genuinely different files cannot:

```
row_behind_file  69d34fcf89157188  AGENTS.md
  catalog holds ccef3ab98d65 but the file hashes to df933f8eb6dd
row_behind_file  0b9c1365555fc686  CLAUDE.md
  catalog holds ccef3ab98d65 but the file hashes to df933f8eb6dd
```

## Reproduction

At `8273331d` on `experiments`:

```
$ ls -l AGENTS.md
lrwxrwxrwx 1 marius marius 9 Sep 12 19:47 AGENTS.md -> CLAUDE.md

$ sha256sum AGENTS.md CLAUDE.md
df933f8eb6dd6da1893a3cfbffb7b3343849e6a3c435c618b944320760ab0601  AGENTS.md
df933f8eb6dd6da1893a3cfbffb7b3343849e6a3c435c618b944320760ab0601  CLAUDE.md

$ sqlite3 ~/.local/share/librarian/catalog.db \
    "SELECT artifact_id, COUNT(*) FROM artifact_chunk
      WHERE artifact_id IN ('69d34fcf89157188','0b9c1365555fc686') GROUP BY artifact_id;"
0b9c1365555fc686|42
69d34fcf89157188|42
```

84 chunk rows for one 68,939-byte document.

## Environment

Linux, `experiments` @ `8273331d`, catalog `~/.local/share/librarian/catalog.db`,
binary `target/release/codescout` (`version` reports `git_sha 8273331d`, `git_dirty true`).
Six sessions live in this checkout at 2026-09-16T21:41+03:00.

## Root cause

`src/librarian/indexer.rs` has **no symlink handling of any kind** — measured 2026-09-16:

```
$ grep -rn "symlink\|is_symlink\|follow_links\|file_type()" src/librarian/indexer.rs
(no output)
```

So this is absence, not a decision. The walk yields the symlink as a path; identity is
`id = sha256(abs_path)`, and the symlink's absolute path differs from its target's, so it
mints a second id for the same bytes. Every downstream per-artifact fact — chunk rows,
doctor findings, `find` hits — is duplicated with it.

## Evidence

### The served semantic index does NOT duplicate — and this is the half that hides it

Stated because it is the confirmation, not because it is the catch. If the duplicate were
visible in search, someone would have hit it long ago.

A 40-result `semantic_search` for a phrase that lives in this document ranks **`CLAUDE.md`
twice** (buffer lines 123 and 264) and `AGENTS.md` **zero times**, despite its 42 chunks
being byte-identical. So the duplication is real in the catalog and absent from the surface
people actually check.

### A control is what makes the vector reading a measurement

Joining `artifact_chunk` to `artifact_vec_v2_rowids` reports `vectorised = 0` for both rows
— and **also 0 for `docs/conventions/gate-ordering.md`, which demonstrably ranks 2nd, 6th
and 7th in that same search.** So the 0 is a fact about `artifact_vec_v2` (the legacy local
sqlite-vec store, 61,489 of 104,093 chunks) and says nothing about AGENTS.md. Every result
carries `"source": "stack"` — a different store, which the plain `sqlite3` CLI cannot open
(`no such module: vec0`).

Without that control the reading is "nothing is vectorised", which is flatly false.

## Hypotheses tried

1. **Hypothesis:** the two doctor rows are one file reported twice by a doctor bug.
   **Test:** `cmp AGENTS.md CLAUDE.md`; `stat -c '%i %h %n'`.
   **Verdict:** rejected — `cmp` reports identical, and `ls -l` shows `AGENTS.md` is a
   symlink. Two distinct inodes (190522004, 204497121), link count 1 each: a symlink, not a
   hardlink. Doctor is reporting correctly; the catalog genuinely holds two rows.

2. **Hypothesis:** the duplicate pollutes `semantic_search` result slots.
   **Test:** 40-result search on a phrase where `CLAUDE.md` demonstrably ranks.
   **Verdict:** rejected — see Evidence. The served index does not return the twin.

3. **Hypothesis:** already filed.
   **Test:** semantic `find` over `kind=bug`.
   **Verdict:** rejected — but the search was thinner than the verdict sounded. A plain
   `grep -rl symlink docs/issues/` surfaces a third relevant file (named in References) that a
   semantic query about *catalog duplication* does not reach, because it is about symlinks and
   not about duplication. The two nearest by semantic rank are both `fixed` and neither is this:
   `docs/issues/archive/2026-05-17-reindex-abs-path-unique.md` is a **loud** UNIQUE-constraint
   failure on `artifact.abs_path`; `docs/issues/archive/2026-06-13-linked-worktree-indexed-as-project-pollutes-catalog.md`
   is about worktrees. This one raises nothing.

## Fix

Not yet fixed. Direction, not yet chosen:

1. **Skip symlinks in the walk** whose resolved target is already inside the project root.
   **SHIPPED — see `## Fix provenance` below.** Cheapest, and matches what a reader expects.
   **The risk this bullet originally named did not survive contact with the repo, and the
   correction is recorded rather than quietly edited:** it read *"a symlink into a directory
   outside the project is a real document a user may want indexed, and this would silently
   drop it — trading a silent duplicate for a silent omission."* That is false of the
   predicate as written. `find . -type l` returns exactly one outward-pointing link in this
   repo (`.claude/skills/claude-traces` → `/home/marius/agents/llm-proxy/…`), and a predicate
   keyed on *resolves **inside*** does not touch it. The risk was real for a predicate keyed
   on *is a symlink* — which is not what was written — and the two were indistinguishable to
   me until I ran the check this file's own `## Resume` prescribed. **It is now the fixture
   that discriminates them**: mutating `starts_with(root)` to `true` turns the shipped
   predicate into the one this paragraph feared, and the test kills it.
2. **Canonicalise before hashing the id.** Makes identity follow the document rather than the
   path. Larger blast radius: `id = sha256(abs_path)` is relied on throughout, and changing
   what feeds it re-keys artifacts. **Canonicalisation asymmetry has already cost this repo
   once, in the opposite direction:** `docs/issues/archive/2026-09-06-the-unpushed-ledger-guard-goes-silent-on-a-symlinked-path.md`
   (`3a9eb5153e5311a8`, fixed) is a guard comparing an un-canonicalised `abs_path` against
   libgit2's canonicalised `workdir`; the `strip_prefix` returned `Err` and — because every
   failure path there allows — it reported *"no unpushed commits"* for a ledger that had them.
   So the two halves of this project already disagree about whether a path is canonical, and
   direction 2 picks a side rather than inventing a policy.
3. **Leave it and document it.** Defensible — the observable cost today is a duplicate `find`
   row and a doubled doctor line, both cosmetic.

Whichever lands, note the archived precedent: `2026-05-17-reindex-abs-path-unique.md`'s own
Reproduction names "symlink resolution" as a way to get "two artifact rows for the same
logical file". That bug fixed the *crash*; the duplicate row it describes is still reachable.

## Fix provenance

- **SHA:** `0c8ff65d` (`experiments`)
- **patch-id:** `ab4bc1c4efee0b25541f00b1f24f9ff21b055721`

Direction 1, at `src/librarian/indexer.rs`. Direction 2 (canonicalise the id) was **not**
taken: it re-keys every artifact, and the seam it touches is the one
`docs/issues/archive/2026-09-06-the-unpushed-ledger-guard-goes-silent-on-a-symlinked-path.md`
already shows this project disagreeing with itself about. Direction 3 (document it) is
superseded.

**The fix canonicalises BOTH sides**, and that is the half a reviewer should check rather
than assume. `abs_root` is whatever the caller held; `canonicalize()` always fully resolves.
On macOS `tempfile::tempdir()` returns `/var/…`, itself a symlink to `/private/var/…`, so
comparing a canonical target against a raw root makes `starts_with` false for **every** link
and the skip silently never fires — on the one platform CI runs and most sessions do not.
That is `3a9eb5153e5311a8` one layer up: same asymmetry, same silent-allow direction.
## Tests added

`librarian::indexer::tests::a_symlink_resolving_inside_the_root_is_not_indexed_as_a_second_artifact`
(`src/librarian/indexer.rs`). Unix-only — Windows needs elevation to create symlinks and the
walk has no platform branch here.

**The pre-fix RED is a mutation, stated because the test was written after the fix and an
unobserved red is not evidence.** Both sites mutated via `./scripts/mutation-probe.sh`, in an
isolated worktree, so no peer's `cargo test` ever saw an armed red:

| mutation | meaning | verdict |
|---|---|---|
| `starts_with(root)` → `true` | skip **every** symlink | KILLED — `left: 1, right: 2` |
| `path.is_symlink()` → `false` | guard never fires | KILLED — `left: 3, right: 2` |

Two-sided from one fixture, and the second reproduces the pre-fix behaviour exactly: three
artifacts for two documents.

**Why both links live in ONE case.** Split apart, the inside-resolving case alone does not
kill the first mutation — skipping every symlink still yields no `alias.md` row. That
mutation is the *expensive-direction* defect (silent omission), so a suite without the
outward fixture would have gone green on it. The outward link is the only thing that
discriminates `is_symlink()` from `is_symlink() && resolves_inside`.

Gate: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, read by **name** out of the default lane, with the
lean lane returning **0** tests for `librarian::indexer` — the documented vacuity, which is
why the default lane is the one read.
## Workarounds

`librarian(action="reindex")` does not help: the walk re-derives both rows. Deleting the
`AGENTS.md` row by hand would be undone by the next walk. If the duplicate is in the way,
remove the symlink; that costs non-Claude harnesses their instruction file.

## Resume

N/A — fixed, and the check this section previously named has been RUN. Result below, because it did not come out the way the happy branch expected.

`librarian(action="reindex")` on a binary containing `0c8ff65d` reports `removed: 0`, and the
`AGENTS.md` row **survives** with its 44 chunk rows. That is the branch this record's
`unverified:` field named in advance — *"a surviving row is a SEPARATE defect (stale row, no
live file), not this one"* — and it is now filed as
`docs/issues/2026-09-17-the-reclamation-predicate-follows-the-symlink-it-was-asked-about.md`:
the reclamation loop's `Path::exists()` traverses the symlink, so a row the walk deliberately
skips is never examined for removal.

**Two things a later reader should not take from that reindex.** It does **not** confirm this
fix fired: `CLAUDE.md` was unchanged, so both rows' stored hashes read the live content and
*"skipped"* and *"visited, unchanged"* predict identical catalog state. The evidence for this
fix is the regression test and its two killed mutations, nothing else. And it does **not** mean
this fix is incomplete — the sibling is a distinct predicate at a distinct call site with its own
repair, which is why it is a separate record rather than a reopening of this one.
## References

- `docs/manual/src/concepts/after-onboarding.md:77-79` — the manual prescribing `AGENTS.md`
- `docs/issues/archive/2026-05-17-reindex-abs-path-unique.md` — the loud twin, fixed
- `docs/issues/archive/2026-06-13-linked-worktree-indexed-as-project-pollutes-catalog.md`
- `docs/issues/archive/2026-09-06-the-unpushed-ledger-guard-goes-silent-on-a-symlinked-path.md`
  — a *different* defect (`cluster/guard-narrower-than-its-name`: a guard that silently allows
  when two path forms disagree), but the same seam, and the reason Fix direction 2 is not a
  free choice
