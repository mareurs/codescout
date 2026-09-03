---
kind: bug
status: wontfix
tags:
- cluster/unclassified
- librarian
- catalog
- trackers
- entry-ids
closed: null
opened: 2026-09-03
owner: marius
related: []
severity: low
unverified: Not established that the burn is unintended rather than by-design; no maintainer ruling. Reproduced once, from a real accident, not from a minimal script.
---

# BUG: reverting a guarded ledger burns an entry id, and the success response never says so

## Summary

**Resolved `wontfix` on the day it was filed: the burn is deliberate and the source
says so.** Filed on the reasonable belief that a silently skipped entry id was a
defect; reading the allocator showed it is an accepted, documented trade-off, and
that the silence is reasoned rather than absent.

The mechanism is real and unchanged. `doc(action="append_entry")` allocates from
`max(body_max, reserved_max, frontmatter_max) + 1`, and the reservation lives in the
**catalog** while the entry lives in the **file**. A `git checkout --` reverts the file
and cannot reach the catalog, so the next allocation skips the orphaned id. Observed on
`docs/trackers/resume-get-guide-section-grain-phases-2-3.md`, which runs `GG-9 → GG-11`
with no `GG-10`.

What did not survive contact with the source is the *defect* claim. Kept as a record
because the investigation locates where this decision lives, and because the ledger now
has a visible gap someone will otherwise re-investigate.

## Symptom (Effect)

First allocation succeeded:

```
{"id": "GG-10", "section_written": true,
 "body_max": 9, "reserved_max": 9, "frontmatter_max": 9}
```

The file was then reverted with `git checkout -- <ledger>` (the section and the
frontmatter bump were both uncommitted, so both went). Re-appending the identical
body returned:

```
{"id": "GG-11", "section_written": true,
 "body_max": 9, "reserved_max": 10, "frontmatter_max": 9}
```

No error, no warning, no `warning:` field. `GG-10` is now permanently
unallocatable and undefined.

**The whole signal is `reserved_max: 10` against `body_max: 9` and
`frontmatter_max: 9`.** A caller has to notice three sibling integers disagree,
and know which of them the allocator reads, to learn that an id was burned.

## Reproduction

Commit `3339e26c` (branch `experiments`), against any ledger with a declared
`entry_prefix`:

1. `doc(action="append_entry", id=<ledger>, id_prefix=<P>, anchor_heading=…, title=…, body=…)`
   → returns `P-N`, writes the section, bumps `entry_high_water_<P>` in frontmatter.
2. **Do not commit.**
3. `git checkout -- <ledger path>` — section and frontmatter bump both revert.
4. Repeat step 1 with identical arguments → returns **`P-(N+1)`**, and
   `reserved_max` is `N` while `body_max` and `frontmatter_max` are `N-1`.

`P-N` is now defined nowhere and allocatable nowhere.

## Environment

codescout MCP, `experiments` @ `3339e26c`, Linux, shared checkout with 3 git
worktrees and several concurrent sessions. Catalog at
`~/.local/share/librarian/catalog.db` (machine-local, gitignored).

## Root cause

The allocation high-water is stored **twice, in stores with different
revertibility**:

