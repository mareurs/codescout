---
id: '6962d98218162987'
kind: bug
status: open
title: 'BUG: entry_without_definition asserts the entries are omissions without checking whether anything cites them'
owners:
- marius
tags:
- librarian
- doctor
- misleading-error
- link-scan
- trackers
topic: catalog-drift
closed: null
opened: 2026-08-19
owner: marius
related:
- '6e2cafbb1dea1678'
severity: med
---

# BUG: `entry_without_definition` asserts the entries are omissions without checking whether anything cites them

## Summary

`doctor`'s `entry_without_definition` counts an augmented ledger's `params` entry ids
that its body never defines as `## <ID> — <title>` headings. The **count is correct**.
The **explanation attached to it is an assumption the check cannot observe**, and on at
least one ledger it is false:

> "… so these are omissions — add a heading for each."

For `docs/trackers/provenance-subsystem.md` the finding reads *"42 of 68 `items` entries
have no heading … This ledger defines its other entries, so these are omissions."* That
ledger's body states the opposite policy in prose, three lines above its first entry
heading:

> **Separately from narrative, an entry needs a `PV-N — <title>` heading to be citable at
> all** … § *Defining sections for cited entries* below carries a compact heading for
> every `PV-N` **another file references**; add one there when you cite a new entry from
> elsewhere.

Define-on-citation is a coherent, documented convention. The 42 undefined entries are
entries nothing references, and **measurement confirms zero of them are cited** (see
*Evidence*). An agent following the finding's instruction would add 42 headings to an
1100-line tracker for entries no citation reaches — growing the file, and cementing a
premise the file itself contradicts.

## Symptom (Effect)

```
librarian(action="doctor")
→ "entry_without_definition": 1
  path:   docs/trackers/provenance-subsystem.md
  detail: "42 of 68 `items` entries have no `## <ID> — <title>` heading, so every
           citation of them resolves to nothing: PV-1, PV-3, PV-6, PV-9, PV-10, PV-11,
           PV-12, PV-13 … (+34 more). This ledger defines its other entries, so these
           are omissions — add a heading for each."
