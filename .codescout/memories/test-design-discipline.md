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
