---
id: c9abdc5134bebce1
kind: bug
status: mitigated
title: edit_code replace refuses a declaration with a new name, so a rename that also changes the body takes two calls
tags:
- cluster/unclassified
- edit-code
closed: 2026-11-11
opened: 2026-11-11
owner: marius
related:
- be1321a7959b5b23
severity: low
unverified: 'The effect on agents is not measured: the hint carries the calls, but the refused call and the two that follow it remain, so the cost per refusal is unchanged in calls. A running server shows the new hint only after a rebuild and /mcp. The baseline for a re-measure is in Resume. Usage figures are from one checkout''s usage.db and cover every agent that ran against it, not one user. The ledger counts 25 sessions where my query gives 18 for the same window; not reconciled. The next-three-calls reading does not prove that each call repaired the refused edit. The time lost per refusal was not measured. rename was not tried in the exact half-edited state of the real case. The reproduction used a function with no callers.'
---

# BUG: edit_code replace refuses a declaration with a new name, so a rename that also changes the body takes two calls

## Summary

`edit_code(action="replace", symbol=X, body=<a complete declaration of Y>)` is refused when `Y` differs from `X`: "`replace` cannot rename". The refusal is deliberate and correct (it prevents deleting `X` and adding an unrelated `Y`), and its hint names the way out. The cost is real and recurs: the refused call, then a `rename` and a `replace`. In this project's usage log (2026-10-13 to 2026-11-11) the guard fired 46 times in 21 parent sessions (38 session or subagent ids). The frictions ledger already classes this gate as healthy, because the agent recovers on the next calls (see `## Evidence`); this record does not contest that. It holds the numbers re-counted today and the one option that would remove the refused call.

## Class fit

No class fits a refusal that is right and still costs a call every time. I use `cluster/unclassified` and not a forced class. It differs from a defect in that nothing is wrong with the answer; what can be improved is how often an agent has to be told, and the number of calls that the legitimate operation (rename a symbol and change what it is) takes.

## Symptom (Effect)

An agent that wants to rename a function and change its signature or body in one step sends `replace` with the new declaration. The call is refused (`File restored`, nothing changed). The agent then renames, then replaces the body, or falls back to `edit_file`.

## Reproduction

Scratch crate (`stale5`, rust-analyzer), session `5f715d8f`, 2026-11-11. `lib.rs` has `pub fn same_file_caller() -> u32 { target_fn(1) }` and no caller of it.

| # | Call | Result |
|---|---|---|
| 1 | `edit_code(replace, symbol="same_file_caller", body="pub fn renamed_caller(extra: u32) -> u32 {\n    target_fn(1) + extra\n}")` | refused: `edit_code replace('same_file_caller') was given a complete declaration, but it declares `renamed_caller` instead of `same_file_caller`. `replace` cannot rename — applying this would have removed `same_file_caller` and added a different symbol. File restored.` Hint: `To rename: edit_code(action="rename", symbol=..., new_name=...) — then replace the body in a second call if it also changed.` |
| 2 | `edit_code(rename, symbol="same_file_caller", new_name="renamed_caller")` | ok, 1 file, 1 edit |
| 3 | `edit_code(replace, symbol="renamed_caller", body=<the same declaration>)` | ok, lines 5-7 replaced |
| 4 | `cargo check --offline` in the crate | `Finished` with no error; `grep` finds `renamed_caller` once and `same_file_caller` nowhere |

So the path the hint names works: 3 calls instead of 1, the first one refused. In the real case that I hit the same day (`src/tools/symbol/call_edges/cache.rs`, renaming `add_signatures_column_if_missing` and adding a parameter) I did not try `rename`: the tree was already half edited and I changed the name on the declaration line with a single-line `edit_file`, then replaced the body. Whether `rename` would have worked in that state was not tested.

## Environment

codescout HEAD `1c96dd8f3`, Linux. Usage figures come from `.codescout/usage.db` of this checkout (read-only), which holds all agents and subagents that ran against it.

## Root cause

