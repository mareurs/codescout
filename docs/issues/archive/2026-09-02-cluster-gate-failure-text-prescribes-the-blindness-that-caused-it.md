---
status: fixed
opened: 2026-09-02
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md
  - docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md
tags:
  - cluster/hint-composed-without-the-request
kind: bug
---

# BUG: the cluster gate's failure text prescribes the re-derivation that reproduces the blindness which caused the failure, and names one side of a two-file fix

## Summary

`tests/issue_clusters.rs`'s count-gate panic message tells the reader to *"Re-derive rather than
adjust by the delta"* and hands them a `git grep -clE` command. In the situation that actually
produces the red on a shared checkout — a peer's half-finished archive move — that command
reproduces the exact worktree/index blindness that caused the red, and returns a plausible wrong
number rather than an error. The message is correct about the comparison it can see and blind to
the caller's situation.

A second, independent defect on the same surface: the message names one side of the comparison and
never discloses that the repair is a **two-file atomic edit**, whose other half may be unavailable.

**This class currently has n=2 and, by its own § *Promotes to*, zero instances reproducible at
`HEAD`. This one reproduces at `HEAD` today.**

## Symptom (Effect)

Verbatim, from a real failure on 2026-09-02 at HEAD `09c68634`:

```
thread 'every_index_count_matches_the_corpus' panicked at tests/issue_clusters.rs:1144:5:
the Index table's `n` column disagrees with the corpus:
  cluster/gate-keyed-on-unobservable-event — table says 23, corpus has 22
  cluster/hint-composed-without-the-request — table says 2, corpus has 1
  cluster/repro-env-diverges-from-gate-env — table says 13, corpus has 12

Re-derive rather than adjust by the delta — per slug:
    git grep -clE '^[[:space:]]*-[[:space:]]*cluster/<slug>[[:space:]]*$' -- 'docs/issues/*.md' | wc -l
```

No count had drifted. A peer session held three bug files mid-archive-move — deleted from
`docs/issues/`, an untracked copy in `docs/issues/archive/` — and the three classes short by one
were exactly the three `cluster/` tags on those three files.

Following the prescribed command returns `11`, `1`, `12` — agreeing with the panic, confirming the
"drift", and pointing at the ledger as the thing to edit. Editing the ledger to match would have
written three false counts that go stale the moment the peer stages.

## Reproduction

At HEAD `09c68634`, with three bug files mid-archive-move in the working tree:

```
git status --short docs/issues/
#  D docs/issues/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md
#  D docs/issues/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
#  D docs/issues/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md
# ?? docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md
# ?? docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
# ?? docs/issues/archive/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md

cargo test --test issue_clusters     # 2 failed, message as above
```

Then follow the message's own instruction for any of the three slugs — it agrees with the panic.
Then read the tags on the three moved files: the correspondence is exact, one file per short class.
The red cleared with no ledger edit when the peer committed at `7d2b3ee7`; the gate is 18/18 green
at `7d2b3ee7` with the ledger byte-identical.

## Environment

Shared checkout, 7 concurrent sessions across 3 profiles. Not reproducible on a single-session
checkout, which is the point: the message is correct whenever it is not needed.

## Root cause

Two defects on one surface, `tests/issue_clusters.rs`'s panic message.

**1 — the prescribed re-derivation shares the failing instrument's blind spot.**
`git grep` searches *tracked files in the working tree*. A file that is tracked but deleted from
the worktree is unreadable, so it is counted by neither path; its untracked copy at the new location
is not tracked, so it is counted by neither path either. The file falls out of the corpus entirely
for the duration of the move. The gate's own corpus derivation has this property — that is
`docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md`, already
filed — and the **remediation instruction hands the reader the same instrument**, so a faithful
re-derivation cannot distinguish "the ledger drifted" from "a peer is mid-move".

The instruction is not merely unhelpful; it is wrong *only* in the circumstance that triggers it.
`git status --short` is what identifies the real cause, and the message does not mention it.

