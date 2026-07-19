---
status: open
opened: 2026-07-19
closed:
severity: low
owner: marius
related: []
tags: [doc-drift, error-handling]
kind: bug
---

# BUG: `RecoverableError`'s `Display` doc comment claims it omits the hint; the code includes it

## Summary
The doc comment on `impl std::fmt::Display for RecoverableError` (the host
`RecoverableError` in `src/tools/core/types.rs`) asserts that `Display`
renders only `message` and intentionally omits `hint`/`guidance`. The actual
`fmt` body appends the guidance text (`" — {field_name}: {text}"`) whenever
`self.guidance` is `Some`. Doc and code disagree.

## Symptom (Effect)
Reading the doc comment leads a caller to believe `err.to_string()` will
never contain hint text, and that hint-bearing assertions require
downcasting + `.hint()`. In practice `err.to_string()` already contains the
hint. This was discovered while implementing Task 5 of the scoped-edit-miss
diagnostics plan: `scoped_edit_miss_surfaces_rich_diagnostic` and
`body_edits_visible_drift_on_augmented_nudges_params` both assert on
`err.to_string()` containing hint-only text ("re-read", "changed", "params")
and pass — which only works because `Display` DOES include the hint,
contradicting the doc comment.

## Reproduction
```
cargo test --lib scoped_edit_miss_surfaces_rich_diagnostic
```
passes today; inspect `src/tools/core/types.rs:286-301` — the doc comment
above `impl std::fmt::Display for RecoverableError` vs. the `fmt` body.

## Environment
codescout, branch `experiments`, commit at time of discovery: see `git log`
around commit `798119ad` (Task 5 of the scoped-edit-miss-diagnostics plan).

## Root cause
Doc comment (`src/tools/core/types.rs:286-292`) is stale relative to the
`fmt` implementation (`src/tools/core/types.rs:293-301`). The code:
```rust
impl std::fmt::Display for RecoverableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(g) = &self.guidance {
            write!(f, " — {}: {}", g.field_name(), g.text())?;
        }
        Ok(())
    }
}
```
appends guidance text, but the doc comment above it says "Display renders
only `message`. The structured `hint` ... are intentionally omitted here."
Unknown which is the "intended" behavior — possibly the comment described
an earlier version of `fmt` and the append was added later without updating
prose, or the append was added deliberately and the comment simply never
got revised.

## Evidence
### `src/tools/core/types.rs:286-301` (read via `symbols`/`read_file`, force=true)
```
/// Display renders only `message`. The structured `hint` and `recovery_steps`
/// are intentionally omitted here so existing `to_string().contains(...)` test
/// assertions stay stable. Production callers surface the full payload via
/// `route_tool_error` (see `src/tools/mod.rs`), which emits `hint`/steps as
/// dedicated JSON keys. If you need the hint programmatically, downcast to
/// `RecoverableError` and call `.hint()` — do not parse it out of Display.
impl std::fmt::Display for RecoverableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(g) = &self.guidance {
            write!(f, " — {}: {}", g.field_name(), g.text())?;
        }
        Ok(())
    }
}
```

## Hypotheses tried
1. **Hypothesis**: maybe `guidance` is always `None` in practice so the `if
   let` branch never fires, making the doc comment true in effect.
   **Test**: ran `scoped_edit_miss_surfaces_rich_diagnostic`, which builds a
   `RecoverableError::with_hint(...)` (guidance = `Some(Hint(..))`) and
   asserts `err.to_string()` contains hint-only text ("re-read"/"changed").
   **Verdict**: rejected — the test passes, proving `guidance` is appended
   for at least this real code path.
   **Evidence link**: see Evidence section above; test at
   `src/tools/markdown/tests.rs` (`scoped_edit_miss_surfaces_rich_diagnostic`).

## Fix
Not fixed. Two options, either is a one-line doc edit:
1. Update the doc comment to say `Display` DOES append guidance (matches
   current, tested behavior) — safest, since existing behavior is relied
   upon by the Task 5 tests.
2. Or strip the `if let Some(g) = ...` append from `fmt` to match the doc's
   claimed contract — riskier, would need to audit every
   `err.to_string().contains(...)` test across the codebase first (the doc
   comment itself claims such tests exist and rely on the omission, which
   the code contradicts, so a behavior change here is not zero-risk).
Recommend option 1 (fix the comment, not the code) since production
behavior is already depended upon.

## Tests added
None — this is a doc-only drift, not a behavior bug; no regression test
needed. The existing `scoped_edit_miss_surfaces_rich_diagnostic` and
`body_edits_visible_drift_on_augmented_nudges_params` tests already pin
down the actual (append-guidance) behavior going forward.

## Workarounds
None needed — current behavior (append guidance to Display) is what
callers should rely on; just don't trust the doc comment's claim that it's
omitted.

## Resume
Fixed at `916e8c3a` (branch `experiments`) — doc comment on `impl Display for
RecoverableError` (`src/tools/core/types.rs`) now describes the actual,
tested append-guidance behavior. No code change (option 1, as recommended).
Kept `open`->`fixed` in `docs/issues/`; archives once the fix ships to
`master`.
## References
- `src/tools/core/types.rs:227-301` (`RecoverableError` struct + `Display` impl)
- `src/tools/markdown/tests.rs` — `scoped_edit_miss_surfaces_rich_diagnostic`
- `src/librarian/tools/update.rs` (tests module) — `body_edits_visible_drift_on_augmented_nudges_params`
- Discovered during Task 5 of the scoped-edit-miss-diagnostics plan
  (`docs/superpowers/plans/2026-07-19-scoped-edit-miss-diagnostics.md`)
