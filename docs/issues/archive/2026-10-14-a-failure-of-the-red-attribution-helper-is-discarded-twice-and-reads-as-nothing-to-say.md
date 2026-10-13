---
id: 84a755ff078acfa0
kind: bug
status: fixed
title: 'BUG: a failure of the red-attribution helper is discarded twice, so the author line vanishes with no trace and reads as "nothing to say"'
owners:
- marius
tags:
- cluster/unclassified
topic: authorship attribution at a red
closed: 2026-10-14
opened: 2026-10-14
owner: marius
severity: medium
unverified: 'STANDING — a helper that TIMES OUT still ends silent, by design: a test pins it, because a scan that ran out of time is not a verdict. The failure notices are proved by unit tests on engine_outcome and by tests/attribute-red.sh, not through a built binary against a real red. Gate 2026-10-14: FMT=0 CLIPPY=0 LEAN=0; DEFAULT failed only native_context_wire_reaches_interactive_shell, the open bug 098fca009ecbdd78.'
---

# BUG: a failure of the red-attribution helper is discarded twice, so the author line vanishes with no trace and reads as "nothing to say"

## Summary

`run_command` runs `scripts/attribute-red.py` to name the author of a dirty file at a red. When the helper cannot work, it exits `0` with empty stdout, and the Rust caller discards stderr and an empty answer alike. The red then carries no author line and nothing says why. This hid `docs/issues/archive/2026-10-13-embedded-attribution-helper-ships-two-of-its-six-modules-so-a-red-loses-its-author-silently.md` in production. The 2026-10-13 fix removed that cause and added tests; the silent-on-failure design is unchanged. It was step 4 of the original plan and was not done.

## Symptom (Effect)

Measured 2026-10-14 by reading the code:

- `scripts/attribute-red.py:186-191`: if `file-provenance.py` cannot be imported, `main` prints its message only `if explain:` and returns `0`.
- `scripts/attribute-red.py:76-77`: `_load_provenance` catches every `Exception` from `exec_module` and returns `None`.
- `src/tools/run_command/attribution.rs:377`: `.stderr(std::process::Stdio::null())`.
- `src/tools/run_command/attribution.rs:393-394`: an empty stdout becomes `None`.

So a missing or broken sibling module, a Python error in the loader or an unreadable history all end in the same output as "no dirty file in this red".

## Reproduction

Not run end to end. By the code: materialize the helper with one sibling missing, run it with a red that names a dirty path, and read the result. Before `3fb612b9` this was the production state (the fixed bug above).

## Environment

Linux, `experiments` at `388416f2`.

## Root cause

Silence is the helper's documented answer for "nothing to attribute" (`attribute-red.py` USAGE: `--explain` says "why it stayed silent"), and the same silence is what a tooling failure produces. The caller cannot tell them apart. The code comment at `attribution.rs:390` says "Silent, per the doc above — never 'nobody wrote it'", so the design is deliberate; the cost is that a tooling failure is invisible. Inferred from the lines above, not measured at run time.

## Evidence

The fixed bug above: in production the import failed for as long as `eed25579` (2026-10-11) was in a build, and no test or response showed it. `the_materialized_set_is_closed_under_its_own_local_imports` now guards the import set only; it does not guard other failure causes.

## Hypotheses tried

None.

## Fix

Done 2026-10-14.

- `scripts/attribute-red.py`: `_load_provenance` returns `(module, reason)` instead of swallowing the exception. When the import fails, `main` prints `[codescout] red attribution is unavailable: file-provenance.py could not be imported (<exception>). This is a tooling failure, NOT evidence that the red is nobody's.` to STDOUT on every run, not only under `--explain`.
- `src/tools/run_command/attribution.rs`: the helper's stderr is piped, not nulled. The new `engine_outcome` turns a non-zero exit (an uncaught exception, or a signal) into the same kind of notice, with the last stderr line as the cause. A zero exit keeps its old behaviour: empty stdout is silence, text is text. A timeout is still silence.
- The test `the_materialized_engine_imports_in_an_empty_directory` read stderr only, and the notice now lives on stdout, so it could no longer fail. It reads both streams.

The wording was a decision the bug named. The text above is the choice made, and it keeps the rule that silence never reads as "nobody wrote it".

## Fix provenance

- **SHA:** `4f5b16e4730592c662d5b9e484bbc33b35971120` (`experiments`; the same commit adds the CI job for `tests/attribute-red.sh`)
- **patch-id:** `bb101f16a080b6ba3f57381953117d7d57f92fca` (`git show 4f5b16e4 | git patch-id --stable`)

## Tests added

- `tests/attribute-red.sh`: `the import failure reaches stdout without --explain` and `the notice names the cause`. Red before the change (empty output), 45 of 45 after.
- `tools::run_command::attribution::tests::a_crashed_helper_is_a_tooling_notice_and_never_silence`, `a_helper_killed_by_a_signal_is_a_tooling_notice` and `a_clean_exit_keeps_its_stdout_and_its_silence`. Red before the change (they did not compile: `engine_outcome` did not exist), green after.

Measured 2026-10-14: `cargo test --lib tools::run_command::attribution` 14 passed.

## Workarounds

Run `scripts/attribute-red.py --explain` by hand when a red should have named an author and did not.

## Resume

Complete. No next action.

## References

- `scripts/attribute-red.py`, `src/tools/run_command/attribution.rs`
- `docs/issues/archive/2026-10-13-embedded-attribution-helper-ships-two-of-its-six-modules-so-a-red-loses-its-author-silently.md`
