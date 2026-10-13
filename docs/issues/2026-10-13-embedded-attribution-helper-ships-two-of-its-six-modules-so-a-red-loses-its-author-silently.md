---
id: '84114310d21d2c1b'
kind: bug
status: fixed
title: 'BUG: the embedded red-attribution helper ships 2 of its 6 modules, so in production the import fails and a red''s author line disappears silently'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
topic: authorship attribution at a red
closed: 2026-10-13
opened: 2026-10-13
severity: medium
unverified: 'Gate 2026-10-13: fmt, clippy and the lean lane green; the default lane failed one test, every_ledger_append_entry_recipe_is_one_the_code_accepts, on another session''s untracked docs/trackers/tracker-archiving-session-log.md:118, outside this change. Not run through a built binary against a real red. A helper failure is still silent by design (plan step 4 not done). Fix SHA and patch-id not yet recorded.'
---

# BUG: the embedded red-attribution helper ships 2 of its 6 modules, so in production the import fails and a red's author line disappears silently

## Summary

`run_command` embeds only `scripts/attribute-red.py` and `scripts/file-provenance.py` and writes those two to a temp directory. Since `eed25579` (2026-10-11) `file-provenance.py` also imports four sibling modules that are not embedded. In the temp directory the import fails, `attribute-red.py` exits `0` with empty stdout, and the Rust caller reads that as "nothing to say". A red that should name the dirty file's author names nobody, and nothing says why.

Reported 2026-10-13 by a user auditing the `experiments` branch at `18a007aa`. Re-verified here at `5ae71b35`, which differs from `18a007aa` only in `scripts/codex_history.py`, one test file and one doc.

## Symptom (Effect)

Reproduced in isolation 2026-10-13: two files copied to an empty directory, then

```
$ cd <empty dir with attribute-red.py and file-provenance.py only>
$ python3 -B attribute-red.py --explain </dev/null
attribute-red: file-provenance.py could not be imported; no attribution is possible. This is a tooling failure, NOT evidence that the red is nobody's.
$ echo $?
0
```

In production the same message is lost: the spawn at `src/tools/run_command/attribution.rs` (`wip_author_diagnostic_with`) sets `.stderr(Stdio::null())`, passes no `--explain`, and returns `None` when stdout is empty.

## Reproduction

1. `git rev-parse HEAD` is `5ae71b35` (or `18a007aa`).
2. `mkdir D && cp scripts/attribute-red.py scripts/file-provenance.py D/`.
3. `cd D && python3 -B attribute-red.py --explain </dev/null; echo $?`.
4. Expected by the design: the helper imports and runs. Observed: the message above, exit `0`.

This is the exact shape `materialize_into` produces. It was not run through the new binary.

## Environment

Linux, Python 3.12, branch `experiments`. Platform-independent: the failure is a missing file.

## Root cause

- `src/tools/run_command/attribution.rs:96-103` embeds exactly two scripts with `include_str!`, and `materialize_into` (`:200-227`) writes exactly those two. The directory name is a hash of those two bodies only.
- `scripts/file-provenance.py:92-96` imports `claude_history`, `provenance_records`, `codex_history` and `session_context`. Line 91 puts the script's own directory first on `sys.path`, so the imports resolve only if the siblings sit beside it.
- `scripts/claude_history.py:9-10` and `scripts/codex_history.py:9-10` import `provenance_records` and `session_context` in turn, so the closure is 6 files, not 2.
- `scripts/attribute-red.py:62-76` (`_load_provenance`) catches every exception from `exec_module` and returns `None`. `main` then returns `0` after printing to stderr only when `--explain` is given (`:185-191`).
- The Rust side discards stderr and treats empty stdout as no message.

Measured 2026-10-13: the isolated reproduction above. Introduced by `eed25579` (`git log -S"from claude_history import" -- scripts/file-provenance.py`, 2026-10-11). Inferred from the code, not measured: the production hook output is absent on a real red.

