---
status: fixed
opened: 2026-10-17
closed: 2026-10-18
severity: medium
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md, docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md]
tags:
- cluster/unclassified
kind: bug
unverified: "The real binary (codescout start, MCP client) was not run against the fix; the tests drive call_tool_inner in process. Not covered by the fix: run_command (a shell writes any file), [embeddings].url, a human edit. Fix is on branch fix/config-security-2026-10-18 (0d81e0138, patch-id 2456cc74c748ef8d5a8d4b5244906f2e234b195a), not yet on experiments."
---

# BUG: `.codescout/project.toml` sits under the always-writable project root, and `profile = "root"` in it turns every write gate off

## Summary

The project root is always a write root, so the write tools can edit
`<project>/.codescout/project.toml`. That file holds `[security] profile`, and
`classify_write_path_in` returns `Allowed` for every path when the profile is `Root`
(`src/util/path_security.rs`). The security config is re-read when the file's mtime moves
(`Agent::security_config_for_call`, `config_is_stale` then `reload_config_from_disk`), so the change
takes effect on the next tool call, not on the next activation. Measured end to end on 2026-10-17:
a `create_file` of that file with `profile = "root"` was accepted, and the next `create_file` to a
path outside the root, which had asked for an approval before, was written without one.

## Class fit

No existing class names a policy file that the policy it configures lets you write. Filed
unclassified; it is a design question, not an implementation slip.

## Symptom (Effect)

A session that can write inside the project root can write `profile = "root"` (or add
`extra_write_roots`, or change `file_write_enabled` / `shell_command_mode`) and from the next call
write anywhere the deny list does not name.

## Reproduction

Measured 2026-10-17 on branch `fix/write-allowlist-port-2026-10-17` (the tool path is unchanged
from `experiments` 27efb11d: no diff under `src/tools/`, and under `src/agent/` only a test). A
throwaway test, not committed (patch kept in the session scratchpad `port/toml-probe.patch`), built
the same `ToolContext` the `edit_file` tests use over a fresh project and called the real
`CreateFile` tool three times. The outside target was a file under `~/.cache`, removed afterwards.

```
project.toml exists before: false
1 create_file(outside) under the default profile -> Ok({"pending_ack": "@ack_...", "reason": "'/home/marius/.cache/cs-probe-root-profile.txt' is outside the project root", "hint": "create_file(path=\"@ack_...\") to write it and approve /home/marius/.cache for this session"})
  outside exists: false
2 create_file(.codescout/project.toml, "[project]...[security]\nprofile = \"root\"\n") -> Ok("ok")
3 create_file(outside) after the edit -> Ok("ok")
  outside exists: true
cleaned up; exists now: false
```

The classifier half, with the real library (`pathsec_probe` F9c/F9d, `experiments` 27efb11d, from the
comparison run in the session scratchpad `cmp-pathsec/probe-exp-c.txt`, not re-run on the branch):
`<project>/.codescout/project.toml` is `[ALLOWED]` under the default profile, and with
`profile = Root` every probe target, `~/.bashrc`, `~/.claude/settings.json`,
`~/.config/codescout/.env`, `/etc/probe-x`, `/home/otheruser/x` and `/probe-x` included, is
`[ALLOWED]`; only `Root` also admits `~/.ssh/authorized_keys`.

## Environment

Linux, rustc 1.97.1, codescout `experiments` 27efb11d, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

The security configuration is a file inside the sandbox it configures, and it is trusted on reload.
Measured 2026-10-17: the three-step run above, with the project root as `allowed[0]`.

## Evidence

The run above. Not measured: whether any later check refuses a profile change made through the
write tools in a real server session (the test used the in-process tool context, not an MCP
client); the search for a protection on `.codescout/project.toml` was by keyword in `src/`, not
exhaustive.

## Hypotheses tried

None.

## Fix

Fixed on branch `fix/config-security-2026-10-18` (based on `experiments` 7a09623bd): `0d81e0138`, patch-id
`2456cc74c748ef8d5a8d4b5244906f2e234b195a` (follow-up test commit `242bd66b2`). Not yet on `experiments`.

