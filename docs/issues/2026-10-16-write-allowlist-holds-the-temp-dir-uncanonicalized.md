---
status: open
opened: 2026-10-16
closed:
severity: low
owner: marius
related: [docs/issues/archive/2026-05-24-ci-macos-tempdir-canonicalization.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: the write allowlist holds the temp dir un-canonicalized, so a write into a symlinked TMPDIR is denied as outside the root

## Summary

`classify_write_path` (`src/util/path_security.rs`) canonicalizes the write target
(`canonicalize_write_target`) and the project root (`best_effort_canonicalize(project_root)`),
but pushes the temp dir as `crate::platform::temp_dir()` as-is. When `$TMPDIR` is, or sits
under, a symlink, the canonical target never starts with the literal temp dir, so the
"system temp directory is always writable" rule does not apply and the write is
`OutsideRoot`.

## Class fit

No existing class names a comparison of a canonical path against a root held in its literal
form. `IC-5` (the reproduction environment is not the gating environment) is nearest because it
only shows on a host with a symlinked temp dir, but the fault is in the production comparison,
not in a repro. Filed unclassified; revisit if a second allowlist root shows the same shape.

## Symptom (Effect)

A write to `<symlinked TMPDIR>/x.txt` is refused with "outside the project root. Call
approve_write(...)". Fails closed, so no security exposure; the cost is a spurious approval
prompt, and on macOS (`/var` -> `/private/var`) the test `write_to_tmp_allowed` is `#[ignore]`d
for this reason.

## Reproduction

Scratch test (not committed), run alone: create `real = tempdir()`, symlink `link -> real`,
`set_var("TMPDIR", link)`, then `classify_write_path(link/x.txt, project, default_config(), [])`.

## Environment

Linux, codescout `experiments` at 3923da8c plus 8eeb4c90.

## Root cause

`src/util/path_security.rs`, in `classify_write_path`: `allowed.push(crate::platform::temp_dir())`
is not passed through `best_effort_canonicalize`, unlike `project_root`, `cwd`, `extra_write_roots`
and `session_roots` directly below it.

## Evidence

Measured 2026-10-16 with the scratch test above:
`temp_dir="/home/marius/.cache/cc-tmp/.tmpyi5xYh/tmplink"`,
`decision=OutsideRoot { resolved: "/home/marius/.cache/cc-tmp/.tmplT6k5I/x.txt" }`
(the resolved path is the symlink's target, so the literal root cannot match).

## Hypotheses tried

None needed; read at the bytes and confirmed by the run above.

## Fix

Not applied. Canonicalizing the temp root is a change to what path_security.rs allows (it admits
the real directory behind a symlinked TMPDIR), so it was left for a decision rather than made
under a test-fix task. Proposed: `allowed.push(best_effort_canonicalize(&crate::platform::temp_dir()))`
with a red test using a symlinked TMPDIR, then drop the macOS `ignore` on `write_to_tmp_allowed`.

## Tests added

N/A. Not fixed; the reproduction above is the red test to add with the fix.

## Workarounds

`approve_write` on the real temp directory.

## Resume

Decide whether canonicalizing the temp root is wanted, then implement as proposed.

## References

- `docs/issues/archive/2026-05-24-ci-macos-tempdir-canonicalization.md`
- Found while fixing `write_to_tmp_allowed` (8eeb4c90).