- `entry_high_water_<P>` in the file's YAML frontmatter — in git, so `git
  checkout` restores it.
- the reservation high-water in the catalog — **not in git**
  (`docs/conventions/cross-machine-catalog-resume.md` establishes the catalog is
  machine-local and gitignored), so no git operation can restore it.

`append_entry` allocates from `max(reserved_max, …)`, which is **correct on its
own terms**: an id that may already have been observed by another host must never
be reissued, and a reverted working file is not evidence that nobody saw it. The
guard that refused this same call twice earlier today (*"this ledger has commits
that are not on its upstream branch"*) exists for exactly that collision.

So the mechanism is not "the allocator is wrong". It is that **a git-level undo
covers one of the two stores, and the operation reports success without naming
the half it could not undo.**

*Measured 2026-09-03: the two `append_entry` responses quoted verbatim above, from this
session, six minutes apart on the same ledger. **Mechanism since confirmed at source** —
`src/librarian/catalog/augmentation.rs:1033` (the three-way `max`) and `:1021` (the
`entry_reservation` row the catalog holds); the arithmetic reproduces the observed ids
exactly. This paragraph previously said the mechanism was read from response fields and
not from source; that caveat is discharged, and reading the source is also what
overturned the file's verdict — see* Hypotheses tried *#3 and #4.*

## Evidence

### The three fields, across the two calls

| field | call 1 (`GG-10`) | call 2 (`GG-11`) |
|---|---|---|
| `body_max` | 9 | 9 |
| `reserved_max` | 9 | **10** |
| `frontmatter_max` | 9 | 9 |

`reserved_max` is the only one that survived the revert, and it is the one the
allocator reads.

### The resulting ledger

```
$ grep -c "^## GG-" docs/trackers/resume-get-guide-section-grain-phases-2-3.md
11
$ grep -n "GG-10" docs/trackers/resume-get-guide-section-grain-phases-2-3.md
(no match outside the History note explaining the gap)
$ grep -n entry_high_water_GG docs/trackers/…
14:entry_high_water_GG: 11
```

## Hypotheses tried

1. **The revert also cleared the catalog reservation, and `GG-11` is a
   re-allocation bug.** — *Test:* compare `reserved_max` across the two responses.
   *Verdict:* **rejected.** `reserved_max` went 9 → 10; the reservation persisted
   exactly as designed.

2. **The allocator reads `reserved_max`.** — *Test:* originally inferred from the
   returned id. **Now confirmed at source**, `src/librarian/catalog/augmentation.rs:1033`:

   ```rust
   let next = body_max
       .map_or(0, |m| m + 1)
       .max(reserved_max.map_or(0, |m| m + 1))
       .max(frontmatter_max.map_or(0, |m| m + 1))
       .max(1);
   ```

   with the comment *"Max of all THREE, so no single input can walk the counter
   backwards … each source is unreliable in a different way, and none of them is ever
   wrong in the high direction."* *Verdict:* **confirmed.** Reproduces the observation
   arithmetically: call 2 saw `reserved_max` 10, `body_max` 9, `frontmatter_max` 9, so
   `next = max(10, 11, 10, 1) = 11`.

3. **This is by design and not a defect.** — *Test:* read the allocator's own doc
   comments. *Verdict:* **confirmed, and it closes the record.**
   `augmentation.rs:932` states it outright:

   > *A reserved-but-never-written id leaks an integer. Deliberate: integers are cheap,
   > and every ledger convention in this repo already forbids reuse.*

   and `:843` calls the same thing *"the already-accepted failure mode … a crash before
   the write leaks an integer, which every ledger convention here tolerates."*

4. **Even if the burn is fine, the silence is a defect — no response field names it.**
   — *Test:* look for an existing divergence-reporting path. *Verdict:* **rejected, and
   this is the one I expected to survive.** `append_entry.rs:247` already carries a
   `compaction_note`, and its lead comment is explicit that the omission is a choice:

   > *Which input governed is the diagnostic the caller could not see. **Only one
   > relation earns words**: the committed mark leading BOTH the live body and this
   > machine's reservation table …*

   It further reasons about the register — the note is deliberately **not** under
   `warning:`, because that means *"off-golden-path, reconsider before proceeding"* and
   *"tagging it would train agents to repair it."* That argument applies to a burned id
   with more force, not less: it **cannot** be repaired, so a warning would train agents
   to attempt a repair that does not exist. The silence is the considered position, and
   my proposed `warning:` field would have been a regression.

## Fix

**None, and none is wanted.** Status is `wontfix`, and the justification is hypotheses
3 and 4 above: the burn is an accepted trade-off documented at
`src/librarian/catalog/augmentation.rs:932`, and the absence of a response warning is
reasoned at `src/librarian/tools/append_entry.rs:236-247` under
`PROGRESSIVE_DISCOVERABILITY` Pattern 5a.

The `warning:` field this file originally proposed is **withdrawn**. It would have
tagged a state that is correct and unrepairable, which is the exact failure the existing
comment argues against.

**One residual, stated rather than fixed.** The accepted-leak notes describe
*reserve-then-never-write* (and *reserve-then-crash*). This case is
*write-then-revert* — the same end state (a catalog reservation the body does not
claim) reached by a path those comments do not name literally. Nothing suggests the
authors would rule differently, and the tolerance argument transfers intact; but the
verdict here is read from doc comments, not given by a maintainer, which is what the
`unverified:` field records.

## Tests added

**None, and none is owed** — the record closes `wontfix`, so there is no behaviour to
pin. The regression test this file originally sketched (a two-call fixture around a
revert) would have asserted a `warning:` field that hypothesis 4 withdrew; writing it
would have frozen the wrong verdict.

What already guards the surrounding behaviour, found while confirming hypothesis 2:
`append_entry.rs:860-888` asserts `reserved_max` and `frontmatter_max` are **present
even when null** — *"or absent reads as zero"* — and `augmentation.rs:2809-2812` pins
that a reservation survives a read (`R-42` then `R-43`).

## Workarounds

**Do not `git checkout --` / `git restore` a ledger with a declared
`entry_prefix` while it holds an uncommitted `append_entry`.** Commit the entry
first, or edit through `doc(action="update", patch={body_edits: […]})`, which
does not touch reservations.

If an id is already burned, it cannot be recovered — **record the gap where a
reader will hit it** rather than leaving a silent hole in the sequence. Done here
in the ledger's `## History` section.

