---
status: open
opened: 2026-08-16
closed:
severity: high
owner: marius
related: []
tags: [trackers, data-integrity, id-allocation, reconnaissance, citations]
kind: bug
---

# BUG: `reconnaissance-patterns.md` reuses 10 of its 91 ids for unrelated lessons, so 57 kin-citations are ambiguous

## Summary

The R-N ledger contains **153 entry instances under 91 distinct ids**. Ten ids
carry more than one *different* lesson — not a summary and its body, but two
unrelated laws recorded on different dates under the same number. The ledger's
entries cite each other 57 times, and a citation like "kin R-57" now resolves to
two different rules depending on which instance the reader lands on.

This is the collision predicted in
`docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md`
(BL-30), which argued that hand-allocated ids race when two sessions grep for the
next free number. It is no longer a prediction.

## Symptom (Effect)

Measured 2026-08-16 at `3b6b303f`:

```
heading entries : 63  (63 distinct ids — no duplicate headings)
index-table rows: 90  (86 distinct ids)
union distinct  : 91        total entry instances: 153

ids with TWO index rows:            R-72, R-73, R-74, R-76
ids where the index row and the body section carry DIFFERENT dates:
   id     row-date     body-date    row-line  body-line
   R-35   2026-06-16   2026-05-29        61        873
   R-55   2026-08-06   2026-08-08        69       1698
   R-56   2026-08-06   2026-08-08        70       1714
   R-57   2026-08-06   2026-08-08        71       1726
   R-58   2026-08-06   2026-08-08        72       1763
   R-59   2026-08-06   2026-08-08       104       1802
   R-76   2026-08-14   2026-08-15        91       1997
```

Union of affected ids: **R-35, R-55, R-56, R-57, R-58, R-59, R-72, R-73, R-74,
R-76** — ten, about 11% of the ledger.

Worked example, R-57:

- index row (2026-08-06): *"When the seam is a TOOL, the scout is one real
  invocation whose output you read — and first, a check that the artifact you
  invoked was built from current source."*
- body section, line 1726 (2026-08-08): *"Miss: an identifier's shape says
  nothing about whether the thing exists — check its declared root."*

Two unrelated laws. Nothing in either says the other exists.

## Reproduction

```
run_command("grep -c '^## R-' docs/trackers/reconnaissance-patterns.md")   # 63
run_command("grep -c '^| R-'  docs/trackers/reconnaissance-patterns.md")   # 90
```

Then compare any affected id's row date against its body's `**Observed:**` date.

## Environment

codescout `experiments` at `3b6b303f`. `docs/trackers/reconnaissance-patterns.md`
— 2,221 lines, 242 KB, entries dated 2026-05-19 to 2026-08-16.

## Root cause

Two mechanisms, both consequences of hand-allocated ids in a file that also
changed shape mid-life.

1. **No server-side allocation for prose trackers.** `append_entry(id_prefix=…)`
   allocates under a catalog transaction, but this tracker is prose: the next
   `R-N` is found by grepping headings and taking the maximum. That read is not
   atomic with the write, and it does not see the *other* format.
2. **The file carries two entry formats and the allocator only sees one.** Early
   entries are `## R-N` heading + body; roughly R-60..R-86 put the entire lesson
   inside an index-table row with no body at all. An author grepping `^## R-`
   for the maximum id misses every table-row entry — which is exactly how a
   number already used by a row gets assigned to a new body. That predicts the
   observed pattern precisely: the five-in-a-row R-55..R-59 collisions are rows
   dated 08-06 and bodies dated 08-08, i.e. one authoring session that grepped
   headings only.

measured 2026-08-16: the counts above, produced by parsing both formats
separately rather than by the heading grep that caused the defect.

## Evidence

### The index is not an index

For 58 ids the table row and the body coexist legitimately (row summarises body).
For 7 of them the row is a *separate entry*. A reader cannot tell which case they
are in without comparing dates, so the table cannot be trusted as a map of the
file — which also means every consumer that reads only the table (the cheap way
to survey 242 KB) silently gets a different corpus than one reading the bodies.

### The citation graph rests on these ids

57 entries carry `kin R-N` / `recurrence of R-N` references. Where the target id
is one of the ten, the reference is ambiguous. Two of the largest self-declared
recurrence chains pass through affected ids: the search-scope chain
(R-3 → R-73 → R-77 → R-79 → R-81 → R-87) and the instrument chain
(R-81 → R-86 → R-89 → R-91).

## Hypotheses tried

1. **Hypothesis** — the duplicates are deliberate: an entry revised in place with
   a new date. **Test** — read both instances of R-57. **Verdict** — rejected;
   the two state unrelated laws with no cross-reference, which is authorship of a
   new entry rather than revision of an old one.

## Fix

The decision this needs is whether ids are stable identifiers or ordinals, and
that is a call for the maintainer, not an implementation detail:

1. **Disambiguate without renumbering.** Give each colliding instance a suffix
   (`R-57a` / `R-57b`), leave every existing citation pointing at the base number,
   and add a note naming the split. Preserves the 57 citations; costs a permanently
   irregular numbering.
2. **Renumber the later instance** of each collision to the next free id
   (R-92+). Clean sequence afterwards; breaks any citation that meant the moved
   instance, and there is no record of which meant which.
3. **Retrofit the tracker to an augmented artifact** with an `entry_collection`,
   making `append_entry(id_prefix="R")` the only way to add one. Removes the
   cause rather than the symptom, and is BL-30's fix; the entries' long prose is
   the obstacle, since the augmented shape wants fixed fields.

(1) then (3) is the sequence that loses nothing.

## Tests added

None yet. The invariant worth gating once a shape is chosen: no id appears twice
across headings and index rows in any tracker. That check is cheap and belongs
next to `librarian(action="doctor")`, which already scans for catalog drift.

## Workarounds

When citing an R-N in the affected set, cite the line number or quote the law's
first clause alongside the id. When allocating a new id, count **both** formats:

```
grep -o '^## R-[0-9]*' <file>; grep -o '^| R-[0-9]*' <file>
```

The heading-only grep is what produced this defect.

## Resume

Decide between fixes 1-3 with the user before editing — every option touches
citations, and option 2 destroys information that no longer exists anywhere else.
Until then, do not add new R-N entries by the heading-grep method; BL-30's
workaround section carries the two-format command. The classification pass that
found this (three subagents over R-1..R-91, 2026-08-16) produced a per-entry
theme + canonical-law + supersession table that should be attached to whichever
option is chosen, since it is the input to the archive pass the ratified
`archive-cadence-policy.md` now authorises.

## References

- `docs/trackers/reconnaissance-patterns.md`
- `docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md` — BL-30, predicted this
- `docs/trackers/archive-cadence-policy.md` § Ratified — 2026-08-16
- `docs/TAXONOMY.md` — the seven hand-allocated id prefixes
