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

# `dense_and_sparse_legs_run_concurrently` flakes under load — superseded record

> **SUPERSEDED by `docs/issues/2026-08-30-concurrency-timing-test-flakes-under-full-suite-load.md`
> (id `00d04f1dd6689bb1`), which is the fuller and more accurate record.** Filed by a
> concurrent session **21 seconds** before this one, for the same defect — neither of
> us re-filed knowingly. Kept as a stub rather than deleted only because that file was
> still uncommitted when this was written; delete this once it lands.

## What this file got wrong, kept because both errors are instructive

**1. It stated the mechanism as fact.** This file asserted that the mocks'
`std::thread::sleep(500)` calls serialize under load. That is a *hypothesis about
mockito's threading*, and confirming it would need instrumentation of request arrival
times. The superseding file labels the identical reading **"inferred, not measured"**
and notes that its fix does not depend on which reading is right — which is the
correct treatment, and the reason its fix is the better one.

**2. It offered raising the ceiling as merely the *worse* direction.** It is not a
trade-off; it is unavailable. Measured there: loaded-concurrent runs at **847–903 ms**
while sequential-at-rest is ~1000 ms. **The two ranges have met.** No ceiling both
passes a healthy loaded run and fails a genuine sequential regression — wall clock has
stopped being a discriminator here rather than merely becoming noisy. That is a
stronger and different claim than "the margin is tight", and this file missed it.

## The durable facts, so nothing is lost if the successor is abandoned

- **Rate:** 3 failures in 5 loaded runs, 0 in 5 isolated. Failure values 903.3 ms,
  860.2 ms and 847.5 ms against an 800 ms ceiling.
- **Not a regression.** The test predates today (`ee8794ce`, 2026-07-27) and passes
  5/5 in isolation at HEAD — which a genuine sequential regression could not do, since
  that costs ~1000 ms regardless of load. Isolation is the discriminator.
- **The fix** is to observe overlap directly instead of inferring it from a sum: a
  two-party rendezvous in the mock handlers, so a sequential regression deadlocks into
  a deterministic timeout on any machine at any load. Strictly stronger than the timing
  assertion, and ~1 s faster.
- **It is a standing generator of false peer reports.** On the project's own gate
  command this failure is indistinguishable from the defect it guards —
  `reconnaissance-patterns:R-129`.

## Provenance

Both files were written 2026-08-30 within half a minute of each other, from the same
failure in two sessions' final gates. Recorded because the duplicate is itself an
instance of the shared-checkout class: `CLAUDE.md` says *don't re-file a filed bug as
new*, and neither session could have complied — the other file did not exist when each
began writing, and an untracked file is invisible to `artifact(find)` until a reindex.

