---
kind: bug
status: taken
title: The write-lock refusal cannot tell a CLEARED holder record from an ABSENT one, and names neither cause
tags:
- cluster/hint-composed-without-the-request
topic: write-guard diagnostics
claimed_at: 2026-09-08
claimed_by: 5399543d-22d6-4ed9-9ebb-876be459989f
closed: null
opened: 2026-09-08
owner: marius
related: []
severity: low
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

Fixed at `acquire()`, exactly where the class analysis above says it belongs — `read_holder_record`'s
signature is untouched, so the safety collapse its doc comment argues for is intact and no future
caller inherits a widened return it could misread as "no holder".

A new `why_no_holder_record(&Path) -> &'static str` re-reads the sidecar's metadata at the call
site and returns one of **four** hints, not the prescribed two. The last two are states the
three-state table above names but a bare `exists()` split cannot reach:

| disk state | cause | remedy the hint now gives |
|---|---|---|
| exists, 0 bytes | a current codescout cleared it on release | **retry immediately** — an ordinary lost race |
| exists, non-empty | the record does not parse | inspect the file; a retry reports the same state |
| absent (`NotFound`) | nothing current ever wrote one | today's text, which is accurate here |
| unreadable (other `Err`) | permissions | check permissions; the lock is held either way |

**The re-read can disagree with `read_holder_record`'s under a concurrent release, and that is
accepted rather than overlooked.** Every arm prescribes retry or investigation and **none claims
the lock is free**, so the safety property stays in the caller's `Err` and never in this text. A
stale classification costs a reader one wrong sentence; the collapse it replaces cost them a wrong
hypothesis and a `strings` run.
## Tests added

Two, in `src/agent/write_guard.rs`, built to the prescription this section carried before the fix
existed — kept below, in the imperative it was written in, because it is the reason the pair
discriminates:

- `a_cleared_holder_record_reports_a_lost_race_not_an_old_binary`
- `an_absent_holder_record_still_reports_a_missing_writer`

Both seed contention by taking the flock **directly** rather than through `acquire()`, which
writes a holder record on success and would route the contender to the *named* branch, never
reaching the anonymous one under test. The absent case also asserts `!record.exists()` as a
control, so `open_lock_file` silently creating the sidecar could not quietly turn that test into a
duplicate of its sibling.

**Observed RED before green, mutating the PRODUCTION path rather than the tests' inputs — twice,
in the two directions the pair claims to separate:**

| mutation applied to `acquire()` | cleared test | absent test | other 7 |
|---|---|---|---|
| *(none — the shipped fix)* | ok | ok | ok |
| restore the collapsed one-message hint | **FAILED** | **FAILED** | ok |
| point the absent arm at the lost-race text | ok | **FAILED** | ok |

The middle row is the regression that happened, and it also measures the claim this bug rests on:
the seven pre-existing write-guard tests stay green under it, so the suite that reported 18/18 on
`d2900ecb` was structurally incapable of seeing this. The bottom row is why the control is not
symmetry — under the fix applied in the wrong direction the cleared-case assertion is **still
green**, and that test is the only thing in the suite that reds.

**Gate, stated precisely rather than as "green"** (2026-09-08, four commands in the load-bearing
order):

| command | exit |
|---|---|
| `cargo fmt` | 0 |
| `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` | 0 |
| `cargo test --workspace --no-default-features` | 0 |
| `cargo test --workspace` | **101** — 5245 passed, 1 failed |

The one failure is `peer::server::tests::run_exits_after_idle_timeout_with_no_connections`, which
is separately filed and still open at
`docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`. Reproduced
twice under load, passes **3/3** in isolation, and `src/peer/` holds **zero** references to
`write_guard`, `WriteGuard` or `holder_record` — so there is no path from this diff to it. The
load was this session's own concurrent `--workspace` runs on a checkout shared by 11 live
sessions.

Recorded this way on purpose: "gate green" would have been false, and an `unverified:` field would
have implied doubt about *this* fix rather than naming someone else's filed flake. The nine
`agent::write_guard` tests pass in **both** lanes — read out by name, never from a lane total.

---

The prescription, as written before the fix:

The guard is cheap and needs no race: seed a temp root with a live flock and a
**truncated** holder file, contend, and assert the refusal does **not** say "an older codescout".
Then the same with the file **removed**, asserting it does.

Both assertions are needed and for different reasons. The truncated case is the regression that
actually happened. The absent case is the control — without it the pair is monotone under a
change that makes every refusal say "released, retry", which would be the same defect inverted.

Annotate the fixture: **the holder file must be truncated, not deleted**, and that detail is the
whole test. A tidy-up that deletes it instead leaves the assertion passing against the wrong
state.

**Cite the precedent in the same file rather than inventing the shape.**
`releasing_the_lock_clears_the_holder_record` already carries exactly this control, annotated as
such on the test:

```
/// LOAD-BEARING: the pre-drop assertion is the control. Without it, a build
/// in which the record is never written at all would satisfy the post-drop
/// assertion and this test would be monotone under the feature's removal.
```

Its post-drop assertion is also what establishes truncate-not-delete, and it does so twice over:
`read_to_string(&record).unwrap()` pins **existence** (it would panic on a deleted file) and
`assert_eq!(…, "")` pins **emptiness**.


### The author had already defended the neighbouring case, which is what makes this a gap rather than an oversight

The same test's doc comment reasons about a **fourth** state and rules it out by design:

> Releasing must clear the record. Otherwise the NEXT holder — one whose own record write failed
> — is reported under the PREVIOUS holder's name, which is strictly worse than anonymous: it
> sends a refused party to message someone who has already exited.

That is the same harm class this bug describes — **a refusal message routing the reader
somewhere useless** — identified and closed, in this function, by whoever wrote it. Truncation on
release exists precisely to prevent a stale identity being reported as a live one.

So the contract this bug rests on was not undocumented. It was written down, in the subsystem,
by someone who had thought about exactly this failure mode. Two sessions still reasoned about it
from `stat` output instead, because neither had a reason to open a test file while diagnosing a
refusal — `CLAUDE.md` § *Observer Blindness*, third position: published to an audience that never
reads that surface.

The gap is the sibling of the case that *was* closed. Preventing a **stale** identity from being
reported as live is done; distinguishing **no identity yet** from **identity deliberately
cleared** is not, and only the second has "just retry" as its remedy.
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