## Resume

N/A — closed `wontfix`. Nothing is owed.

If someone reopens this, the question is **not** "should the id be recoverable" (it
should not) but the narrow residual above: whether *write-then-revert* deserves naming
alongside *reserve-then-crash* in `augmentation.rs`'s accepted-failure note. That is a
comment edit, not a behaviour change.

## Class — why `cluster/unclassified`

**Read this with the `wontfix` verdict in hand: the tag records where the finding was
filed, not a live defect instance.** It is left in place because the ledger's
`**Members:**` field names it and the pair must stay consistent, and because the
derivation below is the useful part — but nobody should count this toward a promotion
threshold. `cluster/unclassified` drives no threshold, so the distortion is nil.

The escape hatch's admission conditions (`issue-clusters.md`, adjudicated 2026-09-03 in
`efbe6a46`) are (a) name the classes checked and why each fails, and (b) name a
candidate class for the second instance. Both below.

**(a) Checked against all 22, closest first — each fails on its own claim:**

| class | why it does not hold |
|---|---|
| `IC-21` instrument omits the dimension that grows | closest near-miss: the response *does* report three counts. But the missing thing is the **meaning** of their disagreement, not a magnitude the instrument declined to measure — and hypothesis 4 later showed the meaning is withheld on purpose. |
| `IC-12` transient shared state lies to every reader | the catalog is neither transient nor lying — it is durable and **correct**. Nothing it reports is false. |
| `IC-8` a record asserts a completed action nothing re-checked | the record asserts a completed allocation, and the allocation *did* complete. No unchecked claim. |
| `IC-14` a guard's coverage is narrower than its name | the reservation guard does exactly what its name says, across exactly its intended scope. |
| `IC-17` a shared resource carries no owner | the catalog is shared, but ownership is not what fails here; a single-user, single-session run reproduces it. |
| `IC-1` blast radius exceeds visibility | about **peers** you cannot enumerate. The unseen party here is a *store*, not a session. |
| `IC-13` / `IC-19` / `IC-20` capped, windowed, floor-as-total | nothing is capped, windowed or truncated. |
| `IC-11` doc contradicted by code | the reverse, in fact: the code's own comments **state** the behaviour, and it was this file that contradicted them. |
| `IC-15` accepted parameter silently dropped | no parameter is dropped — the burn is in stored state, not in the call's arguments. |
| `IC-3`, `IC-9`, `IC-16`, `IC-22` | not about declaration/wiring, assertions, or hint composition. |

**(b) Candidate class for a second instance:** *an undo covers one of two stores that
jointly hold one fact* — provisional slug `partial-undo-across-split-stores`. The shape
to look for is any pair where one half is in git and the other is in the catalog,
`usage.db`, or a cache, and a `git checkout`/`restore`/`reset` is treated as a full
revert. **BL-48 is the same split observed in the other direction** (a raw frontmatter
edit reaching the file and not the catalog). Note that this class would be about the
*surprise*, not about a defect — as here, the split may be deliberate on both sides.

## References

- `docs/trackers/resume-get-guide-section-grain-phases-2-3.md` § *History*
  (2026-09-03) — the reader-facing note that `GG-10` will never exist.
- `docs/conventions/cross-machine-catalog-resume.md` — establishes the catalog as
  machine-local and gitignored, which is the asymmetry this bug rests on.
- `CLAUDE.md` § *Session Intelligence Trackers* — BL-48, the same catalog/file
  split observed in the opposite direction (a raw frontmatter edit not reaching
  the catalog).
