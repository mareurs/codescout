---
status: open
opened: 2026-09-08
closed:
severity: low
owner: marius
related: []
tags:
- cluster/hint-composed-without-the-request
kind: bug
title: The write-lock refusal cannot tell a CLEARED holder record from an ABSENT one, and names neither cause
topic: write-guard diagnostics
---

# BUG: a cleared holder record and an absent one produce one message, naming neither cause

## Summary

`acquire()` refuses a contended write lock with one of two messages. When `read_holder_record`
returns `None` it emits:

```
another codescout instance is writing to this project
hint: The holder recorded no identity — an older codescout, or an exit between taking
      the lock and writing the record. Check for a running reindex before retrying.
```

`None` has **three** producers, and they do not share a remedy. The commonest one is benign and
is not among the two the hint names.

## Symptom (Effect)

A session whose acquire lost a race against a holder that then released is told to look for a
stale binary or a crashed process, and sent to "check for a running reindex" — when the correct
action is to retry immediately, and there is no reindex to find.

Observed live 2026-09-08: a peer session hit this, reasonably read it as evidence that the
holder's binary predated `d2900ecb`, and was about to file a bug about a stale-binary defect that
did not exist. The holding session's binary was current — `strings` on it finds
`write.lock.holder` — and the lock had simply been released between the failed acquire and the
record read.

## Reproduction

Two sessions, one checkout. B attempts a write while A holds the lock, and A releases in the
window between B's flock failing and B reading the record. B gets the anonymous branch.

Cheaper, no race required: truncate `.codescout/write.lock.holder` to zero bytes while a holder
is live, then contend. Same message as deleting the file entirely.

## Root cause

Two facts, and the defect is their conjunction.

**1. Release TRUNCATES rather than deletes.** Verified at the bytes: the guard's own regression
test does `std::fs::read_to_string(&record).unwrap()` *after* release, so the file must still
exist for the test to pass. A cleared record is therefore an **empty file**, not an absent one —
and after any clean release that is the state on disk. Observed: both `.codescout/write.lock`
and `.codescout/write.lock.holder` at 0 bytes.

**2. `read_holder_record` collapses three states into one `Option`.**

```rust
fn read_holder_record(path: &Path) -> Option<(u128, String)> {
    let buf = std::fs::read_to_string(path).ok()?;   // absent / unreadable
    let (ts, holder) = buf.trim_end().split_once('\t')?;  // empty or malformed
    let holder = holder.trim();
    if holder.is_empty() {                                 // present but blank
        return None;
    }
    Some((ts.parse().ok()?, holder.to_string()))
}
```

Its doc comment is careful and correct about the **safety** property — *"Every `None` branch
means 'no usable owner information' and NEVER 'no holder'"* — and that reasoning is sound for
deciding not to claim the lock is free. It is not a reason to collapse the states for
**diagnosis**, and the two purposes were not separated.

The distinction is real and maps to opposite remedies:

| on disk | means | remedy |
|---|---|---|
| file exists, 0 bytes | a current codescout wrote the record and cleared it on release | **retry** — you lost a benign race |
| file absent | nothing current ever wrote it | older binary, or first use |
| non-empty, unparseable | a real corruption | investigate |

**The composer has the information.** `acquire()` holds `holder_path` and can call `.exists()`.
It composes the message from the callee's collapsed return rather than from the state available
at the call site — so the message is correct about `read_holder_record` and wrong about the
caller's situation.

## Evidence

`acquire()`'s refusal, both branches keyed off one `Option`:

```rust
return Err(match read_holder_record(&holder_path) {
    Some((since_ms, held_by)) => /* names the holder */,
    None => RecoverableError::with_hint(
        "another codescout instance is writing to this project",
        "The holder recorded no identity — an older codescout, or an exit \
         between taking the lock and writing the record. Check for a running \
         reindex before retrying.",
    ),
});
```

Neither named cause is the released-race, which is the state every clean release leaves behind.

## Why this is `IC-22` and not the unclassified escape hatch

`cluster/hint-composed-without-the-request`'s **seventh** member is the precedent and the fit is
close to exact: there, an error enumerated the four actions `plan_section_edit` implements while
the caller supported five, and it was *"composed from the callee's capability rather than the
request that reached it"*, with the complete list sitting thirty lines above. Here the hint is
composed from `read_holder_record`'s collapsed `None` while `holder_path` — the discriminator —
is a live local in the composing function.

The seventh member's remedy transfers too, and it is why this is not `IC-11`: correcting the
*string* would be wrong. `read_holder_record`'s contract is right for its own callers, and
widening it to report which branch fired would push diagnosis into a function whose whole point
is the safety collapse. The fix belongs at `acquire()`, where both the path and the situation are
known.

Considered and rejected: `cluster/unclassified`, where the sibling
`classify-conflates-two-malformed-reasons-under-one-message` sits. That one has no composer
holding the discriminator — the two defect shapes genuinely arrive at one code path. Here the
discriminator is in hand and unused, which is `IC-22`'s claim rather than a bare conflation.

## Fix

Not implemented. One `exists()` at the call site splits the `None` arm in two:

- **empty/cleared** → *"the holder released while you were acquiring — retry."*
- **absent** → keep today's text, which is then accurate.

## Tests added

None yet. The guard is cheap and needs no race: seed a temp root with a live flock and a
**truncated** holder file, contend, and assert the refusal does **not** say "an older codescout".
Then the same with the file **removed**, asserting it does.

Both assertions are needed and for different reasons. The truncated case is the regression that
actually happened. The absent case is the control — without it the pair is monotone under a
change that makes every refusal say "released, retry", which would be the same defect inverted.

Annotate the fixture: **the holder file must be truncated, not deleted**, and that detail is the
whole test. A tidy-up that deletes it instead leaves the assertion passing against the wrong
state.

## Workarounds

On seeing "recorded no identity", check `.codescout/write.lock.holder` directly. A zero-byte
file means the holder released — retry. Absent means investigate.

## References

- `src/agent/write_guard.rs` — `read_holder_record`, `acquire`, `holder_record_path`, and
  `releasing_the_lock_clears_the_holder_record` (the test establishing truncate-not-delete).
- `docs/issues/archive/2026-09-08-the-write-guard-holder-tests-fail-on-every-windows-lane.md` —
  the sidecar's origin. This bug is downstream of that fix, not a regression of it.
- `docs/trackers/issue-clusters/IC-22-hint-composed-without-the-request.md` — the class.

## Attribution

Surfaced by sessionId `ad379a7c`, who hit the refusal, reported that the identity field came back
empty, and declined to file the stale-binary bug once the binary was shown current — leaving the
real defect visible underneath. Diagnosed and filed by `59112612`.
