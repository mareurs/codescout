---
kind: bug
status: fixed
title: The cluster growth gate's refusal names a field, not a file, and the Index confirms the wrong one
tags:
- cluster/hint-composed-without-the-request
topic: shared-checkout gate correctness
closed: 2026-09-08
opened: 2026-09-08
owner: marius
related: []
severity: medium
---

# BUG: the cluster growth gate names a FIELD and calls its home "the ledger", which has been two files since the split

## Summary

`scripts/pre-commit-ledger-counts.py`'s growth check refuses with *"a class gained a member and
its `**Members:**` does not name it"*, then spends twenty lines explaining **why** the edit is
owed and **what shape** it takes. It never names the **file**. It says *"the ledger"* — a phrase
that stopped denoting one file when class definitions moved into `docs/trackers/issue-clusters/`,
one file per class.

The predicate is correct. The remedy text arrives, is read, and routes the author to
`docs/trackers/issue-clusters.md`, where every available signal says *append here* and the append
cannot work.

## Symptom (Effect)

Two refusals per author, and the second is byte-identical to the first — so the author has no
signal that the **file** was the error rather than the edit.

Two sessions paid it in one morning (2026-09-07). A third had already documented the *spelling*
half of the trap in the Index and it did not prevent either.

## Reproduction

File a bug carrying a `cluster/<slug>` tag whose class already exists, and commit.

1. The gate refuses, naming `**Members:**` and "the ledger".
2. `git grep 'cluster/<your-slug>' docs/trackers/issue-clusters.md` -> **0**.
3. That zero reads as *"this class does not exist here"*, not as *"wrong file"*, because the
   generic probe agrees the file is right:

```
git grep -c 'cluster/'                                docs/trackers/issue-clusters.md   ->  39
git grep -c 'cluster/selector-narrower-than-its-population'  ...same file               ->   0
```

4. Apply the remedy the Index itself documents under *"One slug, two spellings"* — the pattern
   `(?:cluster/)?<slug>` — and it returns **2** hits, neither of which is your class's field:

| line (HEAD) | what it is | why it does not work |
|---|---|---|
| 319 | your class's roster row | the roster carries no `**Members:**` field at all |
| 500 | a **real** `**Members:**` field naming your slug in its prose | it belongs to the `cluster/unclassified` block whose `**Slug:**` sits at 498 |

5. Append to the line-500 field — the only `**Members:**` in the file that mentions your slug —
   and the gate refuses again with the same text.

Measured at `HEAD` (`da93a823`), not the worktree: 22 class files under
`docs/trackers/issue-clusters/`, 22 carrying a `**Members:**`, and exactly 2 `**Slug:**` blocks in
`docs/trackers/issue-clusters.md` — `cluster/unclassified` and the template. Line numbers are cited
as a reading of that commit and will drift; the durable addresses are the headings.

## Root cause

`members_fields()` keys each `**Members:**` line by the nearest preceding `**Slug:**`:

```python
elif line.startswith("**Slug:**"):
    cur = backticked_cluster_slug(line[len("**Slug:**"):])
elif line.startswith("**Members:**") and cur is not None and cur in valid:
    out[cur] = line
```

`read_ledger()` concatenates the Index with every class file, so an append inside the Index's
`cluster/unclassified` block is keyed to `unclassified`. It cannot satisfy the check for the
author's slug, and it silently attaches their derivation to a class that is not theirs.

The composition defect is upstream of that. The message is assembled from the **violation's
shape** — which slug, which field — and never consults the **request**: which file the author
staged, or where that slug's definition actually lives. Both are known to the process. `valid` is
built by walking `docs/trackers/issue-clusters/`, so the gate holds the path it declines to print.

## Why the existing note does not cover this

`docs/trackers/issue-clusters.md` § *"One slug, two spellings — a `cluster/`-prefixed pattern
cannot see the Index"* is a good note and it is about a **different zero**. It fixes the spelling
mismatch; following it correctly still lands you in the wrong file, because the field the gate
wants is not in that file for any real class. The note ends *"The note you are reading is the
read-surface half the regex fix could not supply"* — and this bug is the read-surface half **it**
could not supply, one layer out.

## Correction owed to a published claim

`591785fc`'s commit message states *"the file CLAUDE.md cites for the closed set is the wrong one
for 21 of 22 classes."* **That sentence is false as written and this file supersedes it.**