Why no test caught it: `embedded_scripts_are_the_repo_copies` and `materialize_writes_both_scripts_beside_each_other` (`attribution.rs` tests) both assert exactly the two files. Both pass while the closure is wrong. Inferred, not checked: `tests/attribute-red.sh` runs the scripts from `scripts/`, where every sibling exists, so the repro environment is not the production environment (the `repro-env-diverges-from-gate-env` class).

## Evidence

The reproduction output is quoted under Symptom. The imports are at `scripts/file-provenance.py:92-96`, `scripts/claude_history.py:9-10`, `scripts/codex_history.py:9-10`.

## Hypotheses tried

1. **Hypothesis:** the import resolves through some path other than the script directory. **Test:** ran the helper in an empty directory without `PYTHONPATH`. **Verdict:** rejected. **Evidence:** Symptom block.

## Fix

Implemented 2026-10-13 in the working tree. The fix SHA and patch-id are recorded below once the change is committed.

1. `src/tools/run_command/attribution.rs`: the two `include_str!` constants became one list, `EMBEDDED_SCRIPTS`, of all six modules (`attribute-red.py` first, as the entry point). The directory hash covers every name and body, and `materialize_into` writes every entry.
2. The closure test (`the_materialized_set_is_closed_under_its_own_local_imports`) reads the imports out of the files that were actually written and requires every sibling that exists under `scripts/` to be beside them. It needs no hand-kept list, so a seventh module cannot be forgotten silently.
3. `the_materialized_engine_imports_in_an_empty_directory` runs the materialized entry point with no `PYTHONPATH` and fails on the import-failure sentence.
4. `a_red_naming_a_dirty_file_reaches_the_engine` and `the_same_red_at_exit_zero_still_answers` used to skip on any missing hint. They now assert when `python3` is present. Before the fix they printed `skipping: no diagnostic (python3 absent?)` while `python3` was present, so the two real-engine tests were vacuous. After it, `a_red_naming…` runs the real scan (19 s for 11 tests).

NOT done, from the original plan: step 4, making a helper failure visible. `attribute-red.py` still prints its import failure only under `--explain`, and the Rust caller still discards stderr. The closure test is now what guards the import set; the silent-on-failure design is unchanged.

## Tests added

- `tools::run_command::attribution::tests::the_materialized_set_is_closed_under_its_own_local_imports` — red before the fix, naming `claude_history.py`, `codex_history.py`, `provenance_records.py` and `session_context.py`.
- `tools::run_command::attribution::tests::the_materialized_engine_imports_in_an_empty_directory` — red before the fix with the exact import-failure sentence.
- `a_red_naming_a_dirty_file_reaches_the_engine` and `the_same_red_at_exit_zero_still_answers` — hardened from a skip to an assertion; the first was red before the fix (`python3 is present but the engine produced no hint …`).
- `materialize_writes_both_scripts_beside_each_other` became `materialize_writes_every_embedded_script_beside_the_entry` and `embedded_scripts_are_the_repo_copies` now covers all six.

Measured 2026-10-13: before the fix 7 passed / 4 failed; after it 11 of 11 pass (`cargo test --lib tools::run_command::attribution`).

## Workarounds

Run `scripts/file-provenance.py` from the repository directly, where all siblings exist.

## Resume

Fixed in the working tree on 2026-10-13. Remaining: record the fix SHA and patch-id after the commit, and archive once the gate is green (`docs/issues/archive/`). Related: `docs/issues/2026-10-13-attribute-red-compares-a-raw-session-id-with-qualified-owners-so-your-own-write-reads-as-a-peers.md`, fixed in the same change.

## References

- `src/tools/run_command/attribution.rs`
- `scripts/attribute-red.py`, `scripts/file-provenance.py`
- Commit `eed25579` (2026-10-11)
- `docs/issues/archive/2026-09-12-the-red-attribution-hook-fires-only-on-a-non-zero-exit-the-mandated-gate-never-produces.md` — the earlier case where silence from this hook was read as an answer.
