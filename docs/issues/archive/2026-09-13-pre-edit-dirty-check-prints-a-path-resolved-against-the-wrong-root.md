---
id: 796944d5cac8e20f
kind: bug
status: fixed
title: 'BUG: pre-edit-dirty-check prints a path resolved against the session cwd, and keys its seen-marker on the spelling'
tags:
- cluster/gate-keyed-on-unobservable-event
---

## Summary

`pre-edit-dirty-check.mjs` takes the tool's path argument **verbatim** — `inputPath()`
(`lib.mjs:564-567`) returns `tool_input.path` with no resolution — and then uses that one
unresolved string for three jobs. Only the first tolerates a relative path:

| use | line | relative path OK? |
|---|---|---|
| git pathspec, run with `cwd = projectRoot` | `:75` | **yes** — resolves against the repo root |
| `relative(projectRoot, targetPath)` for display | `:78` | **no** — Node resolves a relative `to` against `process.cwd()` |
| `sha256(targetPath)` as the seen-marker key | `:51-56` | **no** — keys on the SPELLING, not the file |

So the *check* is right and the *advisory* is wrong: it names a path that does not exist,
and it can claim "this session did not write it" about a file this session created.

## Symptom (Effect)

Observed 2026-09-13. `edit_file(path="tests/env_mutation_isolation.rs")` — a file this
session had created minutes earlier — produced:

```
[cs-hint] `.buddy/tests/env_mutation_isolation.rs` already has uncommitted changes
          that this session did not write.
```

Both halves are wrong, in different ways:

- **`.buddy/tests/…` does not exist.** `ls -d .buddy/tests` → `No such file or directory`,
  and `git status --porcelain -- .buddy/tests` returns nothing. The advisory's own text says
  *"This states only what `git status --porcelain` proves"* — and for the path it printed,
  git proves nothing. The claim is true of `tests/env_mutation_isolation.rs`, which is what
  git was actually asked about.
- **"this session did not write" is false.** This session created the file as
  `tests/env_isolation.rs`, then renamed it. The marker is `sha256(targetPath)`, so the new
  spelling had no marker and the file read as untouched.

## Reproduction

Deterministic, and it needs a session whose cwd is **not** the project root:

1. Work in a session whose `cwd` is a subdirectory (here `<repo>/.buddy`, this project's
   configured primary working directory).
2. Create any file via a codescout write tool using a **project-relative** path.
3. Edit it again at that same relative path, or rename it and edit the new name.

The printed path is prefixed with the cwd's offset from the project root. For a session
whose cwd IS the project root, `relative(root, "tests/x")` resolves to `<root>/tests/x` and
prints correctly — which is why this has gone unseen.

## Environment

codescout-companion `hooks/pre-edit-dirty-check.mjs`, `hooks/lib.mjs`. Observed from
codescout `experiments`, session cwd `/home/marius/work/claude/codescout/.buddy`.

## Root cause

One unresolved string used three ways. `resolveProjectRoot(cwd)` is computed at `:47` and
correctly identifies the repo, and `git()` is handed it explicitly — which is exactly why
the pathspec works and hides the other two.

`path.relative(from, to)` resolves **both** arguments against `process.cwd()` when they are
relative. The hook's `process.cwd()` is the session cwd, not `projectRoot`, so a relative
`targetPath` acquires the offset between them.

The marker is a **proxy** for the unobservable question *"did this session write this
file?"*. Keying it on the path spelling means a rename, or the same file addressed
absolutely rather than relatively, mints a fresh key — and the proxy fails the way
`issue-clusters:IC-2` describes, by returning a plausible answer rather than an error.

## Evidence

`lib.mjs:564-567` — no resolution:

```js
export function inputPath(input) {
  const toolInput = (input && input.tool_input) || {};
  return toolInput.path || toolInput.file_path || toolInput.filePath || toolInput.uri || '';
}
```

`pre-edit-dirty-check.mjs:46-47, 51-56, 75, 78` — the root is known and used for git, not for
the other two.

## Impact

Low severity, but it degrades the one thing the hook exists to buy. Its own header is careful
about what it does **not** cover, and its comment at `:80-83` says *"Claim only what the check
proves … naming an unchecked cause ends the search for the real one."* A reader who follows
the printed path finds nothing there, and a reader who believes "this session did not write
it" about their own renamed file learns to discount the hint. A guard that cries wolf is one
nobody reads — which the hook says of itself at `:69-70`.

It cannot produce a wrong **decision**: the hint never denies, and the git check underneath is
correct. It produces a wrong **address** and a wrong **attribution clause**.

## Hypotheses tried

**"The check is broken"** — rejected by reading. `git(projectRoot, [...])` runs with the repo
root as cwd, so the relative pathspec resolves correctly and the non-empty result that
triggered the warning was real (`?? tests/env_mutation_isolation.rs`). Only the rendering and
the marker are affected. Worth recording because the symptom — an advisory about a
nonexistent path — reads like a broken check, and that is the first place a re-investigator
will look.

## Fix

**Fixed 2026-09-13.** Shipped `codescout-companion:0e49d2a` (repo `claude-plugins`, patch-id
`be1809f6287126bc4f627635033aa8ac2ef43de0`).

`targetPath` is now resolved to an absolute path exactly once, immediately after
`projectRoot` is computed (`const absTargetPath = isAbsolute(targetPath) ? targetPath :
join(projectRoot, targetPath)`), and that value is used for both the marker hash and the
displayed `rel`. The `git()` call is untouched, per this file's own note that it was never
affected (it runs with `-C projectRoot` explicitly).

Not done via `inputPath()` itself, per this file's own warning: the fix lives at the call
site that already has `projectRoot`, not inside a helper with no root to resolve against.

Reproduced the exact reported symptom before fixing: a session with cwd at the plugin's
own `.buddy` subdirectory, editing a project-relative path, printed
`` `.buddy/tests/example.rs` `` — a path that does not exist — byte-for-byte matching this
file's Symptom section.

## Tests added

`codescout-companion/hooks/pre-edit-dirty-check.test.sh` (new file), 5 cases: from the
project root (control, correct either way), from a subdirectory (the reported
reproduction — confirmed RED against the pre-fix source via `git stash`, printing the
exact `.buddy/...` symptom text), and a same-file-different-spelling case (relative vs.
absolute path in the tool payload) proving the marker converges on one key. All 5 pass
post-fix; the subdirectory and spelling cases fail pre-fix.

## Workarounds

Read the path as relative to the project root, ignoring any leading offset to the session's
cwd. The check underneath is sound; it is the label that is wrong.

## Resume

Unclaimed. Filed on notice while working an unrelated fix; the hook's behaviour was
incidental to that work and nothing depends on it.

## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the hazard
  the hook exists to narrow
- `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md`