- The count is wrong: 22 of 23, having counted the template as a class.
- The framing is wrong, which matters more. `CLAUDE.md` cites the Index **to choose a tag from the
  closed set**, and for that operation it is exactly right — the roster lists all 22 slugs. It is
  the wrong file for **authoring** a `**Slug:**`/`**Members:**` block, an operation `CLAUDE.md`
  never mentions. The Index is correct for reading the set and wrong for writing to it, and
  nothing marks the boundary.

The commit message cannot be amended without rewriting eight commits across three sessions on a
shared checkout, which is not this file's call to make. The correction lives here instead.

## Fix

**Both candidates shipped.** `experiments` `9852c474`, patch-id `47737e2383bfb0238740c09f0c996a1626ec18bf`.

**Gate green @ 2026-09-08 11:00–11:03** — fmt 0, clippy 0, LEAN 0, DEFAULT 0. The stamp is not
decoration: on a checkout with four live sessions a whole-tree gate result has a validity window
*shorter than the gate's own runtime*, so it describes a tree at a time rather than a property of
the change. An earlier run of this same gate reported `LEAN 101, DEFAULT 101` against a tree that
had already stopped existing — a peer's uncommitted compile error, fixed 20 seconds before the
commit body quoting it was written. (Point and derivation: sessionId `ad379a7c`.)

DEFAULT needed a second run. The first failed on
`peer::server::tests::run_exits_after_idle_timeout_with_no_connections` — the known open flake
`ee9d8d80ad5ecdc8`, in a subsystem this change does not touch — and cargo stops at the failing
binary, so `issue_clusters` did not run in that lane at all. The re-run is what that bug file's
own workaround prescribes, and it passed with both the flake and this fix's regression test green.

1. **The refusal names the path**, resolved per slug by `class_file_for`. It returns a path
   rather than an `Option`: falling back to the Index is the *second real case*, not a default —
   `cluster/unclassified` genuinely has no class file and its field genuinely is there — so
   there is no "unknown" outcome to represent, and a `None` would put the old silence back one
   layer down.
2. **The Index carries a banner** saying it is the roster, that definitions live per-class, and
   that a 0 from grepping your slug there means *wrong file* rather than *no such class*.

Both branches exercised end to end by staging a probe bug file and reading the real refusal.

## Tests added

None yet. The tractable guard is a **shape** assertion, not a prose pin: assert the growth
refusal contains a path under `docs/trackers/issue-clusters/`. It reds on exactly the deletion
that caused this and survives arbitrary rewording — the same shape `CLAUDE.md` § *Testing
Discipline* prescribes for a remedy that names two addressees.

**And the naive form of that assertion is already vacuous — measured, not predicted.** Running
the refusal live (2026-09-08, staging this very bug file without its derivation) shows the message
*does* end by naming a path: `docs/conventions/shared-checkout-commit-sequence.md`, for the commit
sequence. So `assert refusal contains a path` passes today with the routing defect fully present.
The assertion has to name the directory — a path under `docs/trackers/issue-clusters/` — or it
guards nothing.

That is `CLAUDE.md` § *Testing Discipline*'s arrival-versus-answerability ceiling one level lower
than it is stated there: a shape test for *"names a path"* is satisfied by the **wrong** path, so
it buys neither. Write the ceiling into the test's own comment; the fixture detail that a second,
unrelated path is present in the same message is load-bearing, and a tidy-up that drops it leaves
the assertion passing and no longer discriminating.

## Workarounds

The `**Members:**` field for `cluster/<slug>` is in `docs/trackers/issue-clusters/` — find it with
`git grep -l '<slug>' docs/trackers/issue-clusters/`, never by grepping the Index.

## References

- `scripts/pre-commit-ledger-counts.py` — `members_fields`, `read_ledger`, and the growth refusal.
- `docs/trackers/issue-clusters.md` § *One slug, two spellings* — the sibling note, and the
  spelling zero this one is not.
- `docs/trackers/issue-clusters/IC-22-hint-composed-without-the-request.md` — the class.
- `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md` — the other
  half of this gate's friction; index-vs-worktree, not routing.

## Attribution

The scope correction (`CLAUDE.md` is right for tag-selection; the split has no reader-facing
statement) and the shape-assertion remedy are sessionId `ad379a7c`'s, made against a draft of mine
that would have shipped the false claim above as the finding. The two-spellings measurement is
theirs as well, already landed in the Index. The two refusals, the 39-versus-0 confirming signal
and the line-500 wrong-field trap are mine.
