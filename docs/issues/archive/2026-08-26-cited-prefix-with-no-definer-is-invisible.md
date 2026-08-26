---
id: c2a65e6e1814524b
kind: bug
status: fixed
title: link_scan and doctor have no check for a cited prefix with zero definers — the citations are inert, and the report's capped arrays invite the opposite wrong conclusion
tags:
- librarian
- link-scan
- doctor
- reporting
- link-graph
closed: 2026-08-26
opened: 2026-08-26
owner: marius
related: []
severity: medium
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

**All three shipped 2026-08-26.**

**SHA:** `c0ec5ace` (`experiments`)
**patch-id:** `b07570e84e40c7dfd8a5664d9793175c0b40ca25`

`feat(doctor): add cited_prefix_with_no_definer check`.

1. **[done] Primary — the `doctor` check.** `scan_cited_prefix_with_no_definer`
   (`src/librarian/tools/doctor.rs`) starts from the citation graph rather than a known
   ledger's claimed entries — one combined corpus pass over `link_scan::extract`, reusing
   the same extraction `corpus_cited_tokens` and `body_defined_indices` already use for the
   citation and definition halves, so this check can't disagree with either about what
   counts as one. Emits `cited_prefix_with_no_definer` naming the prefix, citation count,
   and citing files. Guarded by a citation-volume threshold (≥3 citations across ≥2 files)
   to stay quiet on incidental prose (`## R-4` quoted in a design doc) — the threshold is the
   bug's own suggested starting point, not yet swept against the full catalog to tune.
   Silent when the prefix is defined elsewhere (link_scan's dangling territory) or declared
   via `entry_prefix` (`ledger_defines_nothing`'s territory), to avoid double-reporting the
   same underlying namespace under two check names.
2. **[done] Documentation.** `get_guide("tracker-conventions")` § *Entry headings* now
   states the third citation state (resolved / dangling / inert) and the heading-level
   non-rule two readers had already misread.
3. **[done] Secondary — a `truncated` flag per finding array on `link_scan`.**
   `counts.truncated.{ambiguous,dangling,cross_repo}`, gated on `total > len(array)`, in
   `src/librarian/tools/link_scan/mod.rs`. `len(dangling) == 50` no longer has to be compared
   against `counts.dangling` by hand — the report states directly which arrays were cut.

   **SHA:** `a14215ce` (`experiments`)
   **patch-id:** `079e5eea8768c62e0b82b9727fa0b408cc5c9570`

   `fix(link-scan): flag which finding arrays were truncated by the cap`.
## Tests added

Four tests in `src/librarian/tools/doctor.rs`, all confirmed RED (function didn't exist)
before GREEN:

- `cited_prefix_with_no_definer_fires_above_threshold` — 2 files, 4 citations, fires with
  the right check name, count, and both citing files named.
- `cited_prefix_with_no_definer_is_silent_below_threshold` — 1 incidental citation, silent.
- `cited_prefix_with_no_definer_is_silent_when_prefix_is_defined_elsewhere` — a defined
  sibling entry makes the prefix known; other ids in it dangle (link_scan's job), silent here.
- `cited_prefix_with_no_definer_is_silent_when_prefix_is_declared` — `entry_prefix:` declared
  but nothing defined; silent here, `ledger_defines_nothing`'s finding instead.

One test for the secondary — `counts_flags_truncation_per_finding_array_when_the_cap_is_exceeded`
(`src/librarian/tools/link_scan/mod.rs`): seeds 51 dangling citations against a target with
one defined entry (so the prefix is known, not inert), confirms the array stays capped at 50
while `counts.dangling == 51`, and asserts `counts.truncated.dangling == true` while an empty
arm (`ambiguous`) reports `false`. RED confirmed as an assertion failure against the live
fixture (51 real dangling citations, correctly capped) before the field existed.
## Workarounds

Run a **positive control** before believing any categorical claim from the report: resolve
one token per state you believe exists — a known-dangling one, a known-resolved one, and the
token in question. A token matching none of the expectations is the discovery. One call.

Use `*_by_source` for census; never infer from absence in the finding arrays.

## Resume

All three Fix items shipped (`c0ec5ace`, `a14215ce`). One optional item remains, not a
blocker for archiving this bug:

1. Sweep the `cited_prefix_with_no_definer` citation-count threshold (currently ≥3 across
   ≥2 files) against the full catalog to see where incidental prose quotation actually falls
   off. Calibration, not correctness — the shipped threshold is a deliberate starting point,
   not a guess that needs fixing. Worth its own lightweight tracker note if anyone picks it
   up rather than reopening this file.
## References

- `docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md` —
  **sibling, same shape on a different surface**: an instrument reporting healthy because it
  never checks coverage. Two instances in one day argues the shape, not the instance.
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule*, § *One entry format, never two*
- `claude-plugins:roster-audit-session-log:F-6` — the friction entry this issue is filed from
- `claude-plugins:roster-audit-session-log:F-4` — the wrong finding it produced, corrected same day
- `claude-plugins:reconnaissance-patterns:R-4` — the reasoning-side counterpart: the positive-control law was loaded and still missed
- `claude-plugins` `docs/issues/2026-08-26-active-plan-t-n-row-only-uncitable.md` — the downstream instance
