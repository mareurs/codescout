---
id: f9608c9978ad8e69
kind: bug
status: fixed
title: 'BUG: the write-path ADR says every unpinned write already carries wrote_to; only repos with linked worktrees do'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- adr
- documentation
- tool-contracts
closed: 2026-10-16
opened: 2026-10-16
owner: marius
related:
- docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md
severity: low
---

# BUG: the write-path ADR says every unpinned write already carries `wrote_to`; only repos with linked worktrees do

## Summary
`docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md` says `annotate_write_root` promotes a bare `"ok"` to `{"status":"ok","wrote_to":<root>}` "for every unpinned write", and frames its own change as narrow because a path is "already half-present as `wrote_to`". The code returns early unless the repo has linked git worktrees, so in an ordinary single-checkout repo a write answered a bare `"ok"`, and the ADR's `annotate_write_path` is the first change to that response shape there.

## Symptom (Effect)
A reader of the ADR believes write responses already carried a path object everywhere and that `abs_path`/`rel_path` only widened it. In single-checkout repos the decision turned a bare `"ok"` into an object for every write — the no-echo convention changed more than the ADR says.

## Reproduction
At `a16f7d42`:
1. ADR lines 27-29: "promotes a bare `"ok"` to `{"status":"ok","wrote_to":<root>}` for every unpinned write"; line 56-57: "half-present as `wrote_to`".
2. `src/tools/core/types.rs:524-527`: `fn annotate_write_root(...) { if crate::util::path_security::list_git_worktrees(root).is_empty() { return; } … }`; its doc comment (`:521-523`) says the single-checkout case is untouched.

## Environment
codescout, branch experiments, HEAD `a16f7d42`. ADR `status: draft`; its decision shipped.

## Root cause
The statement was false when written: the worktree gate existed before the ADR (added in `22e60ee9`, 2026-08-27, per the verifier). The ADR states a value that is right for repos with linked worktrees under the scope word "every".

## Evidence
See Reproduction. Provenance: found by the effort-steering Stage A critic experiment (on the ADR at its own commit `1291f468`), verified there, and re-checked at HEAD on 2026-10-16.

## Hypotheses tried
N/A.

## Fix

Fixed 2026-10-16 by amending the ADR, not the code. At the bytes: `src/tools/core/types.rs` `annotate_write_root` begins `if crate::util::path_security::list_git_worktrees(root).is_empty() { return; }` (gate added in `22e60ee9`, 2026-08-27, before the ADR), so the ADR was the wrong side. `docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md`: Context and the "narrow widening" sentence now say `wrote_to` exists only in repos with linked worktrees, and a new `## Amendment 2026-10-16` states that in a single checkout `annotate_write_path` is the first change of the write response shape (bare `"ok"` to object). The decision is unchanged.

Fix commit `2d56edc2` on branch `worktree-agent-aa532b30b45f11948` (to be merged to experiments), patch-id `9787be24db6a362ca759ab8dee4085108e823018` (`git show 2d56edc2 | git patch-id --stable`; the commit also carries the two sibling doc fixes). `src/tools/core/types.rs` was not touched.

## Tests added

N/A — documentation. The behaviour described (no annotation when `list_git_worktrees` is empty) is the code's existing, documented behaviour; nothing in the code changed.

## Workarounds
N/A.

## Resume

N/A — fixed. Archive once the fix is on experiments with the gate green.

## References
- `docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md`
- `src/tools/core/types.rs` (`annotate_write_root`, `annotate_write_path`)
