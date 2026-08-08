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

`memory` accepts `project_id` (or its `project` alias) verbatim from tool input and routes it
through `Workspace::memory_dir_for_project`, which resolves **any** id that is not the root
project into `<workspace root>/.codescout/projects/<id>/memories`. Nothing checks that the id
names a real project.

Two consequences, both reproduced on demand. A **write** with a bad id succeeds silently and
leaves an untracked directory in the repo. A **read** with a bad id creates an empty directory
and answers *"no memory topics exist yet"* with `available_topics: []` — telling the caller the
project has no memories when the truth is that the project does not exist. The misleading read
is the worse of the two: litter is noise, but a confident empty answer is acted on.
## Symptom (Effect)

Three effects, all reproduced on demand 2026-08-08 (see Reproduction). Ranked by how badly
they mislead.

**1 — A read against a non-existent project reports that the project has no memories.**

```
memory(action="read", topic="nonexistent-topic", project_id="zz-read-path-probe")
→ { "ok": false,
    "error": "topic 'nonexistent-topic' not found",
    "hint": "no memory topics exist yet — create one with memory(action='write', …)",
    "available_topics": [] }
```

`zz-read-path-probe` is not a project. The correct answer is "no such project"; the answer
given is "this project has no memories yet", which an agent will act on. `available_topics`
being `[]` reads as authoritative emptiness.

**2 — A write against a non-existent project succeeds silently and litters the repo.**

```
memory(action="write", topic="zz-probe-delete-me", project_id="zz-definitely-not-a-project", content="…")
→ "ok"
```

```
$ ls -1 .codescout/projects | wc -l      # 9 before, 10 after
$ find .codescout/projects/zz-definitely-not-a-project -type f
.codescout/projects/zz-definitely-not-a-project/memories/zz-probe-delete-me.md
$ git status --porcelain .codescout/projects
?? .codescout/projects/zz-definitely-not-a-project/
```

**3 — Both paths create the directory; only the write path makes it visible to git.** The
read probe created `.codescout/projects/zz-read-path-probe/` and left it empty, so the
directory count went 10 → 11 while `git status` still reported exactly one `??` entry. Git
cannot see an empty directory.

That asymmetry explains both observed populations without any host difference:

| origin | on disk | `git status` | observed as |
|---|---|---|---|
| read with a bad id | empty dir | invisible | the two phantoms on this host — `claude-plugins/…/mcp-server`, `eduplanner-site/…/optaplanner` |
| write with a bad id | dir + file | `??` | the eight on the VDI |
## Reproduction

Fully reproducible. Two calls, on any workspace, no fixture setup. Measured 2026-08-08 on
`experiments` at `9cbe4002`, codescout MCP over stdio:

```
# 1 — write path: silent success, visible litter
memory(action="write", topic="zz-probe-delete-me",
       project_id="zz-definitely-not-a-project",
       content="probe")
ls -1 .codescout/projects | wc -l          # 9 → 10
git status --porcelain .codescout/projects # ?? .codescout/projects/zz-definitely-not-a-project/

# 2 — read path: misleading answer, invisible litter
memory(action="read", topic="nonexistent-topic", project_id="zz-read-path-probe")
ls -d .codescout/projects/zz-read-path-probe   # exists, empty
ls -1 .codescout/projects | wc -l              # 10 → 11
git status --porcelain .codescout/projects      # still ONE ?? entry
```

Cleanup: `rm -rf .codescout/projects/zz-definitely-not-a-project
.codescout/projects/zz-read-path-probe`, then confirm the count returns to 9 and the status
is empty.

The `project` alias reaches the same code path (`resolve_memory_dir` reads `project_id` then
falls back to `project`), so both spellings reproduce it.
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

`find(...) == None` and `find(...) == Some(non-root project)` are indistinguishable
downstream. The `unwrap_or(false)` is exactly where "unknown" silently becomes "sub-project".

The docstring states this is deliberate:

> An unknown `project_id` is treated as a sub-project (returns a per-project subdirectory
> under `projects/<id>/`). Previously defaulted to the root memory dir on unknown ID, which
> silently co-mingled memories from typos or stale IDs with the workspace root's memories —
> a bug that would go unnoticed until a user noticed crossed-over memories.

