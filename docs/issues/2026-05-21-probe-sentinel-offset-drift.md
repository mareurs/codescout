---
status: fixed
opened: 2026-05-21
closed: 2026-05-21
severity: low
owner: marius
related: []
tags: [tests, probe, fixture-drift]
kind: bug
---

# BUG: probe::sentinels_at_expected_offsets fails — first sentinel at offset 319, not 200

## Summary
`PROBE_DESCRIPTION`'s intro paragraph grew past 200 bytes, so the first
sentinel marker `SENTINEL_0200_AA` lands at offset 319 instead of the
expected ~200. The padding loop can only push *forward*, never back —
once `s.len()` already exceeds the target offset, the marker is stamped
at the current cursor and the assertion fails. Pre-existing on
`experiments`; not caused by the auto-inline patch.

## Symptom (Effect)

```
---- tools::probe::tests::sentinels_at_expected_offsets stdout ----
thread 'tools::probe::tests::sentinels_at_expected_offsets'
panicked at src/tools/probe.rs:119:13:
marker SENTINEL_0200_AA at offset 319, expected near 200
```

Tolerance in the assertion is 20 bytes; observed drift is 119 bytes.

## Reproduction
```
git rev-parse HEAD        # ae5c107c experiments
cargo test --lib sentinels_at_expected_offsets
```

## Environment
codescout @ `experiments`, default features, any host.

## Root cause
`src/tools/probe.rs:18-55` — `build_probe_description()` pushes a static
intro string before the sentinel loop:

```rust
s.push_str(
    "PROBE_BEGIN: this is a diagnostic tool used to measure how much \
     of an MCP tool description reaches the model. ...",
);
```

The current intro is 319 bytes. The sentinel loop then runs:

```rust
for (offset, marker) in sentinels {
    while s.len() + marker.len() + 4 < *offset {
        s.push_str("filler ");
    }
    s.push_str(marker);
    s.push(' ');
}
```

For the first sentinel `(200, "SENTINEL_0200_AA")`: the `while` guard is
already false (319 ≥ 200), so no filler is pushed and the marker lands at
byte 319. Subsequent sentinels (500, 1000, ...) re-establish position
because their targets are still ahead of the cursor — only the first
sentinel is broken.

Mechanism: forward-only padding can't compensate for an intro that
overflows the first target offset. Someone edited the intro text without
updating either the intro length budget or the first sentinel's target
offset, and the test caught the drift but the test never got fixed.

## Evidence
### Cursor math at first sentinel
- Intro length: 319 bytes (measured by the test failure message).
- First sentinel target: 200 bytes.
- Guard `319 + 16 + 4 < 200` is false → marker stamped immediately.
- Observed: marker at offset 319.

### Forward-only padding
`src/tools/probe.rs:36-40` — `push_str("filler ")` in a `while` loop;
no shrink/rewrite path exists.

## Hypotheses tried
1. **Hypothesis:** test off-by-one in tolerance.
   **Test:** read `pos.abs_diff(target) < 20` — that's 20 bytes.
   Observed drift is 119 bytes, well outside any reasonable tolerance.
   **Verdict:** rejected.

## Fix

**Applied 2026-05-21 (option A):** trimmed the intro paragraph in
`src/tools/probe.rs:35-39` from 319 bytes to 156 bytes. New intro
preserves the diagnostic intent (instruct the model to recite
SENTINEL_NNNN_XX markers it sees) but drops the verbose framing.

Mechanics: with intro length 156 and forward-padding loop incrementing
by 7 bytes per `"filler "`, `s.len()` lands at 184 just before the
first sentinel is appended (target 200, observed 184, abs_diff 16 — well
under the 20-byte tolerance).

Commit SHA: TBD.
## Tests added

N/A — the failing test (`tools::probe::tests::sentinels_at_expected_offsets`)
*is* the regression test. It detects intro-paragraph drift past the first
sentinel's tolerance window. Future edits to the intro that exceed ~170
bytes will trip this same assertion.
## Workarounds
`cargo test --lib --skip sentinels_at_expected_offsets` to bypass while
running the real suite.

## Resume

N/A — fixed.
## References
- `src/tools/probe.rs:18-55` — `build_probe_description`
- `src/tools/probe.rs:108-124` — failing test
