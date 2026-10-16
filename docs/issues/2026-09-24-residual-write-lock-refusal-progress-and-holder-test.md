---
id: c7f81780e767ee79
kind: bug
status: wontfix
title: 'RESIDUAL: Emit progress or a starting estimate to parties refused by a held write lock, and cover the named-holder branch in tests/cross_process_write_lock.rs'
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md
severity: low
unverified: 'CLEARED 2026-10-16. Was: PARTIAL FIX (9f6cc269): the named-holder refusal is now tested across processes. NOT done: the progress/ETA half, which is a design decision. Answered: the design decision was made (wontfix, see ## Resolution) and the test half was re-verified by a mutation run on 2026-10-16.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Emit progress or a starting estimate to parties refused by a held write lock, and cover the named-holder branch in tests/cross_process_write_lock.rs.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md` (status `fixed`):

> No progress surface. This file's § Fix named three remedies; bullets 1 (name the holder) and 3 (stop asserting a false duration) shipped at d1b6146d, bullet 2 (emit progress, or a starting estimate, for long-running write calls) did NOT. A refused party can now identify and message the holder, which is the operational harm closed; they still cannot see how far along a 12-minute reindex is. Separately, the named-holder path has unit coverage only — tests/cross_process_write_lock.rs takes a raw flock from the test process, so it exercises the anonymous fallback branch, not the named one.

## Fix

Test half done 2026-10-06 (see `## Partial fix (2026-10-06)`); progress/ETA half not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## Partial fix (2026-10-06)

- **SHA:** `9f6cc269` (`experiments`)
- **patch-id:** `a2e20942bc28c9b2e02ab78fe5b7a7d18f7fa29d`

What is covered: the second half of the parent's caveat, the named-holder branch having no cross-process coverage. In `tests/cross_process_write_lock.rs` the spawn / handshake / edit body is now a shared helper `contended_edit_response(holder_record: Option<&str>)`. The new test `write_lock_contention_names_the_holder_recorded_in_the_sidecar` takes the flock, writes `.codescout/write.lock.holder` as `{now_ms}\tcodescout:sid-alpha reindex` after taking the lock (the order `write_guard::acquire` uses), spawns the binary, and asserts the refusal contains `write lock held by codescout:sid-alpha reindex` and does not contain "another codescout instance". The existing anonymous-branch test `write_lock_contention_produces_recoverable_error` gained the inverse assertion (no "write lock held by" when no record exists). The commit message records a mutation proof: making `read_holder_record` return `None` reds only the new test. That mutation result is quoted from the commit message; this bookkeeping pass did not re-run it.

Side change in the same commit: `lock_file.unlock()` became `FileExt::unlock(&lock_file)`, because clippy flagged `incompatible_msrv` (`File::unlock` is stable in 1.89, the crate MSRV is 1.88).

What is NOT covered: the progress / ETA half. A new channel from the lock holder to refused callers (a progress surface or a starting estimate) is not built and remains a design decision. Nothing in this commit touches `src/`.

## Resolution (2026-10-16)

**Status `wontfix` for the progress / ETA half; the test half stands as `fixed` at `9f6cc269`.**

Operator decision (relayed by `main` on 2026-10-16): option C. The named holder, the elapsed
hold time and the no-deadline wording (`write lock held by <sid> <tool>`, "Held for 12m10s...
Message the holder rather than retrying", shipped at `d1b6146d`) are judged sufficient. No
progress channel and no starting estimate will be built. The options that were on the table,
for the record: (A) an estimate in a third field of `.codescout/write.lock.holder` (reindex
only, host-dependent: ~38 vectors/s, mean 19.6 chunks per artifact); (B) a `write.lock.progress`
sidecar written by the holder, which needs a channel from `librarian` reindex into the guard
and a clear-before-unlock rule; (C) neither.

**Revisit trigger:** a refused party again waits on a 12-minute reindex and cannot tell it from
a leak despite the named holder. Re-open then and start from option A.

**The test half was re-verified, not just quoted.** On 2026-10-16 the mutation
`read_holder_record` returning `None` (`src/agent/write_guard.rs`, replacing
`Some((ts.parse().ok()?, holder.to_string()))` with `{ let _ = (ts, holder); None }`) was run
through `scripts/mutation-probe.sh` against `cargo test --test cross_process_write_lock`:
`KILLED (rc=101, 2 test(s) ran)`. The only failing test was
`write_lock_contention_names_the_holder_recorded_in_the_sidecar`; the anonymous-branch test
passed, so each branch's test fails for its own branch's regression and neither passes on a
constant. The test bytes were also read at HEAD: `contended_edit_response(holder_record)` is
shared, and the named test asserts both `write lock held by codescout:sid-alpha reindex` and the
absence of `another codescout instance`.

The parent's `unverified:` caveat (`TRACKED c7f81780e767ee79`) is cleared in the same commit, so
`doctor`'s `terminal_status_with_caveat` does not report the parent again for this file closing.

## Resume

N/A. Closed by operator decision, see `## Resolution (2026-10-16)`; the revisit trigger is recorded there.

## References

- `docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md` — parent
