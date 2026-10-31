---
id: c035180192023734
kind: bug
status: mitigated
title: 'RESIDUAL: Extend the cross-host high-water guard and entry_defined_twice detection to params ledgers (params allocation path and ''| PREFIX-N |'' index tables)'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Extend the cross-host high-water guard and entry_defined_twice detection to params ledgers (params allocation path and '| PREFIX-N |' index tables).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` (status `mitigated`):

> Three open gaps, not one. (1) Peer-ahead direction: detection is complete but prevention is partial by construction and stays so — the guard catches only the direction where THIS host is ahead. `@{upstream}` is a remote-tracking ref, stale until someone fetches, so a peer who allocates and pushes while this host has not fetched still collides undetected, and an unpushed peer commit is unreachable by any local check. That is why status is `mitigated`, not `fixed`. (2) Params-ledger gap, concrete surface: both components cover PROSE ledgers only. Component B's guard sits inside the `a.entry_collection.is_none()` branch, so the params allocation path has no upstream guard. Component A is built on `entry_sections` (`## PREFIX-N — Title` headings), so a params ledger whose body carries an index TABLE rather than headings yields no EntrySection and a duplicate there is neither prevented nor detected. This is not closed by 'rows are machine-local': a params ledger's `entry_high_water_<PREFIX>` frontmatter AND its `| PREFIX-N |` index table are BOTH committed and BOTH merge — `body_claimed_indices` (`src/librarian/catalog/augmentation.rs:1282-1292`) counts index rows toward allocation, so the committed surface collides exactly like the prose one and is simply uncovered by either component. (3) Detector's real-world track record: a whole-branch review ran entry_defined_twice against this repo's corpus (1451 markdown files, 37 declared ledgers) and found 3 findings, all false positives, all sub-headings repeating their own entry's token (e.g. `### A-28` nested under `## A-28`); those are now excluded (0cb617cc) by dropping a definition strictly deeper than, and inside the span of, an earlier definition of the same token. Post-fix measured corpus output is zero findings — the check has never yet fired on a real collision; its true-positive rate is validated only against fixtures, not against a real occurrence.

## Fix

**Re-verified at the bytes 2026-10-30 (Group F pass); the bug text is half stale.**

- *Component B no longer exists, for prose or params ledgers.* `append_entry.rs` (the prose branch, `if a.entry_collection.is_none()`) carries the comment `ALLOCATE OPTIMISTICALLY` — the unpushed-ledger refusal was removed 2026-09-11 (`docs/issues/archive/2026-09-10-append-entry-refuses-on-unpushed-commits-with-a-remedy-no-session-may-perform.md`) because its only remedy was a push no session may perform, and the collision is now caught at `pre-push` (`scripts/pre-push-foreign-session-guard.sh`). So "extend the upstream guard to the params path" has no guard to extend; that half is **withdrawn, not done**. Re-adding allocate-time refusal would reverse a recorded decision and is not proposed.
- *Component A's params-ledger gap was real* and is fixed here: `duplicate_definitions` reads `## PREFIX-N` headings only, so a duplicated `| PREFIX-N |` index row was invisible.

**Fix.** New `duplicate_index_rows` (`src/librarian/tools/doctor.rs`, beside `duplicate_definitions`), called from `scan_entry_defined_twice` under the same `entry_defined_twice` check, with the message labelled `(as | PREFIX-N | index-table rows)`. It reads the same leading-cell surface `body_claimed_indices` counts toward allocation. Two scoping rules, the second found by running the first version over this repo's `docs/`: rows are compared with rows only (a heading plus its own index row is the documented flow), and **within one contiguous table only**, because `docs/trackers/operator-rules.md` lists OP-1..OP-4 in an index and again in an analysis table (8 false positives in the first version). Frontmatter and fenced lines are skipped.

**Not covered.** The detector has still never fired on a real collision; only fixtures. A duplicate row split across two tables is not seen, by design of the false-positive rule. The peer-ahead prevention gap (1) is unchanged and stays a `pre-push` concern. Observed red only at unit level: `duplicate_index_rows_finds_a_row_token_listed_twice` failed against an empty-returning stub with `left: [] right: [("R-7", [5, 7])]`; the integration test `entry_defined_twice_fires_on_a_duplicated_index_row` was written with the implementation and its pre-fix red was not separately observed.

## Tests added

`duplicate_index_rows_finds_a_row_token_listed_twice`, `duplicate_index_rows_is_silent_on_the_legitimate_shapes` (heading plus row, undeclared prefix, prose mention, `R-1` vs `R-12`, frontmatter, two separate tables, fenced), `entry_defined_twice_fires_on_a_duplicated_index_row` — all in `src/librarian/tools/doctor.rs`. The 4 existing `entry_defined_twice*` and 9 `duplicate_definitions*` tests still pass.

## References

- `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` — parent
