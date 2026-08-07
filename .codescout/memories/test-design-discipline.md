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

### Corollary: an invariant test proves the SHAPE, never the ASSIGNMENT

When a bug is "these numbers don't add up", the obvious test is the arithmetic — and it is
the one that cannot catch the bug's twin. An identity that holds for *any* assignment
verifies the shape and says nothing about whether each item is in the right place.

2026-08-07, measured rather than reasoned. `audit_doc_refs`'s three summary counters covered
7 of 10 `Verdict` variants, so 2,426 of 47,094 refs were counted in `n_refs_found` and in no
bucket. The fix routed every verdict through one exhaustive `match`, and got two tests:
`summary_counters_partition_every_verdict` (one finding per verdict; buckets must sum to
found) and `resolved_basename_counts_as_resolved_everywhere` (that verdict specifically must
land in `resolved`). Mutating `bucket()` to put `ResolvedBasename` in `broken` **killed the
second and left the first green** — because summing correctly is true under every possible
assignment. Writing only the obvious test would have let the mutation live.

So: for any partition, grouping, routing table, or dispatch map, write **two** tests — one
for the invariant, one for at least one specific member's placement, chosen as the member
whose misplacement would be most consequential. And prefer an **exhaustive `match` with no
wildcard arm** over any test: adding a variant then fails to compile until it is handled. A
test asserts a partition; an exhaustive match *is* one.

### Corollary: a first measurement is a warm-up artifact until a second one agrees

Not a test-design rule strictly, but it fails the same way — a number that looks legitimate
because it has units and is internally consistent. Three instances on 2026-08-07 (W-14):
sparse embed read 146.8 ms cold and **16.8 ms** warm (8.7×, and it fed a shipping decision);
the `audit_doc_refs` tally migrated by up to 69 refs cold and was byte-identical warm; and
`resolve_file_symbol` returned `SymbolMissing` for symbols that exist when the server answered
before finishing indexing — the same trap promoted from a latency error into a false claim
about the code. In a benchmark, discard the first iteration and **say in the write-up that you
did**; a mean over a run that started cold folds a one-off into every per-item number.

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
