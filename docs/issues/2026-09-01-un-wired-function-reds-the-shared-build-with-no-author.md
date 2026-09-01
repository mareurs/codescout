---
kind: bug
status: open
tags:
- cluster/authorship-unrecoverable-after-the-fact
closed: null
opened: 2026-09-01
owner: marius
related: []
severity: medium
---

# BUG: a peer's un-wired function reds the shared build for everyone, and nothing can say whose it is

## Summary

On a shared checkout, the window between writing a function and wiring it up is a window in
which `-D dead-code` **aborts compilation for every session in the tree**. The failure is
correct — the build really is broken — but it carries no authorship, so it presents as *your*
breakage. This is the read-side of `IC-10`: the write-side asks "who wrote this?", the read-side
asks "is this mine?", and neither is answerable from the tree.

Filed as a distinct record rather than folded into a peer's, because the cost was mine and the
measurement is mine.

## Symptom (Effect)

Running the documented gate mid-task, having touched none of the implicated files:

```
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
error: function `scan_unterminated_fence` is never used
    --> src/librarian/tools/doctor.rs:4940:4
     = note: `-D dead-code` implied by `-D warnings`
error: could not compile `codescout` (lib) due to 1 previous error
```

```
cargo test --lib librarian::tools::
  doctor::tests::unterminated_fence_fires_only_where_a_fence_is_left_open      FAILED (0 != 1)
  doctor::tests::unterminated_fence_names_the_line_the_silenced_region_starts_at  FAILED (0 != 1)
test result: FAILED. 953 passed; 2 failed
```

`-D dead-code` **aborts the lib build**, so no other lint in the workspace is reached. A session
running the gate to check its *own* work gets no information about its own work at all.

## Reproduction

On a checkout shared by ≥2 sessions: session A writes a new `fn` and its tests but has not yet
added the call site. Any other session running the documented gate sees the above. Window closes
when A wires it up; here that was roughly ten minutes.

Note this needs no mistake by A. Writing the function before the call site is ordinary, and the
tests-first ordering this repo asks for makes it *more* likely, not less.

## Environment

Linux, Rust, codescout `0.15.0`, branch `experiments`, one working tree shared by 5 concurrent
Claude Code sessions (`peer-sessions.sh` count; `ListAgents` reported 4 — see Root cause).

## Root cause

Two things compose, and only the second is a defect.

1. **`-D dead-code` is correct and fails in the safe direction.** An unreferenced function is
   genuinely dead, the error is loud and local, and the abort is the desired behaviour for a
   single-owner tree. Nothing to fix here.

2. **A working-tree modification carries no author, so the failure cannot be attributed.** Git's
   author field is a constant across sessions (`IC-10`), and for *uncommitted* state there is
   not even a commit to carry a `Session-Id` trailer. So the reader has a red build and no
   channel that names its origin.

**Measured 2026-09-01.** Diagnosing "not mine" was cheap and took two commands:

```
git status --short                                   # doctor.rs modified — I never opened it
git grep -c scan_unterminated_fence HEAD -- src/librarian/tools/doctor.rs
                                                     # ABSENT from HEAD -> new and uncommitted
```

Diagnosing "**whose**" was not available at all, and it produced **three** wrong answers in one
evening — from three different parties, every one of them actively reasoning about attribution
at the time:

1. I asserted the work belonged to the peer I had just been talking to (`codescout-e6`), and
   messaged them saying so. Wrong. They showed their commits touch zero `.rs` files.
2. Correcting me, that peer named a fourth session (`bcc98c22`) on the basis that it owned other
   commits under `src/librarian/tools/`. Also wrong — and, as they filed against themselves,
   *"elimination by directory adjacency, structurally the same error I had just corrected you
   for, committed inside the message doing the correcting, while citing F-80 as the reason not
   to."*
