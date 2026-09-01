---
id: d6a3b70e57498ec8
kind: bug
status: open
title: 'BUG: the citation-volume gate selects FOR the prose it exists to exclude — 8 of 14 findings are acronyms, and the top four by volume are all noise'
tags:
- cluster/gate-keyed-on-unobservable-event
- librarian
- doctor
- link-scan
- false-positive
closed: null
opened: 2026-09-01
owner: marius
related:
- docs/trackers/open-issue-work-queue.md
severity: low
---

## Summary

`cited_prefix_with_no_definer` is documented as firing *"only above a citation-volume
threshold to stay quiet on incidental prose"* (`get_guide("tracker-conventions")`). Volume
is the wrong discriminator, and not weakly — **the highest-volume prefixes in the report are
exactly the prose**, because common acronyms are the most-repeated tokens in a technical
corpus. The gate is anti-correlated with the property it was chosen to select for.

## Symptom (Effect)

Measured 2026-09-01 on this repo, all 14 in-repo findings, sorted by the volume the gate
uses. The four **highest**-volume prefixes are all non-citations:

| prefix | cites | what it actually is | citation? |
|---|---:|---|---|
| `TC-N` | 106 | `TC-07 score=0/3` — benchmark test-case ids | no |
| `UTF-N` | 20 | `UTF-8` | no |
| `SG-N` | 20 | `(SG-1)` — real reference | yes |
| `SHA-N` | 19 | `SHA-256` | no |
| `N-N` | 15 | `start_line=N-9` — placeholder arithmetic in a code example | no |
| `SF-N` | 13 | `the SF-4 bug file` — real reference | yes |
| `RFC-N` | 6 | `RFC-7396` | no |
| `MF-N` | 6 | (archived TODO-review) | probably |
| `ZZ-N` | 5 | `## ZZ-4` quoted as a **test fixture** in prose | no |
| `KT-N` | 5 | (archived TODO-review) | probably |
| `GPT-N` | 4 | `GPT-4.1`, `GPT-5` | no |
| `O-N` | 4 | `O-1 vs O-2` — option labels | borderline |
| `CC-N` | 4 | (archived TODO-review) | probably |
| `CI-N` | 3 | `` `CI-2`-shaped prose `` — a **mention of the class**, in the very doc describing it | no |

**8 of 14 are not citations at all.** Ranking by volume puts `TC-N` (106) at the top and
`CI-N` (3) at the bottom, which is close to the inverse of the useful ordering.

## Reproduction

```
cargo build --bin codescout && ./target/debug/codescout doctor \
  | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['summary']['by_check']['cited_prefix_with_no_definer'])"
```

Then read each finding's prefix back against the corpus with
`grep -rhoE '.{28}\bPREFIX-[0-9]+\b.{18}' docs CHANGELOG.md | head -1` — one line of context
is enough to classify every one of the 14.

## Environment

codescout `experiments` at `c4f27360`. `src/librarian/tools/doctor.rs`.

## Root cause

Not read at the bytes — this is filed from the **output**, deliberately, and that limit is
stated rather than hidden. What is established is the report's contents and the
documentation's claim about them; which line implements the threshold is not.

The design intent is sound and is documented in `link_scan`'s resolver
(`src/librarian/tools/link_scan/resolve.rs:335`), which suppresses exactly these:

```
None // UTF-8 / SHA-256 / GPT-4 prose noise
```

`doctor`'s check is the deliberate **complement** of that suppression — it exists to surface
the state `link_scan` stays silent on — so it inherits none of that gate and substitutes
volume. The two halves were correct separately: the resolver must stay quiet so real
citations are not drowned; the doctor check must speak so a wholly-broken namespace is not
invisible. Volume was chosen to keep the second from re-admitting the noise the first
excluded, and it does not do that.

## Evidence

### The gate's own documentation names the cases it fails on

`get_guide("tracker-conventions")` § *Entry headings*: *"it resolves as prose noise — the
same gate that keeps `UTF-8`/`SHA-256` silent"*. `UTF-N` and `SHA-N` are findings #2 and #4
by volume in the live report.

