---
status: open
opened: 2026-08-08
closed:
severity: medium
owner: marius
related: []
tags: [workspace, memory, validation, repo-hygiene]
kind: bug
---

# BUG: an unvalidated `project_id` resolves to a fresh per-project memory directory, so any typo or stale id gets its own tree

## Summary

`memory` accepts `project_id` (or its `project` alias) verbatim from tool input and routes
it through `Workspace::memory_dir_for_project`, which resolves **any** id that is not the
root project into `<workspace root>/.codescout/projects/<id>/memories`. Nothing checks that
the id names a real project. A typo, a stale id, or a foreign workspace's name therefore
gets its own directory under the *active* repo — silently, with no error and no diagnostic.

## Symptom (Effect)

Directories under `.codescout/projects/` whose id is not a subdirectory of the repo. Two on
this host, both empty, measured 2026-08-08:

```
$ find /home/marius/work/claude/claude-plugins/.codescout/projects/mcp-server -type f
$ find /home/marius/work/mirela/eduplanner-site/.codescout/projects/optaplanner -type f
$ ls -d /home/marius/work/claude/claude-plugins/mcp-server
ls: cannot access '.../mcp-server': No such file or directory
$ ls -d /home/marius/work/mirela/eduplanner-site/optaplanner
ls: cannot access '.../optaplanner': No such file or directory
```

`optaplanner` is a **sibling** of `eduplanner-site` at `mirela/optaplanner`, not a child.

Empty directories are invisible to `git status`, so on this host the effect is inert. Where
something also *writes* into them they become permanent `??` entries — the eight reported in
`docs/issues/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md`.

## Reproduction

Path resolution is confirmed by reading; the directory-creating call site is **not yet
identified**. Best lead:

```
memory(action="write", topic="x", project_id="definitely-not-a-project")
```

on any workspace, then `ls .codescout/projects/`. Predicted result — a
`definitely-not-a-project/memories/` tree, no error. Run this before working the bug; it is
one command and it decides whether the write path or a read path does the `mkdir`.

## Environment

Linux, `experiments` at `9cbe4002`, codescout MCP over stdio. Also observed on a Windows VDI
checkout at `88316ac9` (see the sibling bug file). Not environment-specific — the code path
has no platform branch.

## Root cause

Two hops, neither of which validates the id.

- `src/tools/memory/mod.rs` — `resolve_memory_dir` reads `project_id`, falling back to the
  `project` alias, then to `ws.focused`, then to `ROOT_PROJECT_ID`. The input value is passed
  through as a `String` with no membership check against `ws.projects`.
- `src/workspace.rs` — `Workspace::memory_dir_for_project` looks the id up in
  `self.projects`; on miss, `is_root` is `false` via `unwrap_or(false)` and the id lands in
  the `else` branch, producing `self.root/.codescout/projects/<id>/memories`.

The docstring states this is deliberate:

> An unknown `project_id` is treated as a sub-project (returns a per-project subdirectory
> under `projects/<id>/`). Previously defaulted to the root memory dir on unknown ID, which
> silently co-mingled memories from typos or stale IDs with the workspace root's memories —
> a bug that would go unnoticed until a user noticed crossed-over memories.

That earlier fix was right to stop polluting the root memory store. It replaced one silent
failure with another: instead of writing a typo's memory into the root, the typo now gets a
directory of its own. The missing third option — reject the id — was never taken.

**Measured 2026-08-08:** `find` over both phantom paths → zero files (above); `ls -d` over the
matching repo-relative paths → `No such file or directory`. **Inferred, not measured:** that
`resolve_memory_dir` is the caller that produced these two specific directories, and which
call site performs the `mkdir` — `create_dir_all` appears in `src/tools/memory/` only inside
`tests.rs`, so the creating call lives elsewhere and has not been located.

## Evidence

### The resolution site, `src/workspace.rs`

```rust
let is_root = self
    .projects
    .iter()
    .find(|p| p.discovered.id == project_id)
    .map(|p| p.discovered.relative_root == std::path::Path::new("."))
    .unwrap_or(false);          // <-- unknown id falls through as "not root"
if is_root {
    self.root.join(".codescout").join("memories")
} else {
    self.root.join(".codescout").join("projects").join(project_id).join("memories")
}
```

`find(...)` returning `None` and `find(...)` returning a non-root project are indistinguishable
downstream. The `unwrap_or(false)` is where "unknown" becomes "sub-project".