3. The actual owner was **`codescout-68` (`c2a08c22`)**, settled by the `Session-Id` trailer on
   commit `800f1dec` (*"fix(doctor): structured_fix_pointers used a hand-rolled fence toggle;
   add unterminated_fence"*) once the work was committed, and confirmed by that session
   volunteering it.

**The instrument that works, and the limit that matters — the rule splits by state.** The
`Session-Id` commit trailer separates sessions cleanly and reaches sessions no socket enumerates,
including exited ones. But it exists **only on commits**:

| state | instrument |
|---|---|
| committed | `Session-Id` trailer. Positive, exact, one `git log`, reaches exited sessions. |
| **uncommitted** | **none exists.** Adjacency, directory, `ListAgents`, `git status` and dirty-file lists are all elimination in disguise. Ask the session. Until it answers, the honest claim is **"not mine"**, never "yours". |

The disputed work was uncommitted, so no trailer existed for it. Both wrong answers came from
reaching for the nearest proxy without noticing the instrument had been swapped mid-argument —
the sentence "the trailer is the positive instrument" was true, and did not apply to the object
in front of us.

**Why this is `IC-10` and not `IC-12`.** `IC-12` requires the standard diagnostic to *lie* —
report transient state as settled truth. Here clippy is telling the truth: the build is red.
What is missing is authorship, which is `IC-10`'s claim exactly. And it passes `IC-10`'s
admission test where a more careful reader would not have helped: for uncommitted state the
information does not exist. Three parties, three inferences, zero available channel.

## Evidence

### The population gap is real, and was load-bearing in NEITHER misattribution

```
ListAgents        -> 4 peers (claude-plugins-ed, system-9c, codescout-e6, codescout-68)
peer-sessions.sh  -> 5 sessions in this checkout
```

That `ListAgents` under-reports sessions in a checkout is true and documented (`BL-58`). It is
also **irrelevant to what actually happened**, and both parties reached for it anyway:

- The real owner, **`codescout-68`, was in the visible four** — in *both* sessions' listings, at
  the time each was reasoning. The gap did not contain the answer; it contained nothing either
  argument needed.
- I blamed the gap for my error. I had not reasoned from an enumeration at all — I reasoned from
  conversational salience, from who I had most recently been talking to.
- The correcting peer named `bcc98c22`, a session they could **not** see, and wrote *"it does not
  appear in my ListAgents"* as though that **corroborated** the identification — using an
  inability to see a candidate as evidence for it. Their own re-diagnosis: `3aca8639`.

**This is the durable kind of wrong explanation, and it is worth more than the original bug.**
Both stories were assembled entirely from **true parts** — `ListAgents` does under-report,
`BL-58` is real, directory adjacency is a genuine signal — so no individual claim in either ever
reads as false, and both survive exactly the review that catches a false one. A correctly
recalled limitation stood in for a diagnosis nobody performed. The tell is not falsity; it is
that **no step of either explanation was checked against the object in front of us**, and
checking took one `git log`.

### Knowing the class prevented none of the three

All three misattributions were made by parties who had just read `IC-10`, in messages *about*
attribution failure. The second was made **inside the message correcting the first**, citing
F-80 by name as the reason not to do it. That is `OB-1`'s signature — *the author,
specifically* — and it is the standing argument for answering this class with a mechanism
rather than with care.

### The cost, itemised

- ~10 minutes establishing the failures were not mine, mid-task, with my own gate result unknown
  for the duration.
- Three wrong assertions between two sessions, each costing the recipient a turn to establish a
  negative about their own work. A misattribution is not free because the ask is "nothing needed
  from you".
- Both wrong claims would have entered bug files uncorrected. One nearly did.
## Hypotheses tried

1. **Hypothesis:** the failures were caused by my own edits.
   **Test:** the two failing tests use a tempdir fixture and an in-memory catalog with literal
   seeded bodies (`doctor.rs`), reachable by none of my five changed files.
   **Verdict:** rejected.

2. **Hypothesis:** the modification belongs to the peer active in this checkout (`codescout-e6`).
   **Test:** asserted it to them; they checked their four commits with
   `git show --stat | grep -c '\.rs'` → 0 for every one.
   **Verdict:** **rejected — my error.** Proximity is not evidence.

3. **Hypothesis:** it belongs to `bcc98c22`, which owns other commits under `src/librarian/tools/`.
   **Test:** the `Session-Id` trailer on `800f1dec`, once the work was committed.
   **Verdict:** **rejected — the correcting party's error**, filed by them against themselves.
   Directory adjacency is elimination wearing a positive ID's clothes.

4. **Hypothesis:** it belongs to `codescout-68` (`c2a08c22`).
   **Test:** `Session-Id` trailer on `800f1dec`, plus the session volunteering it unprompted.
   **Verdict:** **confirmed** — and note it was only confirmable *after* the work was committed.
   During the window that caused the cost, no test existed that could have returned this answer.

## Fix

No fix proposed for `-D dead-code`; it is correct. The gap is the missing channel, and the
candidate remedies are `H`-shaped (a mechanism, not a discipline) exactly as `IC-10` predicts:

- **A provenance channel for working-tree state.** `.buddy/by-ppid/<pid>/session_id` already
  exists on disk for unrelated reasons. A `git status`-adjacent helper that maps modified paths
  to the session that last wrote them would answer "is this mine?" directly — the question the
  reader actually has — without needing to answer "whose?".
- **Cheaper and available today:** the two-command diagnostic above, promoted somewhere a
  session hits it *before* losing the ten minutes. It only occurs to you after.

Deliberately **not** proposed: relaxing `-D dead-code`, or asking authors to wire before writing.
The first removes a good gate to paper over a missing channel; the second inverts the tests-first
ordering this repo asks for.

## Tests added

None — this is an interaction between independent sessions on one filesystem, not a code path.
A regression test would have to spawn two sessions and race them; the honest guard is the
provenance channel above, and until that exists this record is the artifact.

## Workarounds

`git status --short` and `git grep -c <symbol> HEAD -- <path>` settle **"not mine"** in seconds.
**Stop there.** For uncommitted work that is the correct terminal state, not a step short of one
— three parties tried to go further and three were wrong. Do not proceed to "whose" from
`ListAgents`, directory adjacency, or who you were last talking to. If you need the owner, ask
the sessions; until one answers, the supportable claim is "not mine".

## Resume

Decide whether the working-tree provenance channel is worth an `H` entry in
`docs/trackers/observer-blindness.md`, or whether `IC-10` reaching n=2 should drive it. Cross-check
against `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`, which holds
the write-side twin.

## References

- `docs/trackers/issue-clusters.md` — `IC-10` (this class), `IC-12` (the read-side class this is
  *not*, and why).
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the write-side twin.
- `reconnaissance-patterns` F-80 — elimination over an incompletely-reported population, sent as a
  positive ID. This bug is another instance, committed while reading about the class.
