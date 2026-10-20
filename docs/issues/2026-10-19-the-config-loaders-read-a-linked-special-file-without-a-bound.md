---
status: fixed
opened: 2026-10-19
closed: 2026-10-20
severity: low
owner: marius
related: [docs/issues/2026-10-17-project-toml-under-the-writable-project-root-can-set-security-profile-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: "The /dev/zero case was never read on the real code; a FIFO stood in for it (same mechanism: reported length 0, no end). O_NOCTTY has no test (it needs a terminal device); the open of a device still happens before fstat refuses it (documented in the helper's rustdoc). Reads of repository files that this branch did not touch are listed in docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md."
---

# BUG: the config loaders check a file's reported length and then read it without a bound, so a project.toml that is a link to an endless special file is read until memory runs out

## Summary

`ProjectConfig::load_with_global_base`, `ProjectConfig::read_project_toml_table`,
`protected_files::security_on_disk` and `GlobalConfig::load_from_dir` each call `metadata().len()`,
refuse above 1 MiB, then `std::fs::read_to_string(path)` with no bound on the read itself. A path
whose reported length is 0 but which never reaches end of file (a link to `/dev/zero`, or a FIFO
nobody writes to) passes the length check and is then read until memory is exhausted, or forever.
`project.toml` is repository content and can be such a link.

## Class fit

A size limit that is enforced on a number the file reports about itself rather than on the bytes
actually read, so a file that lies about its size defeats it. Filed unclassified: it is a bound on
the read, not a path-containment fault, and no existing class names a limit checked before and not
during the operation it protects.

## Symptom (Effect)

By reading, not measured: activating (or reloading) a project whose `.codescout/project.toml` links
to `/dev/zero` never returns a config, and the process grows until the OS stops it. The same read
runs on every reload (`ActiveProject::reload_config_from_disk`) and in the write guard
(`security_on_disk`) on the next `atomic_write` of that path.

## Reproduction

Not run, deliberately. Shape: `ln -s /dev/zero <project>/.codescout/project.toml`, then start
`codescout` on the project. Do not do this on a machine you need.

## Environment

Linux, branch `fix/config-security-2026-10-18` (base `experiments` 7a09623bd). The sites are
older than that branch (`load_with_global_base` and `load_from_dir` predate it); the two helper sites
were added by it and copy the loader's pattern.

## Root cause

The 1 MiB cap is applied to `std::fs::metadata(path).len()`, then `std::fs::read_to_string(path)`
reads to EOF. For a regular file the two agree; for a character device, a FIFO or `/proc` entry the
reported length is 0 (or arbitrary) and says nothing about what a read returns. Sites, in the tree
measured: `src/config/project.rs` `load_with_global_base` and `read_project_toml_table`,
`src/config/protected_files.rs` `security_on_disk`, `src/config/global.rs` `load_from_dir`.
Inferred from the code, not measured.


**Measured 2026-10-20** (the premise above, no longer only inferred): with the pre-fix loaders
restored byte for byte under the new tests (`git diff 79dcf2c58` showed only the added tests and
one doc comment), a FIFO standing in place of `project.toml`, of the global `config.toml`, and of
the file `security_on_disk` reads, each held its call past a 15 s deadline (`... did not return
within 15s: it is blocked reading a special file`), and a link to a directory came back as the bare OS
error (`Is a directory (os error 21)`) instead of a refusal naming the reason (the `/dev/null` half
of that project test was not reached on the old code: the loop stops at the first failure). A
link to
`/dev/zero` was never read on the real code; the FIFO shares the mechanism (`metadata().len()`
is 0, `read_to_string` has no end).

## Evidence

The four call sequences above (`metadata` then `read_to_string`, no `take`).

## Hypotheses tried

None.

## Fix

Applied on branch `fix/config-bounded-read-2026-10-20` (base `experiments` 79dcf2c58):

- `6a47e0341` (patch-id `c47ce2d3be1a6c6c98d134bb4d3ebedf342c905f`): `src/util/fs.rs` gains
  `read_regular_bytes` / `read_regular_text` and the config forms `read_regular_config_bytes` /
  `read_regular_config_text`, bounded by `MAX_CONFIG_FILE_BYTES` (1 MiB, `cap-class: NOT_A_CAP`).
  The open is `O_NONBLOCK` and follows links; the type is checked on the opened descriptor
  (`fstat`), so a directory, FIFO, socket or device is refused without a read; the length is
  bounded by the reported size and again by `take(max + 1)` on the bytes read. `BoundedReadError`'s
  `Display` names the reason only (no path, no content).
- `ee2b59468` (patch-id `42ea2e0523bfc1885f59718c250c5e57b2f7bff1`): the call sites use it:
  `ProjectConfig::load_with_global_base`, `ProjectConfig::read_project_toml_table`,
  `protected_files::security_on_disk`, `GlobalConfig::load_from_dir`, the startup dotenv in
  `load_startup_env`, and the two `workspace.toml` reads in `src/agent/mod.rs`. A refused file
  is an `Err` from the project and global loaders, which `load_project_config` already maps to
  `ConfigLoadFailed` (so the pin refusal and the failed-reload notice name the file and show
  nothing from it); `read_project_toml_table` and `security_on_disk` return `None` (an unreadable
  file, which tightens the gates and counts as an empty "before"), as for any other unreadable file.

Review round 1 (an Opus review of `a3bdc48fb` found the helper correct and these gaps), fixed by:

- `a2046871a` (patch-id `06cd9dea525f8b34f48c6be6b3572b35eae84318`), **a security regression in
  the first pass**: `security_on_disk` returned `None` for a file over the cap or not a regular
  file, and the write guard compared `None == None` as "unchanged". Two steps then dropped
  `[security]` from a protected file: pad `project.toml` past 1 MiB with the table unchanged
  (allowed), then write a small file with no table (allowed; at 79dcf2c58 it was refused, because
  the old code read any size). `security_on_disk` is now `Result<Option<_>, String>`: a file that
  is present and cannot be read as it is (non-regular, over the cap, or an OS error other than
  not-found / non-UTF-8) is an `Err`, and `check_write_with` refuses every write to it. Absent,
  dangling-link and non-TOML / non-UTF-8 files keep the old meaning (an empty "before"). Whether
  the write tools can deliver more than 1 MiB was not driven end to end; a grep of `src/tools` for a
  content-size limit found none, and `edit_file` and `create_file` take the text as a tool argument.
- `a47a425cc` (patch-id `8b39e8ce0971b3dc64eac8710d5a9d2cec314c60`), reads the first pass missed:
  `ProjectStatus` (`workspace(action="status")`) and `build_system_prompt_draft` read
  `workspace.toml` with a plain `read_to_string` (a link to a FIFO: no answer, driven by the
  reviewer on the dev binary), and server start-up read `.codescout/cc_session_id` the same way (a
  FIFO held startup; a link to `/dev/stdin` made the server read the client's `initialize` as its
  session id). All four `workspace.toml` reads now use one `config::workspace::load_workspace_config`;
  `cc_session_id` goes through `read_cc_session_id` with a 4 KiB cap (`MAX_SESSION_ID_FILE_BYTES`,
  `cap-class: NOT_A_CAP`).
- `def666def` (patch-id `c5ae922b20a18bb7aea2bcdc0fc8fb9e57d35aef`): the project and global loaders
  wrapped the helper's error in a context layer, so a caller printing `{e}` (the embedding-config
  loader) saw `reading <path>` and no reason. The reason is now in the message
  (`exceeds the 1048576 byte limit`, `not a regular file ...`, or the OS error), still no content.
  `read_dotenv_pairs` is split out of `load_startup_env` as a test seam.
