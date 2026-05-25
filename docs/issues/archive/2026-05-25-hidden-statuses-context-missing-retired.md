---
status: fixed
opened: 2026-05-25
closed: 2026-05-25
severity: low
owner: marius
related: []
tags: [librarian, status-visibility, partial-fix]
kind: bug
---

# BUG: `HIDDEN_STATUSES` split-brain — `context.rs` omits `"retired"`, leaking retired artifacts into the gather path

## Summary

The librarian defined `HIDDEN_STATUSES` twice with divergent values (`find.rs`
3-element incl. `retired`; `context.rs` 2-element). Effect: the `context`
tool's **topic-query branch** included `retired` artifacts that the equivalent
`find` query hides. Narrow blast radius — the anchor branch ignores status by
design and the no-args branch is protected by an `eq: active` gate. Root cause:
incomplete F-11 fix (`2003e91a`) updated `find` but not `context`. Fixed by
hoisting a single shared constant.
## Symptom (Effect)

A tracker with `status: retired` (MRV in-place redirect) is correctly absent
from `find`, but appears in the `context` tool's output when queried by a
matching `topic`. The regression test makes it concrete — pre-fix:

```
thread '...topic_search_hides_retired_artifacts' panicked at context.rs:448:
assertion `left == right` failed: retired artifact must be hidden ...
  left: 2
 right: 1
```
## Reproduction
Commit: `772c92b6` (branch `experiments`).
1. Have an artifact with `status: retired` in scope — known instance:
   MRV-poc `2-lane-strategy.md` (in-place redirect, body forwards to canonical
   successor; see `docs/trackers/codescout-lessons-2026-05-20-session-log.md:581`).
2. `artifact(action="find", kind="tracker")` → retired item correctly absent.
3. Invoke the librarian `context` tool over the same scope → retired item
   present, indistinguishable from live trackers.

## Environment
codescout MCP server, Rust, `experiments` branch @ `772c92b6`. Not
platform-sensitive — pure constant divergence.

## Root cause

**Corrected after systematic investigation** — the original draft overstated
the blast radius (it claimed both filter sites leak generally). `context.rs::call`
builds `candidate_ids` through three mutually exclusive branches
(`src/librarian/tools/context.rs:48-302`):

- **anchor_id branch** — walks the link graph; applies **no** status filter (by
  design: an anchored neighborhood returns linked artifacts regardless of
  status). Unaffected.
- **topic branch** (`context.rs:92`) — applies `{"nin": HIDDEN_STATUSES}`. This
  is the **only** branch that leaks: with the 2-element list a `retired`
  artifact matching the topic passes the filter and is included, whereas the
  equivalent `find` (3-element list) hides it.
- **no-args / active-goals branch** (`context.rs:165`) — also references the
  2-element list, but is **harmless**: the branch hard-filters `status == "active"`
  (`context.rs:113`), so a `retired` artifact can never reach the redundant
  `nin` clause.

Underlying cause: two divergent `const HIDDEN_STATUSES` (find.rs 3-element,
context.rs 2-element). `retired` was added to `find.rs` by the F-11 fix
(`2003e91a`) but `context.rs` was missed — a "distance from change" drift. The
F-11 regression test was also deferred (LSP instability per the session log), so
nothing caught the gap.
## Evidence

### Divergent constant definitions
```
src/librarian/tools/find.rs:13
const HIDDEN_STATUSES: &[&str] = &["archived", "superseded", "retired"];

src/librarian/tools/context.rs:11
const HIDDEN_STATUSES: &[&str] = &["archived", "superseded"];
```

### Prior session-log note documenting the find.rs half of the fix
`docs/trackers/codescout-lessons-2026-05-20-session-log.md:581-583`:
> The `retired` row is MRV-poc's `2-lane-strategy.md`. ... Without `retired`
> in `HIDDEN_STATUSES`, this tracker surfaces in default `find kind=tracker`
> listings as if it were live work. **Probable cause:** Status enum drifted
> with MRV-poc convention; librarian `find.rs:13` not updated.

The session log records the `find.rs` fix but does not note the parallel
`context.rs` constant, which is why it was left stale.

## Hypotheses tried

1. **Hypothesis:** the divergence is intentional (context deliberately shows
   retired items). **Test:** read both definitions + all three context branches
   for a justifying comment. **Verdict:** rejected — both surfaces hide
   `archived`+`superseded` identically; `retired` is the same terminal class
   (`find.rs:37`); no comment justifies a difference.
2. **Hypothesis:** the context **topic branch** includes `retired` artifacts
   that `find` hides, due to the 2-element constant. **Test:** added regression
   test `topic_search_hides_retired_artifacts` (two artifacts, one retired, one
   live; topic query). **Verdict:** CONFIRMED — pre-fix `included_ids.len() == 2`
   (expected 1); the retired artifact leaked into the markdown. **Evidence:**
   `left: 2, right: 1` panic at `context.rs:448`.
## Fix

Shipped to master.
Root-cause fix — single shared constant, drift class removed:

- Added `pub(crate) const HIDDEN_STATUSES = ["archived","superseded","retired"]`
  with an archived-vs-retired doc comment in `src/librarian/tools/mod.rs` (the
  shared parent module of `find` and `context`).
- `src/librarian/tools/find.rs` and `src/librarian/tools/context.rs` now
  `use super::HIDDEN_STATUSES;` instead of each defining their own.

`cargo test -p codescout --lib librarian::tools::context` → 10 passed; full
`librarian::tools` → 332 passed; `cargo clippy` clean. Shipped: experiments-side
`c770cd6e`, master-side **`a96af3ae`** (cite the master SHA per CLAUDE.md §
"After cherry-pick").
## Tests added

`topic_search_hides_retired_artifacts` — `src/librarian/tools/context.rs`
(tests module, inserted after `topic_search_returns_matching_artifacts`).
Asserts a `retired` artifact matching the topic is excluded from both
`included_ids` and the rendered markdown. Failed pre-fix (`left: 2`), passes
post-fix. This is the regression test F-11 deferred.
## Workarounds
When auditing live work, trust `find` (not raw `context` output) for the
"is this retired?" question. Or `git mv` retired files to an `archive/` path
and use `status: archived` (codescout's own archival convention), which both
surfaces already hide.

## Resume

N/A — shipped to master (`a96af3ae`) and archived to `docs/issues/archive/`.
## References
- `src/librarian/tools/find.rs:13` — canonical (3-element) definition + `retired` doc at `find.rs:37`
- `src/librarian/tools/context.rs:11` — stale (2-element) definition
- `docs/trackers/codescout-lessons-2026-05-20-session-log.md:581-583` — prior note on the find.rs half
- Surfaced during 2026-05-25 brainstorm on agent-memory integration (Approach C, temporal/forgetting piece — adding `expired` to the hidden set would inherit this drift).
