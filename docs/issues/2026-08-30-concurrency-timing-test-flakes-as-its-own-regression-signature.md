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

# `dense_and_sparse_legs_run_concurrently` flakes under load, and wall clock can no longer separate concurrent from sequential

`src/retrieval/embedder.rs:1926` asserts the dense and sparse legs overlap by measuring
wall clock: two 500 ms mock delays must complete under an 800 ms ceiling. Alone it
passes every time. Inside `cargo test --lib` — the configuration this project's gate and
CI both use — elapsed climbs to **847–903 ms** and it fails.

**The sharp part is not that the margin is tight.** Loaded-concurrent has reached
sequential-at-rest (~1000 ms), so **no ceiling both passes a healthy loaded run and
fails a genuine sequential regression.** Wall clock has stopped being a discriminator
here, rather than merely becoming a noisy one.

## Measurements

| condition | result |
|---|---|
| loaded (`cargo test --lib` / `--workspace`) | **3 failures in 5 runs** — 903.3 ms, 860.2 ms, 847.5 ms |
| isolated (single-test filter) | **0 failures in 5 runs**, 0.69–0.79 s wall |

```
panicked at src/retrieval/embedder.rs:1958:9:
legs should run concurrently (~max(500,500)=500ms), took 903.30299ms
  — a sequential-await regression would take ~1000ms
```

Each reds an otherwise-clean suite: `4712 passed; 1 failed`.

## It is not a regression

- The test predates today — introduced `ee8794ce`, 2026-07-27.
- No Rust changed between a failing run and the previous green gate
  (`git diff --stat 5dfa5051..HEAD -- '*.rs'` empty).
- It passes 5/5 **in isolation** at HEAD. This is the discriminator, and it is positive
  evidence rather than absence of failure: a genuine sequential-await regression costs
  ~1000 ms *regardless of load*, so it would fail in isolation too.

## Mechanism — inferred, NOT measured

Both mocks sleep with `std::thread::sleep(500ms)` inside `with_chunked_body`, blocking
the responding thread rather than yielding. The plausible reading is that under
full-suite parallelism the two mockito response threads are not scheduled concurrently,
serializing part of the two sleeps; a ~350–400 ms shift is most of one full sleep, which
fits partial serialization better than scheduler jitter.

**This is a hypothesis about a library's threading and has not been confirmed.**
Confirming it needs instrumentation of request arrival times inside mockito. The
recommended fix does not depend on which reading is right.

## Fix — observe overlap, do not infer it from a sum

Not applied. Replace the wall-clock inference with a direct observation, which is
load-independent: have each mock handler signal its arrival and wait for the other's
before responding — a two-party rendezvous (`Arc<Barrier>` with 2 parties, cloned into
each `with_chunked_body` closure) with a generous timeout. Drop both `sleep` calls and
both `elapsed` assertions.

If the legs truly overlap, both requests are in flight, both handlers meet, and the test
completes fast at any load. If a regression makes them sequential, the first handler
waits for a second request that cannot arrive until it returns — a deadlock the timeout
converts into a deterministic, correctly-attributed failure.

Strictly stronger than the timing assertion: it fails on sequential awaits on *any*
machine at *any* load, and cannot fail on a healthy one. It also removes ~1 s of sleep.
The "the mocks actually ran" intent of the lower-bound assertion is already carried by
the existing `assert_async()` calls.

Acceptance check is a mutation: replace `try_join!` with two sequential `.await`s,
confirm the rewritten test fails deterministically, restore. **Announce that mutation
window before opening it** (`reconnaissance-patterns:R-129`) — this checkout is shared.

## Rejected alternatives

- **Raise the ceiling.** Unavailable, not merely worse: above 903 ms leaves under 100 ms
  separating a healthy loaded run from a real regression, and the loaded distribution's
  upper tail is unmeasured. It would keep the test green while quietly ending its
  ability to catch the regression it exists for.
- **Retry once on a timing miss.** Cheap; weakens the guard.
- **Compare against a deliberately-sequential helper.** Self-calibrating, but doubles
  runtime and still measures time.

## A numeric coincidence that will mislead the next reader

The test's doc comment records an **earlier** 300/600 ms configuration measuring
"903.8-905.7ms against a 900ms ceiling". The first failure here was **903.3 ms**. These
are not the same phenomenon: the tree is 500/500 against an 800 ms ceiling, and 903 ms
is the *concurrent* case inflated by load, not a sequential case squeaking under an old
ceiling.

## Provenance, and a duplicate worth recording

Filed twice on 2026-08-30, **21 seconds apart**, by two sessions hitting the same
failure in their final gates. The measurements, the ranges-have-met analysis, the
rendezvous fix and the 903 ms coincidence are the other session's work, folded in here.

Neither session could have complied with `CLAUDE.md`'s *don't re-file a filed bug as
new*: the other file did not exist when each began writing, and an untracked file is
invisible to `artifact(find)` until a reindex.

**What happened next is the part worth keeping.** Each session then read the other's
file, judged it the better record, and deferred — they deleted theirs, and I reduced
mine to a stub pointing at theirs. A **mutual-deference race**: both moves were correct
in isolation, and together they destroyed the fuller analysis and left a citation to a
path that no longer existed. This file is the reconstruction. Sibling in structure to
`reconnaissance-patterns:R-129` — nothing crossed between sessions, the harm was
entirely in each reading the other's state and acting on it — but the failure mode is
*deletion* rather than misinterpretation, and no citation sweep could catch it: at the
moment each commit landed, the cited file still existed.

## References

- `src/retrieval/embedder.rs:1926-1971` — the test and its design comment.
- `docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md` — same class
  (wall-clock assumptions gating the suite), currently `zombie` and scoped to Windows
  CI. This one fails on Linux locally, so it is a separate instance.
- `reconnaissance-patterns:R-129` — in a shared checkout a peer seeing this failure has
  no way to tell it from a real regression. This bug is a standing generator of exactly
  that false report, on the project's own gate command.