- `7270d5c89` (patch-id `fe005621af6255511921b7934e847e7552adf7df`): `read_bounded<R: Read>` is split
  out so the `take(max + 1)` bound is tested alone (the mutant `take(u64::MAX)` survived the first
  pass because the length check after the read still caught it); `O_NOCTTY` added; a socket (`open`
  fails `ENXIO`) is reported as `NotRegular`; `within_deadline` re-raises a panic from its closure
  instead of calling it a hang; the `MAX_CONFIG_FILE_BYTES` doc no longer says "every loader".
- `4f218e6c9` (patch-id `e5fd1ae64174e99d8951d5cf8d2223e36ff001c3`): the status test owns its thread
  and a 120 s deadline (`status` takes ~19 s on this machine even with no `workspace.toml`), and
  `within_deadline_of` is added.

Review round 2:

- `d1e062c90` (patch-id `ef23ef0505f47b5a742d5b925786b90f2cb2fe73`): padding a protected file past the
  cap with `[security]` unchanged was still allowed, after which every write (even the identical
  small content) was refused, the loader refused the file, and only a shell or the user could
  repair it; the refusal also ended with a sentence ("Edits to the rest of the file are
  allowed.") that was false for that case. `check_write_with` now refuses new content longer
  than `MAX_CONFIG_FILE_BYTES` for a path protected by name, as the global config, or by identity,
  and the "before cannot be read" refusal names the reason and the action (shrink the file or
  replace the special file, outside these tools) without the false sentence. The module doc lists
  the four refusal causes and records that the new refusals (permission-denied, link loop, a link
  to `/dev/null`, a socket, a FIFO, a directory at a protected path) are accepted because the
  loader already could not read any of them. A non-TOML or non-UTF-8 "before" that held a
  `[security]` table still lets a table-dropping repair write through, on purpose and now said
  so in the `security_on_disk` docstring.
- `44338ba87` (patch-id `58b0dd6ffa771d2ae198e2de5d3e7f938a0e4a37`): `read_startup_env` is split out
  of `load_startup_env` and `resolve_cc_session_id` out of the server constructor, so a FIFO test
  goes through the code the two call sites run (the mutants that read without the helper in those
  two functions are killed). The status test's deadline is 300 s (status is ~19 s idle and ~60 s
  at 2x CPU oversubscription). The `/proc` test is renamed
  `a_file_reporting_length_zero_is_refused_when_its_bytes_exceed_the_cap` and its doc says it does
  not kill `take(u64::MAX)`; `the_read_itself_stops_one_byte_past_the_cap_and_never_pulls_more`
  does.

