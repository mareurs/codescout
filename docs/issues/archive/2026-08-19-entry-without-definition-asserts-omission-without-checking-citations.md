---
id: 10d7e46375cc3053
kind: bug
status: fixed
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
closed: 2026-08-19
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

**Shipped in two commits.** The first removed an unsupported claim; the second is what makes
the finding actionable, and neither subsumes the other.

`4ffd2803` — the honest interim. The assertion is removed from **both** surfaces:

- `scan_undefined_entries` (`src/librarian/tools/doctor.rs`). The finding no longer says
  *"This ledger defines its other entries, so these are omissions — add a heading for
  each."* It states what the check knows and what it does not.
- `undefined_in_body_note` (`src/librarian/catalog/augmentation.rs`) — the **write-path**
  twin carried the same leap (*"so this one is an omission"*). Found by sweeping for the
  phrase rather than repairing only the surface that reported the problem. Softened the
  same way, and the observable half — *"This ledger defines its other entries"* — is
  **kept**, because it is a fact the code checks. Only the inference drawn from it was
  unsupported, and separating the two is what let the existing test that pins that phrase
  keep passing. `5a72304c` never revisits this file, so this commit is not superseded.
- Both messages said *"every citation of it resolves to nothing"*, which presupposes that
  citations exist. Now *"any citation … would resolve to nothing"*.

`5a72304c` — the citation-aware partition, option 1 from *Resume*. `scan_undefined_entries`
splits each ledger's undefined ids into **cited** (a reference that resolves to nothing
today — named first, because it is the half a reader can act on) and **uncited** (an
informational count that may be a define-on-citation convention working correctly), and
emits a different message for each.

- New `corpus_cited_tokens` collects cited entry tokens across every artifact body via
  `link_scan::extract` — the same pure function `body_defined_indices` already uses for the
  definition half, which is what keeps the two halves agreeing on what a citation *is*.
- **Lazy.** The sweep runs only when some ledger has a non-empty undefined set, so a healthy
  catalog pays nothing for a check that finds nothing.
- Counts the file-stem-qualified form (`CrossRepoToken`) as well as bare `EntryToken`, and
  counts self-citations — see the closing note of § *Substrate resolved*.
## Tests added

Three fixtures, differing by **one index row**, because a single fixture cannot tell an
omission from a define-on-citation convention — which is the defect this closes:

- `undefined_entries_names_only_the_undefined_rows_in_a_defining_ledger` — `BL-3` sits in
  an index row. Rows define nothing but ARE scanned for citations, so this is the **cited**
  half: a reference that resolves to nothing today.
- `undefined_entries_separates_an_uncited_entry_from_a_cited_one` — same ledger with the
  row removed. Nothing mentions `BL-3`, so nothing is broken and the finding must say so
  differently.
- `undefined_entries_counts_a_stem_qualified_citation` — `log:BL-3`. A file-stem qualifier
  and a cross-repo qualifier are one syntactic shape, both emitted as `CrossRepoToken`, so
  matching only bare `EntryToken`s would read a qualified citation as no citation at all.

Mutations applied and the **observed** result:

| # | Mutation | Observed |
|---|---|---|
| M1 | skip the citation sweep (`cited` always empty) | cited + qualified tests FAIL, uncited passes |
| M2 | drop the `CrossRepoToken` arm | qualified test FAILS, alone |
| M3 | invert the partition predicate | all three FAIL |

Zero survivors. M3 is the one the pair was built for: the uncited fixture failed by
emitting the **cited** message, so the inversion is caught in both directions rather than
one.

Gate: fmt, clippy `--all-targets -D warnings`, `cargo test` 4266 passed / 45 ignored.
## Workarounds

Ignore the finding for `provenance-subsystem.md`. **Do not add headings for uncited
entries**; the ledger documents define-on-citation and adding one heading per row would
contradict it at 42 places. When a new `PV-N` is cited from another file, add its heading
to § *Defining sections for cited entries* at that moment, which is what the convention
already says.

## Substrate resolved, and a measurement that changes the stakes (2026-08-19)

**The hazard this file recorded as blocking does not exist.** *Resume* worried that a
citation-aware check would have to read `entry_cite`, which only `link_scan(write=true)`
materializes, making `doctor` report against a stale graph. It does not have to:

- `link_scan::extract(body) -> DocExtract { definitions, citations, declared_prefixes }`
  is a **pure function over a document body**. Citations come out of it directly, computed
  fresh, with no table involved and nothing to go stale.
- That is already the pattern. `body_defined_indices` — which `scan_undefined_entries`
  uses for the *definition* half — calls the same `extract`, pinned by
  `defined_indices_delegate_to_link_scans_own_definition_rule`. The citation half would
  use the same door.

