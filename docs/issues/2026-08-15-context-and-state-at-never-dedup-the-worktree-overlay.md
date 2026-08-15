---
id: b5080e6c7a73ab44
kind: bug
status: open
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

Not observed at runtime — see Root cause. Predicted shape: a `context` bundle
whose `markdown` contains two `## <title>` sections for the same document, one
from the main checkout and one from the worktree shadow, differing by whatever
the worktree session has written. `included_ids` carries both ids with nothing
marking them as a lineage pair, and `overflow.omitted` is inflated because the
duplicate consumed budget.

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

Not yet implemented, and it needs a decision first: should `context` **drop** the
main twin (matching `find`), or **keep both and label them**? Keeping both is
arguable for an orientation tool whose stated purpose is broad visibility — but
then the pair must be marked, because two unlabelled near-identical sections are
worse than either one alone.

Whichever is chosen, `apply_scope`'s comment should stop naming `find` as the
place dedup happens and instead state the obligation on the caller, with the
callers that opt out saying why.

## Tests added

None yet. The regression test wants a worktree-session fixture — the current
`src/librarian/tools/context.rs` tests construct `main_root: None` only, so no
existing test can fail on this no matter how the overlay behaves.

## Workarounds

Pass `scope="umbrella"` or `scope="all"` on `librarian(action="context")` to skip
the over-selecting `project`/`repo` arms — though `all` still admits both rows by
admitting everything. There is no scope value that dedups; the dedup step simply
does not exist on this path.

## Resume

Run Reproduction steps 1-3 from a real worktree to convert the inferred mechanism
into a measured one, and record which of the two Fix options is wanted. If step 3
shows a single section rather than two, this file is a false alarm — say so and
close it `wontfix-false-alarm`, because the `apply_scope` comment would then be
describing a dedup that happens somewhere this investigation did not find.

## References

- `docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md` — the
  scope-width sibling, closed `wontfix` as intended behaviour.
- `docs/trackers/structural-debt-refactor.md` — SD-10, the prologue extraction
  that surfaced both.
- `src/librarian/tools/scope.rs` — `apply_scope` and the contract comment.

