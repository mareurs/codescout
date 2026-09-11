---
id: f4bebcd6e149f152
kind: bug
status: open
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

Split the workflow scan: collect features named on a `cargo test` line separately from features
named on any line, and require a non-exempt feature's coverage to come from the **test** set. Keep
the any-line set for `every_exempt_feature_is_still_compiled_somewhere`, which is asking the
compile question and is correct as written.

`local-embed-dynamic` then needs an `EXEMPT` entry, and **the reason must be the narrow one**. It
is *not* "it has no unique test surface of its own" — that reads as "there is nothing there to
test" and would stop the next person looking. What is true is narrower: no test is gated
*exclusively* on it, so a `cargo test --features local-embed-dynamic` lane would run exactly the
assertions the `local-embed` lane already runs. The dynamic backend is a genuinely different code
path — `dlopen` at runtime against statically linked — and **that path stays uncovered either way**.
The exemption buys nothing about the loader; it only records that a test lane would not buy anything
either.

Add a vacuity guard beside it, in the shape `the_guard_is_not_vacuous` already uses in this file:
assert the test-line set is non-empty, or a scan that silently stops matching `cargo test` marks
every feature uncovered and the gate inverts into noise.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. The acceptance criterion is an **observed RED** from the § Reproduction shape — a feature
whose only lane is `cargo check` must fail the strengthened gate — with a control showing the same
feature passes once its lane becomes `cargo test`.

## Resume

Run § Reproduction first. It is four steps and it distinguishes the two things this file claims:
that the gate is green in that state, and that the test genuinely never runs. Those are separate
facts and only the second is the damage.

Then implement the split. Do not start by adding `local-embed-dynamic` to `EXEMPT` — write the
strengthened check first and let it tell you which features it rejects, so the exemption list is a
measurement rather than a prediction.

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
