---
status: fixed
opened: 2026-05-02
closed: 2026-05-09
severity: low
owner: marius
related: []
tags: ["recoverable-error", "display", "test-footgun"]
kind: bug
---

# BUG: `RecoverableError` guidance / hint not included in `Display` / `to_string()`

## Summary

Calling `.to_string()` on a `RecoverableError` emitted only `self.message`; the attached `guidance` field (Hint / Warning / MustFollow) was serialized only in the MCP JSON response body, invisible to `Display`. Tests asserting `to_string().contains(hint_text)` failed because the hint text never reached `Display`. No runtime data loss — MCP JSON output was correct — but tests had to assert on the JSON response rather than the typed `Display`.

## Symptom (Effect)

```rust
let err = RecoverableError::with_hint("symbol not found", "did you mean 'Y'?");
assert!(err.to_string().contains("did you mean"));  // FAILS
```

## Reproduction

Construct any `RecoverableError` via `with_hint` / `with_warning` / `with_must_follow` and assert on `to_string()`.

## Environment

- Date: 2026-05-02
- Component: `src/tools/core/types.rs` — `impl std::fmt::Display for RecoverableError`

## Root cause

`Display` rendered only `self.message`. The `guidance` field (Hint / Warning / MustFollow) was serialized only in the MCP JSON response body via serde, not in the typed `Display` impl.

## Evidence

Direct: test assertion failure on `to_string().contains(...)` after `with_hint` was set. MCP wire output included the hint correctly (verified manually); only the `Display` path was lossy.

## Hypotheses tried

1. **Hypothesis:** Move the hint into `self.message` and keep `hint` only for static usage guidance. **Verdict:** Adopted as the initial workaround 2026-05-02; replaced by the proper fix 2026-05-09. **Evidence link:** see Fix section.

## Fix

**Initial workaround (2026-05-02):** Moved suggestions into the `message` string itself (`"symbol not found: X — did you mean 'Y'?"`), kept the `hint` for static usage guidance.

**Proper fix (2026-05-09):** `Display` now appends attached guidance as `" — <field_name>: <text>"` when `guidance` is `Some(_)`, surfacing hint / warning / must_follow content in `to_string()`. The MCP JSON output is unchanged (it uses serde, not `Display`) so no double-rendering. Audit confirmed no existing test asserts exact-equality on `to_string()` for `RecoverableError`; only the canary test `recoverable_error_display_shows_message` did, and it has been updated to assert the new contract.

## Tests added

- `display_includes_hint_text`
- `display_includes_warning_text`
- `display_includes_must_follow_text`
- `display_no_guidance_just_message`

All in `src/tools/core/tests.rs`.

## Workarounds

Pre-fix: cram hint text into the message string (initial 2026-05-02 workaround).

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-052** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
