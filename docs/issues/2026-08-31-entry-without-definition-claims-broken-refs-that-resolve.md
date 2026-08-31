---
status: open
opened: 2026-08-31
severity: low
owner: marius
related: []
tags: [librarian, doctor, entry-definition, cross-file, misleading-output]
kind: bug
---

# BUG: entry_without_definition claims citations "resolve to nothing" after a sibling artifact defines them

## Summary

`entry_without_definition` reads only the ledger's **own** body when deciding whether an
entry is defined, then states in its detail text that the cited ones *"resolve to nothing
right now"*. Once a sibling artifact defines those tokens — an archive companion, which
this project's own compaction ladder prescribes — the claim is false while the finding
persists unchanged.

## Symptom (Effect)

After `9ac9e6d5` moved 38 `PV-N` definitions into an archived companion, `doctor` still
reports against `docs/trackers/provenance-subsystem.md`:

```
38 of 68 `items` entries have no `## <ID> — <title>` heading. Cited despite that: 31 —
PV-1, PV-3, PV-6, PV-10, PV-12, PV-16, PV-17, PV-19 … (+23 more), whose references
resolve to nothing right now. Fix those first; each needs a `## <ID> — <title>` heading,
the only shape `link_scan` binds a token to.
```

They do not resolve to nothing. Measured the same minute: `link_scan` `counts.dangling`
fell 637 → 574 on that commit, and **zero** `PV-` tokens appear in either the 574-entry
dangling list or the 557-entry ambiguous list.

The count half ("38 of 68 have no heading **in this file**") is true and useful. The
consequence half ("their references resolve to nothing") is the false part, and it is the
half that tells a reader what to do.

## Reproduction

1. Take an augmented ledger whose `params` hold entry ids its body does not define.
2. Define those ids in a **separate** artifact — e.g. `docs/trackers/archive/<ledger>-…md`
   with `status: archived` — using `## <ID> — <title>` headings.
3. `librarian(action="link_scan")` → the tokens resolve; dangling drops.
4. `librarian(action="doctor")` → `entry_without_definition` is unchanged and still says
   the references resolve to nothing.

Observed at `9ac9e6d5` (patch-id `513492b57e477e9a92fce31d586f17ed35255b4b`). The SHA is
positional and dies when `experiments` is rebased, which happens routinely here — it was
already rewritten once, from `27e543cc`, between this file being written and committed. The
patch-id is a content hash of the diff and survives both rebase and cherry-pick.

## Environment

codescout 0.15.0, Arch Linux, project `codescout`, branch `experiments`.

## Root cause

`scan_undefined_entries` computes the defined set from `body_defined_indices(&ledger.body,
prefix)` — the ledger's own body text — so a definer in any other artifact is invisible to
it. `link_scan` resolves a token against **all** definers, preferring the sole active one
where several exist, so the two subsystems answer different questions and only one of them
consults the graph.

The detail text then overreaches: the same function's own doc comment says *"This check
reads the citation graph, so the split is measured rather than assumed"* — which is true of
the **cited / uncited** split it computes, and not true of the resolves-to-nothing claim it
attaches to that split.

*Mechanism located during Task 1 review at `src/librarian/tools/doctor.rs:3227` and `:3240`
(`body_defined_indices`); symptom measured independently by running `doctor` and
`link_scan` back to back at `9ac9e6d5`.*

## Evidence

### The two subsystems disagree, same commit

`doctor`: 31 cited entries "resolve to nothing right now".
`link_scan`: `dangling` 637 → 574 on this commit; 0 `PV-` tokens in the dangling list.

### Why the ledger-scoped read is defensible

Archival is a supported end state — `get_guide("tracker-conventions")` § *Compaction and
archival* gives the ladder as *"live body → archived section (heading kept) → nothing
further"* and states a unique definer resolves even when archived. So a ledger legitimately
ends up with definitions outside its own file, and the check has no bug in its **count** —
only in what it asserts follows from that count.

## Hypotheses tried

1. **Hypothesis:** Task 1 put the headings in the wrong place, and they should have gone in
   the live body. **Test:** read the parent tracker's § *Defining sections for cited
   entries*, which states a measured policy against mass-promotion, and confirm the tokens
   resolve. **Verdict:** rejected — the placement is what the ledger's own convention asks
   for, and the citations resolve. The check's wording is what is wrong.
   **Evidence:** *The two subsystems disagree, same commit*.

## Fix

Not implemented.

Smallest correct change: keep the count ledger-scoped, and make the **consequence** clause
consult the same resolution `link_scan` uses before claiming a reference is broken. Where a
sibling defines the token, say so and name the definer — a reader following the current
text would add a duplicate heading to the live body and create an ambiguous token, which is
the opposite of the repair intended.

Cheaper interim: soften the clause to name its scope — "no heading **in this file**" — so
the finding stops asserting a graph property it did not check. This is the
negative-results-name-their-scope rule
(`docs/adrs/2026-08-27-negative-results-name-their-scope.md`) applied to a positive claim.

## Tests added

None — no fix written. A regression test should define a ledger's entry in a **second**
artifact and assert the finding does not claim the references are broken. A test using a
single artifact is monotone under this defect and passes either way.

## Workarounds

Cross-check `link_scan`'s `dangling` / `dangling_by_source` before acting on an
`entry_without_definition` detail. If the tokens are absent from the dangling list, the
references resolve and only the ledger-local count is outstanding.

## Resume

Read `scan_undefined_entries` (`src/librarian/tools/doctor.rs:3169` onward) and decide
between the two fixes above; the deciding question is whether the scan already has a
resolved-definer map available or would need to build one, which determines whether the
correct fix costs a lookup or a second pass.

## References

- `src/librarian/tools/doctor.rs:3169` — `scan_undefined_entries`; `:3227`, `:3240` —
  `body_defined_indices`
- `src/librarian/tools/link_scan/extract.rs:319-322` — `def_re`, the shared definition shape
- `docs/issues/archive/2026-08-19-entry-without-definition-asserts-omission-without-checking-citations.md`
  — the same check's earlier overreach, fixed; this is a second, distinct one
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
- Introduced-by (not caused-by): `9ac9e6d5` / patch-id
  `513492b57e477e9a92fce31d586f17ed35255b4b`, which created the legitimate sibling definer