**Decision.** The Resume left the design open. A blanket path-level deny of writes to
`.codescout/project.toml` was the first reading and was REJECTED after a grep found a legitimate flow
that writes the file through `edit_file`: the onboarding prompt's embedding-model step
(`src/prompts/source.md`, Phase 0, "User types 2, 3, etc.: Call `edit_file` on
`.codescout/project.toml`. Change the `model` line ...", and the `url` variant). A deny would have broken
that step and forced a change to an onboarding prompt surface. So the guard is **content-aware**:

- `config::protected_files::check_write` (`src/config/protected_files.rs`) refuses a write to a file
  named `project.toml` directly inside a directory named `.codescout` (ASCII-case-insensitive), or to the
  global config (`global_config_path()`, matched by exact path or by the canonical parent directory),
  when the `[security]` table the write produces differs from the one on disk, or when the new content
  is not valid TOML (it cannot then be shown to leave the table alone). An absent or unparseable "before"
  counts as no table, so a create cannot carry one and a broken file cannot be used to smuggle one in.
  Everything else in the file stays editable.
- It is called from `util::fs::atomic_write`, the funnel `create_file`, `edit_file` (text, append and
  batch grammars; measured, each reaches it) write through. A write tool added later is covered on the day
  it is added. Two paths do NOT go through `atomic_write` and were measured landing the file before the
  fix, so they ask the same guard: `doc(create)` (`src/librarian/tools/create.rs`, a plain `fs::write`)
  and `doc(move)` (`src/librarian/tools/mv.rs`, a `rename`; a markdown file holding `[security] profile =
  "root"` was moved onto a fresh `project.toml`).
- The refusal says what is theirs to do ("Ask the user to make that edit in an editor; the server re-reads
  the file on its next call"). `CONFIG_IS_CACHED_REMEDY` (`src/config/project.rs`) used to prescribe
  `edit_file` for the `[security]` keys its three callers are about (`indexing_enabled`,
  `shell_command_mode`, `max_index_bytes`); that is now a refused action, so the text names an editor, and
  it no longer says the config is cached for the life of the process (untrue since 2026-09-14).

**Measured NOT to be entry points for `project.toml`:** `edit_file`'s markdown grammar (heading, action,
frontmatter: "apply to markdown files only"), `edit_code` ("unsupported file type" for a `.toml`),
`doc(update)` (needs a catalogued artifact, and a `.toml` is not one), `memory(write)` with a topic that
climbs out of `.codescout/memories` (returned ok, `project.toml` unchanged: the topic is sanitised).
Those are pinned only where a test could fail if the guard were removed; the memory test is a pin of
measured behaviour, not a test of this guard.

**Alternatives considered.**

1. Blanket path deny in `classify_write_path_in` (one site, covers every path-taking tool, also a future
   one): rejected because it breaks the onboarding embedding step above. Still the simpler choice if that
   step is changed to tell the user to edit the file, or moved into the `onboarding` tool.
2. Tighten-only reload (a mid-session reload of `[security]` may only tighten): stops a mid-session
   escalation by ANY writer including the shell, but a `profile = "root"` left on disk applies at the next
   server start, unreviewed. Not chosen; compatible with this fix if wanted as a second layer.
3. Do it per tool at each write site: seven sites, and a new writer is unguarded until someone remembers.

**Residual, stated so it is not read as wider.**

- `run_command` can write the file (a shell writes anything the user can); it is gated by
  `shell_command_mode`, not by this guard. A session with an enabled shell is not stopped from editing
  `project.toml`.
- A human editing the file, and `onboarding` itself (it writes `project.toml` with `std::fs::write`), are
  unaffected by design.
- `[embeddings]` stays model-editable (the flow above), including `url`: a session could point the
  embedding endpoint elsewhere. Not addressed here.
- Under `profile = "root"` the classifier already allows every write; the guard does not change that.
- The global-config match is by exact path or canonical parent and is case-sensitive; a case-insensitive
  filesystem spelling of the global path is not covered (the project name rule is case-insensitive).
- A hard link to `project.toml` created by a shell is not detected.
- The `ConfiguredOff` refusal in `util::path_security` still says the config is cached for the life of the
  process; that predates this fix and was left alone.

## Tests added

`src/server/protected_config_write_tests.rs` (real dispatch, `call_tool_inner`; the file is compared byte
for byte afterwards) and the unit tests in `src/config/protected_files.rs`.

Red line before the fix (measured 2026-10-18, base `experiments` 7a09623bd plus the tests;
`create_file` refuses an existing file without `overwrite`, which the tests pass; 15 tests then):

```
test server::protected_config_write_tests::a_tool_cannot_raise_its_own_profile_through_project_toml ... FAILED
assertion `left == right` failed: project.toml changed through a write tool: { "status": "ok", ... }
test server::protected_config_write_tests::edit_file_text_edit_cannot_change_the_security_section ... FAILED
test server::protected_config_write_tests::edit_file_append_cannot_change_the_security_section ... FAILED
test server::protected_config_write_tests::edit_file_batch_cannot_change_the_security_section ... FAILED
test server::protected_config_write_tests::the_protected_name_is_matched_ignoring_ascii_case ... FAILED
test server::protected_config_write_tests::a_tool_cannot_change_the_global_configs_security_section ... FAILED
test result: FAILED. 6 passed; 9 failed; 0 ignored; 0 measured; 7431 filtered out
```

The `doc(create)` and `doc(move)` tests were added after that run and were red on their own runs (both
landed the file) before their guards were added. The first run of the tests, before they were
corrected, also failed some controls for test-side reasons only (`create_file` refusing an existing
file; an assertion on a config field that an env override wins over).

After: all pass. Controls that must keep working (they pass before and after): the onboarding embedding
edit through `edit_file` lands AND reaches the running config; a rewrite that leaves `[security]` equal
(comment, reordered tables, extra section) is allowed; other files under `.codescout/` and a
`docs/project.toml` stay writable.

Mutation probes, one literal mutation per guarded site, each reverted: 3 call-site removals
(`atomic_write`, `doc(create)`, `doc(move)`), 17 inside the guard (every `if`/match arm of the name,
global-path and equality logic, the TOML-parse refusal, the table name) and 2 on the remedy text. One
survivor (the "before" for a file with no `[security]` table) got its own test and is killed. No
unkilled mutants remain from the set run. The mutation set was not re-run after the last test-only
commit `242bd66b2`, which only adds tests.

One test (`a_memory_topic_cannot_reach_project_toml`) passes with the guard removed: it pins measured
behaviour of a path that is not an entry point. Not run: the real binary through an MCP client.

## Workarounds

None needed for a trusted session; for an untrusted one, the profile is a property of the project
directory the session is pointed at.

## Resume

Nothing outstanding for the write tools. Carry the fix to `experiments`; then archive this file with the
SHA there and its patch-id. Decide whether to also close the shell route (`shell_command_mode`) and the
`[embeddings].url` route; see Residual.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
