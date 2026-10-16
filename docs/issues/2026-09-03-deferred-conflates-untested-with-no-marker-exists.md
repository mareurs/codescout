---
id: '1a0a6887f5998777'
kind: bug
status: fixed
title: Coverage::Deferred conflates "untested" with "no marker exists" — 4 of 5 no-marker rows went unescalated
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-09-03
closed: 2026-10-16
severity: medium
---

## Summary

On the `result-cap-marker-gate` branch, `Coverage::Deferred`'s reason strings mix two populations that need opposite remedies: **43 of 48** say "no test drives this cap past its bound" (a worklist item — write a test), and **5 of 48** say "no marker exists to test for" (a production defect — the cap truncates and discloses nothing, which is `IC-13` membership, not a coverage gap). Only one of the five (`preview.plan_open_next`) was escalated as a possible bug; three others state `IC-13`'s own defining property in their own reason text and were left as ordinary backlog items.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live, and the population
grew.** `Coverage::Deferred(&'static str)` is still one free-text variant (`src/tools/core/cap_probe.rs:74`,
confirmed by the coordinator). By grep, 48 `Deferred(` against 23 `Probed {` rows. Reading all 48 reasons,
**8** now state that no marker exists, not 5: the five named here plus `preview.plan_task_text` (:438, "appends
no ellipsis or other marker"), `doctor.exposure_threshold` (:459, "no field reports how many were dropped"), and
`doctor.recently_touched_walk` (:1071, added by `8fe822af`). `## Resume` step 2 (one IC-13 bug per no-marker row)
has not been done — no such files exist.

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

Candidate (a) of the original filing, plus the structural half of (c).

- `Coverage::Deferred(&str)` is replaced by `Coverage::Untested(&str)` (a marker exists; the remedy is a test) and `Coverage::NoMarker { reason, bug }` (the cap discloses nothing; the remedy is a code change, and `bug` names the `docs/issues/` file that tracks it). `Tally` reports `untested` and `no_marker` as separate numbers, each counted from its own variant (no `total - probed` subtraction).
- `probe_rows_are_well_formed` refuses a `NoMarker` row whose `bug` is not an existing `docs/issues/*.md` file, so the escalation cannot be skipped by choosing the variant.
- `no_untested_row_describes_a_cap_that_discloses_nothing` scans the real table's `Untested` reasons for the phrases that say so (option (b), as a standing guard rather than an audit step). It is a text backstop, not a proof: it cannot see a use site the reason never mentions.
- Nine rows were reclassified as `NoMarker` and filed (the issue's eight, re-derived against the tree at `f5f1c6f7`, plus `format.shape_scalars`, which a use-site read showed drops scalars past `MAX_SCALARS` with no count while its reason named only a missing test): `preview.plan_open_next` and `preview.plan_task_text`, `doctor.exposure_threshold`, `format.shape_scalar_len` and `format.shape_scalars`, `references.corroborate_files_scan`, `audit_doc_refs.basename_index`, `call_graph.workspace_files_scan`, `doctor.recently_touched_walk` -- seven bug files `docs/issues/2026-10-16-*.md`.
- Tally before: 86 total, 37 probed, 49 deferred, 36 mutation-verified. After: 84 total, 35 probed, 40 untested, 9 no-marker, 34 mutation-verified (the two probe rows removed are `context.attestation_exposure` and `read_file.json_path_route_room`, both `NOT_A_CAP` floors; see the sibling issue).

## Tests added

In `src/tools/core/cap_probe_tests.rs`: `no_untested_row_describes_a_cap_that_discloses_nothing` (seen RED against the unmigrated table, listing the nine rows plus one false positive whose reason said "silently bind"), `the_classification_guards_flag_what_they_exist_to_flag` (fixtures for the phrase scan and the bug-file check), the extended `tally_distinguishes_killed_from_not_yet_and_deferred` (a fixture with 2 `NoMarker` rows; mutating the `no_marker` count to count `Untested` reds it), and the `NoMarker` arm of `probe_rows_are_well_formed`. `tests/result_caps.rs` passes in full (71 tests).

## Workarounds

A reader triaging the 48 `Deferred` rows should read each reason string for "no marker exists" language before assuming it is a plain test-writing task.

## Resume

Not audited, stated rather than implied: the phrase scan and the row reasons only see what a reason mentions. A use-site read of the remaining 40 `Untested` rows covered about half; `lsp.did_open_size` (`client.rs:1172-1181`, a `tracing::debug!` and `return Ok(())` for a file over 10 MiB) and `workspace.scan_files` / `workspace.scan_depth` (silent bounds on a language guess) were seen to drop without telling the caller but were not traced to a caller-visible consequence, so they stay `Untested`. If either is confirmed to surface a wrong answer, move it to `NoMarker` with a bug file.

## References

- `src/tools/core/cap_probe.rs` (all `Coverage::Deferred` rows)
- Surfaced during `result-cap-marker-gate` branch's whole-branch review (2026-09-03), session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, finding I3
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (artifact `8a9dd5a27cd03480`) — extends the standing owed item there, which previously named only `preview.plan_open_next`