### One finding is the class citing itself

`CI-N`'s hit is `` it suppresses `CI-2`-shaped prose `` — a document *explaining* the
suppression, counted as a violation of it. Same shape as
`docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md`: an id cannot
be discussed without being cited.


### Both options the Resume narrows to are CIRCULAR — read at the bytes 2026-09-01

The first version of this file recommended deciding between (a) and (c). Neither is viable,
and the same predicate kills both.

`src/librarian/tools/link_scan/resolve.rs:331` — the resolver's suppression:

```rust
0 => {
    if index.prefix_is_known(&citation.raw) {
        Some(Outcome::Dangling)
    } else {
        None // UTF-8 / SHA-256 / GPT-4 prose noise
    }
}
```

And `known_prefixes` (`:71`, `:95`) is populated from **both** halves, for every artifact
including archived ones:

- every `## PREFIX-N — <title>` definition's prefix;
- every declared `entry_prefix`.

So `!prefix_is_known` **is** this check's population, by construction. Option **(a)**,
*"reuse the resolver's suppression"*, would suppress 14 of 14 — it does not filter the check,
it deletes it. Option **(c)**, *"require the prefix to appear in at least one
`## PREFIX-N — <title>` heading anywhere, including archived"*, is the **first half of that
same set**, so it suppresses 14 of 14 too. The two options that read as most distinct are
the same dead end.

Found by re-entering a bug file this session had written an hour earlier — `R-49` firing on
a file young enough that nothing else would have re-opened it.

### The discriminator is already in the check's own output, unused

The check prints *"cited N times across M files"* and gates on **N alone**. `M/N` — how
thinly the citations are spread — separates this corpus far better, and needs no new data.
All 14 live findings, sorted by dispersion:

| prefix | cites | files | files/cites | what it is | `>= 0.8` |
|---|---:|---:|---:|---|---|
| `CI-N` | 4 | 4 | 1.00 | prose about the class | **suppress** |
| `RFC-N` | 7 | 7 | 1.00 | `RFC-7396` | **suppress** |
| `SHA-N` | 21 | 21 | 1.00 | `SHA-256` | **suppress** |
| `UTF-N` | 22 | 22 | 1.00 | `UTF-8` | **suppress** |
| `N-N` | 16 | 14 | 0.88 | `start_line=N-9` | **suppress** |
| `ZZ-N` | 6 | 5 | 0.83 | test fixture quoted in prose | **suppress** |
| `CC-N` | 4 | 2 | 0.50 | archived TODO-review | keep |
| `GPT-N` | 6 | 3 | 0.50 | `GPT-4.1`, `GPT-5` | keep |
| `O-N` | 6 | 3 | 0.50 | option labels | keep |
| `SF-N` | 14 | 6 | 0.43 | real reference | keep |
| `KT-N` | 5 | 2 | 0.40 | archived TODO-review | keep |
| `MF-N` | 6 | 2 | 0.33 | archived TODO-review | keep |
| `SG-N` | 21 | 3 | 0.14 | real reference | keep |
| `TC-N` | 107 | 13 | 0.12 | benchmark test-case ids | keep |

**Zero real prefixes are suppressed**; 6 of 8 noise prefixes are. Precision moves 6/14 → 6/8.
Note the ordering is close to the *inverse* of the volume ordering the gate uses now —
`UTF-N` and `SHA-N` are simultaneously the highest-volume and the highest-dispersion, which
is this bug's anti-correlation with a replacement attached.

**Two limits, stated because the table looks better than the evidence is.**

- **The threshold is fitted, not validated.** `0.8` was chosen to separate *this* table,
  n=14, one corpus. What the measurement supports is the *direction* of the signal; the cut
  point is not established, and a second corpus is what would establish it.
