---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- librarian
- artifact
- mv
- workspace-roots
- path-resolution
- nested-project
topic: null
time_scope: null
closed: 2026-06-14
---

# BUG: `artifact(action="move")` joins `new_rel_path` against an ancestor `[[roots]]` entry, silently relocating files OUTSIDE the active project

## Summary
When the active project is nested inside a directory that is itself a registered
legacy workspace `[[roots]]` entry, `artifact(action="move")` resolves the
project-relative `new_rel_path` against the **ancestor root**, not the active
project. Files are physically `rename()`d to a path *outside the project repo*,
the catalog row is updated to point there, and the tool returns `"moved": true` —
no error. Caught while archiving trackers in the `MRV-poc` project, which lives at
`…/southpole/MRV-poc` while `southpole` is a registered root.

## Symptom (Effect)
Active project: `/home/marius/work/stefanini/southpole/MRV-poc` (its own git repo).
Call:

```
artifact(action="move", id="6ad59d4c3582ee04",
         new_rel_path="docs/trackers/archive/gold-audit-signals.md")
```

Returned (note the missing `/MRV-poc/` segment in `new_abs_path`):

```json
{
  "id": "6ad59d4c3582ee04",
  "old_abs_path": "docs/trackers/gold-audit-signals.md",
  "new_abs_path": "/home/marius/work/stefanini/southpole/docs/trackers/archive/gold-audit-signals.md",
  "moved": true
}
```

The file was physically moved to `…/southpole/docs/trackers/archive/` — one
directory *above* the project repo — and removed from
`…/southpole/MRV-poc/docs/trackers/`. Seven trackers were relocated this way in a
single batch before the misresolution was noticed on the post-move `ls`.

## Reproduction
1. Register a directory `P` (e.g. `…/southpole`) as a legacy workspace
   `[[roots]]` entry in `~/.config/librarian/workspace.toml`.
2. Activate a project `P/child` that is its own git repo (e.g. `…/southpole/MRV-poc`)
   — resolved into `ctx.current_project`, typically ABSENT from `[[roots]]`.
3. Create/own a tracker artifact at `P/child/docs/trackers/foo.md`.
4. `artifact(action="move", id=…, new_rel_path="docs/trackers/archive/foo.md")`.
5. Observe: the file lands at `P/docs/trackers/archive/foo.md` (joined against `P`),
   not `P/child/docs/trackers/archive/foo.md`. Tool returns `moved: true`.

codescout `74f53b1f` (branch `experiments`).

## Environment
- OS: Linux. MCP transport: stdio (Claude Code).
- codescout `74f53b1f` on branch `experiments`.
- Active project: `MRV-poc` at `…/southpole/MRV-poc` (own git root).
- Ancestor `…/southpole` present in the global legacy `[[roots]]` registry
  (`~/.config/librarian/workspace.toml`).

## Root cause
**CONFIRMED 2026-06-11** by reading the resolution path.

`mv::call` (`src/librarian/tools/mv.rs:14-75`) computes the destination as:

```rust
let roots = super::managed_roots(ctx);
let root_path = super::containing_root(&roots, &row.abs_path)?;   // first prefix match
let new_full = root_path.join(&a.new_rel_path);                   // joined against that root
```

`managed_roots` (`src/librarian/tools/mod.rs:106-117`) builds the candidate list
with the **legacy `workspace.roots` first**, and only *appends*
`current_project.git_root` / `current_project.abs_path` afterward:

```rust
let mut roots = ctx.workspace.roots.iter().map(|r| r.path.clone()).collect();
if let Some(cp) = ctx.current_project.as_deref() {
    for candidate in [&cp.git_root, &cp.abs_path] {
        if !roots.iter().any(|r| r == candidate) { roots.push(candidate.clone()); }
    }
}
```

`containing_root` (`src/librarian/tools/mod.rs:127-132`) returns the **first**
prefix match:

```rust
roots.iter().find(|root| abs_path.starts_with(root))
```

So when `current_project` (`…/southpole/MRV-poc`) is nested under a legacy root
(`…/southpole`), the artifact's `abs_path`
(`…/southpole/MRV-poc/docs/trackers/foo.md`) matches the **ancestor** `southpole`
entry — which comes first in the list — before `find` ever reaches the appended
`current_project.git_root`. `new_rel_path` is then joined against `…/southpole`,
producing a path outside the project.

