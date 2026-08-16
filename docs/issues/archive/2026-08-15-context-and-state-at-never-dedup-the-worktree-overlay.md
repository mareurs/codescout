---
id: d31233700ca979c2
kind: bug
status: fixed
title: 'BUG: context and workspace_state_at never dedup the worktree overlay apply_scope deliberately over-selects, so one artifact can appear twice unlabelled'
tags:
- librarian
- worktree
- overlay
- context
- contract-violation
topic: librarian worktree overlay
opened: 2026-08-15
owner: marius
related:
- '66e8a27d0ed97a90'
severity: medium
---

# BUG: context and workspace_state_at never dedup the worktree overlay apply_scope deliberately over-selects, so one artifact can appear twice unlabelled

## Summary

`apply_scope` widens `project` and `repo` scope in a worktree session to match
**both** the worktree prefix and the main-checkout prefix, and says in a comment
that the resulting shadow-vs-main duplication is removed downstream. Two of its
four callers never remove it. `librarian(action="context")` can therefore pack
two versions of the same artifact into one bundle — both read from disk, both
charged against the same token budget, neither labelled as a duplicate of the
other.

This is **not** the scope-width question from
`docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md` (closed
`wontfix`: `context` reaching every project is intended). This is about two
versions of *one* artifact, and it survives that decision unchanged.

## Symptom (Effect)

**Now observed, by mutation.** The report was filed as inferred-not-measured;
both halves have since been reproduced against real handler code.

Two distinct defects, not one:

- **C1 — undeduped main twin (as filed).** A worktree session's `context` bundle
  renders one document twice. Measured: `included_ids` = `["main", "shadow"]`
  where `["shadow"]` is correct. Both were read from disk, both charged against
  the same token budget, neither labelled.

- **C2 — foreign shadow rows leak in (NOT in the original report).**
  `exclude_worktrees` is computed for **every** session, not only worktree ones:
  an in-repo layout (`<main>/.worktrees/<n>`) puts another session's shadow rows
  underneath the main checkout's own path prefix, so a plain main-checkout query
  matches them. Measured: `included_ids` = `["mine", "foreign"]` where
  `["mine"]` is correct.

C2 is the more reachable half — it needs no worktree session at all, only a
registered worktree and a row under it. The Reproduction section below was built
entirely around C1, which is why it concluded the bug could not be reproduced.

`artifact(action="find")` on the same catalog state returns the shadow only,
annotated `"overlay": true`.
## Reproduction

Not yet reproduced — no worktree session with shadow rows was active while this
was found. Best lead, in order:

1. From a linked worktree, mutate any main-root artifact (`artifact(action="update", ...)`)
   to trigger fork-on-first-write, seeding a shadow row plus a `worktree_of` link.
2. `artifact(action="find", scope="project")` — expect one row, `"overlay": true`.
3. `librarian(action="context", topic="<matching that artifact>", scope="project")` —
   expect the same artifact rendered twice in `markdown`, and two ids in
   `included_ids`.

Step 2 is the control: it establishes that the catalog really holds both rows and
that the dedup step is what differs.

## Environment

linux, MCP stdio, project `codescout`, branch `experiments` @ `35c748ac`.
Requires a registered worktree session with at least one forked artifact.

## Root cause

`apply_scope` over-selects on purpose. In its `Scope::Project` and `Scope::Repo`
arms it ORs the worktree prefix with the main prefix whenever `cp.main_root` is
set, and both arms carry the same comment naming the obligation it is handing
downstream (`src/librarian/tools/scope.rs:80`, `src/librarian/tools/scope.rs:91`):

```
// Overlay: a worktree session sees its own rows AND the main
// checkout's rows; shadow-vs-main dedup happens post-query in find.
```

`shadow_main_pairs` — the function that discharges that obligation — is called in
exactly two files (`src/librarian/tools/find.rs`, `src/librarian/tools/get.rs`).
`src/librarian/tools/context.rs` contains no call to it, no `exclude_worktrees`
computation, and no occurrence of the string `worktree` at all; its three
`apply_scope` call sites each pass an empty slice.
`src/librarian/tools/workspace_state_at.rs` likewise passes an empty slice and
never dedups.

`scope="all"` reaches the same outcome by a different route: no clause at all, so
both rows are in the pool regardless of the worktree arms.

**Inferred from the code cited above and from the contract comment — NOT measured
at runtime.** The reachability argument is that the overlay machinery is live
(fork-on-first-write, `merge_worktree`, the `find`/`get` dedup) and that
`context` sits on the same catalog with none of it; but no bundle containing a
duplicated artifact has been observed. Treat the mechanism as a hypothesis until
step 3 of Reproduction runs.

## Evidence

Callers of the dedup helper, whole-tree:

```
src/librarian/tools/worktree.rs   3   (definition + internals)
src/librarian/tools/find.rs       2
src/librarian/tools/get.rs        2
```