**2 — the message names one side of a two-file atomic edit.**
The gate compares an Index-table cell against a corpus count. Adding a bug file moves the corpus;
only a ledger edit moves the cell. Both must land together or the gate is red in one direction or
the other. The message names the ledger and never says the corpus side exists, so a reader whose
ledger edit is blocked — contended cell, another session mid-flight, a `git add` they are not ready
to make — has no legal move and no text telling them why.

*Measured 2026-09-02, live:* this session filed
`docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md`, a genuine 12th
`cluster/doc-contradicted-by-code` member. `git add` alone reds **two** tests —
`every_index_count_matches_the_corpus` on `issue-clusters.md:270` and
`every_bare_n_in_a_class_field_matches_the_corpus` on the `**Members:**` bare `n` at `:872`. Both
compensating edits are in the one file. The only self-consistent state is to leave the file
untracked and the cell at 11 — i.e. **the correct action is to make the new evidence invisible**,
and no gate says so.

## Evidence

### The three-class correspondence

| moved file | its `cluster/` tag | class short by 1 |
|---|---|---|
| `2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md` | `gate-keyed-on-unobservable-event` | 23 → 22 |
| `2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md` | `hint-composed-without-the-request` | 2 → 1 |
| `2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md` | `repro-env-diverges-from-gate-env` | 13 → 12 |

Three classes, three files, one tag each. Nothing drifted.

### The blocked pair, derived at the bytes

```
tracked corpus  (git grep -clE, doc-contradicted-by-code)   11
worktree corpus (grep -rlE,     same anchor)                12
IC-11 Index cell,     issue-clusters.md:270                 11
IC-11 **Members:** n, issue-clusters.md:872                 11
```

Independently derived by a second session (`codescout-ca`) and agreeing.

### The blocked state has more than one holder — observed 2026-09-02 05:20, HEAD `e20b794f`

`docs/issues/` held **three** untracked bug files at once, from at least two sessions, in three
different classes:

| file | tag | holder |
|---|---|---|
| `2026-09-02-memory-description-omits-the-refresh-anchors-action.md` | `doc-contradicted-by-code` | this session |
| `2026-09-02-cluster-gate-failure-text-prescribes-the-blindness-that-caused-it.md` | `hint-composed-without-the-request` | this session |
| `2026-09-02-tracked-only-staging-commits-half-an-archive-move.md` | `selector-narrower-than-its-population` | another session, in flight |

Each is a real member invisible to `git grep`, so each is an uncounted instance whose cell bump is
blocked by the same unsplittable ledger. The third arrived from a session this one has never
exchanged a message with, in a class it is not working on — so the state described in § *Root cause*
defect 2 is not one author's unlucky sequencing. It is what the two gates plus a file-wide
constraint produce whenever more than one session files a bug, which on this checkout is most
evenings.

*(The third file is another session's in-flight work, named as observed at a timestamp; it may be
renamed, retagged or abandoned. The count of holders is the durable part, not the filename.)*

### A fourth session hit the mechanism with no contact with this one — and the mechanical half

`codescout-05` (PID 101396) met this exact state while shipping `655c0b6f`, independently, and
resolved it by **dropping its bug file from the commit and documenting the exclusion** — the same
route this session took, reached without any contact between us. Relayed via `codescout-0a`; the
session had exchanged no messages with this one at any point.

That matters more than the three-holder table above. Those three were reachable from one
conversation; this one was not, so it is not explained by a shared frame, a relayed belief, or one
work stream's sequencing. Four sessions, four independent arrivals at "make the new evidence
invisible", which is what the defect prescribes when followed correctly.

**And it supplies the mechanism this file previously had only as a symptom.** The two instruments
read different worlds *at the same instant*:

| instrument | reads |
|---|---|
| `scripts/pre-commit-ledger-counts.py` (the hook that refuses the commit) | the **git index** |
| `cargo test --test issue_clusters` (the gate the author runs to check) | the **working tree** |

So staging the bug file refuses the commit **while the test reads green on the same tree, in the
same second**. The author's own verification step cannot see the thing blocking them, and the two
outputs are not in conflict — they are correct answers about different worlds. That is why the pair
is *atomic* rather than merely awkward, and why neither failure text can name the other's
condition. Identified by `codescout-0a`; recorded here with attribution.

