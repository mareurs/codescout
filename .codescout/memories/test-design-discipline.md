# Test-Design Discipline (review lenses)

Craft-shaped lessons from entry-graph Stage 2 (2026-07-17), where every real defect was an
untested-seam DISCOVERY problem a green suite didn't reveal. Terse codescout-local echo;
full doctrine lives in the `testing-snow-leopard` buddy. Apply as standing review lenses.

## Assert on the cause, not error-presence (discriminating tests)

A test asserting only "an error occurred" (`err.downcast_ref::<X>().is_some()`, `is_err()`)
is NON-discriminating when >1 code path raises the same error type — deleting the code under
test can leave the test green. Stage-2 example: a worktree-cites guard test passed even with
the guard deleted (the error then came from cite-resolution failure). Assert on the specific
cause (message substring / error variant / field), and ask: "would this test still pass if
the code it targets were deleted or inverted?"

## One test per branch; both sides of every condition

Each new `if` / `match`-arm / `Some`-vs-`None` branch needs a test that REACHES that branch.
Stage-2: `resolve_cite_ref` shipped with 2 of 3 resolution branches untested; a read path
gated on slug-present had its slug-None side unexercised. Function-level "is it called by a
test" is NOT enough — a well-tested function can hide a dead branch (coverage tools mark the
function covered; only branch coverage or mutation testing sees the gap).

### Corollary: a minimal fixture never reaches a cap, truncation, or ordering branch

When a change adds a **cap, truncation, aggregation, or ordering**, the fixture must EXCEED
the cap. A one-element fixture proves only the empty and single-element cases, and unit
fixtures are built to be minimal — which is exactly what hides this class.

2026-08-07 (F-12, `docs/trackers/release-promotion-session-log.md`): `grep`'s new
`completeness_warning` shipped with seven tests, clippy clean, 3522 green, and CI 15/15 on
attempt 1 — and its output was useless. Every fixture created exactly ONE hidden entry, so
`if more > 0` was never executed. The real repo has 16, alphabetical ordering put `.github/`
twelfth, and the cap of 5 cut the single entry the feature existed to surface. The
`both sides of every condition` lens above catches it; it was applied to the `Option` two
lines up (three tests pin the `None` side) and not to the truncation two lines down. Having
the lens is not the same as sweeping every new branch with it.

Pairs with W-12: for any change to tool-facing OUTPUT (warnings, hints, summaries, rendered
text), call it once against the real repository and read the bytes. That is a step distinct
from the gate and from `cargo rb` + reconnect — the latter only establishes that the new code
is running, not that what it says is useful.

## Round-trip completeness (writer shape ↔ reader surfacing)

For any writer/reader pair, every distinct shape the WRITER can emit must be reachable and
correctly surfaced by the READER — test the writer's whole shape-space, not just its
happy-path output. Stage-2: the writer produced id-keyed `dst_ref` for non-tracker targets,
but the reader gated the whole block on slug-present, so id-keyed backlinks were invisible;
both tests shared the incidental precondition "target has a slug," masking it. Watch for
shared incidental preconditions between writer and reader tests.

## The mechanical backstop

These three are semantic (a green suite hides them), so the durable catch is
`cargo mutants --in-diff <range>` scoped to the diff at the pre-ship boundary — the only
mechanism that flags a test reaching code without discriminating it. See `docs/RELEASE.md`
Standard Ship Sequence.
