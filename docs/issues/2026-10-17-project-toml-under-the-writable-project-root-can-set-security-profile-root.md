---
status: mitigated
opened: 2026-10-17
closed: 2026-10-18
severity: medium
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md, docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md]
tags:
- cluster/unclassified
kind: bug
unverified: "MITIGATED for the file-writing tools only; NOT a boundary against a session with an enabled shell: under the default shell_command_mode = warn one run_command redirect raises the profile to Root (measured 2026-10-18, see Fix, Open decision). [embeddings].url stays editable (code and credential exfiltration channel, by reading). Windows spellings (trailing dot or space, ::$DATA) of the protected name are unverified. The real binary through an MCP client was not run; tests drive call_tool_inner in process. Fix is on branch fix/config-security-2026-10-18, not yet on experiments."
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

**Status: mitigated for the file-writing tools. Not a security boundary against a session that can
run shell commands.** Fix commits on branch `fix/config-security-2026-10-18` (based on `experiments`
7a09623bd), not yet on `experiments`:

| What | Commit | patch-id |
|---|---|---|
| the guard (name rule) | `0d81e0138` | `2456cc74c748ef8d5a8d4b5244906f2e234b195a` |
| identity match: symlinked `.codescout`, symlinked `project.toml` (review F2, F3) | `6826b1a9e` | `c31c09d79cb63a11a38af1989a1a92c67cd51f79` |

### OPEN DECISION for the user: tighten-only reload (not implemented)

Under the default `shell_command_mode = "warn"`, `run_command` still raises the profile in ONE
unacknowledged call. Measured 2026-10-18 (test `a_shell_redirect_still_writes_the_security_section_known_residual`,
real dispatch; before this run `shell_mode=warn`):

```
F1 profile_before=Default run_command_is_error=false profile_after=Root
on_disk="[project]\nname = \"t\"\n\n[security]\nprofile = \"root\"\n"
```

by `run_command("printf '[project]\\nname = \"t\"\\n\\n[security]\\nprofile = \"root\"\\n' > .codescout/project.toml")`.
By reading: `is_dangerous_command` returns `None` under `Root` (`src/util/path_security.rs`, the
`config.profile == SecurityProfile::Root` early return), and no pattern in
`DEFAULT_DANGEROUS_PATTERNS` matches a redirect onto `project.toml`. So the write tools being guarded
does not stop a session that can run a shell, and this file must not be read as saying it does.

The change that would close this, and the symlink and onboarding variants below for MID-SESSION edits
(review F1, F2, F3, F6), is **tighten-only reload** (Alternative 2): a reload of a changed
`project.toml` during a session may only keep or narrow `[security]`; a loosening takes effect at the
next server start. It is NOT implemented here because it changes the user's own mid-session
edit-to-loosen flow (edit the file, the next call picks it up), which is a user decision. It would not
make a `profile = "root"` left on disk safe at the next start.

### The guard as built