```

Two clauses do different work and only one is supported:

- *"every citation of them resolves to nothing"* — **vacuously true**; there are no
  citations of them.
- *"This ledger defines its other entries, so these are omissions"* — an inference from
  a ratio. It is the clause that tells the reader what to do, and it is the wrong one.

## Reproduction

1. `librarian(action="doctor")` on this repo; read the `entry_without_definition` row.
2. Collect the ledger's defined ids:
   `grep(pattern="^#+ PV-", path="docs/trackers/provenance-subsystem.md")` → 26.
3. Collect the ids anything else actually cites:
   ```
   grep -ohE 'PV-[0-9]+' docs/trackers/provenance-probe-session-log.md \
     docs/trackers/reconnaissance-patterns.md docs/trackers/tracker-hygiene-log.md \
     docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md \
     docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md \
     | sort -u -V
   ```
4. Compare. The cited set is a **subset** of the defined set — nothing is dangling.

## Environment

- 2026-08-19, `experiments` at `711d5d86`, binary built 14:35:59 from the same tree.
- `src/librarian/tools/doctor.rs`, `scan_undefined_entries`.

## Root cause

`scan_undefined_entries` computes `params ids − body-defined ids` and never consults the
citation graph. The assumption is stated openly in its own doc comment, which is why this
is a design gap rather than a slip:

> **Two checks, not one, because the remedies differ.** `entry_without_definition` means
> the ledger **writes definitions and these entries missed** — one heading each, and the
> author is the right person to write it.

That reasoning holds for a ledger whose convention is *define every entry*. It does not
hold for one whose convention is *define on citation*, and nothing in the params/body
difference distinguishes the two. The check infers policy from a ratio: some entries are
defined, therefore all were meant to be.

The harm the definition rule exists to prevent is a **dangling citation**. Undefined and
uncited is not that harm; it is a ledger that has not yet needed the heading.

## Evidence

Measured 2026-08-19 on `docs/trackers/provenance-subsystem.md` (68 `items` entries):

| set | n | ids |
|---|---:|---|
| defined by a heading | 26 | PV-2 4 5 7 8 25 26 27 29 30 31 38 44 53 55 56 58 60 61 62 63 64 65 66 67 68 |
| cited by any other file | 25 | the same, minus PV-5 |
| **cited but undefined** | **0** | — |

`librarian(action="link_scan")` over 1075 artifacts and 3883 citations reports 538
dangling and 423 ambiguous — and **not one carries a `PV-` token**. No `PV-` appears in
`cross_repo` either.

Note the sampling hazard, recorded because it nearly produced the wrong answer here: the
`link_scan` result buffer caps `dangling` and `ambiguous` at 50 rows each, so
`grep -c 'PV-'` returning 0 against that buffer is evidence about a truncated sample, not
about the population. The table above rests on the direct id comparison in *Reproduction*,
not on that grep.

## Hypotheses tried

1. **Hypothesis:** the 42 entries are genuine omissions and the remedy is 42 headings.
   **Test:** read the ledger's own convention, then compare the cited set against the
   defined set.
   **Verdict:** rejected. The convention is explicit and the cited set is a subset of the
   defined set — the ledger is in full compliance with its stated policy.

## Fix

**Partially applied.** The unsupported assertion is removed from both surfaces; the
citation-aware split remains open — see *Resume*.

Applied:

- `scan_undefined_entries` (`src/librarian/tools/doctor.rs`). The finding no longer says
  *"This ledger defines its other entries, so these are omissions — add a heading for
  each."* It now states what the check knows and what it does not: the entries carry no
  heading, so any citation of them **would** resolve to nothing; whether any exists is
  something this check cannot tell, because it does not read the citation graph; and a
  define-on-citation ledger is already correct, so check before adding headings.
- `undefined_in_body_note` (`src/librarian/catalog/augmentation.rs`) — the **write-path**
  twin carried the same leap (*"so this one is an omission"*). Found by sweeping for the
  phrase rather than repairing only the surface that reported the problem. Softened the
  same way, and the observable half — *"This ledger defines its other entries"* — is
  **kept**, because it is a fact the code checks. Only the inference drawn from it was
  unsupported, and separating the two is what let the existing test that pins that phrase
  keep passing.
- Both messages said *"every citation of it resolves to nothing"*, which presupposes that
  citations exist. Now *"any citation … would resolve to nothing"*.

Not applied: the partition on whether an undefined entry is actually cited. That is the
change that would let a reader *act* on the finding, and it needs the substrate decision
recorded in *Resume*.
## Tests added

One assertion added to the existing
`undefined_entries_names_only_the_undefined_rows_in_a_defining_ledger`
(`src/librarian/tools/doctor.rs`): the detail must contain `citation graph` — the finding
must disclose that it does not read citations rather than assert omission.

Mutation applied and observed, not reasoned about: replacing the disclosure clause with
*"these are plainly omissions"* — the exact regression this guards against — made the test
**FAIL**. Restored; whole suite green at 4247 passed / 45 ignored.

The panic output also caught a defect the assertion was not looking for. The rendered
message read *"before adding 1 headings"* on a single-entry ledger, because the count was
interpolated into a plural noun. Reworded to *"before adding the missing headings"*, which
drops the argument entirely. **A failing assertion that prints the whole rendered string is
a cheap proofreading pass on message text** — the grammar bug had been invisible while the
test was green.

Still owed, and part of the real fix rather than this one: a fixture whose undefined entry
**is** cited, asserted to report differently from one whose undefined entries are not. A
single-fixture test cannot tell the two apart.
## Workarounds

Ignore the finding for `provenance-subsystem.md`. **Do not add headings for uncited
entries**; the ledger documents define-on-citation and adding one heading per row would
contradict it at 42 places. When a new `PV-N` is cited from another file, add its heading
to § *Defining sections for cited entries* at that moment, which is what the convention
already says.

## Resume

Pick one, then implement:

1. **Citation-aware partition** (preferred) — split the finding into cited-but-undefined
   (defect, ids named) and uncited-but-undefined (informational count only). Resolve the
   substrate hazard above first.
2. **Message-only fix** (cheap) — drop *"This ledger defines its other entries, so these
   are omissions"*, since the check has no evidence for it. Keeps the count, removes the
   false instruction. Does not help a reader decide whether to act.
3. **Per-ledger opt-out** — let a ledger declare `entry_definition_policy: on_citation`
   in frontmatter and have the check honour it. Cheapest correct behaviour for this file,
   but it puts the burden on every ledger author and only works once they know.

Whichever is chosen, add a regression fixture with a ledger whose undefined entries are
uncited, and one whose undefined entry IS cited, and assert the two are reported
differently — a single-fixture test cannot tell them apart.

## References

- `src/librarian/tools/doctor.rs` — `scan_undefined_entries`, and the doc comment stating
  the assumption
- `docs/trackers/provenance-subsystem.md` — the ledger, and its define-on-citation
  convention
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule*
- `docs/issues/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md` —
  false-positive source any citation-aware check inherits
