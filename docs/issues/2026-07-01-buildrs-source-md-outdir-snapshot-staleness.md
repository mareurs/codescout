---
kind: bug
status: wontfix
tags:
- build.rs
- prompt-surfaces
- snapshot
- incremental-build
- false-pass
closed: 2026-07-20
last_observed: 2026-07-01
opened: 2026-07-01
owner: marius
related: []
severity: low
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

**Leading hypothesis DISPROVEN (2026-07-05).** `build.rs` **does** emit
`cargo:rerun-if-changed=src/prompts/source.md` (and `=build.rs`) at the end of
`emit_prompt_surfaces()` — confirmed by reading `build.rs`. So "missing
`rerun-if-changed`" is not the cause. The dependency chain is sound end to end:
`source.md` change → cargo reruns `build.rs` (rerun-if-changed) → regenerates
`OUT_DIR/server_instructions.md` → `SERVER_INSTRUCTIONS`
(`include_str!(concat!(env!("OUT_DIR"), "/server_instructions.md"))`,
src/prompts/mod.rs:11) recompiles because rustc tracks `include_str!` targets
in its dep-info → the snapshot test sees the new bytes.

Incidental reinforcement: `bake_git_sha()` also emits
`rerun-if-changed=.git/HEAD` / `.git/index` / `.git/refs/heads/`, so *any* git
operation (commit, stage) re-runs `build.rs` and regenerates the OUT_DIR copy —
keeping it aggressively fresh and making a staleness window very hard to hit.

Root cause of the original Task-6 observation remains **unconfirmed** — most
likely a one-off cargo fingerprint anomaly (the kind `cargo clean` resolves) or
a misattribution (e.g. the edited span fell outside the `<!-- @surface -->`
markers, so the slice legitimately didn't change).
## Evidence

Task 6 report (`.superpowers/sdd/task-6-report.md`, "Concerns"): snapshot test
false-passed on the incremental build; `cargo clean -p codescout` + rebuild
then surfaced the real diff, after which `UPDATE_PROMPT_SNAPSHOTS=1` regenerated
the fixture and the test passed legitimately.

## Hypotheses tried

1. **Hypothesis:** `build.rs` missing `rerun-if-changed` for `source.md`.
   **Test:** not yet run — inspect `build.rs`. **Verdict:** deferred.

## Fix

**No code change — cannot reproduce on current HEAD (28ccbb17).**

Controlled repro: injected a distinctive ` ZZZPROBE` token into the
`server_instructions` surface of `src/prompts/source.md`, then ran
`cargo test --lib prompt_surfaces_server_instructions_snapshot` on an
**incremental (no-clean)** build. The test **failed** correctly
(`expected 2167 bytes, actual 2176` — the +9 probe bytes), i.e. the OUT_DIR
copy tracked the edit. Ran a **second** incremental build — failed again
(no false-pass). Reverted the probe → test passes, tree clean. The mechanism
the bug claimed was broken works reliably here.
## Tests added

None needed — the existing `prompts::tests::prompt_surfaces_server_instructions_snapshot`
IS the guard, and it was verified (above) to catch a `source.md` edit on
incremental builds across two consecutive runs. The complementary invariant
`source_md_under_cap` and the build.rs parser-duplication byte-equality test
(build.rs doc-comment) round out the coverage.
## Workarounds

`cargo clean -p codescout` before running prompt snapshot tests after editing
`src/prompts/source.md`, then regenerate the fixture with
`UPDATE_PROMPT_SNAPSHOTS=1` and re-run.

## Resume

Closed as **zombie** (no longer observed; root cause unconfirmed). **Re-open
trigger:** a snapshot test false-passes after a real `source.md` slice edit on
an incremental build AND survives a second `cargo test` without `cargo clean` —
capture the exact `source.md` diff, the cargo command sequence, and
`ls -la $OUT_DIR/server_instructions.md` mtime vs the source.md mtime at that
moment. If it recurs, the durable fix is to have the snapshot test read+slice
`src/prompts/source.md` directly (via the crate-side `extract_surface`) instead
of the OUT_DIR `include_str!` copy — removing the build.rs/OUT_DIR hop from the
test's trust chain entirely.
## References

- Surfaced during grep-improvements Task 6 (`docs/superpowers/plans/2026-07-01-grep-improvements.md`).
- `.superpowers/sdd/task-6-report.md` (Concerns section).
