---
status: open
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: []
tags: [tests, flake, retrieval, shared-machine]
kind: bug
---

# `dense_and_sparse_legs_run_concurrently` flakes under load, and its flake reads as the exact regression it guards

`retrieval::embedder::tests::dense_and_sparse_legs_run_concurrently` asserts that
`embed_one_batch`'s two legs run under `tokio::try_join!` rather than sequentially. It
can only do that by wall clock — a sequential regression produces byte-identical
values — so it sleeps 500 ms in each mock and asserts the total is under 800 ms.

Observed 2026-08-30 during a full `cargo test` on a machine running several
concurrent sessions' cargo builds:

```
legs should run concurrently (~max(500,500)=500ms), took 1.029538517s
  — a sequential-await regression would take ~1000ms
```

**1.03 s is the sequential signature the test's own message names.** It is not a
value that reads as noise.

## It was not a regression

Established before concluding, because the failure text argues for the opposite:

- No Rust changed between the failing run and the previous full gate, which was
  green: `git diff --stat 5dfa5051..HEAD -- '*.rs'` is empty.
- Three isolated re-runs pass, each ~0.69 s wall for a test whose concurrent path is
  ~0.50 s. **A sequential-await regression would take ~1000 ms in isolation too** —
  it does not depend on load. Passing at ~500 ms is positive evidence that the legs
  are concurrent, not merely absence of evidence that they are not.

## Mechanism

The mocks block with `std::thread::sleep(500)` inside `with_chunked_body`, so each
occupies a real OS thread for half a second. On a saturated machine the two sleeps
can serialize, and the measured total lands at ~1000 ms with no code defect
anywhere. The margin available to absorb that is ~297 ms (503 ms measured against
the 800 ms ceiling).

The test's doc comment already documents a *previous* round of this: an earlier
version used 300/600 ms delays and measured only 4–6 ms of margin on the sequential
side. The delays were made equal to maximise the `sum/max` ratio, which is right and
was a real improvement. What it buys is headroom against a *faster* machine; it does
not buy headroom against a *busier* one, and this repo is routinely worked by three
or four sessions in one checkout.

## Why this is worth a file rather than a re-run

The failure is **indistinguishable from the defect it guards** without doing the
isolation check. A reader who sees `took 1.02s — a sequential-await regression would
take ~1000ms` has been handed a confident, specific, wrong conclusion by the test's
own message. That is the same shape as
`reconnaissance-patterns:R-129` (a genuine, reproducible failure whose obvious
reading is wrong), arriving here from load rather than from a peer's mutation.

## Fix options

Not obvious which is right; deliberately not chosen here.

1. **Measure the ratio, not the wall clock.** Run the same batch twice — once through
   `embed_one_batch`, once through a deliberately sequential helper — and assert the
   concurrent one is meaningfully faster. Self-calibrating against machine speed and
   load, at the cost of doubling the test's runtime.
2. **Retry once on a timing miss**, asserting only that *some* run lands under the
   ceiling. Cheap, and weakens the guard.
3. **Raise the ceiling toward ~900 ms.** Keeps the shape, shrinks the gap between
   pass and the ~1000 ms regression value to ~100 ms — trades a flake for a false
   negative, which is the worse direction for a guard.
4. **Assert on request timing rather than total elapsed** — record when each mock was
   entered and assert the two intervals overlap. Tests concurrency directly instead
   of inferring it from a sum, and is immune to load. Most work; most correct.

(4) looks right: overlap is the property the test actually means, and every other
option is a proxy for it that load can perturb.

## Provenance

Noticed 2026-08-30 in the final gate of the T6/T7 session. The suite was otherwise
green — clippy exit 0, lean lane 0 failures, 4712 of 4713 passing — and the isolated
re-run is what separated a flake from a regression.
