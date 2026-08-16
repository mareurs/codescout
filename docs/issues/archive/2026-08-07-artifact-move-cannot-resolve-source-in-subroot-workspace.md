---
id: fce8bfb22d07734a
kind: bug
status: fixed
title: artifact(move) cannot resolve a source path that artifact(create)/find handle, when the managed root is the home dir and the project is a sub-root
tags:
- librarian
- artifact
- path-handling
- tool-quirk
closed: 2026-08-07
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
`move` will work. The reason is now concrete: doctor's `check_backslash` **enforces** the
forward-slash form that `containing_root` could not match. Doctor was not merely
blind to the inconsistency — it was policing the half of it that broke the
comparison, so it could not have caught this defect at any severity.

**Follow-up, not done here:** doctor could gain a check that every row's stored
path resolves under some managed root — precisely the condition `move` enforces.
That check would have caught this on day one and would catch any future
respelling. Left unimplemented because it is a new invariant rather than part of
this fix; worth filing separately.

## Hypothesis (unverified)

**Rejected by measurement.** The stored `rel_path` is not the project-relative
form; `create` and `move` agree on which path is stored. The divergence is in
how that path is *spelled*.

Read directly out of `catalog.db`:

```
('52710fd548bee569',
 '//?/C:/Users/MAILINCA.BRN.002/work/claude/codescout/docs/issues/2026-08-07-inject-tee-sf4-...md')
```

All 1314 rows share the `//?/C:/Users` prefix — forward-slash-normalized, with
a forward-slash verbatim prefix. `current_project`, canonicalized at the adapter
boundary, holds the native spelling instead: `\\?\C:\Users\...\codescout`.

Rust's Windows prefix parser matches literal backslashes only, so `//?/C:/...`
parses with **no prefix component at all**, while `\\?\C:\...` parses as
`VerbatimDisk('C')`. `Path::starts_with` compares components, so the two can
never match — regardless of workspace shape. The sub-root layout is not the
trigger; it is simply where the failure was first noticed.
## Root cause

`containing_root` (`src/librarian/tools/mod.rs`) compared with
`Path::starts_with`, whose doc comment asserted the comparison was sound because
both sides are canonical-absolute. Both sides *are* canonical-absolute; they are
not canonical-*spelled*. Two independently reasonable conventions collide:

| source | spelling | enforced by |
|---|---|---|
| catalog `abs_path` | `//?/C:/Users/...` | doctor `check_backslash` |
| `current_project` | `\\?\C:\Users\...` | `canonicalize()` at the adapter boundary |

Because `containing_root` also backs `delete`, both `move` and `delete` were
unable to resolve any catalog row on Windows.

## Fix

`e4b86447`.. → `a8253b62` (experiments) — `src/librarian/tools/mod.rs`.

Added `comparable_path()`: unifies separators and strips the verbatim prefix,
Windows only. On Unix `\` is a legal filename byte, so only the trailing
separator is trimmed — rewriting separators there would corrupt real names.

`containing_root` now compares normalized strings, followed by an explicit
separator check to preserve the component-boundary guarantee `Path::starts_with`
provided for free. That boundary is security-relevant: this is the guard that
refuses paths outside every managed root, so `/work/sub` must not be treated as
containing `/work/subterfuge`.

`a8253b62` is an `experiments` SHA and stays one: it was decided 2026-08-07 that
`master` (897 commits behind, 0 ahead) is not being synced, so no cherry-pick and
no master-side SHA is pending.

## Tests added

`src/librarian/tools/mod.rs`, module `containing_root_tests` — 6 tests:

- `matches_catalog_forward_slash_form_against_native_verbatim_root` — the
  regression, using the exact spellings read out of `catalog.db`
- `matches_when_only_one_side_is_verbatim`
- `does_not_match_a_sibling_sharing_a_name_prefix` — the security boundary
- `matches_the_root_itself`
- `returns_none_when_no_root_contains_the_path`
- `prefers_the_first_matching_root` — first-match ordering is load-bearing
  (`managed_roots` puts the active project ahead of an ancestor `[[roots]]`
  entry, 1a5acfc0); normalizing must not disturb it

Verified to fail without the fix: reverting only the comparison fails **4 of 6**.
The 2 that still pass under the revert are the negative cases — they pass
vacuously when nothing ever matches, which is itself the argument for asserting
both directions rather than only the "should not match" ones.

## Workaround

None applied. The affected bug file
(`docs/issues/2026-08-07-inject-tee-sf4-allowlist-rejects-windows-paths.md`,
status `fixed`, verified on `experiments` at bc94e67f) was **left unarchived**
rather than `git mv`-ed, to avoid orphaning its catalog row.

## Resume

N/A — fixed. `comparable_path()` normalizes both sides before comparison
(`src/librarian/tools/mod.rs`), with 6 regression tests including the
security-relevant component boundary, and the SF-4 bug file was archived in
`ccea32f2`.

**The SHAs in this file are pre-merge branch SHAs and are not stable.** This
branch was rebased onto `f244ad17` on 2026-08-08, which orphaned the SHAs
originally cited here — `git cat-file -t` resolved neither `b5c8bbb0` nor
`94a63c32`. They were repaired by matching commit subjects (`a8253b62`,
`e4b86447`). A further rebase re-orphans them: re-check with
`git cat-file -t <sha>` before trusting any SHA in this file, and record the
settled `master`-side SHA after the promotion.
