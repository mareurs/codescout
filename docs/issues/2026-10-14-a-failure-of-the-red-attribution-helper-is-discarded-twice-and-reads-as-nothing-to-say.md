---
id: '7badc94a57797d7d'
kind: bug
status: open
title: 'BUG: a failure of the red-attribution helper is discarded twice, so the author line vanishes with no trace and reads as "nothing to say"'
owners:
- marius
tags:
- cluster/unclassified
topic: authorship attribution at a red
closed: ''
opened: 2026-10-14
owner: marius
severity: medium
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

Not done. One direction, not chosen: have the helper print a one-line tooling-failure notice to stdout (not only under `--explain`) when the import fails, and have the caller keep stderr. That changes what a red looks like to every user, so the wording and the opt-out need a decision first.

## Tests added

None.

## Workarounds

Run `scripts/attribute-red.py --explain` by hand when a red should have named an author and did not.

## Resume

Decide the wording of a tooling-failure line, then add a test that runs the materialized helper with a sibling removed and requires that line. Keep the "never 'nobody wrote it'" rule.

## References

- `scripts/attribute-red.py`, `src/tools/run_command/attribution.rs`
- `docs/issues/archive/2026-10-13-embedded-attribution-helper-ships-two-of-its-six-modules-so-a-red-loses-its-author-silently.md`