That earlier fix was right to stop polluting the root memory store. It replaced one silent
failure with two: the typo now gets a directory of its own, and a *read* against the typo
reports the project as empty rather than absent. The missing third option — reject the id —
was never taken.

**Measured 2026-08-08** (`experiments` @ `9cbe4002`, full transcript under Reproduction):
`memory(write, project_id="zz-definitely-not-a-project")` → `"ok"`, directory count 9 → 10,
one new `??` entry; `memory(read, project_id="zz-read-path-probe")` → `available_topics: []`,
directory count 10 → 11, still one `??` entry. Both directories were created by the calls; the
read-created one is empty, which is why it is invisible to git.

**Still inferred, not measured:** which function performs the `mkdir`. `create_dir_all` appears
in `src/tools/memory/` only inside `tests.rs`, so the creating call is elsewhere and has not
been located — but it is now known to be on a path both `read` and `write` traverse, which
narrows it to the shared resolve/ensure step rather than the write handler.
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
   **Test:** ran the two-call reproduction on `9cbe4002`.
   **Verdict:** **rejected — confirmed live.** Both calls created a directory that did not
   exist a second earlier.
2. **Hypothesis:** the ids come from foreign-workspace registration rather than from a bad
   `project_id`.
   **Test:** `Workspace::memory_dir_for_project` derives the path from `self.root`, so a
   foreign workspace's own state cannot land under another repo's root. `optaplanner` is a
   sibling directory name and `mcp-server` matches no directory at all. Then reproduced the
   exact shape with an explicit bad `project_id`.
   **Verdict:** rejected — the path is anchored to the active workspace, and the id arrives as
   an argument.
3. **Hypothesis:** only the write path creates the directory, so read-only use is safe.
   **Test:** `memory(action="read", topic="nonexistent-topic", project_id="zz-read-path-probe")`
   on a clean tree.
   **Verdict:** **rejected.** The read created the directory too. It is empty, which is the
   entire reason this host's two phantoms were invisible to `git status` while the VDI's eight
   were not — so the population difference between the two hosts is read-vs-write traffic, not
   configuration.
4. **Hypothesis:** the two hosts differ because of workspace-root misresolution on the VDI.
   **Test:** superseded by 3 — a plain bad `project_id` reproduces both populations on one
   host, with no misresolution involved.
   **Verdict:** withdrawn. This was the review pass's own first guess, and it was wrong in the
   same way the original report was: a plausible mechanism adopted before the cheap experiment
   was run. The VDI `project_status` check is still worth doing, but it is no longer load-bearing
   for this bug.
## Fix

Landed on `experiments` in `c0bdeec7` — *master-side SHA still to be recorded after
cherry-pick.*

- `src/tools/memory/mod.rs` — `resolve_memory_dir` rejects an unknown
  caller-supplied `project_id` with a `RecoverableError` naming the id and listing
  the workspace's real ids. The `project` alias reaches the same check.
- `src/workspace.rs` — added `Workspace::has_project`, the named check that answers
  what `memory_dir_for_project` structurally cannot, and reused it inside
  `set_focused`.
- `src/workspace.rs` — rewrote `memory_dir_for_project`'s doc comment. It had
  presented the unknown-id behaviour as a settled, deliberate fix for an older
  co-mingling bug, which read as reassurance and stopped inquiry. It now states the
  precondition and points here.

**`ws.focused` was deliberately NOT validated, and the plan's step 3 is withdrawn.**
`Workspace::new` seeds `focused` from `projects` and `set_focused` refuses an
unknown id, so it is provably always a real id — a premise this file previously
asserted needed fixing, now falsified by reading both. `focused_is_always_a_real_project_id`
locks it down, so a future change that breaks the invariant fails a test rather than
silently widening the hole. The `ROOT_PROJECT_ID` last resort is untouched, leaving
the no-argument path byte-identical.

**Out of scope, deliberately.** `MemoryStore::from_dir` still calls `create_dir_all`
in its constructor, so a `read` against a *real* project with no memories yet still
creates the directory. That is harmless — an empty directory, invisible to git, and
legitimate for a project that exists — and splitting the constructor touches nine
call sites for no measured symptom.

