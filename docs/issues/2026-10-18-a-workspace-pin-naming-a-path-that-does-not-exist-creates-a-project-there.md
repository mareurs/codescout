---
kind: bug
status: fixed
title: 'BUG: a `workspace=` pin that names a path which does not exist creates a codescout project there, so a placeholder pin leaves `x/.codescout/` in the caller''s cwd'
tags:
- cluster/unclassified
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-injected-guide-block-is-joined-to-the-tool-text-with-no-newline.md
severity: medium
closed: 2026-10-18
unverified: The server-stack feature tests and the heavy Python suites were not run (gate.sh reports no lane compiles server-stack here); no platform other than this Linux machine; the fix is on an unpushed branch, not yet verified on experiments.
---

# BUG: a `workspace=` pin that names a path which does not exist creates a codescout project there, so a placeholder pin leaves `x/.codescout/` in the caller's cwd

## Summary

A tool call whose `workspace=` argument is not an existing directory is not refused: the server keeps the raw (relative) string as the project root, makes it resident, and the first touch creates `<value>/.codescout/` with a `.gitignore`, `memories/`, `private-memories/`, `usage.db` and (for a write call) `write.lock` and `write.lock.holder` there. Found 2026-10-18 as an untracked `x/` in the main checkout, created 2026-10-18 00:09:12 +0300 by one `edit_file` call that passed `workspace="x"`. Not a test leaking into the cwd: no test passes that value (see Evidence).

## Status (2026-10-18)

Fixed on branch `fix/workspace-pin-2026-10-18` (based on `fix/small-items-2026-10-18`, itself on `experiments` 7a09623bd), not pushed, not merged. Fix commit `9bd0df3df` (patch-id `e23b7099d411aa35765db6bb01d8bce2e62139d7`), tests commit `d7f38b3a8` (patch-id `e2629706b2bb48bc0ca3cf482ce4f8a89b514811`). The root cause named below was confirmed at the code and by red tests before the fix (see `## Tests added`): a pin of `x` was served (`memory list` answered `0 topics`) and built a project at `<cwd>/x`; an absolute pin to a missing directory was built the same way; a pin to an existing FILE failed deep in residency with `Not a directory (os error 20)` instead of being refused as a pin. After the fix all three are refused with a `RecoverableError` that names the path, and nothing is created. Live check against a debug build of this tree is under `## Tests added`. `./scripts/gate.sh` on the tree at `067eb506b` (the commit before this status flip) ended `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0 FMT_STATE=passed` and `PYTHON LIGHT EXIT -> PYTHON_LIGHT=0`; its own list of what green does not cover (heavy Python suites, server-stack, librarian code in LEAN, the two local ONNX weight tests) applies here too.

The filing agent's "decide separately whether a pin may be relative at all" was decided from the code: a relative pin still resolves against the SERVER's cwd, because `workspace(activate)` (`ActivateProject::call`: `PathBuf::from(path).is_dir()` then `canonicalize`) and `--project` (`server::run`: `canonicalize`) already do exactly that, and a pin that exists keeps working unchanged. The refusal for a relative pin names that cwd.

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

Implemented (commit `9bd0df3df`):