### The unvalidated input, `src/tools/memory/mod.rs`

```rust
let project_param = input
    .get("project_id")
    .or_else(|| input.get("project"))
    .and_then(|v| v.as_str())
    .map(|s| s.to_string());
...
let project_id = project_param
    .or_else(|| ws.focused.clone())
    .unwrap_or_else(|| crate::workspace::ROOT_PROJECT_ID.to_string());
Ok(ws.memory_dir_for_project(&project_id))
```

Note the `ws.focused` fallback: a stale or foreign value in `focused` routes **every**
unqualified memory call into a phantom directory, not just calls that name a bad id.

### Cross-host sweep

Nine repos with a `.codescout/projects/` tree, checked across every root registered in
`~/.config/librarian/workspace.toml`. Two carried phantom ids; both were empty. Full table in
the review addendum of the sibling bug file.

## Hypotheses tried

1. **Hypothesis:** the phantom dirs are stale leftovers from an older layout, not something
   still being produced.
   **Test:** read both call sites; the `unwrap_or(false)` fall-through is present at
   `9cbe4002`, and `resolve_memory_dir` still passes tool input through unchecked.
   **Verdict:** rejected as an explanation for the *mechanism* — the code that would produce
   them is live. Whether these two specific directories are recent or old is undetermined and
   does not change the fix.
2. **Hypothesis:** the ids come from foreign-workspace registration rather than from a bad
   `project_id`.
   **Test:** `Workspace::memory_dir_for_project` derives the path from `self.root`, so a
   foreign workspace's own state cannot land under another repo's root. `optaplanner` is a
   sibling directory name, and `mcp-server` matches no directory at all.
   **Verdict:** rejected — the path is anchored to the active workspace, so the id had to
   arrive as an argument.

## Fix

Not yet applied. Plan:

1. Distinguish the two cases in `Workspace::memory_dir_for_project` — a lookup miss is not a
   sub-project. Either return `Option<PathBuf>`/`Result` and let callers decide, or keep the
   signature and add a sibling `resolve_project_id` that callers must go through first.
2. In `resolve_memory_dir`, reject an unknown `project_id` with a `RecoverableError` listing
   the workspace's real project ids — the guidance case in `get_guide("error-handling")`: a
   deterministic input mistake the agent can correct on the retry. Do **not** auto-guess a
   nearest match on a write path.
3. Validate the `ws.focused` fallback the same way, and clear `focused` when it names a
   project that no longer exists.
4. Sweep the existing phantoms after the fix lands: they are safe to delete once nothing
   recreates them.

## Tests added

None yet. Three are wanted, and all three are cheap because the resolution is pure:

- `memory_dir_for_project` distinguishes an unknown id from a known non-root project — the
  discriminating case the current `unwrap_or(false)` collapses.
- A `memory(action="write", project_id="<unknown>")` call returns a `RecoverableError` naming
  the valid ids, and creates no directory.
- A stale `ws.focused` does not route writes into a phantom directory.

The first is the one that matters: a mutation flipping `unwrap_or(false)` to `unwrap_or(true)`
would change behavior in the opposite direction and no current test would notice.

## Workarounds

Always pass a `project_id` that appears in `project_status`, or omit it entirely and let it
fall back to the root project. Delete stray directories with
`rmdir .codescout/projects/<id>/memories .codescout/projects/<id>` — they will be recreated if
the bad id is used again.

## Resume

Run the one-command reproduction above (`memory(action="write", topic="x",
project_id="definitely-not-a-project")` then `ls .codescout/projects/`) to identify the
`mkdir` call site. Then implement fix step 1 in `src/workspace.rs` and step 2 in
`src/tools/memory/mod.rs`, with the three tests above. The `.gitignore` half of this is
tracked separately in
`docs/issues/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md` — do not
close that one on this fix alone, or vice versa.

## References

- `src/workspace.rs` — `Workspace::memory_dir_for_project`, the `unwrap_or(false)` site
- `src/tools/memory/mod.rs` — `resolve_memory_dir`, the unvalidated input site
- `src/agent/mod.rs` — the other caller; passes `p.discovered.id`, so always valid
- `docs/issues/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md` — the
  symptom this bug produces on a host where something writes into the phantom directories
- `get_guide("error-handling")` — `RecoverableError` vs `anyhow::bail!` for fix step 2
- PR https://github.com/mareurs/codescout/pull/11
