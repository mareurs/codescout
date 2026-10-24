---
kind: bug
status: fixed
title: 'two guide-hint tests fail intermittently under build load: a_subagent_moving_the_default_is_reported_to_the_parent_via_the_stamped_principal and a_subagent_switching_projects_by_id_is_reported_to_the_parent'
tags:
- cluster/gate-keyed-on-unobservable-event
- flaky-test
- guide-hints
closed: 2026-10-24
opened: 2026-10-17
owner: marius
related: []
severity: low
unverified: A natural failure was never reproduced; the mechanism was reproduced by forcing a 6 s sleep between the two activations. Nothing pins the upper side of the production 5 s constant or the `<` versus `<=` boundary.
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

Fixed in `src/agent/mod.rs`, `src/server.rs` (test seam) and the two tests (a test flaw; production behaviour unchanged). `Agent::note_activation` compares real elapsed time between two activations with the 5 s `CONCURRENT_ACTIVATION_WINDOW`, so two real activations with a slow build in between fell outside the window. A `#[cfg(test)]`-only `activation_window_override` field and `set_activation_window_for_test()` let the two tests use a 24 h window. The sibling `agent::tests::note_activation_tells_a_parent_from_its_subagent` is deliberately left without the override so a zero window is still caught.

- Fix SHA (on `experiments`; merged 2026-10-24 by fast-forward, tip `e0255583b`): `982a9403e0f286532182db199716e8b18f880397`
- patch-id: `b9806d894021619d934135c41e637e868d691738`

## Fix provenance

- **SHA:** `982a9403e0f286532182db199716e8b18f880397` (`experiments`)
- **patch-id:** `b9806d894021619d934135c41e637e868d691738`

## Tests added

No new test; the two named tests now set the override. With a forced 6 s sleep between the two activations both failed before the fix and pass after. Window expiry is still covered by the pure `concurrent_switch_warning` tests (they pin 60 s against a 5 s window; neither the `<` boundary nor the production constant is pinned). Gate on the merged tree: exit 0.

## Workarounds

Re-run the group; the tests pass in isolation.

## Resume

First failure: keep the assertion message. Then run each test with a `sleep(6)` between its two
activations to confirm or refute the window.

## References

- `src/agent/mod.rs` (`CONCURRENT_ACTIVATION_WINDOW`, `concurrent_switch_warning`)
- `src/server.rs` `guide_hint_tests` (the two tests)