**No `.gitignore` change accompanies this, and the sibling bug file's fix 2 is
withdrawn.** A phantom project's `memories/` is structurally identical to a real
one's, so no glob can hide the former while keeping the latter visible. Verified
2026-08-08 in a throwaway repo: under `/.codescout/projects/*/*` plus
`!/.codescout/projects/*/memories/`, `git check-ignore` reports the phantom's
memory file as NOT ignored — exactly as it reports a real one's. Source-side
validation was the only available fix, not merely the preferable one.
## Tests added

Five, all in `c0bdeec7`.

- `src/workspace.rs` — `has_project_distinguishes_unknown_id_from_known_non_root_project`.
  The load-bearing one. Asserts both arms, and additionally that a known id and a
  typo produce same-shaped `projects/<id>/memories` paths — which is *why* a caller
  can never use the return value to detect the typo. A mutation flipping either arm
  of `has_project` fails here.
- `src/workspace.rs` — `focused_is_always_a_real_project_id`. Guards the premise that
  justifies validating only the caller-supplied id: seeded focus is always a real
  project, a refused `set_focused` does not mutate focus, and an accepted one does.
- `src/tools/memory/tests.rs` — `memory_write_with_unknown_project_id_errors_and_leaves_no_directory`.
- `src/tools/memory/tests.rs` — `memory_read_with_unknown_project_id_says_no_such_project_not_no_topics`,
  which asserts the combined message+hint does **not** contain "no memory topics
  exist yet" — the exact phrasing that reported an absent project as empty.
- Both tool-level tests assert the phantom directory does **not** appear, not merely
  that an error came back. The error and the litter are separate failures and a fix
  could close one without the other.

**An existing test was passing because of this bug.**
`memory_write_accepts_project_alias_for_project_id` wrote `mcp-server/package.json`
as `{}`, which is too empty for discovery to register a project — so the
`mcp-server` its own comment claimed to create did not exist, and its
`.codescout/projects/mcp-server/memories/` assertion was satisfied entirely by the
phantom-directory behaviour. Fixing the validation broke it, which is how it was
found. The manifest now matches the sibling fixture's
`{"scripts":{"build":"tsc"}}`, so the alias is tested against a project that is
actually there.

That is worth keeping in mind when reading the sibling fixture
`memory_write_routes_to_project_dir`: its path assertion also passes under either
behaviour, so it never demonstrated that discovery worked. The new tests' hint
assertions (`hint.contains("mcp-server")`) are the first thing in the suite that
can tell a discovered project from an invented one.

Full gate on the fix commit: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`
clean; `cargo test` 3554 passed / 0 failed / 44 ignored.
## Workarounds

Always pass a `project_id` that appears in `project_status`, or omit it entirely and let it
fall back to the root project. Delete stray directories with
`rmdir .codescout/projects/<id>/memories .codescout/projects/<id>` — they will be recreated if
the bad id is used again.

## Resume

Fix is in. Remaining, in order:

1. Confirm CI green on `experiments` at the commit containing `c0bdeec7`, then flip
   `status: fixed` / `closed: 2026-08-08` and archive via
   `artifact(action="move", …, new_rel_path="docs/issues/archive/…")` — never a bare
   `git mv`. Label the SHA `experiments`; the master-side SHA still needs recording
   after cherry-pick, since an `experiments` SHA orphans on rebase.
2. Delete the two pre-existing phantoms, which the fix cannot retroactively remove:
   `claude-plugins/.codescout/projects/mcp-server/` and
   `mirela/eduplanner-site/.codescout/projects/optaplanner/`. Both are empty, so
   `rmdir` suffices. Nothing recreates them now.

Do **not** re-run the reproduction to confirm the bug — the transcript is above and
the behaviour is now rejected by design. Do **not** apply a `.gitignore` glob for the
litter; that was tried and falsified (see Fix).
## References

- `src/workspace.rs` — `Workspace::memory_dir_for_project`, the `unwrap_or(false)` site
- `src/tools/memory/mod.rs` — `resolve_memory_dir`, the unvalidated input site
- `src/agent/mod.rs` — the other caller; passes `p.discovered.id`, so always valid
- `docs/issues/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md` — the
  symptom this bug produces on a host where something writes into the phantom directories
- `get_guide("error-handling")` — `RecoverableError` vs `anyhow::bail!` for fix step 2
- PR https://github.com/mareurs/codescout/pull/11
