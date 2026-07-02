---
status: open
opened: 2026-07-01
closed:
severity: low
owner: marius
related: []
tags: ["build.rs", "prompt-surfaces", "snapshot", "incremental-build", "false-pass"]
kind: bug
---

# BUG: `build.rs` OUT_DIR copy of `source.md` not reliably regenerated on incremental builds → prompt snapshot test false-passes

## Summary

During the grep-improvements Task 6 (editing `src/prompts/source.md`), the
`prompt_surfaces_server_instructions_snapshot` test *passed on a stale
incremental build* even though the rendered `server_instructions` slice had
changed — the `build.rs`-managed `OUT_DIR` copy of `source.md` was not
regenerated despite cargo printing a "Compiling codescout" line. A
`cargo clean -p codescout` + rebuild was required before the snapshot test
saw the real (changed) output. Risk: a source.md edit can ship with a stale
snapshot fixture, and the gate that is supposed to catch prompt-surface drift
silently agrees with itself.

## Symptom (Effect)

- Edit `src/prompts/source.md`.
- `cargo test --lib prompt_surfaces_server_instructions_snapshot` → PASS
  (unexpectedly — the change should have either failed the snapshot or the
  regenerated fixture should differ).
- `cargo clean -p codescout` + re-run → the test now reflects the real change
  (fails until the snapshot fixture is regenerated with `UPDATE_PROMPT_SNAPSHOTS=1`).

## Reproduction

Not yet minimally isolated. Best lead: edit `src/prompts/source.md`, run the
snapshot test twice without a clean; observe whether the `OUT_DIR` copy tracks
the edit. Confirm `build.rs`'s `cargo:rerun-if-changed` covers `source.md`
(and any file it `include_str!`s / copies into `OUT_DIR`).

- Commit context: `f6a41fec` (branch `experiments`).
- Observed by the Task 6 implementer subagent, 2026-07-01.

## Environment

- codescout build via cargo; incremental (non-clean) builds.
- Files: `build.rs`, `src/prompts/source.md`, `tests/fixtures/prompt_surfaces/server_instructions.md`.

## Root cause

Unknown — under investigation. Hypothesis: `build.rs` either does not emit
`cargo:rerun-if-changed=src/prompts/source.md` (so cargo doesn't re-run the
script when only that file changes), or it copies `source.md` into `OUT_DIR`
in a way that a warm incremental build skips. The "Compiling codescout" line
is misleading — it reflects the crate recompiling for the test edit, not
`build.rs` re-running.

## Evidence

Task 6 report (`.superpowers/sdd/task-6-report.md`, "Concerns"): snapshot test
false-passed on the incremental build; `cargo clean -p codescout` + rebuild
then surfaced the real diff, after which `UPDATE_PROMPT_SNAPSHOTS=1` regenerated
the fixture and the test passed legitimately.

## Hypotheses tried

1. **Hypothesis:** `build.rs` missing `rerun-if-changed` for `source.md`.
   **Test:** not yet run — inspect `build.rs`. **Verdict:** deferred.

## Fix

Plan (not implemented): audit `build.rs` for `cargo:rerun-if-changed`
directives covering `src/prompts/source.md` (and every file copied to `OUT_DIR`
or `include_str!`'d for a snapshot). If missing, add them. Consider making the
snapshot test read `source.md` directly rather than an `OUT_DIR` copy, if the
copy is the staleness source.

## Tests added

N/A — under investigation. A regression test would edit source.md in a temp
harness and assert the rendered slice tracks it without a clean build (may be
impractical; manual verify may be the pragmatic gate).

## Workarounds

`cargo clean -p codescout` before running prompt snapshot tests after editing
`src/prompts/source.md`, then regenerate the fixture with
`UPDATE_PROMPT_SNAPSHOTS=1` and re-run.

## Resume

Inspect `build.rs` for `rerun-if-changed` coverage of `src/prompts/source.md`
and any OUT_DIR copy step. Confirm whether the snapshot test reads from OUT_DIR
or the repo path. Reproduce with a double-run (no clean) after a source.md edit.

## References

- Surfaced during grep-improvements Task 6 (`docs/superpowers/plans/2026-07-01-grep-improvements.md`).
- `.superpowers/sdd/task-6-report.md` (Concerns section).
