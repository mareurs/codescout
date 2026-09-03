---
id: '7f023c2ec0ae7856'
kind: bug
status: open
title: 'BUG: a held project write lock names no owner, no progress and no expected duration, so a refused party cannot tell a 12-minute reindex from a leak'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
opened: 2026-09-03
severity: medium
---

# BUG: a held project write lock names no owner, no progress and no expected duration, so a refused party cannot tell a 12-minute reindex from a leak

## Summary

`src/agent/write_guard.rs` serialises every write-tool call behind one flock per project. A
`librarian(reindex, reembed=true)` holds it for its whole duration — **measured ~12m10s** — and
the refusal a blocked session receives is:

```
another codescout instance is writing to this project
  hint: Retry in a moment — the holder should release shortly.
```

That message names no holder, offers no progress, and asserts a duration ("shortly") that was
wrong by two orders of magnitude. A refused party has no supported way to distinguish a
healthy long operation from a leaked guard, and the hint tells them to do the one thing that
will not work.

**The holder cannot see the problem at all** — nothing degrades on their side, no error is
raised, and the only observer is somebody else being refused.

## Symptom (Effect)

Six consecutive `edit_file` refusals over ~8 minutes, identical text each time. Investigating
by hand:

```
/proc/locks   62: FLOCK ADVISORY WRITE 507902 00:1c:4632062 0 EOF
holder        pid 507902 (codescout), parent 985365 = a peer session socket
continuity    sampled 6× over ~6s — same holder every sample
activity      etime 06:46, CPU 00:00:17, state SNl+
```

Which led to the **wrong** conclusion — see *Hypotheses tried*.

## Reproduction

1. Session A: `librarian(action="reindex", reembed=true)` on a project with a large corpus.
2. Session B: any write-classified tool call on the same project.
3. B is refused, repeatedly, for the whole duration of A's run, with a hint telling it to retry
   shortly.

`git rev-parse HEAD` at the time → `596a8d7a`, `experiments`.

## Environment

Linux, six live codescout sessions in one checkout, embedder at `127.0.0.1:48081`, Qdrant
per-project collections.

## Root cause

Not a defect in the guard's *scope* — one write serializer per project is correct for a single
write, and narrowing it is the wrong fix. `src/server.rs:674`'s
`acquire_write_guard_if_writing(name, input)` keys on the **tool name**, never the target path,
and takes the active project's `.codescout/write.lock`; that is deliberate.

**The gap is disclosure.** A reindex is not a single write, and nothing says so:

- no announcement at the start that this call will hold the lock for minutes,
- no progress surface a refused party can consult,
- no way to ask *what* holds the lock or *how far along* it is,
- and a hint that positively asserts the opposite ("shortly").

measured 2026-09-03 by the holding session, on an inode watch of `.codescout/write.lock`:
hold ~12m10s (±30s — only the 4m21s tail was instrument-measured; the start is from the
holder's own invocation timestamp), 27,762 vectors written, ~38/sec sustained, 2 failures.

## Evidence

### The discriminator a refused party would reach for does not discriminate

**This is the half most worth reading.** Low CPU + sleeping + holding an exclusive lock is
**not weak evidence of a leak — it is zero evidence either way.** An I/O-bound embedding loop
sleeps on socket reads for essentially its whole life, so `17s CPU / 6:46 elapsed / SNl+` is
the *expected* reading in a healthy world and in a leaked one alike. The observation cannot
move the posterior at all.

Stated this strongly on the holder's own advice, and the distinction is load-bearing: "weak
evidence" invites someone to combine it with a second weak signal and feel justified; "no
evidence" does not.

### What does discriminate: probe the work, not the holder

The holder looks identical in both worlds; only the thing being written moves in one of them.

- Qdrant point count climbing — observed `9760 → 10518` in 12s during the run.
- `artifact_vec_v2` row count on a sqlite-vec deployment.
- `SELECT COUNT(*) FROM artifact_chunk` for an a-priori estimate: hold time scales roughly
  linearly with total corpus chunks at ~38 vectors/sec on this host.

### Sizing: use the mean, and a retraction

The holder first reported the chunk distribution as long-tailed (one artifact at 564 chunks)
and warned that per-artifact estimates mislead. **They then measured it and retracted:** across
all 1457 codescout artifacts, mean 19.6, **median 16**, max 565, top six only 6% of the total.
Broad-based, not skewed. So `artifacts × ~20` *is* a serviceable estimator, and the 564-chunk
example was a real outlier generalised from one observation.

The retraction is recorded rather than the original claim, and it is the third
non-discriminating inference between two sessions in one evening.

## Hypotheses tried

1. **Hypothesis** — the guard is leaked; the holder is idle. **Test** — `/proc/locks`
   continuity sampling, `ps -o etime,time,stat`. **Verdict** — **rejected, and the test was
   worthless.** See *Evidence*: the reading is identical in both worlds. Reported to the holder
   as a probable leak before asking; they corrected it with a live progress count.
2. **Hypothesis** — writes to paths outside the project are wrongly gated. **Test** — read
   `src/server.rs:674`; the guard keys on tool name, not path. **Verdict** — confirmed as
   designed, though the message's *"writing to this project"* is imprecise when the target is
   elsewhere.

## Fix

Not fixed. The remedy is an instrument that reports the hold to a party who can act:

- name the holder (sessionId) and the operation in the refusal, not just "another instance";
- emit progress for long-running write calls, or a starting estimate;
- soften or condition the "retry shortly" hint, which is a claim and was false here.

**Do not narrow the lock.** A file reading "the guard is too coarse" would send the next person
to fix the wrong thing; coarse is correct for a single write.

Possibly moot for most users: the holder's operator is reviewing whether chunk-grain should be
opt-in. If it lands default-off, a default reindex drops from ~27,762 vectors to ~1,457 — about
30 seconds rather than 12 minutes — and this becomes the non-default path.

## Tests added

None. A test here would assert about disclosure content, which does not exist yet.

## Workarounds

Native `Bash`/`Edit`/`Write` do not route through codescout's write guard, so a blocked session
can keep working on unrelated files. That is a bypass of a concurrency serializer and should be
a considered choice: it is defensible when the holder is provably making progress on a
different corpus, and not otherwise.

## Resume

Decide the disclosure surface. Cheapest useful version: put the holder's sessionId and the tool
name into the refusal, which `acquire` could record in the lock file itself before blocking.

## References

- `src/agent/write_guard.rs` (`acquire`, `open_lock_file`), `src/server.rs:674`.
- Measurements contributed by the holding session, sessionId `ffb95976-dc89-4cca-87aa-c026544faf2f`.
- Class note: filed `IC-17` on the remedy test, after both parties independently reached it and
  the holder withdrew a `blast-radius-exceeds-visibility` suggestion they had made from
  symptom-similarity. The remedy here is *an instrument that names the owner of a shared
  resource*, which is `shared-resource-carries-no-owner`; reasoning from the symptom
  (invisible to the holder, visible only to the refused) is the same move that makes
  topic-shaped cluster slugs bad.