Occurrences of the contract comment, whole-tree — twice, both in `apply_scope`:

```
src/librarian/tools/scope.rs:80
src/librarian/tools/scope.rs:91
```

A case-insensitive search for `worktree` across
`src/librarian/tools/context.rs` returns zero matches. The only overlay-adjacent
token in that file is a test fixture setting `main_root: None`
(`src/librarian/tools/context.rs:855`) — i.e. the one worktree-shaped input its
tests construct is explicitly *not* a worktree session.

## Hypotheses tried

1. **Hypothesis:** `context` dedups somewhere other than via `shadow_main_pairs`.
   **Test:** read `call` in full (379 lines) this session; grepped the file for
   `shadow_main_pairs|overlay|dedup|main_root` and for `worktree`.
   **Verdict:** rejected — candidate discovery goes straight to `find`/
   `semantic_find`, then to `rows_map`, then to rendering, with no dedup step.

2. **Hypothesis:** the duplication is unreachable because a shadow and its main
   twin never satisfy the same candidate query.
   **Verdict:** rejected on the `apply_scope` arms — they are written precisely
   to admit both, and the comment says so. Not yet confirmed end-to-end, which is
   what Reproduction step 3 is for.

3. **Hypothesis:** for `workspace_state_at` this is intended, since a forensic
   time-travel tool may legitimately want to show both versions as of a commit.
   **Verdict:** deferred — plausible, but nothing in the code says so, and the
   shared comment in `apply_scope` names `find` as the place dedup happens
   without carving out any caller.

## Fix

**Implemented 2026-08-16.** Both filters `find` already applied are now applied
at every caller, and the inferred mechanism was converted into a measured one
first (see Symptom).

Three shared helpers in `src/librarian/tools/worktree.rs`, so the spelling lives
in one place instead of being re-derived per caller:

- `overlay_exclusions(cat, current)` — every ACTIVE registration except the
  caller's own, for `apply_scope`'s `exclude_worktrees`.
- `shadowed_main_ids(cat, current)` — main ids this session's worktree
  supersedes, to drop from a result set. Empty for a non-worktree session.
- `is_under_any(abs_path, roots)` — for candidate paths that never passed
  through a scope clause.

Applied at all five previously-undeduped call sites:

| File | Sites | Note |
|---|---|---|
| `context.rs` | 3 | plus a post-filter at the point every candidate path converges, because the anchor-graph and semantic paths bypass the scope clause entirely |
| `workspace_state_at.rs` | 1 | |
| `link_scan/mod.rs` | 1 | **not named in the original report** — and the only one that WRITES, so an undeduped pair there materializes `cites` edges out of both copies |

`apply_scope`'s comment no longer claims `find` performs the dedup; it states
the obligation on the caller.

**`workspace_state_at` dedups rather than shows-both** — resolving the
"deferred" verdict in Hypotheses tried #3. A shadow *is* the artifact's state
for this session, so replaying its stale main twin beside it reports two states
for one document. Show-both would be defensible only if the pair were labelled
as a lineage pair; an unlabelled duplicate is worse than either choice. Recorded
in the code comment so the next reader sees a decision, not an omission.
## Tests added

Two, in `src/librarian/tools/context.rs`, each mutation-verified to fail against
its own defect and pass against the other:

- `a_worktree_session_drops_the_main_twin_its_shadow_supersedes` — C1. With
  `shadowed_main_ids` neutralized: `got ["main", "shadow"]`.
- `a_main_checkout_never_pulls_in_another_worktrees_shadow` — C2. With
  `overlay_exclusions` neutralized: `got ["mine", "foreign"]`.

The note in the original report that "no existing test can fail on this no
matter how the overlay behaves" was correct: every pre-existing `context.rs`
fixture sets `main_root: None`, i.e. explicitly not a worktree session. Both new
tests set it to `Some(..)` or register an active worktree.
## Workarounds

Pass `scope="umbrella"` or `scope="all"` on `librarian(action="context")` to skip
the over-selecting `project`/`repo` arms — though `all` still admits both rows by
admitting everything. There is no scope value that dedups; the dedup step simply
does not exist on this path.

## Resume

Closed. Both halves measured, fixed, and covered. Gate green on `experiments`
(3779 lib tests, clippy `-D warnings` clean).

One thing this bug taught that outlives it: the report's own Reproduction
recipe was the reason it looked unreproducible. It demanded a live worktree
session with a forked artifact — expensive setup nobody had — while the more
reachable half (C2) needed neither, and a unit fixture reproduced both in
milliseconds. Reaching for the runtime recipe first is what kept this filed as
"inferred".
## References

- `docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md` — the
  scope-width sibling, closed `wontfix` as intended behaviour.
- `docs/trackers/structural-debt-refactor.md` — SD-10, the prologue extraction
  that surfaced both.
- `src/librarian/tools/scope.rs` — `apply_scope` and the contract comment.