By design. Read at the bytes:

- `src/tools/symbol/edit_code.rs:1504`: `CorruptionVerdict::TargetRenamed(new_name)` builds the message above.
- `src/usage/db.rs:794`: the usage log classifies it as `err_family = "replace_would_rename"`, with a comment that distinguishes it from `replace_dropped_sibling`.
- `src/tools/symbol/tests.rs`, `a_replace_whose_body_renames_is_told_to_use_rename` (a test comment): the remedy in the hint is pinned by an assertion on "the callable form", "measured 2026-09-13 by deleting the routing from the hint, which left that assertion GREEN". So the guard and its hint have already been measured once.

## Evidence

`.codescout/usage.db`, `tool_calls`, window 2026-10-13 to 2026-11-11 (134 848 rows):

- `edit_code` calls: 4 840; with `action=replace`: 1 930.
- `err_family = 'replace_would_rename'`: **46 refusals**, in 38 distinct `cc_session_id` values and 21 distinct parent sessions (the first 36 characters; a subagent adds a suffix). That is 2.4% of the `replace` calls. In the ledger's own window (2026-10-12 to 2026-11-10) the same query gives 41 rows, the figure the ledger holds; I get 18 parent sessions there where the ledger says 25, and I did not find why (the ledger may count another session column).
- The 8 most recent (2026-11-09 to 2026-11-11) come from 6 different sessions or subagents, including mine (row 316218, 21:11:59). In the next three calls of the same session after each of them: 5 of the 8 contain a successful `edit_code rename` (two of them, rows 291045 and 295291, followed by a successful `replace`); 2 (rows 311109 and 316218, the latter mine) go through `edit_file` first; 1 (row 298147) shows a `recoverable_error` on an `insert` before `run_command` calls. I did not check that each of those calls repaired the refused edit.

Script: `usage-replace-would-rename.py` in the session scratchpad (not in the repo).

**Already measured by the project, and judged healthy.** `docs/trackers/frictions/index.md` lists `replace_would_rename` under "Healthy — not frictions" (gates that fire often while the agent recovers on the next call; "do not rank them by count"): 41 rows, 21 of 41 renamed then replaced within 3 calls. `docs/research/2026-11-11-usage-stats-baseline.md` has the same 41 rows. The archived bug `docs/issues/archive/2026-09-13-edit-code-replace-with-a-renamed-body-misattributes-the-error.md` fixed the message of this very gate. My sample (5 of the last 8 followed by a `rename`) agrees with the ledger's reading.

## Hypotheses tried

- The guard blocks a legitimate edit: **not supported**. In every sampled case the refused declaration named a different symbol, and the same operation succeeds as `rename` then `replace`.
- The hint is unclear: **not supported** for the cases I looked at (the follow-up calls show the agents did what the hint says), but I did not measure the time between the refusal and the recovery.

## Fix

Candidates listed when the bug was opened, none tried then: (1) let `replace` take an explicit `new_name` and perform `rename` and then the body replacement, in one call (a cost: `replace` is a single-file edit today and `rename` writes to other files); (2) keep the refusal and make the guide for `edit_code` say it before the first use, so the refused call is not needed; (3) accept the cost: 46 refused calls in 29 days across all agents.

Before this change the project's position was (3), because the frictions ledger lists the gate as healthy. Chosen on 2026-11-11, after the measurements in Evidence: none of the three candidates as listed. The refusal stays, and its hint carries the calls ready to send.

- `replace_would_rename_reason` in `src/tools/symbol/edit_code.rs` builds the hint for `CorruptionVerdict::TargetRenamed`: a `rename` call with this call's `symbol`, `path` and the last segment of the declared name path as `new_name`, then a `replace` call on the renamed symbol, addressed in the caller's own name-path form. `TargetRenamed` holds a name path (`Agent/new`), not a bare name, so the helper takes the last segment; building the second symbol from the payload would give `Agent/Agent/new` (`bug-fix-session-log` W-159).
- `replace_by_several_reason`, same file, does the same for `CorruptionVerdict::TargetReplacedBySeveral`: `replace` with only the target's declaration, `insert` after it, or `rename`.
- The refusal message text is unchanged, so `usage::db::normalize_err_family` still files it under `replace_would_rename`.

