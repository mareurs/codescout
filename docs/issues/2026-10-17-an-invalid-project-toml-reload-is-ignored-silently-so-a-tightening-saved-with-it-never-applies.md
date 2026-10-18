---
status: fixed
opened: 2026-10-17
closed: 2026-10-18
severity: medium
owner: marius
related: [docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md, docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md]
tags:
- cluster/unclassified
kind: bug
unverified: The real binary (codescout start, decoy HOME, MCP client, driver mcp_r.py scenario B4) was not re-run against the fix; the regression tests drive call_tool_inner in process. Fix is on branch fix/config-security-2026-10-18 (b5e2ed28b, patch-id 9571e4cf9c53b304067bc7ac807827a1769f2778), not yet on experiments.
---

# BUG: a project.toml edit that fails to load is ignored in a running session, so a tightening saved with an invalid entry never takes effect

## Summary

`ActiveProject::reload_config_from_disk` (`src/agent/mod.rs`) keeps the last good config when the
re-read fails, advances the file stamp, and says nothing. A user who saves `file_write_enabled =
false` in the same edit as any entry the loader refuses keeps writes ENABLED, silently, until the
file is fixed or the server restarted. The loader gained new refusals on branch
`fix/write-allowlist-port-2026-10-17` (a relative, empty, `~` or `..` `extra_write_roots` entry), so
a class of edits that used to load now takes this path. The reload policy itself is older; THIS
file's Fix section changes it (the sentence read "not changed here" when the issue was filed).

## Class fit

A failure that has no reader: the config is rejected and the only trace is the absence of the
change the user asked for, on the one setting (a security tightening) where a missed change is
unsafe. Filed unclassified; it is the same silent-failure shape as the other "drop the whole
input" paths, not a path-containment fault.

## Symptom (Effect)

Real binary (`codescout start --project <p>`, decoy `HOME`, scrubbed environment; driver
`port/mcp_r.py` scenario B4 in the session scratchpad), measured 2026-10-17 on the branch before the
follow-up commits:

```
B4a create_file({"path": "a.txt", "content": "a\n"}) isError=False   {"status": "ok", ...}
# project.toml rewritten: [security] file_write_enabled = false, extra_write_roots = ["~/scratch"]
B4b create_file({"path": "b.txt", "content": "b\n"}) isError=False   {"status": "ok", ...}
  b.txt exists: True
```

The base (`experiments` 27efb11d) does not show it for this edit because `["~/scratch"]` loaded there, so
the tightening applied (reported by the reviewers; the run above is the branch). By reading, not
measured: an edit that carries an entry the base ALSO refuses, an `extra_write_roots` list over 128
entries, takes the same silent path on the base.

## Reproduction

`python3 mcp_r.py <codescout binary> <workdir>`; scenario B4 is the last block. Unit-level, the
same decision is `reload_config_from_disk`'s `if let Ok(fresh) = ... { self.config = fresh; }`.

## Environment

Linux, rustc 1.97.1, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

`reload_config_from_disk` swallows the `Err` from `ProjectConfig::load_or_default`. Its doc comment
states why: keeping the last good config means a malformed file cannot relax a gate by falling
back to permissive defaults, and advancing the stamp anyway means a file that stays broken is
parsed once instead of on every gated call. Both reasons stand; neither requires saying nothing.
Measured: the run above.

## Evidence

The run above. The doc comment cited is `src/agent/mod.rs:473-479` in the tree measured.

## Hypotheses tried

None.

## Fix

Fixed on branch `fix/config-security-2026-10-18` (based on `experiments` 7a09623bd): `b5e2ed28b`, patch-id
`9571e4cf9c53b304067bc7ac807827a1769f2778` (follow-up test commit `40defbf50`). Not yet on `experiments`.

**Decision: fail closed, and say so.** An invalid reload may never leave a policy looser than the one
the file would give with the invalid entry absent.

- `ActiveProject::reload_config_from_disk` (`src/agent/mod.rs`) records the loader's error in a new
  field `config_load_error` (cleared by the next reload that loads) and calls
  `SecuritySection::tighten_for_unloadable_file` (`src/config/project.rs`). That function makes each
  security setting the STRICTER of the previous policy and what the file says. It reads the file
  only to tighten: `file_write_enabled` / `indexing_enabled` `false` closes the gate and a value that
  is not a boolean counts as closed; a stricter known `shell_command_mode` replaces the previous one
  and an unknown or non-string value counts as `disabled`; `profile = "root"` and each
  `extra_write_roots` entry survive only if the file still says so (absent means the default for
  these two keys, and the global config cannot set either). Nothing is loosened, so a root the file
  added, `profile = "root"` or `file_write_enabled = true` saved next to an invalid entry has no
  effect.
- A file that cannot be read as TOML at all (syntax error, unreadable path, over the loader's 1 MiB
  cap: `ProjectConfig::read_project_toml_table` returns `None`) is treated as an empty file and
  additionally disables file writes: nothing in it can be confirmed, and writes are the gate whose
  silent survival the user cannot see.
- Loud: `config_load_error` rides the existing `_workspace_notice` channel on EVERY response
  (`config_load_notice`, `src/tools/core/types.rs`; follows the call's `workspace=` pin; not
  one-shot, because the stamp has already advanced past the broken file), and
  `CodeScoutServer::check_tool_access` appends it to every refused call, which would otherwise blame
  a `file_write_enabled = false` the unparseable file does not contain. Text:
  `config::project::failed_reload_notice`.
