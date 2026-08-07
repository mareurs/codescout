---
id: '8405aa53461ea537'
kind: bug
status: open
title: artifact(move) cannot resolve a source path that artifact(create)/find handle, when the managed root is the home dir and the project is a sub-root
tags:
- librarian
- artifact
- path-handling
- tool-quirk
---

## Summary

`artifact(action="move")` fails with

```
no managed root contains docs/issues/2026-08-07-inject-tee-sf4-allowlist-rejects-windows-paths.md
```

for an artifact that `artifact(action="create")` created successfully minutes
earlier and that `artifact(action="find")` returns without complaint. The file
exists on disk and is committed.

## Environment

The workspace root here is the **user home directory**, with codescout as one
sub-project among several:

```
workspace.name = "codescout"
projects: root=".", ..., codescout root="work\claude\codescout", researcher, ...
```

So a project-relative `docs/issues/x.md` and a root-relative
`work/claude/codescout/docs/issues/x.md` are different strings for the same file.

## Reproduction

1. Active project = codescout (a sub-root of the managed home root).
2. `artifact(create, kind="bug", rel_path="docs/issues/<name>.md", ...)` — succeeds,
   returns an id, writes the file to `<project>/docs/issues/<name>.md`.
3. `artifact(find, filter={rel_path:{contains:"2026-08-07"}})` — returns the row.
4. `artifact(move, id=<id>, new_rel_path="docs/issues/archive/<name>.md")` —
   **fails**: `no managed root contains docs/issues/<name>.md`.
5. Retrying with the root-relative destination
   (`work/claude/codescout/docs/issues/archive/<name>.md`) fails **identically**,
   and the error still names the *source* path — so the failure is in resolving
   the source, not the destination.

## Why it matters

`docs/issues/` archival is a documented CLAUDE.md workflow, and it explicitly
forbids the obvious workaround:

> Archive via `artifact(action="move", …)`, never a bare `git mv` — `id =
> sha256(abs_path)`, so a hand-move orphans the catalog row's events/augmentation.

So on this workspace shape the sanctioned path is unavailable and the documented
alternative is prohibited. A fixed bug cannot be archived without knowingly
orphaning its catalog row.

## Notable: `doctor` is clean

`librarian(action="doctor")` reports `violations: 0`, `hidden_rows: 0`,
`move_candidates: 0`. Whatever inconsistency `move` trips over is not one of the
invariants doctor checks — so doctor's green result is not evidence that
`move` will work. Worth considering whether doctor should gain a check that
every row's stored path resolves under some managed root, which is precisely the
condition `move` enforces.

## Hypothesis (unverified)

`create` resolves `rel_path` against the **active project** (documented: "Omit
`repo` to infer from active project — rel_path is then treated as
project-relative and the subdir prefix is prepended automatically"), while
`move` appears to resolve the stored source path against the **managed roots**
without applying that same project prefix. If the row stores the
project-relative form, only `create`'s convention can read it back.

## Workaround

None applied. The affected bug file
(`docs/issues/2026-08-07-inject-tee-sf4-allowlist-rejects-windows-paths.md`,
status `fixed`, verified on `experiments` at 20d12b5f) was **left unarchived**
rather than `git mv`-ed, to avoid orphaning its catalog row.

## Resume

Confirm the hypothesis by reading the stored `abs_path`/`rel_path` for a row
created this way and comparing how `create` and `move` each resolve it. Then
either make `move` apply the same project-prefix inference as `create`, or have
`create` store the root-relative form. Once fixed, archive the SF-4 bug file.