What this does not do: it does not remove the extra call. A caller still sends the refused `replace`, then `rename`, then `replace`. It removes the work of rebuilding the arguments of the calls that follow. Candidate (1), a `new_name` on `replace`, was rejected because it joins a single-file edit to a multi-file LSP write and leaves a half-done state if the second step fails. A repair that runs the rename on its own was rejected because the body has two readings (rename `X` to `Y`, or delete `X` and add an unrelated `Y`), and the guard exists for the second.

## Fix provenance

- **SHA:** `3cd876b1c` (`experiments`)
- **patch-id:** `44f4f399f9cc390dd0ade93131ce08ec14199295`
- **SHA:** `9c96ac3e1` (`experiments`)
- **patch-id:** `5884ca8ba391b363ce611c36b3d2b296f2a226a2`

## Tests added

In `src/tools/symbol/tests.rs`:

- `the_rename_refusal_hint_carries_both_calls_ready_to_send`: red on the old hint (`symbol=..., new_name=...`), green after.
- `the_rename_refusal_hint_for_a_nested_symbol_does_not_double_the_parent`: red on the old hint, green after.
- `the_rename_refusal_stays_in_its_telemetry_family_whatever_the_names_say`: a control. It passed before and after, and guards the `usage.db` family against names that hold another family's phrase.
- `the_several_symbols_refusal_hint_carries_its_calls_ready_to_send`: red on the old hint, green after.

No mutation was run on these tests.

## Workarounds

Send `rename` first, then `replace` on the new name. Or change the name on the declaration line with a single-line `edit_file`, then `replace` the body.

## Resume

Open as `mitigated`, low severity. NOT added to the frictions ledger as an `FR-N` entry: the ledger deliberately keeps this gate under "Healthy", and my numbers agree with it. Not measured: the time lost per refusal, whether the agents that fell back to `edit_file` did so because `rename` looked unsafe, how many of the 46 were followed by a successful repair of the same edit. Related: `docs/trackers/frictions/index.md` (`be1321a7959b5b23`), `docs/research/2026-11-11-usage-stats-baseline.md`, the archived bug of 2026-09-13, and the FR-13 detail file on a Python variant of the same message.

Next action: after a build that carries `3cd876b1c` and `9c96ac3e1` has run for about two weeks, re-run the query in Evidence on `.codescout/usage.db` for `err_family = 'replace_would_rename'`, counting only rows whose `codescout_sha` is that build or later.

Baseline, 2026-10-13 to 2026-11-11: 46 refusals in 38 sessions, 2.4% of 1932 `replace` calls; 18 followed by `edit_code rename` as the next `edit_code` call; 5 followed by an `edit_code` call that ended `recoverable_error` (4 `insert`, 1 `replace`). The hint can change the last number: a caller that has the arguments should send fewer wrong calls. It cannot change the first two.

Move to `fixed` only if the count of next calls that ended `recoverable_error` falls. If it does not, the hint is not what makes agents fail after a refusal. Candidate (2) or (3) in Fix is then the next choice. Other refusals of `replace` in `src/tools/symbol/edit_code.rs` still carry `symbol=...` placeholders, searched for on 2026-11-11: the shrink-guard hint (`edit_code(action="replace", symbol=..., path=..., body=..., force=true)`) and the `TargetDropped` hint (`symbols(symbol=..., include_body=true)`). They name calls whose arguments the tool also holds. They were not changed: neither belongs to the rename refusal this file records, and no count of them was taken.

## References

- `src/tools/symbol/edit_code.rs`: `CorruptionVerdict::TargetRenamed`, around line 1504
- `src/usage/db.rs`: the `replace_would_rename` family, around line 794
- `src/tools/symbol/tests.rs`: the test that pins the remedy, around line 7535
