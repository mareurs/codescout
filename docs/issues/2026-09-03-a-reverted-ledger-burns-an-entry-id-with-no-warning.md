---
kind: bug
status: open
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

`doc(action="append_entry")` allocates from a high-water mark held in the
**catalog**, while the entry it writes lives in the **file**. A `git checkout --`
on the ledger reverts the file and cannot revert the catalog, so the next
allocation skips the orphaned id. The call succeeds, returns a valid new id, and
names the skip nowhere — the only signal is three numeric fields in the response
disagreeing with each other.

Observed on `docs/trackers/resume-get-guide-section-grain-phases-2-3.md`, which
now runs `GG-9 → GG-11` with **no `GG-10` and no way to mint one**.

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

*Measured 2026-09-03: the two `append_entry` responses quoted verbatim above, from
this session, six minutes apart on the same ledger. Mechanism read from the
response fields, **not** from the allocator source — see* Hypotheses tried *#2.*

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
   *Verdict:* **rejected.** `reserved_max` went 9 → 10, i.e. the reservation
   persisted exactly as designed.
2. **The allocator reads `frontmatter_max`.** — *Test:* `frontmatter_max` was 9 on
   both calls; had it been the source, call 2 would have returned `GG-10` again.
   *Verdict:* **rejected** by the returned id. **Not confirmed against the
   allocator source** — this is inference from observed behaviour, and the source
   read is the obvious next step.
3. **This is by design and not a defect.** — *Verdict:* **deferred, and it may
   well be.** Burning the id is the safe branch. What is being filed is the
   **silence**, not the burn: see *Fix*.

## Fix

Not attempted. If adjudicated a defect, the smallest honest change is a
**`warning:` field on the response** when `reserved_max > max(body_max,
frontmatter_max)` at allocation time — naming the burned id and why it cannot be
reissued. The information is already computed and already returned; what is
missing is the sentence that says what it means.

That keeps the safe allocation behaviour untouched and only adds the name of the
state, which is the remedy `CLAUDE.md` § *Testing Discipline* prescribes for
exactly this shape: *where a system already names its own failure state, assert on
the name, not on a proxy for it* — here the system has the state and declines to
name it, so callers are left asserting on a proxy (three integers) or, in
practice, on nothing.

## Tests added

None. The behaviour is not adjudicated as a defect yet, and a regression test
would pin a design decision nobody has made. If the `warning:` field lands, the
test is a two-call fixture around a revert.

## Workarounds

**Do not `git checkout --` / `git restore` a ledger with a declared
`entry_prefix` while it holds an uncommitted `append_entry`.** Commit the entry
first, or edit through `doc(action="update", patch={body_edits: […]})`, which
does not touch reservations.

If an id is already burned, it cannot be recovered — **record the gap where a
reader will hit it** rather than leaving a silent hole in the sequence. Done here
in the ledger's `## History` section.

## Resume

Read the allocator to confirm hypothesis 2 against source rather than behaviour:
find the `append_entry` id-allocation path (grep `reserved_max` under
`src/librarian/`), confirm it maxes over the catalog reservation, and check
whether any caller already surfaces a divergence warning. Then take the
by-design-or-not ruling to the maintainer — the answer decides whether *Fix*
above is worth writing.

## Class — why `cluster/unclassified`

The escape hatch's admission conditions (`issue-clusters.md`, adjudicated
2026-09-03 in `efbe6a46`) are (a) name the classes checked and why each fails,
and (b) name a candidate class for the second instance. Both below.

**(a) Checked against all 22, closest first — each fails on its own claim:**

| class | why it does not hold |
|---|---|
| `IC-21` instrument omits the dimension that grows | closest near-miss: the response *does* report three counts. But the missing thing is the **meaning** of their disagreement, not a magnitude the instrument declined to measure. |
| `IC-12` transient shared state lies to every reader | the catalog is neither transient nor lying — it is durable and **correct**. Nothing it reports is false. |
| `IC-8` a record asserts a completed action nothing re-checked | the record asserts a completed allocation, and the allocation *did* complete. No unchecked claim. |
| `IC-14` a guard's coverage is narrower than its name | the reservation guard does exactly what its name says, across exactly its intended scope. |
| `IC-17` a shared resource carries no owner | the catalog is shared, but ownership is not what fails here; a single-user, single-session run reproduces it. |
| `IC-1` blast radius exceeds visibility | about **peers** you cannot enumerate. The unseen party here is a *store*, not a session. |
| `IC-13` / `IC-19` / `IC-20` capped, windowed, floor-as-total | nothing is capped, windowed or truncated. |
| `IC-11` doc contradicted by code | no doc claims the id would be recoverable; nothing is contradicted. |
| `IC-15` accepted parameter silently dropped | no parameter is dropped — the burn is in stored state, not in the call's arguments. |
| `IC-3`, `IC-9`, `IC-16`, `IC-22` | not about declaration/wiring, assertions, or hint composition. |

**(b) Candidate class for a second instance:** *an undo covers one of two stores
that jointly hold one fact* — provisional slug
`partial-undo-across-split-stores`. The shape to look for is any pair where one
half is in git and the other is in the catalog, `usage.db`, or a cache, and a
`git checkout`/`restore`/`reset` is treated as a full revert. **BL-48 is the same
split observed in the other direction** (a raw frontmatter edit reaching the file
and not the catalog), which is what makes a second instance plausible rather than
hypothetical — but BL-48 is not itself an instance of *this* claim, because
nothing there is undone.

## References

- `docs/trackers/resume-get-guide-section-grain-phases-2-3.md` § *History*
  (2026-09-03) — the reader-facing note that `GG-10` will never exist.
- `docs/conventions/cross-machine-catalog-resume.md` — establishes the catalog as
  machine-local and gitignored, which is the asymmetry this bug rests on.
- `CLAUDE.md` § *Session Intelligence Trackers* — BL-48, the same catalog/file
  split observed in the opposite direction (a raw frontmatter edit not reaching
  the catalog).