- **Dispersion cannot exceed 6/8 here, at any threshold.** `GPT-N` (noise) and `CC-N` / `O-N`
  (real) sit at exactly 0.50. No cut on this signal alone separates them, so lowering the
  threshold to catch `GPT-N` necessarily loses two real prefixes. That is a structural
  ceiling, not a tuning opportunity — written down so nobody re-derives it by tuning the
  number down and quietly losing recall.

**Corpus drift, noted so the two tables in this file can be reconciled.** The Symptom table
was taken earlier the same day and reads `TC-N` 106 / `UTF-N` 20 / `SG-N` 20; this one reads
107 / 22 / 21. Peers committed to the corpus in between. Neither table is stale in a way
that changes any verdict — but a reader comparing them should know the counts are
measurements of an instant, not constants.
## Hypotheses tried

1. **Hypothesis:** the 14 are citation repairs. **Test:** sampled one line of context per
   prefix. **Verdict:** rejected — 8 of 14 are not citations, so most of the "work" is a
   no-op population. This is why the task was not executed as filed.

## Fix

Not fixed — the remedy is a design choice and a wrong one makes a useful check useless.

**(a) and (c) are struck.** Both are circular; see Evidence § *Both options the Resume
narrows to are CIRCULAR*. They are kept struck rather than deleted so the dead end is not
re-proposed by the next reader, who will find them as attractive as this file's author did.

- ~~**a. Reuse the resolver's suppression.**~~ **Circular** — the resolver suppresses exactly
  `!prefix_is_known`, which is this check's entire population. Zeroes the check.
- **b. Shape rule on the numeric part.** `SHA-256`, `UTF-8`, `RFC-7396`, `GPT-4.1` carry
  standard/version numbers; entry ids are small ordinals. Cheap, wrong on `TC-07`, and now
  **subsumed by (e) for every case except `GPT-N`** — which is the one case (e) cannot
  reach, so it survives as a possible second stage rather than as a primary.
- ~~**c. Require a `## PREFIX-N — <title>` heading anywhere.**~~ **Circular** — that is the
  first half of `known_prefixes`, so it is (a) under another name. Zeroes the check.
- **d. Leave it and document the read.** Weakest — an 8-of-14 false-positive rate on a
  read-only check is a report nobody finishes reading, which is how a real finding gets
  missed.
- **e. Gate on DISPERSION (`files / cites`) instead of volume.** *New, and the recommended
  one.* Suppress a prefix whose citations are spread roughly one per file: that is the
  signature of an incidental technical term, and the inverse of a ledger namespace, whose
  citations cluster. Measured: 6 of 8 noise prefixes suppressed, **zero** real prefixes
  lost, precision 6/14 → 6/8. Costs nothing to compute — the check already counts both
  numbers and prints them in its own message. Read its two limits in Evidence before
  implementing: the threshold is fitted to n=14, and 6/8 is a ceiling rather than a starting
  point.
## Tests added

None — nothing is fixed. A fix under (a) wants the 14 real prefixes as a fixture, asserting
the six real ones survive and the eight prose ones are suppressed. Both directions are
required: suppressing everything passes a test that only checks the noise is gone.

## Workarounds

Read the report with one line of grep context per prefix before treating any of it as work.

## Resume

Implement **(e)**, and treat `GPT-N` and `TC-N` as *known residual* rather than as a reason
to keep tuning — the Evidence section shows why no threshold on dispersion alone reaches
them.

Before writing code, settle the one question the measurement does not answer: whether the
threshold ships as a bare constant or as an explicitly-labelled heuristic in the message, the
way `params_status_drift` does (*"This check is a HEURISTIC and both directions of error are
possible"*). That sibling's precedent argues for saying so at the finding, since the cut
point is fitted.

The test needs both directions, and the fixture must contain both: a scattered one-per-file
prefix that must be **silent**, and a clustered prefix that must still be **reported**. The
silence half alone is monotone under "the check does nothing" and would pass against a stub.
## References

- `src/librarian/tools/link_scan/resolve.rs:335` — the resolver's suppression
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule*
- `docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md` — the bug
  this check shipped to close
- `docs/trackers/open-issue-work-queue.md` — `BL-71`
