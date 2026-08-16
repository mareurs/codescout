---
id: '1523556488a95de2'
kind: bug
status: open
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

Not implemented — deliberately. **Re-arming a dormant refusal path is a
behaviour change, not a bug fix**, and this repo currently has two live
worktrees, so turning the guard on starts refusing writes here immediately.
That is the maintainer's call.

Three options:

1. **Gate on the in-session choice.** `Agent::is_project_chosen_this_session()`
   already exists (added 2026-08-16 for the read notice): false at startup
   however the root was found, true only after an `activate` call. Swapping
   `guard_worktree_write` onto it makes the guard mean what its doc comment
   says. Cost: writes in any worktree-bearing repo refuse until `activate` is
   called — loud and cheap, but it is a new refusal in an existing workflow.
2. **Keep `--project` as a choice, demote only the cwd fallback.** Thread the
   distinction from `run_server` into `Agent::new`. More faithful to the
   original intent; more plumbing.
3. **Delete the guard and rely on the read-side notice.** Honest about what
   ships today, and the notice already names the ambiguity on the first read.
   Loses the write-time backstop.

Option 1 is the smallest correct change; option 2 is the most faithful. Both
need the `project_explicitly_activated_with_project` test re-read, since it
pins the current meaning.

Whatever is chosen, **the two comments must stop contradicting each other** —
that contradiction is the whole bug, and it survived because each is locally
plausible.

## Tests added

None yet. When implemented, the discriminating case is a fixture with a seeded
linked worktree built via `Agent::new(Some(root))` — i.e. the startup path, not
`activate` — asserting the write is refused. No current test exercises that
combination, which is why the guard could go dead without anything failing.

## Workarounds

Call `workspace(action='activate', path=...)` after switching trees. As of
2026-08-16 the first read in this situation says so on its own — that notice is
the shipped mitigation for the read half and the practical mitigation for this
one too.

## Resume

Open, and the blocking question is a decision, not an investigation: which of
the three options above, given that option 1 starts refusing writes in this
repo the day it lands.

Do not re-derive the evidence — the probe result and the three code sites above
are the whole picture. The one thing worth re-reading before deciding is
`project_explicitly_activated_with_project` (`src/agent/mod.rs:2426`), because
it is the regression guard that will have to change meaning.