Correction to the first pass: this file said `src/workspace.rs` `read_file_capped` was unbounded on
a linked FIFO. It is not: it caps with `take(max_bytes)` (`:24`) and is reached only for entries the
`ignore` walker reports as regular files by their own type (`entry.file_type().is_file()`, `:61`;
no `follow_links`), so a linked FIFO is never opened there. The unbounded reads that remain are in
`docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md`.

## Tests added

Red before, green after (pre-fix loaders restored, 15 s deadline so a hang is a failure):

- `config::project::tests::a_project_toml_that_is_a_fifo_is_refused_by_the_loader_and_the_salvage`
- `config::project::tests::a_project_toml_that_links_to_a_directory_or_a_device_is_refused`
- `config::global::tests::a_global_config_that_is_a_fifo_is_refused_not_read`
- `config::global::tests::a_global_config_that_is_a_link_to_a_directory_is_refused`
- `config::protected_files::tests::a_fifo_or_a_directory_on_disk_is_an_unreadable_before_and_never_a_hang`
- `agent::existing_root_tests::a_special_file_as_project_toml_becomes_the_marker_not_a_hang_or_a_default`

Green before and after (regression guards, they do not discriminate the old code, which already
refused a regular file over 1 MiB by its length and read a link to a regular file):
`a_project_toml_over_the_cap_is_refused_and_one_at_the_cap_loads`,
`a_global_config_over_the_cap_is_refused_and_one_at_the_cap_is_read`,
`a_project_toml_that_links_to_a_regular_file_still_loads`,
`a_global_config_that_is_a_link_to_a_regular_file_still_loads`.

Added in review round 1 (red before on the pre-fix lines, restored byte for byte for the reads and
as the one-line equivalent for the guard, the message and the two mutants; green after):
`a_file_over_the_cap_refuses_every_write_to_a_protected_path`,
`a_non_regular_file_at_a_protected_path_refuses_every_write`,
`status_answers_when_workspace_toml_is_a_fifo_or_a_link_to_one` (with a regular-file control),
`a_fifo_workspace_toml_does_not_hold_the_draft_and_a_regular_one_still_adds_dependencies`,
`read_discover_settings_refuses_a_special_file_and_reads_a_linked_regular_one`,
`cc_session_id_tests::*`, `a_refused_project_toml_says_why_in_the_top_level_message_and_quotes_nothing`,
`a_refused_global_config_says_why_in_the_top_level_message_and_quotes_nothing`,
`the_startup_dotenv_read_refuses_a_special_file_and_reads_a_regular_one`,
`the_read_itself_stops_one_byte_past_the_cap_and_never_pulls_more`,
`a_socket_is_refused_as_not_a_regular_file`, `within_deadline_reraises_a_panic_from_the_closure`.
No test covers `O_NOCTTY` (it needs a terminal device).

Added in review round 2: `new_content_over_the_cap_is_refused_for_each_protected_path_kind` (three
path kinds, and exactly the cap still allowed),
`the_unreadable_before_refusal_says_the_reason_and_the_action_and_no_false_promise`
(red on the unmodified guard, `red6.log`: both failed there), `the_startup_env_decision_reads_the_chosen_file_bounded`,
`cc_session_id_tests::the_resolution_never_hangs_on_a_special_file_and_still_uses_a_regular_one`
(each fails with a `did not return within 15s` when the helper call in its function is replaced by
a plain read). `a_file_over_the_cap_refuses_every_write_to_a_protected_path` now asserts that the
padding step is itself refused and builds the oversized file by hand for the second step.

Helper tests in `util::fs::bounded_read_tests` (they test the new function, so they have no
"before"): FIFO, link to a FIFO, link to `/dev/zero`, directory and link to one, link to a regular
file, exact cap and one byte over, a `/proc` file that reports length 0 and returns more than the
cap (the bound is on the bytes read), not-found and dangling link, invalid UTF-8, no path or
content in the message.

## Workarounds

Do not point `project.toml` at a special file; a project from an untrusted source should not be
activated without looking at `.codescout/`.

## Resume

Fixed on `fix/config-bounded-read-2026-10-20`; not yet on `experiments`. When it lands there,
archive this file with `doc(action="move")` and label the fix SHA `experiments`. Open follow-up:
the two adjacent unbounded reads named under `unverified`.

## References

- `docs/issues/2026-10-17-project-toml-under-the-writable-project-root-can-set-security-profile-root.md`
