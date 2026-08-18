---
id: bde782f4cc52ac22
kind: bug
status: open
title: 'BUG: every drift check asks whether the body kept up with params — nothing detects params falling behind a body that ran ahead'
owners:
- marius
tags:
- librarian
- augmentation
- drift
- doctor
- entry-identity
topic: tracker-entry-identity
closed: ''
opened: 2026-08-18
related:
- docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
- docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
severity: medium
---

# BUG: nothing detects params falling behind a body that ran ahead

## Summary

Every drift surface codescout has asks one direction of one question: *has the **body** kept up
with `params`?* `update_entry`'s `snapshot_stale`, `append_entry`'s `snapshot_missing` and
`doctor`'s `snapshot_drift` all compare params ids against the body and report what the body lacks.
Nothing reports the inverse — a body that has grown **past** params, so the catalog's structured
index is the stale copy. Found by nearly publishing false statements from it.

## Symptom (Effect)

`docs/trackers/windows-platform-support.md`, measured 2026-08-18 while backfilling entry headings:

```
params.issues   29 rows, ending at WIN-29, with WIN-28 and WIN-29 both status "open"
committed table 35 rows, ending at WIN-36, with WIN-28 and WIN-29 "fixed" + full post-mortems
                and WIN-30, WIN-32, WIN-33, WIN-34, WIN-35, WIN-36 that params never carried
```

`doctor` reported this ledger only under `ledger_defines_nothing`. No surface reported the six
missing rows or the two stale statuses. `snapshot_drift` is structurally incapable of it: it
computes `claimed` from params and asks what the body lacks, so params being the smaller set makes
the difference empty.

## Reproduction

At `bf60b5f3` on `experiments`:

1. `artifact(action="get", id="52451519052d207c", entry_filter={"id": {"contains": "WIN-"}})` — 29
   entries, max WIN-29.
2. `grep -c '^| WIN-[0-9]* |' docs/trackers/windows-platform-support.md` — 35 rows, max WIN-36.
3. `librarian(action="doctor")` — reports nothing about the discrepancy.

## Environment

codescout `experiments` @ `bf60b5f3`, 2026-08-18, Linux, stdio MCP. Not platform-sensitive.

## Root cause

Directional by construction, in all three surfaces:

- `scan_snapshot_drift` (`src/librarian/tools/doctor.rs`) builds `claimed` from
  `params.<entry_collection>[].id` and reports `claimed.difference(&in_body)`. The reverse
  difference is never computed.
- `snapshot_stale_note` (`src/librarian/catalog/augmentation.rs`) takes `claimed` as a parameter
  and asks whether the patched row is in the body.
- `AppendOutcome.warning` is the **only** thing that sees this direction — its doc comment says
  *"Set when the body claimed ids the params array does not carry"* — and it fires only during an
  `append_entry` call. A ledger nobody appends to can drift indefinitely in silence.

Mechanism-language: the catalog is machine-local and git-ignored, so the *body* is the surface that
survives a clone. Every check is therefore built to protect the body's completeness, and the case
where the body is **ahead** reads to those checks as the body being healthy — which it is. What is
unhealthy is the queryable index, and that is the surface agents filter on.

## Evidence

### The near-miss, which is the actual argument for fixing it

Step 4a of `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`
generates one defining heading per entry. The obvious source is `params` — it is structured, it is
what `entry_filter` reads, and it is what every example in `get_guide("tracker-conventions")` uses.
Generating from it would have published:

- `WIN-28` and `WIN-29` as **`open`** when both are `fixed`, one with CI run `31098286970` green at
  3283 passed / 0 failed;
- **no section at all** for WIN-30, 32, 33, 34, 35, 36 — six entries silently absent from the
  citable set, in the same pass whose entire purpose was making entries citable.

It was caught only because the WIN table happened to be read first, for style. Nothing prompted the
check, and no tool would have contradicted the wrong output.

### Why `append_entry`'s warning is not the fix

It is the right predicate in the wrong place. `body_claimed_indices` already computes what is
needed; the gap is that only an append consults it. This ledger's last `append_entry` predates the
six-row divergence, so the one surface that could have reported it never ran.

## Hypotheses tried

1. **Hypothesis:** `doctor`'s `snapshot_drift` covers this and the WIN case slipped through a gate.
   **Test:** read `scan_snapshot_drift` and trace which difference it computes.
   **Verdict:** rejected — it computes `claimed.difference(&in_body)` only. The inverse is absent,
   not gated.

2. **Hypothesis:** the body-ahead case is benign because the body is what reaches git.
   **Verdict:** rejected, and this is the interesting half. The body being ahead is benign *for
   durability* and harmful *for querying*: `artifact(get, entry_filter=…)` — the documented way to
   ask "what is open?" — reads params. A stale params set answers that question wrongly while every
   check reports green.

## Fix

Not implemented. The predicate already exists; it needs a second caller and a name.

1. **Add the inverse to `doctor`** beside `snapshot_drift`: a `params_behind_body` check computing
   `in_body.difference(&claimed)` from the same two sets `scan_snapshot_drift` already builds. Cheap
   — one more difference over data already in hand.
2. **Report ids only, not statuses.** A status mismatch between a params row and a table cell needs
   a text comparison against a rendered column, which is fragile and belongs to a separate decision.
   The id-set difference is exact and is what caught the WIN case.
3. **Name the remedy in the message.** The fix is `append_entry` / `update_entry` for the missing
   rows, *not* editing the body — the opposite of `snapshot_drift`'s advice, and getting that
   backwards would delete the newer record.

Deliberately out of scope: reconciling the WIN ledger's own six rows. That is data repair, tracked
separately, and folding it in would hide whether the check works.

## Tests added

None yet. What the fix must pin:

- `params_behind_body_reports_ids_the_body_has_and_params_lacks` — the WIN shape, minimally: body
  anchors WIN-1..WIN-4, params carries WIN-1..WIN-2.
- `params_behind_body_is_silent_when_params_is_the_superset` — the ordinary `snapshot_drift` case
  must not fire here, so the two checks are provably about different directions.
- `params_behind_body_is_silent_for_a_prose_only_tracker` — a body anchoring no ids at all yields no
  finding, matching `body_claimed_indices`' documented empty-set contract.

## Workarounds

Before generating anything from `params`, compare the id sets by hand:

```
artifact(action="get", id="<id>", entry_filter={"id": {"contains": "<PREFIX>-"}})   # count + max
grep -c '^| <PREFIX>-[0-9]* |' <the tracker>                                        # count + max
```

A mismatch means params is not a safe source. This is now stated in
`get_guide("tracker-conventions")`-adjacent form in the WIN ledger's own § History.

## Resume

Add `params_behind_body` to `src/librarian/tools/doctor.rs`, next to `scan_snapshot_drift`, reusing
the `claimed` and `in_body` sets it already computes — the whole change is the opposite difference
plus a message whose remedy points at `append_entry`, not at a body edit. Write
`params_behind_body_is_silent_when_params_is_the_superset` first: it is the test that proves the two
checks are not the same check twice.

## References

- `src/librarian/tools/doctor.rs` — `scan_snapshot_drift`, the directional check.
- `src/librarian/catalog/augmentation.rs` — `snapshot_stale_note`; `AppendOutcome.warning`, the only
  surface that sees this direction; `body_claimed_indices`, the predicate a fix reuses.
- `docs/trackers/windows-platform-support.md` § History 2026-08-18 — the measured divergence.
- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` —
  the sibling that found it, and whose step 4 nearly published from the stale side.

