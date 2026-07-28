---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- flaky-test
- heartbeat
- test-env-isolation
topic: null
time_scope: null
closed: '2026-07-05'
opened: '2026-07-02'
opened-by: claude
owner: marius
related:
- docs/issues/2026-07-02-guide-hint-artifact-not-registered-ci-flake.md
severity: medium
---

# BUG: heartbeat CURRENT_OP test races every concurrent tool dispatch

## Summary
`heartbeat::tests::note_background_op_prefixes_and_is_observable` asserts on the
process-global `CURRENT_OP` (src/heartbeat.rs:301) while `server.rs:628` calls
`note_tool(&req.name)` on EVERY tool dispatch from any concurrently running,
non-serial test. Any tool-dispatching test scheduled in the race window
overwrites the op between `note_background_op` and `current_op()`. Flaked once
on CI (ubuntu default, rerun of run 28603561282).

## Symptom (Effect)
```
thread 'heartbeat::tests::note_background_op_prefixes_and_is_observable' panicked at src/heartbeat.rs:301:9:
assertion `left == right` failed
  left: "semantic_search"
 right: "bg:auto_index:foo"
```
`left` is another test's tool dispatch landing in the window.

## Reproduction
Not deterministic — window is a few instructions wide; needs an adjacent
tool-dispatching test on a loaded 2-core runner. Same source is green in most
runs (local 2987/0/43 many times; ubuntu default green at 218e0a4c).

## Environment
GitHub Actions ubuntu-latest, default features, parallel cargo test.

## Root cause
Shared mutable global (`CURRENT_OP` mutex in src/heartbeat.rs:68-71) asserted on
by a non-`#[serial]` test while non-serial writers exist by design
(`note_tool` on every dispatch, `note_background_op` in
src/agent/mod.rs:1548 and src/tools/semantic/index.rs:311). `#[serial]` cannot
fix this alone — the writers are unmarked tests.

## Evidence
CI run 28603561282 (rerun), ubuntu default job log (scratchpad
ubuntu-rerun.log, session 2026-07-02). One incidence in 3 runs of identical
source; the other two runs failed on the sibling guide_hint race instead (see
related bug file).

## Hypotheses tried
1. **Hypothesis:** introduced by the perf-vdi-closure branch. **Test:** red and
   green runs share identical Rust source (delta = ci.yml + docs). **Verdict:**
   rejected — pre-existing race; the branch's +8 tests may shift scheduling
   neighborhoods and incidence, not the defect.

## Fix

Implemented option (a) — the smallest, API-honest fix. Extracted the two
`CURRENT_OP` entry constructors (`tool_op_entry`, `bg_op_entry`) so production
and tests build the slot's contents identically, then added a test-only
`set_current_op_and_read(entry) -> String` helper that writes the entry and
reads the op name back **under a single `CURRENT_OP` lock acquisition**. Both
global-state tests (`note_background_op_prefixes_and_is_observable` and its
sibling `note_tool_then_current_op_returns_name`, which had the identical
latent race) now call it, so a concurrent `note_tool` from another test's
dispatch can no longer land between the write and the read. The public API
(`note_tool`, `note_background_op`, `current_op`) is unchanged.
## Tests added

No new test — the two existing global-state tests were rewritten to use the
race-free `set_current_op_and_read` helper (which itself is the test seam). The
meaningful assertions are preserved: a foreground tool is stored verbatim, a
background op is stored with the `bg:` prefix. `cargo test --lib heartbeat::`
→ 9/9 pass; the whole class of two tests is now deterministic.
## Workarounds
Re-run the job; or run the heartbeat tests single-threaded.

## Resume

Done — fixed in `src/heartbeat.rs` (this file's fix commit). No follow-up
needed; the process-global `CURRENT_OP` remains last-writer-wins by design for
production, and the tests no longer assume no concurrent writer exists.
## References
- src/heartbeat.rs:66-71, :295-302; src/server.rs:626-628
- docs/issues/2026-07-02-guide-hint-artifact-not-registered-ci-flake.md (sibling
  race, same 3-run evidence base)
