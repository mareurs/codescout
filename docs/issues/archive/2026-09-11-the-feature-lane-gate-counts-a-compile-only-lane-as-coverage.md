---
id: 2f250fc269280a82
kind: bug
status: fixed
title: 'BUG: the feature-lane gate counts a compile-only CI lane as coverage, so tests behind it are green by never running'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`every_declared_feature_has_a_lane_or_a_reason` (`tests/feature_lanes.rs`) accepts a feature as
covered when a CI workflow names it in **any** `--features` flag. It never inspects the verb, so
`cargo check --features X` and `cargo test --features X` are indistinguishable to it.

A reader takes "has a lane" to mean the feature is built **and** its tests run. What the gate
enforces is that it is built. The difference is invisible from the gate's name, its output, and its
green.

## Symptom (Effect)

A feature whose only lane is `cargo check` passes the gate while none of its tests ever execute —
so tests written against it are green by never running, which is indistinguishable from green by
passing.

Measured 2026-09-11: `dashboard` was in exactly that state. It is in neither `default` nor any test
lane, its only CI appearance was `cargo check --features dashboard --all-targets`, and
`every_declared_feature_has_a_lane_or_a_reason` was green throughout. The behavioural tests added
with `docs/issues/archive/2026-09-08-dashboard-memory-write-bypasses-the-shrink-guard-and-the-anchor-update.md`
would have compiled in CI and never run.

## Reproduction

1. Add a feature to `Cargo.toml` that is in no other lane.
2. Name it in a workflow under a `cargo check --features <name>` step only.
3. Add a deliberately failing `#[test]` gated on it.
4. `cargo test --test feature_lanes` — **green**. CI — green. The failing test never runs.

## Environment

`experiments`, 2026-09-11, after `dashboard` was moved to a `cargo test` lane. That move fixed the
one live instance; this file is the mechanism that let it exist.

## Root cause

`features_named_in_workflows()` (`tests/feature_lanes.rs`) scans each `.yml` for the literal
`--features` token and takes the next whitespace-delimited word. The surrounding command is never
read. It is deliberately narrower than a substring search — its own doc comment records that
`librarian` appears in a step *name* and a plain `contains` would count that — so the author was
alert to false coverage from the wrong direction, and this one still got through.

**The asymmetry is the precise gap.** `EXEMPT` means "cannot be RUN on a runner", and
`every_exempt_feature_is_still_compiled_somewhere` enforces that an exempt feature is at least
compiled — the run-vs-compile distinction, correctly drawn, in one direction. Nothing mirrors it:
no check requires a **non-exempt** feature's lane to actually run anything. So the file already
contains the concept it fails to apply.

## Evidence

- `tests/feature_lanes.rs` — `features_named_in_workflows()` and the `EXEMPT` /
  `every_exempt_feature_is_still_compiled_somewhere` pair.
- `.github/workflows/ci.yml` — the `feature-check` job, whose comment argued a `cargo check` was
  sufficient "because the question is whether they still compile, not whether their behaviour is
  covered". True while the features it named had no tests; false once `dashboard` gained some.

**Scope check, run rather than assumed.** `dashboard` was the only live instance. The other
check-only lanes are `e2e` and `retrieval-e2e` — both `EXEMPT` by design, documented, and
deliberately compile-only — and `local-embed-dynamic`, whose gated tests are all
`cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))` and therefore run in the
`local-embed` lane.

## Fix

The workflow scan is split in two. `features_from_text(text, only_cargo_test)` is the one
parser; `features_named_in_workflows()` keeps the any-line behaviour and
`features_tested_in_workflows()` restricts to lines that also run `cargo test`.

- `every_declared_feature_has_a_lane_or_a_reason` now takes its coverage from the **test**
  set. Its refusal says why a `cargo check` step is not enough, so a reader who adds one
  learns the distinction at the point they need it rather than from this file.
- `every_exempt_feature_is_still_compiled_somewhere` keeps the **any-line** set. It asks the
  compile question and was always correct; pointing it at the test set would have demanded
  that exempt features run, which is the opposite of what `EXEMPT` means.