**Decision.** The Resume left the design open. A blanket path-level deny of writes to
`.codescout/project.toml` was the first reading and was REJECTED after a grep found a legitimate flow
that writes the file through `edit_file`: the onboarding prompt's embedding-model step
(`src/prompts/source.md`, Phase 0, "User types 2, 3, etc.: Call `edit_file` on
`.codescout/project.toml`. Change the `model` line ...", and the `url` variant). A deny would have broken
that step and forced a change to an onboarding prompt surface. So the guard is **content-aware**:

- `config::protected_files::check_write` (`src/config/protected_files.rs`) refuses a write to a protected
  file when the `[security]` table the write produces differs from the one on disk, or when the new
  content is not valid TOML (it cannot then be shown to leave the table alone). A file is protected if
  it is named `project.toml` directly inside a directory named `.codescout` (ASCII-case-insensitive),
  **or is, by identity, the `.codescout/project.toml` of a project root loaded in this process, or the
  global config** (`same_file`: every link followed, resolved through the deepest existing ancestor).
  An absent or unparseable "before" counts as no table, so a create cannot carry one and a broken file
  cannot be used to smuggle one in. Everything else in the file stays editable.
- **Why identity (review F2, F3, both reproduced first):** the write tools hand the guard a path whose
  parent is already resolved. With `.codescout` a link to `cfg-real`, `create_file(.codescout/project.toml,
  <profile=root>, overwrite)` arrived as `cfg-real/project.toml`, the name rule never fired, and the write
  landed (red: the file changed). With `project.toml` a link to `../shared.toml`, writing `shared.toml` under
  its own name changed what `project.toml` says (red: the file changed). Both are now refused.
- **How the roots reach the guard:** a process-wide, grow-only set (`PROJECT_ROOTS`) filled by
  `ProjectConfig::load_with_global_base`, which every project's config goes through. Agent state was
  rejected because the guard sits in `util::fs::atomic_write`, a free function with no `Agent`; threading
  one through would change the signature of the funnel the guard exists to sit in. The set can only widen
  what is protected (a directory once loaded keeps its `project.toml` guarded after the project is gone),
  so sharing it between servers and tests cannot loosen anything.
- It is called from `util::fs::atomic_write`, the funnel `create_file`, `edit_file` (text, append and
  batch grammars; measured, each reaches it) write through. A write tool added later is covered on the day
  it is added. Two paths do NOT go through `atomic_write` and were measured landing the file before the
  fix, so they ask the same guard: `doc(create)` (`src/librarian/tools/create.rs`, a plain `fs::write`;
  there the guard is defence in depth for the PROFILE, since without it the file starts with `---` and does
  not parse, so the config fails closed anyway, measured: `exists=true profile=Default`; what the guard
  alone prevents is replacing a fresh `project.toml` with an unparseable file) and `doc(move)`
  (`src/librarian/tools/mv.rs`, a `rename`; a markdown file holding `[security] profile = "root"` was
  moved onto a fresh `project.toml`).
- The refusal says what is theirs to do ("Ask the user to make that edit in an editor; the server re-reads
  the file on its next call"). `CONFIG_IS_CACHED_REMEDY` (`src/config/project.rs`) used to prescribe
  `edit_file` for the `[security]` keys its three callers are about; that is now a refused action, so the
  text names an editor and no longer says the config is cached for the life of the process (untrue since
  2026-09-14). The `ConfiguredOff` refusal in `src/util/path_security.rs` is corrected the same way
  (strings only; review F8): it no longer blames `file_write_enabled = false` alone (an unparseable file
  also disables writes) and no longer prescribes a restart as the only route.

**Measured NOT to be entry points for `project.toml`:** `edit_file`'s markdown grammar (heading, action,
frontmatter: "apply to markdown files only"), `edit_code` ("unsupported file type" for a `.toml`),
`doc(update)` (needs a catalogued artifact, and a `.toml` is not one), `memory(write)` with a topic that
climbs out of `.codescout/memories` (returned ok, `project.toml` unchanged: the topic is sanitised).
Those are pinned only where a test could fail if the guard were removed; the memory test is a pin of
measured behaviour, not a test of this guard.

**Related, pre-existing, fixed on the same branch (review F6):** onboarding wrote an explicit `[security]`
table of DEFAULTS into the `project.toml` it creates, which overrides a STRICTER global config for that
project (red: with a global `shell_command_mode = "disabled"`, `indexing_enabled = false`, the created
file gave `("warn", true)`). The created file now omits `[security]` (commit `0d3ce795a`, patch-id
`8eccc515990916b4e6d4bc52267503df43e4bc9a`). The two stamping writers loaded the MERGED config and wrote it
back; measured: that does not relax the values at the moment of the write, it copies the global values into
the project's file where they shadow any later change to the global config; they now patch only the raw
`[project]` table. Not examined: the other default sections onboarding writes (`[ignored_paths]`,
`[memory]`, `[libraries]`) against the global config's `[ignored_paths]`.

**Alternatives considered.**

1. Blanket path deny in `classify_write_path_in` (one site, covers every path-taking tool, also a future
   one): rejected because it breaks the onboarding embedding step above. Still the simpler choice if that
   step is changed to tell the user to edit the file, or moved into the `onboarding` tool.
2. Tighten-only reload: see OPEN DECISION above.
3. Do it per tool at each write site: seven sites, and a new writer is unguarded until someone remembers.

**Residual, stated so it is not read as wider.**

- **`run_command` writes the file** (the open decision above). A session with an enabled shell is not
  stopped from editing `project.toml`; `shell_command_mode = "disabled"` is the only setting that stops it.
- **`[embeddings].url` and `api_key` stay editable through `edit_file`, which is a code and credential
  exfiltration channel, not only a redirect of the embedding endpoint** (pre-existing; review F7). Read, not
  run end to end: `src/retrieval/config.rs` `merge_embed_config` resolves `url = env.or(project)` and
  `api_key = env.or(project_key)` independently, and `crates/codescout-embed/src/remote.rs` posts each
  batch of input text to that endpoint with `bearer_auth(key)`. So a project file that names a URL, with no
  URL in the environment, makes indexing send code chunks to it, and an `EMBED_API_KEY` from the user's
  environment or dotenv is sent as the bearer token. When a URL is set in the environment it wins and the
  channel does not exist. Not addressed.
- Windows spellings of the protected name (a trailing dot or space, an NTFS `::$DATA` stream) may defeat
  the name rule, and `util::path_security` has no normalisation for them. **Unverified**: not run on
  Windows. The identity match (`resolve`, `canonicalize`) may or may not catch them.
- A human editing the file, and `onboarding` itself (it writes `project.toml` with `std::fs::write`), are
  unaffected by design.
- Under `profile = "root"` the classifier already allows every write; the guard does not change that.
- A hard link to `project.toml` created by a shell is not detected (identity compares resolved paths, not
  inodes).
- A project root that was loaded keeps its `project.toml` guarded for the life of the process; a project
  whose root is only ever reached through a path no loaded root resolves to is guarded by the name rule alone.

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

Red lines of the review findings, measured on the tree before each fix (2026-10-18):

```
F2 a_dot_codescout_that_is_a_link_to_another_name_is_still_guarded ... FAILED
   left:  "[project]\nname = \"t\"\n\n[security]\nprofile = \"root\"\n"   (the file after the write)
   right: "[project]\nname = \"t\"\n\n[embeddings]\n... [security]\nshell_command_mode = \"warn\"\n"
F3 the_target_of_a_symlinked_project_toml_is_guarded_under_its_own_name ... FAILED   (same shape)
F6 a_created_project_toml_does_not_relax_a_stricter_global_config ... FAILED
   left: ("warn", true)  right: ("disabled", false)
```

The `doc(create)` and `doc(move)` tests were added after the first red run and were red on their own runs
(both landed the file) before their guards were added.

After: all pass. Controls that must keep working (they pass before and after): the onboarding embedding
edit through `edit_file` lands AND reaches the running config; a rewrite that leaves `[security]` equal
(comment, reordered tables, extra section) is allowed; other files under `.codescout/` and a
`docs/project.toml` stay writable.

**Mutation probes, re-run on the final tree** (one literal mutation per guarded site, each reverted,
byte-for-byte): the Bug 2 set is 3 call-site removals (`atomic_write`, `doc(create)`, `doc(move)`), the
guard's name, global-path, identity and equality logic, the TOML-parse refusal, the table name, the "before"
reader and the remedy text, plus per-branch mutants for the identity match (identity never, `same_file`
reduced to spelling, leaf link not followed, root comparison by spelling, `any` replaced by `all`, root
never registered). 81 mutants were run in all (this file's and the reload issue's, one table), **81
killed, 0 survived** (two were first REFUSED by a find-string that no longer matched the formatted code,
and were re-run with the corrected string and killed). The leaf-link mutant is killed by the F3 test
alone; the symlinked-directory case has no mutant that only its own test kills: it shares the identity
mutants with F3.

One test (`a_memory_topic_cannot_reach_project_toml`) passes with the guard removed: it pins measured
behaviour of a path that is not an entry point. One test
(`a_shell_redirect_still_writes_the_security_section_known_residual`) asserts the residual EXISTS; if it
fails, the residual has closed and the issue and the manual must change. Not run: the real binary through
an MCP client.

## Workarounds

None needed for a trusted session; for an untrusted one, the profile is a property of the project
directory the session is pointed at.

## Resume

Carry the branch to `experiments`. **Decide the tighten-only reload** (OPEN DECISION above): it is the
change that closes the shell route and the symlink and onboarding variants for mid-session edits, and it
changes the user's own mid-session edit-to-loosen flow. **Decide the `[embeddings].url` exposure**
(Residual). Then archive this file with the SHA on `experiments` and its patch-id.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
