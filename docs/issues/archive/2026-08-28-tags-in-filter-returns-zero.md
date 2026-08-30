---
id: 1d085bcddf13d685
kind: bug
status: fixed
title: 'BUG: filter tags.in returns zero while tags.contains finds the row, and the guide teaches the broken form'
tags:
- librarian
- find
- filter
- misleading-zero
closed: 2026-08-30
opened: 2026-08-28
owner: marius
severity: med
---

# BUG: `filter={"tags": {"in": [...]}}` silently returns zero, while the guide teaches it as *the* way to filter by tag

## Summary

`artifact(action="find", filter={"tags": {"in": ["handoff"]}})` returns **0** for an
artifact whose catalog row demonstrably carries that tag. The `contains` form of the same
query returns it correctly.

This is the misleading-zero class: nothing errors, and the answer reads as "no artifact has
this tag" rather than "this operator does not work on this field". `get_guide("librarian")`
§ *Filter Syntax* offers `{"tags": {"in": ["foo", "bar"]}}` as its worked example for tag
filtering, so the documented path is the broken one.

## Symptom (Effect)

Same artifact, same session, two operators:

```
filter={"tags": {"contains": "handoff"}}   -> count: 1   (correct)
filter={"tags": {"in": ["handoff"]}}       -> count: 0   (wrong)
```

`artifact(action="get")` on that id shows the tag is present in the row:

```json
"tags": ["handoff", "bug-ledger", "cross-machine", "session-state"]
```

Both probes ran with default scope against the active project, no `kind`/`status`
shortcut, so no AND-interaction is involved. Adding `kind="tracker"` does not change
either result.

## Reproduction

```
artifact(action="create", kind="tracker", rel_path="docs/trackers/x.md",
         title="x", body="# x", tags=["zzz-probe"])
artifact(action="find", filter={"tags": {"contains": "zzz-probe"}})   -> 1
artifact(action="find", filter={"tags": {"in": ["zzz-probe"]}})       -> 0
```

Observed 2026-08-28 on `experiments` @ `2243b477`, against a tracker created minutes
earlier in the same session (so no reindex-lag explanation: the row `get` returns already
has the tags).

## Environment

- branch `experiments`, commit `2243b477`
- artifact created via `artifact(action="create", tags=[...])`, then `status`/`owners`
  patched via `artifact(action="update")` — tags untouched by the update.

## Root cause

**Established** (was "not established" — the two readings below are resolved in favour
of reading 1, and a third failure the file never named was found alongside it).

`tags` and `owners` are stored as JSON arrays, but the array-column branch in
`src/librarian/filter.rs` was gated on the **operator** rather than the **column**:

```rust
if op == LeafOp::Contains && is_array_col
```

So `in` and `nin` fell through to the scalar path and emitted `tags IN (?)`, comparing
the column's raw JSON *text* — `["session-log","bug-fix"]` — against a scalar. Never
equal.

**The `nin` half is worse, and this file did not name it.** `tags NOT IN (?)` is true for
every row whose JSON text differs from the scalar, so `nin` returned **everything**,
including rows that do hold the listed tag. `nin` is the operator `find`'s own
archived-hide uses, so this was not a hypothetical path. The two failures are silent and
point in opposite directions: a zero that reads as "nothing matches", and a full result
set that reads as "correctly filtered".

Of the two readings this file offered, **reading 1 was taken**: `in` on an array column
means membership-any, matching `contains`. Reading 2 (reject the combination) was not
chosen, so no `RecoverableError` routing was added and none is owed.
## Hypotheses tried

- **"The catalog row lacks the tags."** Refuted: `artifact(action="get")` on the id returns
  all four tags.
- **"A `kind`/`status` shortcut is ANDing it away."** Refuted: the bare filter with no
  shortcut behaves identically.
- **"Reindex lag."** Not applicable: `contains` finds the same row in the same session.

## Fix

**Fixed on `experiments`** — `9e4e2d36044e276f01ca29306d66b97057b878fd`, patch-id
`cfac211d37020aa4815ce7e0277c15704559ea13`. `src/librarian/filter.rs`, +193/−25.

The branch now gates on the **column** and lets the operator select the shape: `EXISTS`
over `json_each` for `in`, `NOT EXISTS` for `nin`. Both `in` paths share one
`in_list_params` helper — deliberately, since the defect *was* a second `in` code path
that never learned what the first one knew. The in-memory `eval` twin had the same gap
and got the matching type-driven fix; the two engines answer the same AST and must not
disagree.

One semantic decision was asserted rather than inherited: holding none of the listed tags
includes holding **no** tags at all, so an empty `tags` array matches `nin`.

**No guide edit was owed.** This file predicted one — `get_guide("librarian")` § *Filter
Syntax* teaches `{"tags": {"in": [...]}}` as its worked example — but under reading 1 the
taught form became correct by construction, so the doc needed no change.
## Tests added

Three, in `src/librarian/filter.rs`, all passing (re-run 2026-08-30, 26/26 in
`librarian::filter`):

| Test | Covers |
|---|---|
| `tags_in_matches_a_row_holding_any_listed_tag` | `in` end-to-end against a real `json()` column — the defect yields `[]` |
| `tags_nin_excludes_only_rows_that_hold_a_listed_tag` | `nin` — the defect yields `[t1,t2,t3]` |
| `eval_in_on_an_array_field_means_membership_like_the_sql_side` | eval/SQL parity: hit, miss and complement |

**Why the existing differential test could not have caught this.**
`eval_matches_compile_on_fixture` requires both engines to agree on one AST and is the
strongest-looking guard in the file — but its fixture is
`CREATE TABLE e (id, status, confidence, title)`, with **no array column**, so it cannot
reach the branch. It still passes unchanged. A test's form is no evidence about its reach.
## Workarounds

None needed as of `9e4e2d36`. Before the fix: `{"tags": {"contains": "<one-tag>"}}`, and
for multiple tags an `or` of several `contains` leaves.

Note the pre-fix advice was only half a workaround — it routed around the `in` zero but
said nothing about `nin` returning everything, because this file had not found that half.
## Verification before closing

The fix landed under a `fix(librarian): ...` commit message that did not flip this file's
status, so it sat **zombie-open** for the rest of the day — the exact fix-then-forget
class CLAUDE.md's verify-open cadence exists to catch. Re-verified against the live
release binary on 2026-08-30 before closing, rather than trusting the commit message:

| Probe | Result |
|---|---|
| `in ["shrink-guard"]` vs `contains "shrink-guard"` | identical id sets — the reported zero is gone |
| `in ["shrink-guard","parity"]` | exactly the union of the two `contains` queries |
| a row holding `shrink-guard`, AND `nin ["shrink-guard"]` | 0 — correctly excluded |
| the same row, AND `nin ["zzz-no-such-tag"]` | 1 — correctly retained |

The fourth probe is what makes the third mean anything: a `nin` that excluded *everything*
would also have returned 0. A bare row count could not discriminate them — the unfiltered
`nin` query hits the 200-row page cap, so its count is uninformative by construction.
## References

- `get_guide("librarian")` § *Filter Syntax* — the contract, and the source of the
  example this file called misleading. Correct by construction since `9e4e2d36`.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the standing rule that a
  suspicious zero must name what it examined; this was a zero that named nothing.
- `open-issue-work-queue:BL-47` and `bug-fix-session-log:W-73` — where the fixing session
  logged this.
