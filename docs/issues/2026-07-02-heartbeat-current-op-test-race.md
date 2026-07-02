---
status: open
opened: 2026-07-02
closed:
severity: medium
opened-by: claude
owner: marius
related: [docs/issues/2026-07-02-guide-hint-artifact-not-registered-ci-flake.md]
tags: [flaky-test, heartbeat, test-env-isolation]
kind: bug
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
Not started. Candidates: (a) make the assertion tolerant — set-then-read under
one lock acquisition (add a test-only `note_and_get` that holds the lock across
both), (b) retry-loop the assertion, (c) redesign CURRENT_OP as a keyed/stacked
record rather than last-writer-wins. (a) is smallest and honest to the API.

## Tests added
N/A — this IS a test bug.

## Workarounds
Re-run the job; or run the heartbeat tests single-threaded.

## Resume
Implement (a): add `#[cfg(test)] fn note_background_op_and_get(label) -> String`
in src/heartbeat.rs holding the `CURRENT_OP` lock across write+read; switch the
test to it; keep the public API untouched. One-line clippy/fmt/test gate.

## References
- src/heartbeat.rs:66-71, :295-302; src/server.rs:626-628
- docs/issues/2026-07-02-guide-hint-artifact-not-registered-ci-flake.md (sibling
  race, same 3-run evidence base)
