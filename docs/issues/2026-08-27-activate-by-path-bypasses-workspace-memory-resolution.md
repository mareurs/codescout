---
id: '5b4a1b43dc10b052'
kind: bug
status: open
title: 'BUG: activate by PATH bypasses workspace memory resolution, so a sub-project reports an empty memory set it does not have'
tags:
- memory
- workspace
- multi-project
- activate
- false-negative
closed: null
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
severity: medium
unverified: Found by probe while fixing the sibling id-route bug, not from a session report — no evidence any real session has hit it. The mechanism is confirmed and has an executable (ignored) reproduction; what is undecided is the routing semantics, which is a design question with an argument on each side and no owner yet.
---

## Summary

`workspace(action="activate")` resolves a sub-project's memories two different ways
depending on whether you name it by **id** or by **path**, and the path route reports an
empty memory set for a project that holds topics.

The id route was fixed 2026-08-27 (see
`docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`).
This file covers the half deliberately left out of that fix, because closing it is a
routing decision rather than a reader correction.

## Symptom

Same workspace, same sub-project, same memories on disk — two answers:

| Call | `memories` |
|---|---|
| `activate(path="svc")` (bare id) | `["architecture"]` |
| `activate(path="/abs/path/to/svc")` | `[]` |

The zero is silent. Nothing in the response says the memory set is empty *because of how
the project was addressed*, so a caller reads it as "this project has no memories."

## Reproduction

Executable, in-tree, and currently `#[ignore]`d so it does not break the gate:

```
cargo test -- --ignored activating_a_sub_project_by_path
```

`src/tools/config/tests.rs::activating_a_sub_project_by_path_lists_the_same_memories`.
Its sibling `activating_a_sub_project_lists_the_memories_the_memory_tool_writes` passes
and covers the id route.

## Root cause

`ActivateProject::call` (`src/tools/config/mod.rs`) returns early into
`Agent::activate_within_workspace` **only** when the path contains no separator:

```rust
if !path.contains('/') && !path.contains('\\') {
    let is_project_id = /* … */;
    if is_project_id { ctx.agent.activate_within_workspace(path, read_only).await?; /* … */ }
}
```

`activate_within_workspace` is where the 2026-07-07 fix landed — it resolves memories via
`Workspace::memory_dir_for_project`, matching every branch of the live `memory` tool.

An absolute path skips that branch entirely and falls through to root resolution, which
builds a **standalone** workspace rooted at the target. The tell is in the response: the
by-path result carries no `workspace` array at all. In that standalone workspace the
sub-project is its own root, so `memory_dir_for_project` returns
`<sub_root>/.codescout/memories` — the directory nothing writes to for a project that is
a member of a parent workspace.

Measured on this repo the same day: `crates/codescout-embed/.codescout/memories` does not
exist, while `.codescout/projects/codescout-embed/memories/` holds 5 topics. So activating
`codescout-embed` by path reports 0 for a project holding 5 — and, because `MemoryStore`
creates its directory on open, materialises the empty directory that corroborates the
zero.

## Why it was not fixed with its sibling

The sibling fix moved a *reader* onto the path the *writer* already used — no semantics
changed and no migration was needed. This one cannot be fixed that way, because the
by-path route is not reading the wrong directory: it is correctly reading the memory
directory of a workspace it was correct to build. The defect is upstream, in which
workspace gets built.

The fix is therefore a dispatch decision, and both answers are defensible:

- **Focus-switch.** An absolute path that names a member of the current workspace routes
  to `activate_within_workspace`, exactly as the bare id does. Consistent, but it means
  the same path argument behaves differently depending on what workspace you happen to be
  in.
- **Standalone (today's behaviour).** A path always means "treat this directory as its own
  root." This is how a foreign repo is browsed, and the read-only hint text already assumes
  it — `"Browsing svc (read-only) … remember to activate path=<parent> when done"` reads as
  an excursion, not a focus switch.

Whichever is chosen moves `read_only` defaults, focus, and the response shape, so it needs
an owner rather than a drive-by.

## A third option, cheaper than either

Neither branch has to be chosen to stop the *silence*, which is the part that makes this
a false negative rather than a preference. `activate` could say, when the resolved root is
a member of another workspace whose per-project memory tree is non-empty, that the memory
set it is reporting is this-root-only and name where the other set lives. That is the same
shape as the linked-worktree divergence notice already in `src/tools/config/mod.rs`, which
exists for a structurally identical problem: two legitimate memory sets, one silently
served.

## Not established

Whether any real session has hit this. It was found by probe while fixing the id route,
not from a report. `codescout-embed` is the only populated sub-project on this repo and
nothing is known to activate it by absolute path.

