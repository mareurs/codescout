---
id: '1a0a6887f5998777'
kind: bug
status: open
title: Coverage::Deferred conflates "untested" with "no marker exists" — 4 of 5 no-marker rows went unescalated
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-09-03
severity: medium
---

## Summary

On the `result-cap-marker-gate` branch, `Coverage::Deferred`'s reason strings mix two populations that need opposite remedies: **43 of 48** say "no test drives this cap past its bound" (a worklist item — write a test), and **5 of 48** say "no marker exists to test for" (a production defect — the cap truncates and discloses nothing, which is `IC-13` membership, not a coverage gap). Only one of the five (`preview.plan_open_next`) was escalated as a possible bug; three others state `IC-13`'s own defining property in their own reason text and were left as ordinary backlog items.

## Symptom (Effect)

The branch's tally reports "48 deferred" as a single undifferentiated backlog. A reader triaging it by writing tests for each row will find 5 of them unwritable — because the row itself says the marker being tested for does not exist — and the production defect those five point at stays unaddressed.

## Reproduction

1. Check out `result-cap-marker-gate` at `2a32c043` (or later).
2. Read all 48 `Coverage::Deferred(...)` reason strings in `src/tools/core/cap_probe.rs`.
3. Classify each by whether it says "no test written" vs "no marker/field exists to assert against."
4. The five in the second bucket, at time of filing:
   - `preview.plan_open_next` (~`:399`) — "no companion field discloses the drop" (already escalated separately)
   - `format.shape_scalar_len` (~`:485`) — "emits no marker … silently dropped … unlike `MAX_KEYS`'s '… +N more'"
   - `references.corroborate_files_scan` (~`:511`) — "silently under-scans … a false zero indistinguishable from 'scan stopped early'"
   - `audit_doc_refs.basename_index` (~`:546`) — "silently resolves fewer refs rather than warning indexing was incomplete — a false zero"
   - `call_graph.workspace_files_scan` (~`:984`) — "silently returns fewer or no candidate positions rather than reporting an incomplete scan"

Not independently re-verified line-by-line by this filing beyond reading the whole-branch review's quoted text; line numbers are approximate and should be re-resolved against the merged `cap_probe.rs`.

## Environment

`result-cap-marker-gate` branch/worktree, `src/tools/core/cap_probe.rs`.

## Root cause

`Coverage::Deferred(reason: &str)` is a single free-text variant with no structural distinction between "untested" and "untestable because the marker doesn't exist." The module's classification history shows why: Task 3's original ruling considered the last three rows above and filed them as needing "a different probe shape" (framed as a coverage question); `preview.plan_open_next`, classified later, reframed the identical structural property ("a capped scan whose zero result is indistinguishable from 'nothing found'") as a possible production bug. The reframing was never retroactively applied to the earlier three.

*Read from `cap_probe.rs`'s `Deferred` reasons at `result-cap-marker-gate` HEAD `2a32c043`, as reported by the whole-branch review — this filing did not independently re-derive the full 48-row classification; it should be re-run against the merged file (see Resume).*

## Evidence

Quoted reason strings (from the whole-branch review's finding I3, 2026-09-03):

```
format.shape_scalar_len: "emits no marker … silently dropped … unlike MAX_KEYS's '… +N more'"
references.corroborate_files_scan: "silently under-scans … a false zero indistinguishable from 'scan stopped early'"
audit_doc_refs.basename_index: "silently resolves fewer refs rather than warning indexing was incomplete — a false zero"
call_graph.workspace_files_scan: "silently returns fewer or no candidate positions rather than reporting an incomplete scan"
```

Each of the last three states, in its own words, `IC-13`'s defining claim: *"A result is truncated by a limit ... and returned without a marker the caller can see."*

## Hypotheses tried

1. **Hypothesis:** the reframing genuinely is different for `preview.plan_open_next` (e.g. it has no marker AT ALL vs the other three merely under-report). **Test:** compare `format.shape_scalar_len`'s reason ("emits no marker … unlike `MAX_KEYS`'s sibling behavior") against `preview.plan_open_next`'s — both describe a cap with zero disclosure. **Verdict:** the four read as the same shape; no test run to falsify this beyond re-reading the reason text.

## Fix

Not designed. Candidate directions: (a) split `Coverage::Deferred` into two variants (e.g. `Deferred::Untested(reason)` and `Deferred::NoMarkerExists(reason)`), changing the tally to report them separately; (b) leave the enum alone but grep the 48 reasons for the "silently returns/drops/resolves fewer ... with no field disclosing it" shape as a standing audit step before publishing any future tally; (c) open individual `IC-13`-tagged bug files for the four production-defect candidates named above (three of them still need filing as of this bug).

## Tests added

None yet — this is a classification-hygiene finding, not a code fix.

## Workarounds

A reader triaging the 48 `Deferred` rows should read each reason string for "no marker exists" language before assuming it is a plain test-writing task.

## Resume

1. Re-derive the full 43/5 split against the merged `cap_probe.rs` (line numbers here are approximate, from the worktree pre-merge).
2. File the three still-unfiled production-defect candidates (`format.shape_scalar_len`, `references.corroborate_files_scan`, `audit_doc_refs.basename_index`) as their own `IC-13`-tagged bugs, or as entries under this one if they turn out to share a single remedy.
3. Decide on (a)/(b)/(c) above.

## References

- `src/tools/core/cap_probe.rs` (all `Coverage::Deferred` rows)
- Surfaced during `result-cap-marker-gate` branch's whole-branch review (2026-09-03), session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, finding I3
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (artifact `8a9dd5a27cd03480`) — extends the standing owed item there, which previously named only `preview.plan_open_next`

