---
kind: bug
status: fixed
tags:
- librarian
- doctor
- entry-definition
- cross-file
- misleading-output
closed: 2026-08-31
opened: 2026-08-31
owner: marius
related: []
severity: low
unverified: Not yet re-verified against a rebuilt live MCP — the running server still emits the pre-fix wording. Both branches are test-guarded by a pair of fixtures differing by one artifact, so this is a freshness caveat rather than a coverage gap.
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

**Fixed on `experiments` at `4ef91c82`** — patch-id
`3b624f30488ba42be7301d047cbad0d62fac710f`.

This section offered a *smallest correct* fix (consult the same resolution `link_scan` uses)
and a *cheaper interim* (soften the clause to name its scope). The correct one shipped,
because the deciding question the § *Resume* posed resolved in its favour.

**That question was: does the scan already have a resolved-definer map, or must it build
one?** Neither, exactly — it was already **computing and discarding** one.
`corpus_cited_tokens` reads every artifact in the catalog and calls `extract()`, keeping
`.citations` and dropping `.definitions` from the same `DocExtract`. So the corpus-wide
definition set costs no extra I/O and no second pass; it is the half of a value the loop was
already producing. The interim wording was never needed.

### What changed

The ledger-scoped **count** is untouched and still correct — the heading really is absent
from that body. The **consequence** clause now partitions the cited-and-locally-undefined set
into three states rather than asserting one:

| state | reported as |
|---|---|
| cited, defined **nowhere** | broken — same wording, same count semantics as before |
| cited, defined in a **sibling** | not broken; the definer is **named** |
| uncited | unchanged |

**Naming the definer is load-bearing, not decoration.** A reader following the old text would
add the heading to the live body — creating a *second* definer, which is an **ambiguous**
token, which resolves to nothing. The advice manufactured the break it claimed to have found,
so the finding has to say which case a reader is in, not merely stop lying about it.

Self is excluded explicitly rather than assumed away: the local defined-set comes from the
catalog's stored `body` and the definer map from a fresh read of the file. Those are two
reads, and a disagreement between them must not become a claim about the graph.
## Tests added

Two, in `src/librarian/tools/doctor.rs`, exactly the shape this section prescribed before the
fix existed — *"define a ledger's entry in a **second** artifact"*, because a single-artifact
fixture is monotone under this defect and passes either way.

- `a_cited_entry_defined_in_a_sibling_artifact_is_not_called_broken` — RED first, printing the
  false claim verbatim. It asserts both halves: the finding must not say *"resolve to
  nothing"*, **and** it must name the definer.
- `a_cited_entry_defined_nowhere_is_still_reported_as_broken` — the non-vacuity twin. Without
  it the fix is satisfied by never claiming a break at all, silencing the check's entire
  actionable half while passing. It was green before the fix as well as after, which is
  correct: its job is to fail against an over-broad fix, not against the pre-fix code.

The two fixtures **differ by exactly one artifact**, matching the shape this check's existing
cited/uncited pair already uses — and the load-bearing detail is annotated on the line that
carries it: move `BL-3`'s heading into the ledger body and it leaves `undefined` altogether,
so the test would pass while exercising none of this.

All seven pre-existing tests for this check pass unchanged. The `"Cited despite that: N"`
wording was deliberately kept rather than reworded — it stays accurate, now counted over the
genuinely-broken set — so no existing assertion had to be edited to accommodate the fix. An
assertion changed to fit a fix is one that stopped guarding it.

Counts: default 4977 → **4979**; lean unchanged at 3412, correctly — `doctor.rs` sits behind
the `librarian` feature.

### On the verification that preceded this

Worth recording because the first attempt was wrong in a way that reads as evidence. Checking
"do the `PV-` tokens dangle?" by grepping the `link_scan` report hit a **truncated** buffer —
50 of 572 dangling — whose own header says *absence from a cut list is not evidence*. That
zero describes the sample. The answer came instead from the citation graph: the archive
companion carries live incoming entry links for 38 distinct `PV-` tokens, three of them among
the eight `doctor` named. That is a positive finding rather than an absence, which is what the
claim needed.
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
