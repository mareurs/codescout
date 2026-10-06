---
id: c7f81780e767ee79
kind: bug
status: open
title: 'RESIDUAL: Emit progress or a starting estimate to parties refused by a held write lock, and cover the named-holder branch in tests/cross_process_write_lock.rs'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md
severity: low
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

## Resume

Test half is in on `experiments` (local, not pushed at the time of writing). Status stays `open` for the progress / ETA half. Marius must decide whether a refused caller should ever see progress (and through what channel: the holder record, a separate sidecar, or a poll), or whether the named holder plus duration-free wording is accepted as sufficient and this file closed as wontfix. Until then nothing is actionable here. If it is built, re-check the parent's caveat against HEAD first.

## References

- `docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md` — parent
