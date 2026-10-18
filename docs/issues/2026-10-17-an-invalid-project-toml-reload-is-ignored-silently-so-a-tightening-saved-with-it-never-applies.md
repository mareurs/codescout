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
a class of edits that used to load now takes this path. The reload policy itself is older and is
not changed here.

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

**Residual, by design.** Not salvaged from an invalid edit: `shell_dangerous_patterns` the file adds
(an addition is a tightening, but it was left out to keep the rule small), a lowered
`max_index_bytes`, and a tightening that only the GLOBAL config carries (an absent key keeps the
previous value, because the global config may be what supplies it). A failure whose cause is the
GLOBAL config file (not project.toml) takes the same path and was not exercised by a test: the
salvage reads project.toml only. The text of `CONFIG_IS_CACHED_REMEDY` and the `ConfiguredOff`
refusal still say the config is cached for the life of the process; that predates this fix and is
not changed here.

## Tests added

`src/server/config_reload_tests.rs` (19 tests, real dispatch `call_tool_inner`, out-of-band
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

After: all pass. Mutation probes, one literal mutation per guarded site, each reverted: 31 mutants
on the reload path (the whole tighten call removed, then each branch of `tighten_gate`, the shell
rank and its unknown-value defaults, profile, roots, the file reader's four outcomes, the error
field's set and clear, the accessor, the notice join and its pin, the refusal append and its pin).
30 were killed first time; the survivor (the refusal reading the load error for the wrong project)
got its own test and is now killed. The one test that passes before the fix is the direction guard
`an_invalid_reload_never_loosens_the_shell_mode`, which pins behaviour the old code already had.
Eight of the 19 tests were written after that red run and were NOT run against the pre-fix tree;
their evidence is the mutation probes (the first removes the whole tighten call: 13 of the 17 tests
then present failed).

## Workarounds

Restart the server after editing security settings; fix the entry the loader names first.

## Resume

Nothing outstanding for the reload policy. Carry the fix to `experiments`; then archive this file with
the SHA there and its patch-id. Open items listed under Fix, Residual.

## References

- `docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
