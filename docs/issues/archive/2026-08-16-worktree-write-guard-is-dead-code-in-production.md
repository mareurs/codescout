---
id: a742a50ea6723daf
kind: bug
status: fixed
title: 'BUG: guard_worktree_write returns Ok on its first line in every real session, because the startup cwd fallback sets the flag it gates on'
owners:
- marius
tags:
- worktree
- guards
- dead-code
- doc-vs-code-drift
- workspace-activation
opened: 2026-08-16
owner: marius
severity: medium
---

## Summary

`guard_worktree_write` (`src/tools/core/guards.rs:14`) is documented as blocking
writes when *"worktrees exist AND the project was only implicitly set at
startup"*. It cannot: its first line returns `Ok(())` whenever
`is_project_explicitly_activated()` is true, and that flag is set to true **at
startup** for any resolvable project root — including the `current_dir()`
fallback, which fires in essentially every session.

Found 2026-08-16 while fixing the read-side half
(`docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md`).
That file's premise was that the write guard *"proves the condition is
detectable, and then only spends the detection on half the surface."* The
detection is spent on neither half.

## Symptom (Effect)

No write is ever blocked by worktree ambiguity. The guard's `RecoverableError`
— with its worktree list and its `workspace(action='activate')` hint — is
unreachable in production.

The user-visible consequence is the same silent wrong answer the read-side bug
described, except for writes: a session that has switched into a linked
worktree writes to the main checkout, and the guard built to stop exactly that
waves it through.

## Root cause

Two comments on one flag, saying opposite things.

`src/agent/mod.rs:478-480`:

```rust
// A project provided at startup (via --project or CWD) is treated as explicitly
// activated — the server operator already chose the write target.
let project_explicitly_activated = workspace.is_some();
```

`src/tools/core/guards.rs:12-13`:

```rust
/// Returns `RecoverableError` when writes should be blocked:
/// - Worktrees exist AND the project was only implicitly set at startup
```

`src/server.rs:1429` closes the loop — `project` is defaulted before it ever
reaches `Agent::new`:

```rust
let project = match project.or_else(|| std::env::current_dir().ok()) { ... };
let agent = Agent::new(project).await?;
```

So `workspace.is_some()` is true unless `current_dir()` itself fails. The only
production path to `false` is a server started with no resolvable cwd.

**`--project` and the cwd fallback are not the same thing, and the flag
conflates them.** An operator passing `--project` did choose. A cwd default is
not a choice — and in the worktree scenario it is actively misleading: the
harness switches trees *after* the process starts, so the startup cwd describes
the tree the caller has left.

## Evidence

Measured, not read. A probe inside a fixture with a seeded linked worktree
(`.git/worktrees/feat/gitdir`), built through `Agent::new(Some(root))`:

```
PROBE: explicitly_activated=true
       root=Some("/tmp/.tmpgHx4Jj/main")
       worktrees=["/tmp/.tmpgHx4Jj/wt-feat"]
```

Both facts the guard needs are present and correct; the flag suppresses it
anyway.

The behaviour is **pinned by an existing test** —
`project_explicitly_activated_with_project` (`src/agent/mod.rs:2426-2432`)
asserts `Agent::new(Some(dir))` yields `true`. This is settled behaviour with a
regression guard, not an oversight, which is why it was not flipped in passing.

## Fix

Option 1 from the three above: swapped the gate in `guard_worktree_write`
(`src/tools/core/guards.rs:14-45`) from `is_project_explicitly_activated()`
to `is_project_chosen_this_session()`. One line changed; the doc comment was
updated to name the distinction explicitly and cite this bug, since the old
comment was locally plausible while wrong.

The maintainer's call was made directly: yes, re-arm the guard, accepting
that this repo (two live worktrees right now) starts refusing un-activated
writes immediately. `is_project_chosen_this_session` already existed
(added 2026-08-16 for the read-side notice) and needed no changes — its own
doc comment already stated the exact distinction this fix relies on.
## Tests added

Three, in `src/tools/core/tests.rs`, reusing the `seed_linked_worktree` /
`rooted_ctx` fixtures already built for the read-side notice tests:

- `guard_worktree_write_refuses_when_only_resolved_at_startup` — the
  discriminating case this bug named: `Agent::new(Some(root))` (the
  startup/cwd-fallback path, not `activate`) with a seeded linked worktree.
  Asserts `is_project_explicitly_activated()` is still `true` (the old,
  wrong gate) AND `guard_worktree_write` now returns `Err`.
- `guard_worktree_write_allows_after_explicit_activate` — same fixture,
  plus an `activate` call. Asserts `Ok`.
- `guard_worktree_write_allows_when_no_worktrees_exist` — no worktrees
  seeded, `activate` never called. Asserts `Ok` — the regression guard
  against re-arming the write guard for every ordinary checkout, not just
  the worktree-ambiguous one.

All three pass. `cargo clippy --all-targets -- -D warnings` clean.
`cargo test --lib` (full suite): 3908 passed, 2 failed — both
`librarian::tools::link_scan::tests::*`, in files (`link_scan/mod.rs`,
`link_scan/extract.rs`) that were uncommitted, in-progress edits from a
concurrent session sharing this checkout at the time, not touched by this
fix. Confirmed via `git status` before drawing that conclusion, not assumed
from the test names.
## Workarounds

Call `workspace(action='activate', path=...)` after switching trees. As of
2026-08-16 the first read in this situation says so on its own — that notice is
the shipped mitigation for the read half and the practical mitigation for this
one too.

## Resume

Fixed and closed. The `project_explicitly_activated_with_project` regression
guard named in the original Resume note did not need to change meaning —
it still correctly pins `is_project_explicitly_activated()`'s own semantics
(true for both `--project` and the cwd fallback); this fix only changed
which flag `guard_worktree_write` reads, not what either flag means.