This is a follow-on edge case of the fix in
`docs/issues/archive/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md`,
which introduced `managed_roots`/`containing_root` to stop delete/move from
rejecting projects ABSENT from `[[roots]]`. That fix appended `current_project`
to the candidate list but left it **after** the legacy roots, so an ancestor
legacy root still shadows the active project. The `managed_roots` doc comment even
claims "`git_root` is listed before `abs_path` so … `mv` resolves against the repo
root" — true only *within* the `current_project` candidates; it does not order
`current_project.git_root` ahead of the legacy `workspace.roots`.

## Evidence

### Wrong resolution (first batch)
`new_abs_path` for all 7 moves was under `…/southpole/docs/trackers/archive/`
(missing `/MRV-poc/`). `ls` confirmed the 7 files physically present in
`…/southpole/docs/trackers/archive/` and absent from
`…/southpole/MRV-poc/docs/trackers/`.

### Workaround verified
Re-issuing the move with the project subdir prefixed onto `new_rel_path`:

```
new_rel_path = "MRV-poc/docs/trackers/archive/gold-audit-signals.md"
```

→ `new_abs_path: "docs/trackers/archive/gold-audit-signals.md"` and the file
landed correctly at `…/southpole/MRV-poc/docs/trackers/archive/…`. Joining
`…/southpole` + `MRV-poc/docs/…` reconstructs the project path — confirming the
join base is the ancestor root.

## Hypotheses tried
1. **Hypothesis:** `new_rel_path` is joined against the active project's git_root.
   **Test:** inspected `new_abs_path` in the tool result + `ls` both locations.
   **Verdict:** rejected — it joined against the ancestor `southpole` root.
2. **Hypothesis:** prefixing `new_rel_path` with the project subdir name lands the
   file correctly. **Test:** re-ran move with `MRV-poc/docs/trackers/archive/…`.
   **Verdict:** confirmed — file landed inside the project; join base is the
   ancestor root.

## Fix

**Shipped on `experiments` in `a3198893`** (`fix(librarian): resolve mv/delete against the nested project, not an ancestor [[roots]] entry`). Not yet on `master` — archive after cherry-pick, cite the master-side SHA then.

Implemented **option 2** (order `current_project` first). `managed_roots` (`src/librarian/tools/mod.rs`) now lists the active `current_project` (`git_root`, then `abs_path`) **ahead of** the legacy `workspace.roots`, so `containing_root`'s first-prefix-match prefers the active project over an ancestor `[[roots]]` entry that also contains the artifact. `mv` and `delete` share `managed_roots` / `containing_root`, so the one change fixes both. Plus **option 3** (defense-in-depth): `mv::call` rejects a `new_rel_path` that is empty, absolute, or contains a `..` segment — a move can no longer escape its resolved root.

Chose option 2 over option 1 (longest-prefix): when a project is a *subdir* of its git repo, `abs_path` is deeper than `git_root`, so longest-prefix would resolve a repo-root-relative path against the subdir instead of the repo root — wrong. Ordering preserves the documented git_root-before-abs_path intent.
## Tests added

`src/librarian/tools/mv.rs` tests:
- `move_resolves_under_nested_project_not_ancestor_root` — active project nested under an ancestor `[[roots]]` entry; the move lands under the project, NOT the ancestor.
- `move_rejects_new_rel_path_escape` — `../escape/foo.md` is refused.

Full lib suite 2741 pass; clippy `-D warnings` clean. `delete` shares the helper; its existing tests stay green.
## Workarounds
Prefix `new_rel_path` with the active project's subdir name relative to the
ancestor root — e.g. `new_rel_path="MRV-poc/docs/trackers/archive/foo.md"` instead
of `"docs/trackers/archive/foo.md"`. Always verify `new_abs_path` in the result
(and `ls`) before trusting `moved: true`.

## Resume
Implement fix (1) in `containing_root` (`src/librarian/tools/mod.rs:127-132`):
change `roots.iter().find(...)` to select the longest matching prefix
(`roots.iter().filter(|r| abs_path.starts_with(r)).max_by_key(|r| r.as_os_str().len())`).
Add a regression test in `src/librarian/tools/mv.rs` tests module mirroring the
Reproduction. Verify the `delete` guard benefits from the same change.

## References
- `src/librarian/tools/mv.rs:14-75` — `call` (join `root_path.join(new_rel_path)`)
- `src/librarian/tools/mod.rs:106-117` — `managed_roots` (legacy roots first)
- `src/librarian/tools/mod.rs:127-132` — `containing_root` (first prefix match)
- `docs/issues/archive/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md`
  — the sibling/origin issue that introduced `managed_roots`/`containing_root`