- Load-time behaviour is unchanged: a project whose `project.toml` fails to load at activation is
  still refused loudly, naming the file.

**Alternatives considered.**

1. Surface the error only (no tightening): a tightening saved with an invalid entry would still not
   apply until the file is fixed. Rejected: that is the reported bug.
2. Per-field tolerance only (apply a parsed `file_write_enabled = false`): does not cover an
   unparseable file, `profile = "root"` removal, or a narrowed `extra_write_roots`. Kept as the
   parseable-file half of the chosen design.
3. Refuse every write until the file loads: also fail-closed and the simplest, but a typo in an
   unrelated field would block all writes even where the file asks for nothing stricter. The chosen
   design does this only when the file is unreadable.
4. Treat an unreadable file as the strictest policy on every gate (shell `disabled`, indexing off):
   consistent but turns a typo into a loss of every capability. Not chosen; writes only.

**Review follow-ups (2026-10-18), each reproduced with a failing test first, then fixed:**

- **F4 (reproduced; fixed `2087dac8e`, patch-id `ebe3141680c5b3583db8c3cc9169f71d674711d0`).** An ABSENT
  key kept the previous value: `before=unrestricted`, the save deleted `shell_command_mode` next to an
  invalid entry, `after_failed_reload=unrestricted` though the file without the invalid entry means `warn`.
  That broke this issue's own decision. Now an absent `shell_command_mode`, `file_write_enabled` or
  `indexing_enabled` reads as the global config's value if it sets one, else the built-in default, and the
  policy takes the stricter of that and what it was; a global config that cannot be read counts as the
  strictest value (`config::project::GlobalSecurity`, loaded separately from the project file). Red:
  `an_invalid_reload_that_deletes_a_looser_shell_mode_reads_the_default` (left `"unrestricted"`, right
  `"warn"`), and the two child-process global-config tests (`warn` where `disabled` was owed). This
  retires the earlier residual "an absent key keeps the previous value".
- **F5 (reproduced; fixed `479cb047f`, patch-id `11a5083a1cf74a0cd264ba74e8312f70b0f6d2be`).** The notice
  was documented as riding every response, but `call_content` returned from its error path before it was
  built: a missing `read_file`, a missing `edit_file` target and a `create_file` refused by the guard all
  carried none (`tree` did). It now rides error responses too: a `RecoverableError` as an `extra` field
  (object body), a plain error on an `AdvisedError` wrapper rendered as a `⚠` prefix. One test per error
  shape. The other workspace notices (worktree, displaced default) are still success-path only; not changed.

**Residual, by design.** Not salvaged from an invalid edit: `shell_dangerous_patterns` the file adds
(an addition is a tightening, but it was left out to keep the rule small) and a lowered `max_index_bytes`.
The `CONFIG_IS_CACHED_REMEDY` text was changed by commit `0d81e0138` (the Bug 2 fix: it no longer says
`edit_file`, and no longer says the config is cached for the life of the process); the `ConfiguredOff`
refusal in `src/util/path_security.rs` was corrected by `1be1ae8dc` (strings only). A failure whose cause is
the GLOBAL config file takes the same path and is covered by
`an_unreadable_global_config_counts_as_the_strictest_value`.

## Tests added

`src/server/config_reload_tests.rs` (29 tests, real dispatch `call_tool_inner`, out-of-band
`std::fs::write` edits) and `read_project_toml_table_distinguishes_missing_valid_oversized_and_invalid`
in `src/config/project.rs`.

Red line before the fix (measured 2026-10-18, base `experiments` 7a09623bd plus the tests):

```
test server::config_reload_tests::a_tightening_saved_with_an_invalid_entry_still_disables_writes ... FAILED
writes were turned off in the same save as the invalid entry; the call must be refused. Got: {
  "status": "ok",
  "rel_path": "b.txt"
}
test result: FAILED. 1 passed; 10 failed; 0 ignored; 0 measured; 7410 filtered out
```

After: all pass. **Mutation probes, re-run on the final tree** (one literal mutation per guarded site, each
reverted byte-for-byte): the reload set (every branch of the salvage, the global-config read, the file
reader's four outcomes, the error field's set and clear, the accessor, the notice join and its pin, the
refusal append and its pin, the error-path attach, the router's body and fatal renderings and its `Null`
filter) plus one mutant per new branch of F4 (per key: `file_write_enabled`, `indexing_enabled`,
`shell_command_mode`; global ignored, global key missing, global unreadable, global unknown mode) and F5
(each of the three error shapes). 81 mutants were run in all (one table with the Bug 2 file's), **81
killed, 0 survived**. Earlier reload run: 31 mutants, 30 killed first time, the survivor (the refusal
reading the load error for the wrong project) got its own test. The direction guard
`an_invalid_reload_never_loosens_the_shell_mode` passes before the fix: it pins behaviour the old code
already had. Eight of the first 19 tests were written after the first red run and were NOT run against the
pre-fix tree; their evidence is the mutants.

## Workarounds

Restart the server after editing security settings; fix the entry the loader names first.

## Resume

Nothing outstanding for the reload policy. Carry the fix to `experiments`; then archive this file with
the SHA there and its patch-id. Open items listed under Fix, Residual.

## References

- `docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
