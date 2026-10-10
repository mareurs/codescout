---
id: d6b67f83ff51c3f4
kind: bug
status: fixed
title: 'BUG: the commit-or-timestamp refusals in state_at and workspace_state_at are bare anyhow errors with no hint, and one names a tool that does not exist'
tags:
- librarian
- error-handling
- state_at
- cluster/unclassified
closed: 2026-10-10
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-required-param-corrected-call-hints.md
severity: low
---

# BUG: the commit-or-timestamp refusals in `state_at` and `workspace_state_at` are bare `anyhow!` errors with no hint, and one names a tool that does not exist

## Summary

`doc(action="state_at")` and `librarian(action="workspace_state_at")` refuse a call that supplies both or neither of `commit` and `timestamp`. Both refusals are `anyhow!` errors. The caller gets the bare message and no corrected call. `state_at` has a second one that tells the caller to "run librarian_reindex", a name that is not a tool.

## Symptom (Effect)

Measured 2026-10-06 at `10e935e3`. Each call came back as a tool error block holding only the message. The `RecoverableError` calls made in the same session came back as `{"ok": false, "error": …, "hint": …}`.

```
doc(action="state_at", id="1b5a080fe2efcb6b")
→ supply exactly one of `commit` or `timestamp`

librarian(action="workspace_state_at")
→ supply exactly one of `commit` or `timestamp`

doc(action="state_at", id="1b5a080fe2efcb6b", commit="0000000")
→ commit 0000000 not indexed; run librarian_reindex
```

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

Run the three calls above against the live server, pinned with `workspace="/home/marius/work/claude/codescout"`. All three are reads.

## Environment

Linux, MCP over stdio, `experiments` at `10e935e3`.

## Root cause

Measured 2026-10-06 (the three calls) and read from the source:

- `src/librarian/tools/state_at.rs:166` and `src/librarian/tools/workspace_state_at.rs:93` return `Err(anyhow!("supply exactly one of `commit` or `timestamp`"))`.
- `src/librarian/tools/state_at.rs:46` returns `anyhow!("commit {hash} not indexed; run librarian_reindex")`. `librarian_reindex` appears in the tree only as a test name (`src/server.rs:8450`). The call is `librarian(action="reindex")`.
- The same function already builds a hinted error one screen above: `src/librarian/tools/state_at.rs:157-160` uses `RecoverableError::with_hint` with an `e.g.` call for the missing `id`.
- `get_guide("error-handling")` says an input-driven failure the caller can fix is a `RecoverableError`. Both xor refusals are that.

Other bare errors in `state_at.rs` that are input-driven: line 49 (`has no authored_at timestamp`), lines 50-52 (an ambiguous prefix, which already says what to do) and line 177 (`artifact not found`). They were not changed or measured here.

## Evidence

The archived record `docs/issues/archive/2026-09-24-residual-required-param-corrected-call-hints.md` says in its Resume: "the "exactly one of commit/timestamp" refusals ... are plain `anyhow!` errors with no corrected call, a candidate for their own bug."

## Hypotheses tried

None needed.

## Fix

Not started. Build both refusals with `RecoverableError::with_hint`. Name the right call in each hint, for example `doc(action="state_at", id="<16-hex>", commit="<sha>")` and `librarian(action="workspace_state_at", commit="<sha>")`. Replace `librarian_reindex` with `librarian(action="reindex")`.

## Tests added

N/A — not fixed. A test should assert on the hint text, as the table test `every_required_param_failure_names_its_action_and_routes` does for the required-param sites.

## Workarounds

None needed. Supply exactly one cutoff.

## Resume

The two xor sites are the whole request. The `librarian_reindex` name is a one-word change in the same file.

## Fix provenance (2026-10-10)

This section supersedes `## Fix`, `## Tests added` and `## Resume` above, which describe the state before the fix.

- **SHA:** `78520297` (`experiments`)
- **patch-id:** `372624bd0d9d0bfa6e3af0b27d7e5d5578cf8c55`

The three bare `anyhow!` refusals are now `crate::tools::RecoverableError` with a hint that carries a corrected call, and the refusal conditions are unchanged. The exactly-one-of-`commit`-or-`timestamp` refusal in `state_at` names `doc(action="state_at", ...)`, and the same refusal in `workspace_state_at` names `librarian(action="workspace_state_at", ...)`, each showing both ways to supply the one cutoff. The unindexed-commit refusal no longer names `librarian_reindex`, which was only a test name and never a tool; it names `librarian(action="reindex")` and offers `timestamp` as the cutoff that needs no index. The librarian tools already use the host `RecoverableError` in many places, and the adapter's `bridge_recoverable_error` passes it through unchanged. The third `anyhow!` in the report (`commit {hash} has no authored_at timestamp`) is a stored-data fault, not a caller error, and was left as it is.

Tests: `an_unindexed_commit_is_recoverable_and_names_the_real_reindex_action`, `requires_exactly_one_of_commit_timestamp` (now both the "both" and "neither" cases, asserting the message, the action named and both example arguments) and `the_xor_refusal_is_recoverable_and_routes_to_this_action`. They assert on the error text and the hint, not on `is_err()`. Mutation check, applied and restored byte-identical: putting `librarian_reindex` back into the hint fails the unindexed-commit test with the exact message. The full gate ran green on the merged tree (lean 4471, default 6744, Python 1010 passed, 0 failed).

## References

- `docs/issues/archive/2026-09-24-residual-required-param-corrected-call-hints.md`, Resume (1).
- Cluster: filed as `cluster/unclassified`. Classes checked: `IC-22` (hint composed without the request) needs a hint to exist, and here there is none. `IC-14` (guard narrower than its name) needs a named guarantee; the table test pins a list of sites and does not claim these two. Candidate class for a second instance: *an input-driven refusal is raised as a fatal error type, so it carries no recovery*.