*Consequence for § Fix:* item 3's proposed `git status --porcelain` check is necessary but not
sufficient — the gate would also have to say **which world it read**, because a reader holding a
green test and a refused commit currently has no way to learn that those are compatible.

## Hypotheses tried

1. **Hypothesis:** the ledger counts had genuinely drifted and needed re-deriving, as the message
   says.
   **Test:** read `git status --short docs/issues/`; map the three short classes to the tags of the
   three files a peer held mid-move.
   **Verdict:** rejected — exact 1:1 correspondence, and the red cleared at `7d2b3ee7` with the
   ledger unedited.

2. **Hypothesis:** this is a duplicate of the already-filed worktree-vs-index bug.
   **Test:** read that file's § *Summary*, § *Root cause* and § *Fix*. It is about the gate's
   **corpus derivation** reading two worlds, and its fix is `git show :<path>` / `git grep --cached`.
   Neither its root cause nor its fix touches the panic message, and repairing it would leave both
   defects here standing — the re-derivation command is a separate string, and the two-file
   atomicity is not a read-consistency question at all.
   **Verdict:** rejected — same root mechanism for defect 1, different artifact and different fix.
   Cited as `related`, not merged.

3. **Hypothesis:** defect 2 is caused by a standing agreement among peer sessions not to write
   `issue-clusters.md`, so it is a social problem rather than a defect in the gate.
   **Test:** asked, then asked again when two accounts conflicted. `codescout-0a` states no such
   agreement exists that it is party to; `codescout-ca` holds two unsolicited primary messages,
   quoted verbatim — `codescout-5e`: *"Nobody should commit that path right now, including me. I
   cannot isolate my half — interactive staging is unavailable in this harness — so it waits for the
   other writers to land or withdraw."* and `codescout-20`: *"issue-clusters.md is shared — please
   leave it … There is no way to split that file by hunk safely."*
   **Verdict:** rejected as a cause — but the *reason* is the opposite of what this entry first
   recorded, and the correction matters more than the verdict.

   *(Corrected 2026-09-02. This hypothesis originally read that the freeze "appears to be a phantom
   — sessions each believing one exists because each was told the others agreed". That is false.
   The restriction is real, two-deep rather than circular, and independently arrived at by two
   sessions in their own words. What was wrong was a **scope word**: it was relayed as an agreement
   among "all of us" when the set is three named sessions, and `codescout-0a` — never asked, never
   agreed — was outside it. Both accounts were true as written. A quantifier over an unenumerated
   population is the same defect as a count without its unit, and this is its third instance in one
   session.)*

   The real mechanism is better evidence for defect 2 than the phantom story was.
   `issue-clusters.md` holds 22 independent class records in one file with **no per-hunk commit
   path** — `codescout-5e` states it cannot isolate its own half — so any session with unrelated
   in-flight edits anywhere in that file blocks every other session's cell edit, whatever the cell.
   The contention is therefore file-wide by construction, not by agreement, and the agreement is
   only three sessions correctly describing that constraint to each other.

   This is still not a *cause* of the gate defect: the gate would name one side of a two-file fix
   regardless. It is what makes the defect bite. A failure message that disclosed the pair, and
   which file's cell was owed, would let a reader see that the second half is contended and by whom
   — instead of discovering it by asking three sessions and receiving two conflicting accounts.

## Fix

**Fixed.** Defect 1 at `1b3ac36b` (patch-id `8ff32896b7df6875f770cc7353334d184e6b1bf2`), defect 2
at `aae67302` (patch-id `e17c8da86c4960aefe394b0c8ce93aa5897c60c9`).

**Defect 1 closed structurally, and not by the fix planned below.** The plan was to improve the
message. What happened instead is that the count comparison the message belonged to no longer
exists: `1b3ac36b` stopped the ledger storing derived counts, so there is no `n` cell to disagree
with a corpus and nothing to re-derive. The surviving check reads the **index** (`git show :path`),
never the worktree — so a file tracked but deleted from the worktree is read normally, and the
peer-mid-move blindness is unreachable rather than merely warned about. That is this file's own
option 3 ("have the gate *detect* condition 1 itself… convert a false alarm into a statement"),
reached from a different direction.