The real cost is I/O, not staleness: `params_backed_ledgers` reads each ledger's body with
`std::fs::read_to_string`, and bodies are not in SQL, so a corpus-wide citation sweep means
reading every artifact file (~1075). That is what `link_scan` already does per run. It is
affordable **if it runs only when there is something to check** — compute the undefined set
first, and read the corpus only when it is non-empty.

### The measurement that changes the stakes

This file argued the finding was pure policy, on the evidence that no *external* citation
of an undefined `PV-N` exists. That evidence was real but incomplete — it never looked in
the ledger's own body:

| token | occurrences in `provenance-subsystem.md` | defining heading |
|---|---:|---:|
| `PV-12` | 8 | 0 |
| `PV-3` | 3 | 0 |
| `PV-20` | 2 | 0 |
| `PV-1`, `PV-9` | 1 each | 0 |

`PV-12` is cited eight times, once inside a section heading, and defines nothing. A reader
following it lands nowhere.

**The "roughly five" written here was wrong, and the shipped check is what proved it.** The
figure came from spot-checking eight tokens, finding five with occurrences, and reporting
the SAMPLE's count as the population's — W-4's exact failure, on the third pass over this
same question. Run live against the corpus:

> 42 of 68 `items` entries have no `## <ID> — <title>` heading. **Cited despite that: 33**
> — PV-1, PV-3, PV-6, PV-9, PV-11, PV-12, PV-16, PV-17 … (+25 more) … **Uncited: 9**

So the ledger has **33 dangling references**, not five, and only 9 of the 42 are the
define-on-citation convention working as intended. The convention is real and documented;
it is simply not what most of these entries are. Three rounds of hand-measurement gave
three different answers — "42 omissions", "0 cited", "~5 cited" — and the tool gave the
fourth by reading the whole population instead of a sample of it. That is the argument for
option 1, made better than any reasoning could have.

One design decision this forces: **self-citations count.** `link_scan` reports `self_cites`
separately (843 of 3883) because it creates no self-edges, but a citation that resolves to
nothing is a broken reference whether or not it came from the same file. Every instance
above is a self-citation, and every one of them is a real break.
## Resume

**Closed.** Option 1 shipped in `5a72304c` — see § *Fix* and § *Fix provenance*. Options 2
and 3 are kept below as what was considered, not as open work.

- **Message-only** — shipped first, in `4ffd2803`, as the honest interim: the finding
  disclosed that it does not read the citation graph rather than asserting omission. Option
  1 supersedes its `doctor.rs` half; its `augmentation.rs` half still stands alone.
- **Per-ledger opt-out** (`entry_definition_policy: on_citation` in frontmatter) — cheapest
  correct behaviour for one file, but it asks every ledger author to know the key exists,
  and the measurement in § *The measurement that changes the stakes* shows the policy ledger
  *still* has real breaks a per-ledger exemption would have hidden.

**Surfaced here, deliberately not repaired here:** `docs/trackers/provenance-subsystem.md`
carries **33 genuine dangling `PV-N` references**, measured by the shipped check rather than
by hand. That is a worklist against that ledger, not a defect in `doctor`, and it belongs to
whoever next works that tracker.
## References

- `src/librarian/tools/doctor.rs` — `scan_undefined_entries`, and the doc comment stating
  the assumption
- `docs/trackers/provenance-subsystem.md` — the ledger, and its define-on-citation
  convention
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule*
- `docs/issues/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md` —
  false-positive source any citation-aware check inherits


## Fix provenance

Two anchors, because the fix is two commits and neither covers the other's file. `5a72304c`
is the citation-aware partition in `doctor.rs`; `4ffd2803` is the write-path twin in
`augmentation.rs`, which `5a72304c` never revisits.

- **SHA:** `5a72304c` (`experiments`) — the citation-aware partition in `doctor.rs`
- **patch-id:** `e9f8df63b9113a5b4073deebc5501a2cb623287a`
- **SHA:** `4ffd2803` (`experiments`) — the write-path twin in `augmentation.rs`
- **patch-id:** `c6beb5f60c30e7d637d218819f688d8ffdd0f56d`

The SHAs are positional and do not survive a rebase of `experiments`. The patch-ids are
content hashes of the diffs and survive rebase and cherry-pick.

**This section was first written as a table**, because two commits read better in one — and
that made both anchors invisible to `archived_fix_sha_unresolvable`, which keys on the
`- **SHA:**` and `- **patch-id:**` lines and nothing else. A record a human reads as anchored
and a tool reads as unanchored is
`prompt-surface-compaction-session-log:W-12` running in the other direction. The parser is
now plural (`structured_fix_pointers`, `5a72304c`'s successor commit) specifically so the
two-commit case does not have to reach for a table to stay readable: a shape that makes the
honest case unrepresentable gets worked around, and the workaround is the defect.

If a SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes — Iron
Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep e9f8df63b911 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several branches
(cherry-pick) and any of them is the fix.
