---
id: b3d5eec46e6dfe86
kind: bug
status: open
title: link_scan and doctor have no check for a cited prefix with zero definers — the citations are inert, and the report's capped arrays invite the opposite wrong conclusion
tags:
- librarian
- link-scan
- doctor
- reporting
- link-graph
opened: 2026-08-26
owner: marius
related: []
severity: medium
unverified: The three-state behaviour is established from report output across four namespaces in one downstream repo, not from reading the resolver source. The proposed check's false-positive rate against a prose doc that merely quotes `## R-4` in prose (which the guide warns about for ledger inference) has not been assessed.
---

# BUG: a cited prefix with no definer is reported nowhere

## Summary

A citation can be *resolved*, *dangling/ambiguous*, or **inert** — the prefix has zero
definers repo-wide, so the token is never a resolution candidate and lands in no bucket.
Nothing reports the third state: not `link_scan`, not `librarian(action="doctor")`.

Measured downstream in `claude-plugins`: `active-plan.md` owns a 38-task `T-N` namespace as
table rows, defines no `T-N` heading, and receives ~60 cross-file citations from seven
files. Zero edges, zero warnings, everything reads healthy.

Compounding it, `link_scan`'s `ambiguous` / `dangling` arrays are silently capped at 50, so
the natural way to check — "is my token in the dangling list?" — is unsound in both
directions.

## Symptom (Effect)

Two sessions on 2026-08-26 read the same `link_scan` output about the same namespace and
reached **opposite** wrong conclusions. Neither had the means to notice.

- One reasoned *no `T-N` heading exists → the citations dangle*, and filed a bug whose
  reproduction step (`read dangling_by_source`) demonstrates the opposite of its claim.
- The other reasoned *no `T-*` in the dangling/ambiguous arrays → `T-35`/`T-37`/`D-6` all
  resolved*. `D-6` does resolve; `T-35`/`T-37` do not. The premise rested on absence from a
  50-element sample of a 70-element population.

Neither the resolved nor the broken bucket describes an inert citation, and
`counts.citations` does not decompose in a way that reveals the shortfall.

## Reproduction

In any repo, cite a token whose prefix nothing defines:

```
# a namespace with no heading definitions anywhere
grep -rn '^#\{1,6\} *T-[0-9]\+ *[—–-]' --include='*.md' .   # → (nothing)
grep -c '| T-' docs/trackers/active-plan.md                  # → 38 rows
grep -ro '\bT-[0-9]\+\b' --include='*.md' . | wc -l          # → 200 occurrences

librarian(action="link_scan")
```

`T-*` appears in no bucket. Then confirm the contrast with a positive control in the same
repo:

| Token | Definers | Reported as |
|---|---|---|
| `U-28` | `## U-1 — …` ×5 exist; `U-28` does not | **dangling** ✓ |
| `D-6` | `### D-6 — 2026-05-15 — …` exists | resolved, silent ✓ |
| `T-35` | none for prefix `T` | **nothing at all** ← the bug |

For the capped arrays: compare `counts.ambiguous` (81) and `counts.dangling` (70) against
`len(ambiguous)` and `len(dangling)` (50 each). There is no per-array `truncated` flag,
though `counts.scan_truncated` exists for the artifact sweep.

## Environment

- codescout @ branch `experiments`
- Downstream repo measured: `claude-plugins` @ `2d6cdbe`, 228 artifacts scanned
- `counts`: `citations: 469`, `ambiguous: 81`, `dangling: 70`, `cross_repo: 8`

## Root cause

The report is organised around the two states that have remedies. A prefix nobody has ever
defined is not a *broken* citation in the resolver's terms — it is not a citation at all —
so there is no natural slot for it in a breakage-shaped report.

`doctor`'s existing neighbours both key off **entries**: `entry_without_definition` iterates
declared entries, `ledger_defines_nothing` iterates declared ledgers. `active-plan.md`
declares no `entry_prefix` and owns no entries, so neither check has anything to iterate.
It also carries no frontmatter at all yet is catalogued `kind: tracker, status: active` by
the classifier — so it is scanned, and still invisible to both checks.

Heading **level** is not the discriminator, incidentally: `### D-6 — …` and `#### S-1 — …`
both define. `get_guide("tracker-conventions")`'s `### A-9 Addendum` counter-example fails
on the missing dash, not the level — worth stating in the guide, since two readers took it
as a level rule.

## Hypotheses tried

1. **Hypothesis:** the prefix must be declared via `entry_prefix` to become a candidate.
   **Test:** `grep -rn '^entry_prefix' --include='*.md'` — only `R`, `VG`, and two `F`/`W`
   sequences are declared, yet `U-28` (prefix `U`, declared nowhere) is reported dangling.
   **Verdict:** rejected. Definition by heading, not declaration, is what makes a namespace
   live.
   **Evidence link:** § Reproduction, control table.

## Fix

**Primary — a `doctor` check, `cited_prefix_with_no_definer`.** For every prefix appearing
in ≥1 citation with 0 definers in scope, emit a row: the prefix, the citation count, and the
citing files. Read-only worklist, consistent with `doctor`'s existing posture. This converts
the invisible state into the one thing that makes it fixable — a row someone can see.

Guard against the false positive the guide already warns about for ledger inference: a
design doc quoting `## R-4` in prose is not a namespace. A citation-count threshold (say ≥3
across ≥2 files) keeps incidental prose out.

**Secondary — a `truncated` flag per finding array** on `link_scan`, so
`len(dangling) == 50 < counts.dangling == 70` is stated rather than inferred. The
`*_by_source` maps are the census today but answer "which file", never "which token", so
there is no complete token-level view at any size.

**Documentation —** state in `get_guide("tracker-conventions")` § *Entry headings* that a
prefix with no definer anywhere produces citations that are neither resolved nor reported,
and that heading level is irrelevant to the definition rule.

Fix commit SHA + `git patch-id --stable`: not yet applied.

## Tests added

None yet. The natural regression is a fixture repo with one prefix cited N times and never
defined, asserting the check fires; plus an assertion that
`len(findings) < counts.<finding>` implies `truncated: true`.

## Workarounds

Run a **positive control** before believing any categorical claim from the report: resolve
one token per state you believe exists — a known-dangling one, a known-resolved one, and the
token in question. A token matching none of the expectations is the discovery. One call.

Use `*_by_source` for census; never infer from absence in the finding arrays.

## Resume

Decide whether the check belongs in `doctor` (read-only worklist, matches
`entry_without_definition`'s posture) or in `link_scan`'s report (closer to the data, but
that report is breakage-shaped). Then pick the citation-count threshold against the catalog:
count prefixes with 0 definers by citation volume across all managed roots, and see where
incidental prose quotation falls off.

## References

- `docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md` —
  **sibling, same shape on a different surface**: an instrument reporting healthy because it
  never checks coverage. Two instances in one day argues the shape, not the instance.
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule*, § *One entry format, never two*
- `claude-plugins:roster-audit-session-log:F-6` — the friction entry this issue is filed from
- `claude-plugins:roster-audit-session-log:F-4` — the wrong finding it produced, corrected same day
- `claude-plugins:reconnaissance-patterns:R-4` — the reasoning-side counterpart: the positive-control law was loaded and still missed
- `claude-plugins` `docs/issues/2026-08-26-active-plan-t-n-row-only-uncitable.md` — the downstream instance

