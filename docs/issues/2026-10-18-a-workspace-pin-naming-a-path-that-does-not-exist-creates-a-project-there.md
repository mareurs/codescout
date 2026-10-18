---
kind: bug
status: open
title: 'BUG: a `workspace=` pin that names a path which does not exist creates a codescout project there, so a placeholder pin leaves `x/.codescout/` in the caller''s cwd'
tags:
- cluster/unclassified
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-injected-guide-block-is-joined-to-the-tool-text-with-no-newline.md
severity: medium
---

# BUG: a `workspace=` pin that names a path which does not exist creates a codescout project there, so a placeholder pin leaves `x/.codescout/` in the caller's cwd

## Summary

A tool call whose `workspace=` argument is not an existing directory is not refused: the server keeps the raw (relative) string as the project root, makes it resident, and the first touch creates `<value>/.codescout/` with a `.gitignore`, `memories/`, `private-memories/`, `usage.db` and (for a write call) `write.lock` and `write.lock.holder` there. Found 2026-10-18 as an untracked `x/` in the main checkout, created 2026-10-18 00:09:12 +0300 by one `edit_file` call that passed `workspace="x"`. Not a test leaking into the cwd: no test passes that value (see Evidence).

## Class fit

No class on the roster was walked one by one, so the sanctioned `cluster/unclassified` is used. What the case shows: an argument that names a target is accepted without checking that the target exists, and the failure to check does not fail the call, it silently builds the target, so the mistake surfaces later and elsewhere as a stray directory instead of as a refusal at the call.

## Symptom (Effect)

`/home/marius/work/claude/codescout/x/` (untracked, main checkout), 2026-10-18 00:09:12 to 00:09:13 +0300:

```
x/.gitignore                      29 bytes: ".codescout/private-memories/\n"
x/.codescout/memories/            empty
x/.codescout/private-memories/    empty
x/.codescout/usage.db             20480 bytes
x/.codescout/write.lock           0 bytes
x/.codescout/write.lock.holder    0 bytes
```

The one row in that `usage.db` (`tool_calls`, read read-only with `immutable=1`):

```
id 1  tool edit_file  called_at 2026-10-17 21:09:13.172 (UTC)  outcome recoverable_error
input keys: edits, path, workspace   path = "x"   workspace = "x"
project_root /home/marius/work/claude/codescout/x
session_id 2f5f98b2-efee-47cc-a8c9-5a98b1e4366d
cc_session_id 16bc6017-d129-44f7-91e5-43b0afad5ea9/a98dbb735f5777dce   (a subagent of that session)
error  missing 'old_string' parameter
```

## Reproduction

Measured 2026-10-18 with a build of branch `fix/small-items-2026-10-18`, run with the cwd at the root of a checkout that has no `x/`:

```
mcp call memory -p '{"action":"list","workspace":"x"}' ./codescout start --project <checkout>
git status --short        # ?? x/
```

A READ call creates `x/` with `.gitignore`, `.codescout/{memories,private-memories,usage.db,write.lock}`. With the write call that was actually made:

```
mcp call edit_file -p '{"path":"x","workspace":"x","edits":"[{\"new_string\":\"a\"}]"}' ./codescout start --project <checkout>
```

the result is the same tree plus `x/.codescout/write.lock.holder`: every entry the stray directory has. (`ls -A x x/.codescout` after each; the dir was removed between the two runs.)

## Environment

Linux, codescout built from branch `fix/small-items-2026-10-18` (base `experiments` 7a09623bd), `mcp` CLI at ~/.local/bin/mcp, stdio server.

## Root cause

Two places turn an unusable pin into a root instead of refusing it:

- `CodeScoutServer::extract_workspace_override` (`src/server.rs`, around line 730): `Some(std::fs::canonicalize(&p).unwrap_or(p))`. When `canonicalize` fails because the path does not exist, the raw string is kept, so a relative value stays relative and resolves against whatever the process cwd is.
- `Agent::ensure_resident` (`src/agent/mod.rs`, around line 836): `std::fs::canonicalize(&root).unwrap_or(root)`, the same fallback, then residency on first touch.

`MemoryStore::open_private` (`src/memory/mod.rs`) is what writes the 29-byte `.gitignore` (`.codescout/private-memories/` plus a newline) and `create_dir_all`s `.codescout/private-memories`, which creates `x/` and `x/.codescout` as parents. That the directories come from this path and not from a test is established by the measured reproduction above; which line of the residency path first calls it was not isolated.

Contrast with `workspace(action="activate", path=...)`, which refuses a non-directory (`src/tools/config/mod.rs`, "path '...' is not a directory"). The per-call pin has no such check.

## Evidence

- The `usage.db` row above names the call, its session and its subagent, and carries `workspace = "x"`.
- No test or script in the repo passes that value: `grep` for `"x"` joined to a path, for a relative literal `workspace` value, and for activation calls with relative literals over `src/`, `tests/`, `scripts/` and `examples/` finds none (the `"workspace":` values in the tests read pass `to_string_lossy()` of a tempdir or a `pin` built from one; that every one is absolute was read for the literals, not proven for the computed ones). `cargo test --workspace --no-fail-fast` (default features, `CODESCOUT_SKIP_ONNX_TESTS=1`), `cargo test --workspace --no-default-features` and `scripts/python-tests.py --lane light` were each run in a worktree at this tree and left no `x/` and no other untracked path (`git status --short` empty after each; for the lean run `ls -d x` reported it absent).
- The directory tree of the reproduction matches the stray one entry for entry, including the 20480-byte `usage.db` and the 29-byte `.gitignore`.

## Hypotheses tried

1. **Hypothesis:** a test activates a project at the relative path `x`. **Test:** grep for `"x"` joined to a path, `Path::new("x")`, activation calls with relative literals, then run the three lanes above in a worktree and look for `x/`. **Verdict:** rejected for every test that ran: nothing creates it, and the one row in the stray `usage.db` is a live tool call, not a test. `#[ignore]`d tests (12 in the lib, more in `tests/`) were not run, so they are not excluded by measurement.
2. **Hypothesis:** the project-activation tool creates it. **Test:** read `ActivateProject::call`. **Verdict:** rejected: it refuses a path that is not a directory before anything is created.

## Fix

Proposed, NOT implemented: refuse a `workspace=` pin that is not an existing directory, at the dispatch boundary (`call_tool_inner`, where `extract_workspace_override` is called), with a `RecoverableError` that names the value and says to pass an absolute path to a project root. Decide separately whether a pin may be relative at all (it resolves against the server's cwd, which a caller does not control). Red first: a test that dispatches a read tool with `workspace="does-not-exist"` from a tempdir cwd and asserts the call is refused and nothing was created under the tempdir.

## Tests added

N/A: open, proposal only.

## Workarounds

Pass an absolute path of an existing directory as `workspace=`. Delete a stray `x/` only after the owner of the call is told: it can hold a `usage.db` with that call's record.

## Resume

1. Ask the owner of the subagent `a98dbb735f5777dce` (session `16bc6017-d129-44f7-91e5-43b0afad5ea9`) what `workspace="x"` was meant to be. 2. Decide refuse-vs-resolve for a non-directory pin, write the red test, make the refusal. 3. Remove `x/` from the main checkout.

## References

- `src/server.rs` (`extract_workspace_override`)
- `src/agent/mod.rs` (`Agent::ensure_resident`)
- `src/memory/mod.rs` (`MemoryStore::open_private`)
- `src/tools/config/mod.rs` (`ActivateProject`, the refusal this lacks)
