---
id: c2023c57ceff2672
kind: bug
status: open
title: 'A cap-class: RESULT_CAP annotation on a non-cap-shaped const is silently ignored, and the header points a reader at exactly that no-op'
owners:
- marius
tags:
- cluster/declared-not-wired
opened: 2026-09-03
severity: medium
---

## Summary

The `result-cap-marker-gate` gate's `cap_constants()` parser applies `is_cap_shaped(name)` **before** looking for a `cap-class:` annotation above a `const` declaration. A `const` whose name fails the cap-shaped regex is skipped entirely — its annotation, if present, is never read, no `CapDecl` is produced, and no `ProbeRow` is demanded. The gate's own header documents a live instance of this (`LATEST_OBSERVATIONS`, a real result cap the census cannot see) and the obvious remedy a reader would try — annotating that constant with `cap-class: RESULT_CAP` — is a silent no-op.

## Symptom (Effect)

Given a `const` whose name is not cap-shaped (e.g. `LATEST_OBSERVATIONS: usize = 3`), adding `// cap-class: RESULT_CAP <some.id>` immediately above it does **not** cause the gate to demand a corresponding `ProbeRow`. The census total does not change, no test reds, and nothing signals that the annotation was ignored. The gate's claim "every `RESULT_CAP` has a row" silently narrows to "every `RESULT_CAP` on a cap-shaped const name has a row."

## Reproduction

1. Check out `result-cap-marker-gate` at `2a32c043` (or later, once merged).
2. Locate `src/librarian/preview/memory.rs:9` — `LATEST_OBSERVATIONS: usize = 3`, named as the live example in `tests/result_caps.rs`'s own header (~`:30-38`).
3. Add `// cap-class: RESULT_CAP preview.latest_observations` directly above it.
4. Run `cargo test --test result_caps`.
5. Observe: no test reds, the census count in `every_cap_constant_is_classified` is unchanged, and no `ProbeRow` is required for the new id.
6. Revert the annotation.

Not yet re-run by this filing as a live mutation; the mechanism is read from `tests/result_caps.rs`'s own header, which names the live instance and its floor property, and from `cap_constants()`'s gating order.

## Environment

`result-cap-marker-gate` branch/worktree, `tests/result_caps.rs`'s `cap_constants()` / `is_cap_shaped()`.

## Root cause

`cap_constants()` calls `is_cap_shaped(name)` as an early filter and only calls `annotation_above()` for names that pass. A cap-shaped name is (per the module's own framing) a **floor**, not a census — it is meant to catch obviously-cap-like names, not to gate whether an *explicit* annotation is honored. Because the check runs before the annotation is even read, an explicit `cap-class:` annotation cannot override a name that fails the shape check.

*Read from `tests/result_caps.rs` at `result-cap-marker-gate` HEAD `2a32c043`, and from the header's own documented example — not independently re-run as a live mutation by this filing (see Reproduction).*

## Evidence

`tests/result_caps.rs`'s header (~`:30-38`), quoted from the whole-branch review's finding I4:

> "Floor disclosed with a measured example: `LATEST_OBSERVATIONS: usize = 3` at `src/librarian/preview/memory.rs:9`" — named as a live result cap the census cannot see, immediately followed (per the review) by the same header's description of the "one line above one it does count" — the two constants sit adjacent, one visible to the parser and one not.

## Hypotheses tried

1. **Hypothesis:** an explicit `cap-class:` annotation is meant to be authoritative regardless of the const's name shape. **Test:** traced the call order in `cap_constants()` — `is_cap_shaped` gates before `annotation_above` runs at all. **Verdict:** confirmed the current implementation does not let an explicit annotation override the name-shape floor.

## Fix

Not designed. The straightforward fix is to reorder the check: look for a `cap-class:` annotation first, and only fall back to the name-shape heuristic when no explicit annotation is present. This would make an explicit annotation always authoritative, closing the silent no-op.

## Tests added

None yet. A fix should add a case asserting that a `cap-class: RESULT_CAP <id>` annotation above a non-cap-shaped const name IS picked up and DOES demand a `ProbeRow`.

## Workarounds

A contributor wanting to annotate a non-cap-shaped constant should first check whether `is_cap_shaped()` accepts the name, and rename the constant if not, rather than relying on the annotation alone.

## Resume

1. Once merged, reorder `cap_constants()` to check for an explicit annotation before applying the name-shape filter.
2. Add the regression test named above.
3. Consider whether `LATEST_OBSERVATIONS` itself should be annotated once the fix lands, since it is named as a live, currently-invisible result cap.

## References

- `tests/result_caps.rs` (`cap_constants`, `is_cap_shaped`, `annotation_above`) at `result-cap-marker-gate` branch, worktree `.worktrees/result-cap-marker-gate`
- `src/librarian/preview/memory.rs:9` (`LATEST_OBSERVATIONS`) — the documented live instance
- Surfaced during `result-cap-marker-gate` branch's whole-branch review (2026-09-03), session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, finding I4
- `docs/trackers/issue-clusters/IC-3-declared-not-wired.md`

