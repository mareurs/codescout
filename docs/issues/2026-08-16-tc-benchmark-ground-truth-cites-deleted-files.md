---
id: '5043d3c2e3e4bbfd'
kind: bug
status: open
title: Retrieval benchmark scores against 5 deleted files, so several TCs are unpassable at any setting
tags:
- retrieval
- benchmark
- measurement-validity
- stale-ground-truth
---

## Summary

`scripts/run-tc-benchmark.py`'s per-TC `expected` lists name 5 files that no longer
exist. Any TC whose expected set includes one of them can never score a hit on it, at
any `CODESCOUT_BM25_BOOST` value or retrieval configuration. The benchmark reports a
number; part of that number is a floor nothing can move.

## Symptom (Effect)

Existence check over all 33 distinct paths named in the file, run 2026-08-16 at
`c3310c73`:

```
MISSING  docs/FEATURES.md
MISSING  src/embed/index.rs
MISSING  src/prompts/server_instructions.md
MISSING  docs/TODO-tool-misbehaviors.md
MISSING  src/prompts/onboarding_prompt.md
```

28 of 33 resolve. The 5 above appear in the `expected` lists at
`scripts/run-tc-benchmark.py` lines 31, 82, 95, 125, 132, 150, 166 and 167 — seven TC
entries, several of which lose more than one target.

The worst case is the prompt-surface TC at :166, whose expected set is
`["src/prompts/server_instructions.md", "src/prompts/onboarding_prompt.md",
"src/server.rs"]` — two of its three targets are unreachable, so its ceiling is 1/3
regardless of how well retrieval actually works.

## Reproduction

```
for p in <every path named in run-tc-benchmark.py>; do [ -e "$p" ] || echo "MISSING $p"; done
```

## Environment

codescout `experiments` @ `c3310c73`, 2026-08-16.

## Root cause

Ground-truth drift, not a retrieval defect. Three separate refactors deleted or merged
files without updating the benchmark that scores against them:

- `src/prompts/server_instructions.md` and `src/prompts/onboarding_prompt.md` were
  consolidated into `src/prompts/source.md`, which `build.rs` slices into the two
  surfaces at compile time (`src/prompts/README.md` § Surfaces). **The surfaces still
  exist; only the files are gone** — so this TC is testing for an artifact of the old
  layout, and its subject matter is still perfectly valid.
- `src/embed/index.rs` was removed by the legacy-retrieval work
  (`docs/trackers/archive/2026-05-07-retrieval-session-residuals.md`, L-01/L-02).
- `docs/FEATURES.md` and `docs/TODO-tool-misbehaviors.md` were deleted.

*Measured 2026-08-16 by the existence check above. The claim that scores are depressed
is arithmetic from that, not a benchmark re-run — no before/after numbers were taken.*

## Evidence

Found sideways: `scripts/sweep-bm25-boost.sh:8` carries a comment citing
`src/lsp/client.rs, src/embed/index.rs, docs/FEATURES.md` as examples of the corpus the
benchmark expects. Two of those three were dead, which looked like a stale comment —
until the benchmark it describes turned out to name the same dead paths. **The comment
was accurate; the instrument was not.**

## Hypotheses tried

1. **Hypothesis:** the sweep script's comment is simply stale prose.
   **Test:** grep `run-tc-benchmark.py` for the paths it claims are in the expected lists.
   **Verdict:** rejected — the benchmark really does list them; the comment describes
   reality faithfully and reality is broken.

## Fix

Not implemented, and **deliberately not applied unattended** — this is a pinned
baseline, so any remap changes what the numbers mean.

- `server_instructions.md` / `onboarding_prompt.md` → `src/prompts/source.md` is a
  near-mechanical remap: same subject, current home, and arguably what the TC always
  meant.
- `src/embed/index.rs` → the retrieval stack was rewritten, not renamed; the honest
  options are to repoint at the current equivalent or retire the TC.
- `docs/FEATURES.md` / `docs/TODO-tool-misbehaviors.md` have no successors.

**The comparability question is the real decision.** `docs/trackers/retrieval-benchmark.md`
is a *pinned 25-TC log*; repairing the ground truth raises scores for reasons unrelated
to retrieval quality and breaks comparison against every earlier run. Either re-baseline
explicitly and say so in that tracker, or keep the TCs and record the known-unreachable
floor so the numbers are read correctly. That is an owner decision, not a cleanup.

## Tests added

None. A cheap gate is available and worth considering: assert at benchmark start that
every path in every `expected` list exists, and fail loudly rather than scoring against
a phantom. This is the same shape as intervention I-1 in
`docs/trackers/test-escape-hardening.md` (invariant test over a structure that silently
drifts).

## Workarounds

Read any TC score touching those seven entries as a lower bound, not a measurement.

## Resume

Decide the comparability question first — re-baseline vs. record-the-floor — because it
determines whether the remap is allowed at all. Only then edit the `expected` lists in
`scripts/run-tc-benchmark.py`, and note the decision in
`docs/trackers/retrieval-benchmark.md`.

## References

- `scripts/run-tc-benchmark.py` — the `expected` lists
- `scripts/sweep-bm25-boost.sh:8` — the comment that led here
- `docs/trackers/retrieval-benchmark.md` — the pinned 25-TC log this feeds
- `src/prompts/README.md` § Surfaces — where the two prompt files went

