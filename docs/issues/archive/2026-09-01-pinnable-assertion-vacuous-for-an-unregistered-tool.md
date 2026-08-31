---
id: '4b51e544df704545'
kind: bug
status: fixed
title: 'BUG: the `pinnable` assertion was vacuous for `get_usage_stats` — the name it guarded was never in the set it searched'
tags:
- tests
- vacuous-assertion
- tool-registry
- mcp-server
- cluster/assertion-that-cannot-fail
closed: 2026-09-01
opened: 2026-09-01
owner: marius
severity: low
unverified: 'No regression test for the vacuity itself. `tests/tool_reachability.rs` closes the enabling condition (an unregistered `impl Tool`) but not the shape — an assertion naming a string no tool produces is vacuous by the same mechanism with no unregistered type involved. The positive form (each listed name IS produced by a registered tool, then is absent from `pinnable`) is not built. The fix was also incidental: the subject was deleted, nothing diagnosed the vacuity.'
---

## Summary

`server_advertises_workspace_param_only_for_pinnable_tools` asserted that three tool names are
**not** pinnable:

```rust
let pinnable: HashSet<&str> = server.tools.iter().filter(|t| t.pinnable()).map(|t| t.name()).collect();
for n in ["workspace", "get_guide", "get_usage_stats"] {
    assert!(!pinnable.contains(n), "{n} must NOT be pinnable");
}
```

`pinnable` is derived from `server.tools` — **the registry**. `GetUsageStats` was never registered
(it implemented `Tool` and no agent could reach it), so `"get_usage_stats"` was never in that set
**regardless of what `pinnable()` returned**. Deleting its arm from `Tool::pinnable()` at
`src/tools/core/types.rs:754` would not have failed this test, or any other.

No input made it fail. Not rare, not environment-dependent — a permanent zero.

## Symptom (Effect)

Two mechanisms appeared guarded and were not:

1. `Tool::pinnable()`'s `"get_usage_stats"` match arm — itself unreachable, since `self.name()`
   is only ever evaluated for registered tools and none returned that string.
2. The assertion above, which is the only thing that referenced the arm.

A reader arriving at either sees a named test asserting the exact property, passing.

## Evidence

**The same loop line was LIVE for its two neighbours.** `"workspace"` and `"get_guide"` are both
registered — `Arc::new(Workspace)` and `Arc::new(crate::tools::guide::GetGuide::new())` — so
removing *their* `pinnable()` arms would have failed this assertion correctly. One line, two live
assertions and one permanent zero, sharing a `for` loop and a message. That is why it reads as
covered: the vacuity is not visible at the site, only in the relationship between the site and a
registry three files away.

**The monotone direction is the mechanism.** `!contains` is monotone under **removal**: a name
absent because it was correctly excluded and a name absent because nothing ever produced it are
the same observation. The assertion cannot distinguish them, so it certifies the first while
observing the second.

**Boundary against `IC-9`.** An `IC-9` assertion *can* fail — roughly 1-in-800 for the tempdir
case. This one cannot fail at any frequency, in any environment, given any amount of CI time.
Flake versus permanent zero, which is the distinction `IC-16` was opened to keep.

## How it was found

Not by reading the test. It surfaced 2026-09-01 while resolving whether `GetUsageStats` was
reachable — a question about the *tool*, which then required asking what the three references to
its name actually did. The reachability question is what made the vacuity visible; nothing about
the test itself invited a second look.

That matches `IC-16`'s blind-party claim exactly: *"the reviewer, structurally — a passing test is
the evidence they are given, and vacuity is invisible in exactly that evidence."*

## Fix

Removed in `0f28fc28` (patch-id `467338e4428601351a0801348f2f8419b853c33d`, `experiments`),
which deleted three unreachable tools and took both the `pinnable()` arm and the assertion's
third name with them.

**The fix was incidental and that is worth recording.** Nothing diagnosed the vacuity — the
subject was deleted and its references went along. Had the register-vs-delete decision gone the
other way, the arm would have become live and the assertion meaningful, again with nobody having
noticed it was not.

## Why the test suite could not catch this

`tests/tool_reachability.rs` (`0f28fc28`, hardened at `97adb14b`) now makes the *enabling
condition* impossible: no type can implement `Tool` and go unregistered without failing
`every_impl_tool_type_is_reachable` by name. That closes the route this instance took.

It does **not** close the shape. An assertion naming a string that no tool ever produces — a typo,
a renamed tool, a name retired from the enum — is vacuous by the same mechanism and reachable
without any unregistered type. A guard for that would have to assert the **positive**: each name
in the list is produced by some registered tool, *then* that it is absent from `pinnable`. Not
built.

