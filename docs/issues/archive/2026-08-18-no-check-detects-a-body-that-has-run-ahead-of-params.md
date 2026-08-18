---
id: 0808a5251625e6db
kind: bug
status: fixed
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
closed: 2026-08-18
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

**SHIPPED `87f3b936` (`experiments`).** All three points as filed.

1. `scan_params_behind_body` runs in `doctor` beside `scan_snapshot_drift`, computing
   `in_body.difference(&claimed)` — the same two sets, subtracted the other way.
2. **Ids only.** No status comparison; the id-set difference is exact and is what caught the
   WIN case.
3. **The message names a remedy** and says explicitly *"do NOT re-render the body from
   `params`"*, because inheriting `snapshot_drift`'s remedy here is data loss rather than noise.
   ⚠️ The remedy it first named — `append_entry` / `update_entry` — was **wrong**, and neither
   tool can perform this repair; corrected in `8084f0ea`. See § *Correction* below before reading
   any further clause of this section as current.

Two design decisions, each pinned by a test rather than a comment:

- **Not gated on `body_keeps_snapshot`.** That gate is correct for the row question and wrong
   here — it would silence a body id the catalog has never seen, which is the entire finding.
   Same argument as `scan_undefined_entries`.
- **Extracted `params_backed_ledgers` → `ParamsBackedLedger`.** The three entry-drift scans
   shared a 45-line preamble and this would have been the third copy. Three checks that must
   agree on what a ledger *is* drifting apart is the failure mode this whole family of bugs is
   about. The original bail conditions are preserved exactly, including that an id with no `-`
   aborts the whole collection.

Also stopped enumerating check names on `Violation::check` — that list named eight and had gone
stale by three, because nothing gates a doc comment against the `scan_*` call sites.

Deliberately still out of scope: reconciling the WIN ledger's own six rows. That is data
repair, and leaving it is what let the new check prove itself on real data — see § Resume.
## Tests added

Six, in `src/librarian/tools/doctor.rs`, all written and watched fail before the
implementation existed (5 failed on their assertions; the 6th passes on a stub **by design** —
it asserts the new check stays *silent* on a lagging body, so its discriminating power only
exists once the code can subtract in the wrong order).

| Test | What it kills |
|---|---|
| `params_behind_body_reports_a_body_id_with_no_params_row` | the check existing at all |
| `a_lagging_body_is_snapshot_drift_and_never_params_behind_body` | reversing the subtraction |
| `params_behind_body_fires_where_snapshot_drift_sees_a_complete_snapshot` | folding this into `snapshot_drift` as more samples |
| `params_behind_body_is_not_gated_on_body_keeps_snapshot` | copying the sibling's gate |
| `params_behind_body_names_append_entry_as_the_remedy` | inheriting `snapshot_drift`'s remedy text |
| `params_behind_body_caps_its_sample_and_counts_the_remainder` | a fixture that never reaches the truncation branch |

The cap fixture carries 13 body ids against 1 params row so `if more > 0` actually executes —
memory `test-design-discipline` records that exact hole shipping in `grep`'s
`completeness_warning` with seven green tests.

Gate: `cargo fmt` 0, `cargo clippy --all-targets -- -D warnings` 0, **4137 passed / 0 failed**
(4131 before). All 12 pre-existing `snapshot_drift` / `undefined_entries` tests pass unchanged
across the refactor.
## Workarounds

Before generating anything from `params`, compare the id sets by hand:

```
artifact(action="get", id="<id>", entry_filter={"id": {"contains": "<PREFIX>-"}})   # count + max
grep -c '^| <PREFIX>-[0-9]* |' <the tracker>                                        # count + max
```

A mismatch means params is not a safe source. This is now stated in
`get_guide("tracker-conventions")`-adjacent form in the WIN ledger's own § History.

## Correction (2026-08-18) — the shipped message named a remedy that cannot repair it

Found while trying to execute the repair the check recommends. The first-ship message was
wrong on two counts, and both were mine.

**1. Neither named remedy can do it.** The message said *"Add the missing rows with
`append_entry` / `update_entry`"*. `append_entry` ends with `obj.insert("id", new_id)` — it
overwrites whatever id the caller passes — and allocates `params_next.max(body_max + 1)`,
folding in the very body ids this check is reporting. On the WIN ledger it would mint `WIN-37`,
not the missing `WIN-30`. `update_entry` patches a row that already exists and is pinned never
to change the row count, so it cannot create one either. The only surface that can create a row
at a GIVEN id is the wholesale params write, which also carries a hazard the message therefore
has to state: a params patch **replaces** the array, so a partial one drops the rest.

**2. The reissue claim was overstated.** The message said *"no id was allocated for them, so a
later `append_entry` can reissue the same number"*. The `body_max` fold makes reissue
**impossible** while the body still claims the id — `append_entry`'s own comment says so
(*"Folding the body's max in makes the reissue impossible instead of silent"*). Reissue becomes
possible only after a compaction moves those rows to an archive companion, and then only for a
ledger carrying no committed `entry_high_water_<PREFIX>`. Narrower, and conditional.

That second error mattered beyond wording: it inflated the finding from "params-based queries
miss these entries" to "silent corruption", which is the wrong urgency and the wrong remedy
shape.

**Why no test caught it.** The test guarding the message asserted
`detail.contains("append_entry")` — which passes whether `append_entry` is named as the fix or
as the thing that cannot fix it. A non-discriminating assertion on the one clause whose
correctness was load-bearing. Replaced by
`params_behind_body_names_a_remedy_that_can_actually_repair_it`, which asserts the name of the
tool that CAN do it (`artifact_augment`) — an assertion the old message fails.

This is the same defect class BL-40 exists to catch — a confidently-worded remedy that is wrong
in its direction — occurring inside BL-40's own output. Worth keeping for that reason.

## Resume

Nothing outstanding on the check. **Verified live** against the real catalog
(`codescout doctor --json`), which is the step distinct from the gate — it fired **twice**,
neither instance vacuous:

- `docs/trackers/windows-platform-support.md` — `WIN-30, WIN-32..WIN-36`, 6 of 35. The
  near-miss this bug was filed for, now reported mechanically instead of being caught by
  luck.
- `mirela/backend-kotlin/docs/trackers/solver-invariants.md` — `SI-59..SI-68`, 10 of 68. **A
  different repo, never reported by any surface.** Ten entries whose ids were never allocated,
  so per this check's own message a later `append_entry` there can reissue `SI-59`. The sample
  cap fired on real data here (`… (+2 more)`).

Two pieces of **data repair** are now visible and remain open, deliberately:

- the WIN ledger's 6 rows (left stale on purpose so the check had something true to find);
- `solver-invariants`' 10 — new, and the more urgent of the two, because id reissue is a
  silent corruption rather than a stale display. It is in another repo.

Fix SHA: `87f3b936`, **`experiments`**. `master...experiments` was `0 1017` before this
commit, so promotion is a fast-forward and this SHA is already the master-side SHA — there is
no second SHA to record.
## References

- `src/librarian/tools/doctor.rs` — `scan_snapshot_drift`, the directional check.
- `src/librarian/catalog/augmentation.rs` — `snapshot_stale_note`; `AppendOutcome.warning`, the only
  surface that sees this direction; `body_claimed_indices`, the predicate a fix reuses.
- `docs/trackers/windows-platform-support.md` § History 2026-08-18 — the measured divergence.
- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` —
  the sibling that found it, and whose step 4 nearly published from the stale side.

## Fix provenance

- **SHA:** `87f3b936` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `4b01d47f3a38339d78c50c0ee56d0e775d59f9b2` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 4b01d47f3a38 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