**The parser takes text rather than reading the directory**, which is what makes it
fixture-drivable. A scan that silently stopped matching `cargo test` would mark every
feature uncovered — loud, and the gate catches it. A scan that stopped *distinguishing*
would mark every check-only lane tested — silent, and only a fixture the live corpus does
not contain can catch that.

**Per-line matching is a stated limitation.** A command split with a trailing `\` would put
`cargo test` and `--features` on different lines and read as untested. No workflow here does
that; if one starts, the scan under-reports and the gate refuses a lane that exists — a
false negative, which is the safe direction, and the refusal names what to do.

**`local-embed-dynamic` is now `EXEMPT`, and the reason is the narrow one** § Fix demanded.
Not *"it has no test surface of its own"*: nothing is gated **exclusively** on it, every test
it reaches is `cfg(any(local-embed, local-embed-dynamic))`, and the two are mutually
exclusive by `compile_error!`, so a test lane would re-run the `local-embed` lane's
assertions. The dlopen loader stays uncovered either way, and the entry says so — an
exemption that reads as *"there is nothing there"* stops the next reader looking.

Fix SHA: 8f620f85
Patch-id: 7c7d76c16b85be7a6fde3cc4c1b570058a1bc552
## Tests added

Two, and they answer different questions — which is why neither replaces the other.

- `the_workflow_scan_distinguishes_check_from_test` — fixture-driven. A `cargo check` line
  must not reach the tested set, a `cargo test` line must, comma lists included, and a step
  *name* mentioning a feature must reach neither.
- `the_verb_split_still_separates_the_live_lanes` — corpus-driven non-inertness. The tested
  set is non-empty and a strict subset of the named set, and at least one feature is still
  check-only. If every lane became `cargo test` the two sets would coincide and the
  strengthened gate would be indistinguishable from the old one, passing for a reason that
  has nothing to do with it working. The assertion says which line to delete if that ever
  becomes a real state of the world rather than a broken scan.

**Acceptance, observed rather than asserted.** Reverting the `dashboard` lane to
`cargo check --features dashboard --all-targets` reds
`every_declared_feature_has_a_lane_or_a_reason` with `["dashboard"]`; restoring it passes.

**The control is the part that shows the two questions are now separate rather than one
renamed:** under that same mutation `every_exempt_feature_is_still_compiled_somewhere`
stayed **green**, because a check lane still answers the compile question. Had it redded
too, the split would have collapsed both questions into one and the exempt features would
have been next.
## Resume

Fixed and archived. The one live instance (`dashboard`) was closed in `0cac0eaf` with the
bug that surfaced it; this file is the mechanism that let that state exist.

Nothing to resume. Two things a later reader should not redo:

- **The scope check is done.** `dashboard` was the only live instance; `e2e` and
  `retrieval-e2e` are `EXEMPT` and deliberately compile-only, and `local-embed-dynamic` is
  now `EXEMPT` for the reason in § Fix. Re-deriving it returns the same answer and reads as
  confirmation.
- **Do not point `every_exempt_feature_is_still_compiled_somewhere` at the test set.** It
  looks like the same tidy-up and is the opposite: `EXEMPT` means the tests *cannot* run, so
  demanding a test lane for them inverts the field's meaning. The asymmetry is the fix, not
  an omission in it.

The residual this does **not** close, named so it is not mistaken for closed: the dlopen
loader in `local-embed-dynamic` has no coverage, and now has an `EXEMPT` entry that could be
read as settling the question. The entry's own text says otherwise, which is the most a
lane-gate can do about it.
## References

- Found 2026-09-11 by sessionId `b80a27d4-9729-40ef-8c28-ad8982df6d13`, who verified all three
  reads independently — the scan's blindness to the verb, the `EXEMPT` asymmetry that names the
  concept without applying it in the other direction, and the scope check establishing `dashboard`
  as the only live instance. The asymmetry framing in § Root cause is theirs.
- The instance that surfaced it:
  `docs/issues/archive/2026-09-08-dashboard-memory-write-bypasses-the-shrink-guard-and-the-anchor-update.md`.
- `CLAUDE.md` § *Development Commands* — "It is `test`, not `check`: a `check` compiles the lean
  test targets and never runs them, so a lean-only *runtime* failure is invisible to it." The rule
  was already written down for the local gate; the CI gate did not enforce it.