- `agent::existing_project_dir(raw, what)` (`src/agent/mod.rs`): `canonicalize(raw)` falling back to `raw`, then `is_dir()`; anything else is refused with `RecoverableError::with_hint` (message `<what> '<path>' is not an existing directory`, plus the server's cwd when the path is relative; hint: pass the absolute path of an existing project root, nothing was created). `is_dir`, not `exists`: a file is no project root. Creates nothing.
- Every path that makes a project resident resolves its root through it: `Agent::new` (`--project`; the `index` and `migrate-memories` CLI commands also come here), `Agent::activate`, `Agent::ensure_resident` (the pin), and `Agent::activate_within_workspace` (a sub-project directory deleted after discovery was re-created by `open_lock_file`'s `create_dir_all`).
- `CodeScoutServer::extract_workspace_override` now returns `Result<Option<PathBuf>, RecoverableError>` and validates through the same function. `call_tool_inner` returns the refusal via `route_tool_error` BEFORE the access gate, the write lock, the usage recorder or the tool body can touch the pin, so sibling calls survive. `acquire_write_guard_if_writing` takes the already-resolved pin as a parameter instead of re-deriving it from `input` (its `with_project_at` failure maps to a protocol error, `McpError::internal_error`, so a refused pin must never reach it).
- Unchanged on purpose: `PathSecurityConfig::default()` and `src/util/path_security.rs`; blank/whitespace `workspace` is still "no pin"; `ActivateProject`'s own `is_dir` check.

## Tests added

Red first, recorded before the fix (`scripts/with-slot.sh cargo test --lib existing_root_tests`, then `_pin_`):

```
test agent::existing_root_tests::new_refuses_a_missing_directory_and_creates_nothing ... FAILED
test agent::existing_root_tests::ensure_resident_refuses_a_file_and_leaves_it_untouched ... FAILED
test agent::existing_root_tests::activate_refuses_a_missing_directory_and_changes_nothing ... FAILED
test agent::existing_root_tests::ensure_resident_refuses_a_missing_directory_and_creates_nothing ... FAILED
test agent::existing_root_tests::activate_within_workspace_refuses_a_sub_project_deleted_after_discovery ... FAILED
test agent::existing_root_tests::ensure_resident_still_makes_an_existing_directory_resident ... ok
test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 7426 filtered out

test server::tests::a_pin_naming_a_file_is_refused_and_leaves_it_untouched ... FAILED
test server::tests::a_relative_pin_naming_a_missing_path_is_refused_and_creates_nothing ... FAILED
test server::tests::a_pin_naming_a_missing_absolute_path_is_refused_and_creates_nothing ... FAILED
test server::tests::a_relative_pin_naming_an_existing_directory_still_resolves_against_the_cwd ... ok
test server::tests::a_blank_pin_is_still_no_pin_and_creates_nothing ... ok
  (panic text of the two missing-path tests: "memory list: the refusal must be a JSON body; got: 0 topics")
```

Green after the fix: `existing_root_tests` 6 passed; `_pin_` 21 passed (the new `server::tests::a_relative_pin_is_refused_even_when_the_cwd_is_gone` included); `a_pinned_write_takes` 1 passed; whole `cargo test --lib` 7420 passed, 0 failed, 12 ignored (run before the last test-only commit).

- `src/agent/existing_root_tests.rs` (declared `#[cfg(test)] mod existing_root_tests;` in `src/agent/mod.rs`): each `Agent` entrance refuses a missing directory with a `RecoverableError` naming the path and compares the directory listing afterwards (nothing created); a file is refused and left byte-identical; `ensure_resident` still makes an existing directory resident and never moves the default; a refused `activate` leaves the session's project where it was; a deleted sub-project is not re-created.
- `src/server/workspace_pin_tests.rs` (`include!`d into `server::tests`): through the real `call_tool_inner`, a missing absolute pin is refused for `memory list`, `memory write`, `read_file`, `edit_file` (the 2026-10-18 call) and `create_file` as an `Ok` result (never a protocol error) with `ok: false`, the pin in `error`, `absolute` in `hint`, and the host directory empty afterwards; a file pin is refused; a relative pin `x` is refused and creates nothing in the cwd; an existing relative pin still resolves against the cwd; a blank pin is still no pin; a relative pin is refused even when the cwd itself is gone; a pinned write takes the pinned project's write lock, not the default's. The four tests that need a cwd run in a child process (`platform::test_child`) with the cwd set to a tempdir in the child only, so a regression cannot create `x/` in the repo.

Mutations, one per site, each restored and `git diff` empty afterwards (kill = test that went red): `is_dir`->`exists` in the helper: `a_pin_naming_a_file...`; the relative-cwd branch -> `false`: `a_relative_pin_naming_a_missing_path...`; the cwd-unreadable arm -> empty: `a_relative_pin_is_refused_even_when_the_cwd_is_gone`; dispatch guard replaced by the old `canonicalize(..).unwrap_or(..)`: the three refusal tests (through the `hint` assertion only, because `ensure_resident` still refuses behind it); dispatch refusal -> `None`: the three refusal tests (`0 topics`); `trim().is_empty()` -> `is_empty()`: `a_blank_pin...`; `ensure_resident`, `activate`, `Agent::new`, `activate_within_workspace` each reverted: one `existing_root_tests` test each; write-lock pin threaded as `None`: `a_pinned_write_takes_the_pinned_projects_write_lock_not_the_defaults` (it survived all 168 `server::tests` before that test existed, so it got its own).

Live, debug build of this tree, `mcp call` as the filing agent ran it, in an empty directory:

```
$ cd <empty dir> && mcp call memory -p '{"action":"list","workspace":"x"}' <build>/codescout start --project <other dir>
{ "ok": false, "error": "workspace pin 'x' is not an existing directory (a relative path resolves against the server's working directory, <empty dir>)", "hint": "Pass the absolute path of an existing project root. Nothing was created at that path and no project was made resident." }
$ ls -A <empty dir> | wc -l      # after that call and after the edit_file shape of the 2026-10-18 call
0
```

## Residuals

- The stray `x/` in the main checkout is still there, and nobody has told the owner of the call (see `## Resume`).
- `tools::memory::resolve_memory_dirs` wraps an `ensure_resident` failure in its own refusal whose hint reads "Fix that project's .codescout/project.toml". With the dispatch guard in place a missing-directory pin never reaches it from `call_tool_inner` (measured: with the dispatch guard mutated away, the memory read path returned `workspace pin <p> could not be resolved: workspace pin '<p>' is not an existing directory ... hint: Pass the absolute path` under a project.toml hint). It would mislabel the cause only if the directory vanished between the dispatch check and that call, or for an in-process caller that sets `ToolContext::workspace_override` itself. Not changed, to keep the diff local.
- `Agent::security_config_for_call` still turns a `with_project_at` error into `PathSecurityConfig::default()`. Unchanged by design (that default's meaning must not move); a pin that becomes unusable after the dispatch check reaches the tool body, whose own `with_project_at` then refuses.
- Behaviour change, deliberate: a workspace that is already resident but whose directory has since been deleted is now refused on the next pinned call (it used to be served from the cached entry). Not measured by a test.
- Not examined and not changed: the `dashboard` command (`dashboard::serve(root, ..)` takes `--project` unvalidated; its handlers call `MemoryStore::open(&state.project_root)` per request, read from `src/dashboard/api/memories.rs`, not run). It is a typed CLI flag, not a pin.
- Not verified here: any platform other than this Linux machine (the four child-process tests are `cfg(unix)`).

## Workarounds

Pass an absolute path of an existing directory as `workspace=`. Delete a stray `x/` only after the owner of the call is told: it can hold a `usage.db` with that call's record.

## Resume

1. Nothing to do on the fix. 2. Still open and not mine to decide: ask the owner of the subagent `a98dbb735f5777dce` (session `16bc6017-d129-44f7-91e5-43b0afad5ea9`) what `workspace="x"` was meant to be, then remove `x/` from the main checkout (it can hold that call's `usage.db` row; this session did not touch it). 3. See `## Residuals`.

## References

- `src/server.rs` (`extract_workspace_override`)
- `src/agent/mod.rs` (`Agent::ensure_resident`)
- `src/memory/mod.rs` (`MemoryStore::open_private`)
- `src/tools/config/mod.rs` (`ActivateProject`, the refusal this lacks)

- `src/agent/mod.rs` (`existing_project_dir`, `Agent::ensure_resident`, `Agent::activate`, `Agent::new`, `Agent::activate_within_workspace`)
- `src/server.rs` (`extract_workspace_override`, `call_tool_inner`, `acquire_write_guard_if_writing`)
- `src/agent/existing_root_tests.rs`, `src/server/workspace_pin_tests.rs`
