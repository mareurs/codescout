---
kind: bug
status: open
title: 'two guide-hint tests fail intermittently under build load: a_subagent_moving_the_default_is_reported_to_the_parent_via_the_stamped_principal and a_subagent_switching_projects_by_id_is_reported_to_the_parent'
tags:
- cluster/gate-keyed-on-unobservable-event
- flaky-test
- guide-hints
opened: 2026-10-17
closed:
owner: marius
severity: low
related: []
---

# BUG: two subagent-notice tests flake when the machine is loaded

## Summary

`server::guide_hint_tests::a_subagent_moving_the_default_is_reported_to_the_parent_via_the_stamped_principal`
and `...::a_subagent_switching_projects_by_id_is_reported_to_the_parent` each fail on some runs of a
large test group and pass in isolation. The suspected cause is `Agent::CONCURRENT_ACTIVATION_WINDOW`, a
5 s wall-clock window (`src/agent/mod.rs:1141`) inside which an activation of a different root counts
as concurrent contention and outside which it is silent. Not confirmed.

## Class fit

The behaviour under test is decided by elapsed wall-clock time between two calls, an event the test
cannot observe or control, so a loaded machine changes the verdict without any code changing.

## Symptom (Effect)

Reviewer measurement 2026-10-17: the 366-test `guide_hint_tests` group failed these two tests in 1 of 4
runs (the first, right after compiling); 6 of 6 runs of the two tests in isolation passed. I saw the
same two fail together in two mutation-probe runs (of about thirty, each on a freshly carried working
tree) on 2026-10-17, and pass in every other run of the same group, including the full gates.

## Reproduction

```
scripts/with-slot.sh cargo test --lib -- guide_hint_tests     # intermittent, mostly the first run after a compile
scripts/with-slot.sh cargo test --lib -- a_subagent_            # passes
```

Not reproduced on demand. Delaying the second activation past 5 s (a `sleep` between the two calls in
either test) should turn the expected notice into silence if the cause is the window; that check was
not done.

## Environment

codescout 0.15.0, Linux, branch `worktree-agent-ad70a7016cb7773af-remainder`, 2026-10-17.

## Root cause

Unconfirmed. `concurrent_switch_warning` (`src/agent/mod.rs`) returns a warning only when a different
root replaces the previous one within `CONCURRENT_ACTIVATION_WINDOW`; a test whose two activations are
separated by more than that under load would see no warning where it asserts one.

## Evidence

The intermittence pattern above, and the window constant. No stack trace or failure message was kept
from the failing runs, which is the first thing to capture on the next occurrence.

## Hypotheses tried

1. **Hypothesis:** the 5 s window is exceeded under load. **Test:** not run. **Verdict:** open.

## Fix

Not started. If the window is the cause: give the decision an injectable clock or window (the pure
function already takes `window` as a parameter, so a test can pass a generous one), rather than
reading `Instant` through the real server path.

## Tests added

None.

## Workarounds

Re-run the group; the tests pass in isolation.

## Resume

First failure: keep the assertion message. Then run each test with a `sleep(6)` between its two
activations to confirm or refute the window.

## References

- `src/agent/mod.rs` (`CONCURRENT_ACTIVATION_WINDOW`, `concurrent_switch_warning`)
- `src/server.rs` `guide_hint_tests` (the two tests)