**Reproduced before believing any of it**, per CLAUDE.md. Throwaway clone, three tracked bug files
deleted from `docs/issues/` with untracked copies in `archive/` — this file's exact scenario:

| state | old gate | new gates |
|---|---|---|
| peer mid-move, nothing else | **false red**, 3 classes "drifted" | cargo 17/17, hook exit 0 (index *and* worktree) |
| mid-move + a planted stored count | — | **REFUSED** |
| mid-move + a real new member, no ledger edit | — | **REFUSED**, naming the stem |

The last two rows are why the first is evidence: silent on the transient state *and* loud on real
defects, simultaneously. A structurally blind gate produces the first row alone.

**Defect 2 needed a real fix and got one.** *"The message names one side of a two-file atomic
edit"* survived the inversion in a new form — a class gaining a member still couples the bug file
to the ledger. `aae67302` makes the refusal name both sides, state that a contended ledger is now
a **wait** rather than a re-derivation (the edit is a one-line append carrying no number, so no
peer's commit can invalidate it), forecloses the false mid-move diagnosis by name, and — the part
this file specifically asked for — **says the lossy escape out loud**: an untracked bug file is
invisible to `git ls-files` and the gate will pass, with nothing to remind you the evidence is off
the corpus. The complaint was that the only self-consistent state was to make new evidence
invisible and no gate said so.

**One thing this file predicted that did not survive contact.** It notes the class had "zero
instances reproducible at `HEAD`" and that this one did. Closing it does not change any count —
under the new design there are no stored counts, and `the_count_scan_reaches_the_archive` means an
archived file still counts. Archiving is now free, which is a small consequence of `1b3ac36b`
worth stating where a future reader of this class will look.
## Tests added

None yet. Owed: a test that the panic message names `git status` before it names `git grep`, and one
that it names both files of the pair. Both are string assertions on the message, which is the
artifact under test — per `CLAUDE.md` § *Testing Discipline*, note these are **existence** assertions
and monotone under widening, so verify them by mutating the message the other way (removing the
`git status` clause) rather than only by adding it.

## Workarounds

On a red count gate, run `git status --short docs/issues/` first. If any bug file shows `D` or `??`,
the count is transient peer state — wait, do not edit the ledger. If adding a bug file, either land
the file and both ledger edits in one commit, or leave the file untracked and the cell unchanged;
there is no valid intermediate.

## Notes on classification

Tagged `cluster/hint-composed-without-the-request`. It fits the claim — the message is right about
the comparison it can see and blind to the caller's situation — and it is **not** caught by that
class's *Falsified by* clause, which excludes a hint that is simply stale or mistyped: this command
is correctly derived from the gate's own model of the corpus.

Two things the reader should decide rather than inherit:

- **This is the class's third filed member, so it crosses the ≥3 file bar** that IC-22's
  § *Promotes to* names — and by recurrence, not by the re-partitioning that section explicitly
  declined. It is also the only member reproducible at `HEAD`. The promotion judgement is the
  ledger's, not this file's; recorded here so it is not made by accident.
- **Grain.** IC-22's claim says *request*; here the hint is blind to the caller's **tree state**
  rather than to their arguments. That is the same widening its § *Promotes to* already flags as
  unresolved for member 2. Flagged rather than silently widened.

## Resume

Edit the two panic messages in `tests/issue_clusters.rs` (~`:941`, ~`:1144`) per § *Fix* 1–2; decide
3 separately. Do not touch `docs/trackers/issue-clusters.md` counts as part of this — this file is
untracked at time of writing precisely to avoid moving IC-22's count, and staging it requires the
paired cell bump described in § *Root cause* defect 2.

## References

- `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md` — the
  corpus-derivation half; same mechanism, different artifact, different fix.
- `docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md` — the live blocked
  pair measured in § *Evidence*.
- `docs/trackers/issue-clusters.md` `IC-22`, and `IC-11`'s `**Members:**` field at `:872`, which
  documents this blindness catching an earlier author.
