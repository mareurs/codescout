---
id: d6a3b70e57498ec8
kind: bug
status: open
title: 'BUG: the citation-volume gate selects FOR the prose it exists to exclude — 8 of 14 findings are acronyms, and the top four by volume are all noise'
tags:
- cluster/assertion-satisfiable-by-accident
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

## Hypotheses tried

1. **Hypothesis:** the 14 are citation repairs. **Test:** sampled one line of context per
   prefix. **Verdict:** rejected — 8 of 14 are not citations, so most of the "work" is a
   no-op population. This is why the task was not executed as filed.

## Fix

Not fixed — the remedy is a design choice and a wrong one makes a useful check useless.
Options, cheapest first:

- **a. Reuse the resolver's suppression.** `link_scan` already classifies these correctly;
  `doctor` re-deriving the judgement from a different signal is the duplication that let the
  two disagree. Strongest option, and it removes the second discriminator rather than tuning
  it.
- **b. Shape rule on the numeric part.** `SHA-256`, `UTF-8`, `RFC-7396`, `GPT-4.1` carry
  standard/version numbers; entry ids are small ordinals. Cheap, and wrong on `TC-07`.
- **c. Require the prefix to appear in at least one `## PREFIX-N — <title>` heading
  ANYWHERE**, including archived. Would clear the three archived-TODO prefixes correctly and
  is close to what "no definer" already means.
- **d. Leave it and document the read.** Weakest — an 8-of-14 false-positive rate on a
  read-only check is a report nobody finishes reading, which is how a real finding gets
  missed.

## Tests added

None — nothing is fixed. A fix under (a) wants the 14 real prefixes as a fixture, asserting
the six real ones survive and the eight prose ones are suppressed. Both directions are
required: suppressing everything passes a test that only checks the noise is gone.

## Workarounds

Read the report with one line of grep context per prefix before treating any of it as work.

## Resume

Decide between (a) and (c). (a) is the better shape — one classifier, not two — but needs
the resolver's suppression to be reachable from `doctor` without pulling in the whole
`link_scan` pipeline. Check that first; it may be what decides between them.

## References

- `src/librarian/tools/link_scan/resolve.rs:335` — the resolver's suppression
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule*
- `docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md` — the bug
  this check shipped to close
- `docs/trackers/open-issue-work-queue.md` — `BL-71`

