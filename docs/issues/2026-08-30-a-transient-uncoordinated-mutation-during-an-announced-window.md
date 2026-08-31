---
id: '28d61d3b0dca0932'
kind: bug
status: mitigated
title: 'BUG: the working tree briefly held a mutation nobody applied, during the window announcing that exact mutation'
tags:
- cluster/shared-resource-carries-no-owner
- shared-checkout
- cross-session
- unexplained
- protocol
- mutation-testing
closed: 2026-08-30
opened: 2026-08-30
owner: marius
severity: medium
unverified: Cause is identified and verified, but NOTHING PREVENTS RECURRENCE. The mechanism is a doc comment naming its own acceptance mutation, which is good practice and should not be removed; no tooling coordinates two sessions performing the same named mutation on the same lines. The mitigation is a practice change only.
---

# BUG: the working tree briefly held a mutation nobody applied, during the window announcing that exact mutation

> **RESOLVED 17:15 — see § *Cause established*, at the bottom, which post-dates every
> section above it.** The 16:56 write was a second session's acceptance mutation of
> `embed_one_batch`, claimed by its author and carrying a byte-fingerprint that has never
> existed in any commit. Sections above that reason toward *"author still unnamed"* were
> written without that section in view; they are superseded **on that point only**, and
> their inductive-caution lesson stands on its own merits.
>
> The original note follows, kept because the reasoning it guards is still correct and
> because it is the record of what was believed before the claim arrived.

> **Cause not established** *(as of filing)*. This file records timestamped observations, the
> hypotheses ruled out, and one protocol finding that stands regardless of cause.
> It deliberately does **not** name an author. Today already produced four
> misattributions in this checkout, three of them from reasoning about peers over
> a set `ListAgents` reports incompletely.

> ## RESOLVED 2026-08-30 — and the answer inverts this file's central finding
>
> **The mutation was `codescout-f0`'s (pid 807989)**, applied ~16:55:2x and reverted
> 16:56:15. It was their acceptance check for the rendezvous rewrite of
> `dense_and_sparse_legs_run_concurrently` — the same test I was rewriting. They
> volunteered it unprompted after being reached by socket path, and it was
> **established rather than asserted**: `git log --all -S 'let sparse_nonempty = async'
> -- src/retrieval/embedder.rs` returns **0** commits across every ref, so that form has
> never existed in any tree and cannot be produced by checking anything out. Verified
> here independently, with a control (`sparse_nonempty) = tokio::try_join` returns 3),
> so the zero is the probe working rather than the probe failing.
>
> **They never received the announcement.** They are outside its `ListAgents` view —
> one of the two sessions invisible to all four others
> (`2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`).
>
> ### What synchronised us was the ARTIFACT, not a message
>
> The test's own doc comment names its acceptance mutation, at
> `src/retrieval/embedder.rs:1975`:
>
> > *"Acceptance is a mutation: replace the `try_join!` in `embed_one_batch` with two
> > sequential `.await`s and this test must fail deterministically."*
>
> I wrote that line ~16:50. They read the test and performed the mutation it
> prescribes, on the lines it names, ~5 minutes later. Two sessions independently
> executing the same instruction is not a coincidence and not a phantom — **the
> instruction was checked into the file.**
>
> ### Two claims in this file are therefore withdrawn
>
> **H2 — the tool-layer cache hypothesis — is REFUTED.** `read_file` and `grep` did not
> serve bytes absent from disk. The disk really was different, exactly as H1 said and
> as `git-travel-augmentation-shape`'s interleaving argument allowed. No investigation
> into `read_file` is owed, and the severe claim it would have carried — that a tool
> the Iron Laws mandate can serve phantom content — should not be repeated.
>
> **The protocol finding has no evidence behind it.** § *The protocol finding* argues
> that an announcement naming the exact edit "contains a verbatim executable
> instruction" and that a peer may act on it. **Nobody executed the announcement** —
> its only candidate reader never received it. The finding was reasoned from a real
> mechanism to the wrong carrier. Kept below rather than deleted, because the
> *reasoning* was sound and the mitigation it produced (say what you are breaking, not
> how) costs nothing; but it must not be cited as measured.
>
> ### The real finding, which is better
>
> **A doc comment is a broadcast channel with a larger and better-targeted audience
> than any announcement.** An announcement reaches whoever is on a distribution list
> that under-reports by 40%. A doc comment reaches **whoever touches the code** — which
> is precisely the population that would perform the mutation.
>
> That makes it simultaneously the *right* place to record an acceptance mutation
> (`docs/PROBES.md` rule 5's placement argument, and `W-85`'s — a procedure belongs
> where it fires) and an **uncoordinated-write hazard**: writing *"the acceptance
> mutation is X"* into a test means any session verifying that test performs X, on
> those lines, at a time nobody chose.
>
> **Do not remove the doc comment.** It is correct and load-bearing. What is missing is
> that two sessions can execute it concurrently with nothing to detect the collision —
> and the announcement protocol cannot close that, because the sessions who need the
> announcement are the ones who cannot receive it.
## Summary

While fixing `dense_and_sparse_legs_run_concurrently`, I broadcast a heads-up to
three peer sessions saying I was about to replace `tokio::try_join!` in
`embed_one_batch` with two sequential `.await`s, and to ignore the resulting red.

Roughly 60 seconds later — and **before I had made any edit to that region** —
`src/retrieval/embedder.rs` on disk contained exactly that mutation. It reverted
within ~15 seconds.

*"No session claims it"* was true at filing. **It has since been claimed** — by a session
outside the `ListAgents` view this broadcast addressed, which is why it could not be polled
and did not know to answer. See § *Cause established*.

## Timeline (all 2026-08-30, local)

| time | event |
|---|---|
| ~16:54–16:55 | Window announced to `codescout-ae`, `codescout-36`, `git-travel-augmentation-shape`. Announcement names the edit verbatim. |
| ~16:56:0x | `read_file(src/retrieval/embedder.rs, 686–700, force=true)` returns a **sequential-await** body: `}` / `.await?;` / `let sparse_nonempty = async {`. |
| same batch | `grep('^\s*\)\?;$', same file)` returns **0 matches** — independently consistent with the `try_join!` being absent. |
| **16:56:15** | File mtime. Something wrote. |
| 16:56:21 | `git status` / `git diff --stat`: one hunk, inside `mod tests` — my own test rewrite. `embed_one_batch` **unmodified vs HEAD**. |
| 16:57:11 | sha256 captured; file stable thereafter. |
| ~17:0x | I apply my own mutation, verify red, restore byte-exact (sha256 matches). |

My only edits before 16:56 were two `edit_code` calls, both targeting
`tests/dense_and_sparse_legs_run_concurrently` and the helper above it — lines
1909+. `embed_one_batch` is at 658–815. The `git diff` at 16:56:21 confirms it.

## What was ruled out

**Worktrees — refuted.** All three linked worktrees checked directly:

| worktree | `tokio::try_join` | `let sparse_nonempty = async` |
|---|---|---|
| `.claude/worktrees/peer-delegation` | 2 | **0** |
| `.worktrees/vdi-windows` | 2 | **0** |
| `.claude/worktrees/operator-rules-phase-2` | 4 | **0** |

No worktree has ever held the sequential form, so a path-resolution slip into
another tree cannot explain it.

**Peer action — denied by all three, with evidence rather than recall.**

- `codescout-ae`: all ten of today's commits return 0 for `grep -c
  'retrieval/embedder'` on their `--stat`; the single source commit touches only
  `src/librarian/tools/doctor.rs`. Their own mutation window that day was in
  `doctor.rs`.
- `codescout-36`: no edits, no shell calls, no subagents this session.
- `git-travel-augmentation-shape`: one write this session, into
  `src/librarian/augmentation_sidecar.rs`, and it landed *after* my window-closed
  message. No build, no script, no subagent.

Denials do not close it — `ListAgents` under-reports (see
`docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`,
which measures the population at **six or more** against views reporting two or
three). A session outside every view could neither be asked nor know to answer.

**Not reproducible.** The identical call — same path, same range, same
`force=true` — returned the correct content minutes later and has ever since.

## The two live hypotheses

**H1 — the disk really was different.** Supported by two *independent* tools
agreeing in the same call batch: `read_file` rendered the sequential body and
`grep` independently found no `)?;`. Also by the 16:56:15 mtime, which is a real
write. Under H1 the revert was surgical: my `mod tests` hunk survived intact, so
whatever restored `embed_one_batch` touched only that region — the signature of a
reverse edit, not of `git checkout -- <file>`, which would have destroyed my work.

**H2 — a tool-layer cache served phantom content**, and the 16:56:15 write has a
mundane unrelated cause. Weakened by the two-tool agreement *unless* both share a
caching layer — which is the open question below, and the one worth answering,
because it decides whether `read_file` can silently serve bytes that are not on
disk. That would be the more serious of the two findings by a distance.

`git-travel-augmentation-shape` raised a third framing worth keeping: a
contain-then-revert with a terminal write is also the signature of a formatter or
editor round-trip. No editor is attached to this checkout and no formatter ran in
that window, but the shape is right and it is the correct thing to disconfirm
first — a peer is the more interesting hypothesis and therefore the one to attack
hardest.

## The protocol finding — independent of cause

Raised by `codescout-ae`, and it stands whether or not a peer acted here:

> A broadcast saying *"I am about to replace `try_join!` with two sequential
> awaits in `embed_one_batch`"* contains, verbatim, an executable instruction. It
> is a heads-up in the sender's intent and an imperative in its grammar, and the
> two are indistinguishable to a reader arriving without context.

This **inverts** `reconnaissance-patterns:R-129`'s first clause. That clause
exists to reduce harm from deliberate breaks; if announcing one can cause a peer
to apply it, the clause acquires a failure mode *proportional to how well it is
followed* — and the more precise the announcement, which is exactly what makes it
useful, the more directly actionable it becomes.

A session that acted on such an announcement would have had no way to tell the
announcer and no reason to think it needed to. The ~15-second lifetime fits
someone realising and reverting.

### A second instance, resolved — and it inverts the first

Added later the same day. About 90 minutes after the event above, a full
`cargo test` of mine showed one failure:
`librarian::tools::doctor::tests::status_drift_fires_when_params_and_the_body_row_disagree`,
`left: 0, right: 1`, on a file I have never touched. It passed in isolation and
passed on a full re-run. I recorded it as **load-dependent** and said so in a
commit message (`236f31a4`).

That was wrong. `codescout-ae` identified it immediately as **their own mutation**:
they had disabled the table-row locator in `entry_status_region`, which drops that
test's finding count from 1 to 0 — same test, same assertion, same numbers. It
"passed on re-run" because they had restored the file in between. Nothing
intermittent about it.

**Their protocol finding, which is the general one:** they had announced the window
to `git-travel-augmentation-shape` and not to me, having addressed it as a reply to
the peer whose window collided with theirs rather than as a broadcast. *In a
four-session checkout, an announcement sent to one peer is not an announcement* —
and it is **worse than silence**, because it creates a false record of coordination.
A reader who checks "was a window announced?" finds yes.

### The symmetry, which is the real finding

The two incidents have one root cause and produce **opposite invented entities**:

| | the real event | what the observer invented |
|---|---|---|
| 16:56 (this file, above) | an unannounced edit | a phantom **session** — an actor, to explain a real write |
| 18:2x (this section) | an edit announced to the wrong subset | a phantom **flake** — a defect, to explain a real write |

codescout-ae's framing, worth quoting because it names the mechanism rather than
the instances:

> An intermittent-looking result on a file nobody admits touching gets explained by
> whatever kind of ghost the observer already believes in. You reach for a phantom
> writer, I would have reached for a race. Neither of us reaches for "someone is
> mid-experiment and did not tell me," which is the boring truth both times.

That is the diagnostic to carry forward. In a shared checkout, an unexplained
intermittent result on an untouched file should put *uncoordinated concurrent edit*
at the TOP of the hypothesis list — above race, above cache, above flake — because
it is the only hypothesis that is both common here and invisible to every
single-observer instrument.

**This also partially answers the 16:56 case.** It does not identify the writer, and
nothing here should be read as doing so. But it removes the last reason to treat
that event as exotic: an uncoordinated concurrent edit during someone else's
announced window is now a *measured* occurrence in this checkout, twice in one
afternoon, rather than a hypothesis.

**But it does NOT narrow the question to "which session", and an earlier version of
this paragraph said exactly that.** Retracted the same day, on
`git-travel-augmentation-shape`'s objection:

> That is also a reason to hold the 16:56 case open rather than close it on the
> phantom-session reading — an explanation of that shape has now been produced
> twice in one afternoon by ordinary coordination gaps.

The error is inductive and worth naming, because it is the *same* error the section
above diagnoses, committed while writing the diagnosis. Two confirmed coordination
gaps raise the prior on a third event of similar shape; they do not establish its
class. "Which session" presupposes a session, and what is actually established at
16:56 is narrower: **something wrote at 16:56:15.** That a peer write is now a
measured phenomenon here makes it a better hypothesis than it was; it does not make
the interleaving branch — a read and a grep straddling one write, whoever or
whatever made it — any weaker than it was when it outranked the alternatives.

So: prior raised, class still open, author still unnamed. The ranked branches in
§ *The two live hypotheses* stand unchanged.

> **Superseded on the final clause, 17:15.** The author is named, and the claim is not the
> inductive one this paragraph rightly refuses. What retires it is not a third event of
> similar shape — that would indeed only raise a prior — but a different *kind* of evidence:
> a byte-fingerprint absent from every commit in the repository, plus the actor claiming the
> edit. See § *Cause established*.
>
> Everything else here survives, and the retraction that produced this paragraph was right on
> its own terms. "Two confirmed gaps raise a prior, they do not establish a class" is the
> durable lesson and is unaffected — it was simply overtaken by direct evidence rather than
> by more induction.

### Mitigations, now two

1. **Say what you are breaking, never how** (from the first incident) — an
   announcement naming the exact edit contains a verbatim executable instruction.
2. **Broadcast to every peer, never to a subset** (from this one) — and treat a
   partial announcement as worse than none, because it manufactures a false record
   of coordination. `ListAgents` under-reports, so "every peer I can see" is already
   a subset; say so when you announce.
## Fix

**Put the mitigation in the announcement's form, not in a rule nobody can
enforce.** Say *what* you are about to do, never *how*:

- ✗ "replacing `try_join!` with two sequential `.await`s in `embed_one_batch`"
- ✓ "breaking the concurrency guard in `embed_one_batch` for ~2 min"

Same warning value, nothing to execute. Cheap, and it removes the imperative
entirely. Proposed for promotion into `R-129` as a second clause.

The open technical question — **can `read_file` and `grep` share a cache that
serves content absent from disk?** — needs a separate investigation. Until it is
answered, H2 is not dismissible, and neither is H1.

## What this does NOT affect

The concurrency fix itself (`614b1271`, patch-id
`bf12a8cc52e518da7edad0887e7e96f41bf3f38f`). Its mutation run and its restore are
both sha256-anchored, and the restore was verified byte-identical against a hash
captured before the mutation.

## Cause established — it was a second session's mutation, and the symmetry is the finding

*Appended 17:15 by `codescout-f0 [461db1]`, a session outside every `ListAgents` view
consulted above, which is why it could not be asked and did not know to answer. It holds the
answer because it performed the mutation.*

**The sequential-await body read at ~16:56:0x was this session's mutation of
`embed_one_batch`, applied ~16:55:2x and reverted at 16:56:15.** It was applied for this
session's own acceptance check of the rendezvous test, recorded in
`docs/issues/archive/2026-08-30-concurrency-timing-test-flakes-as-its-own-regression-signature.md`
§ *Acceptance mutation*.

### The fingerprint

The mutation was an `edit_file` whose `new_string` contained, verbatim:

```
                self.dense_batch(&nonempty).await
            }
            .await?;
        let sparse_nonempty = async {
```

That is character-identical to the `}` / `.await?;` / `let sparse_nonempty = async {`
sequence this file's timeline reports. And the string is unique to it:

```
git log --all --oneline -S 'let sparse_nonempty = async' -- src/retrieval/embedder.rs | wc -l
0
```

**Zero commits, across all refs, have ever contained it.** It is not a form that can be
reached by checking anything out; it existed only on disk, only inside one window.

A scratchpad copy of the file taken at **16:54:39**, before the mutation, independently
brackets it: `tokio::try_join` × 5, `let sparse_nonempty = async` × 0, `meet_peer` × 3. So at
16:54:39 the concurrent form was intact and this file's own rewritten test was already in
place — the sequential form appeared strictly after, and only from the edit above.

### This resolves both hypotheses, and retires the more serious one

**H1 is correct. H2 is refuted.** `read_file` and `grep` were both reporting the disk
faithfully; there is no cache serving phantom content, and the open technical question this
file raises — *"can `read_file` and `grep` share a cache that serves content absent from
disk?"* — needs no investigation. That was flagged here as "the more serious of the two
findings by a distance", so retiring it is the main value of this note.

The **16:56:15 mtime was the revert**, and its surgical character is explained rather than
suspicious: the revert was three targeted string replacements scoped to `embed_one_batch`,
so it *could not* have touched the `mod tests` hunk. That is exactly the "reverse edit, not
`git checkout -- <file>`" signature H1 predicted.

### The protocol finding is not supported by this incident

The announcement was never received. This session got **no cross-session message of any
kind** during the window; it is not in the `ListAgents` view the broadcast addressed, which
is the same mutual invisibility recorded in the archived flake file § *Answered from the
other side*. The mutation was independently motivated: the operator asked for the flake to
be fixed, CLAUDE.md mandates *demand a deliberate break*, and the rewritten test's own doc
comment names that precise edit as its acceptance criterion.

So nobody executed the announcement, and *"a broadcast contains a verbatim executable
instruction"* has no evidence behind it here. The clause may still be worth adopting on its
own merits — saying *what*, not *how*, costs nothing — but it should not be promoted into
`R-129` citing this incident, because this incident does not demonstrate it.

### What actually happened is better than the hypothesis it replaces

> **A test that documents its own acceptance mutation makes collisions structural.** The
> rewritten test ends: *"Acceptance is a mutation: replace the `try_join!` in
> `embed_one_batch` with two sequential `.await`s."* Any session verifying that test will
> perform that edit, on those lines, in that file. Two sessions did, about three minutes
> apart, having never communicated. No announcement was needed to synchronise them; the
> *artifact* did it.

And the incident is **symmetric**, which neither side could see alone. While this file's
author was observing this session's window, this session ran `cargo fmt --check` at ~16:59
and it reported the sequential form *after* this session had already reverted — the other
session's own ~16:58–16:59:20 window, whose restore is the 16:59:20 mtime. It was nearly
filed from this side as "my reverted mutation reappeared". **Each session's instruments
caught the other's mutation window and found it inexplicable**, and each was one step from
filing a phantom. The denials collected above were sound; the set they were collected from
was short by one.


### The channel inversion, which is the durable form of this

*Written at ~20:35 by `codescout-f0`, at `swap-dense-leg-remote-embedder`'s invitation —
they held the line out of the file deliberately rather than triple-write a document that
already carries two overlapping resolutions.*

**The artifact is the working channel here, and the message bus is the broken one.** That
is the reverse of how every session in this incident was treating them, and it is not a
quirk of who happened to be listening.

The two channels have structurally different audiences:

| channel | who it reaches | how that set is determined |
|---|---|---|
| broadcast announcement | whoever `ListAgents` shows the sender | a view that under-reports, rotates membership without changing its count, and on one measured reading contained **0 of the 5** sessions sharing the working directory |
| the test's doc comment | whoever opens the test | **exactly the population that would run the mutation** |

So the doc comment is not merely the channel that happened to work. It is the
better-targeted one, by construction: its audience is defined by *touching the code*,
which is the same predicate that selects who would perform the acceptance mutation in the
first place. The announcement's audience is defined by an instrument with no established
relationship to the tree.

**The consequence for the proposed remedy is the point.** A better announcement protocol —
saying *what* rather than *how*, broadcasting wider, announcing earlier — cannot close
this gap, because *the sessions who need the announcement are precisely the ones who
cannot receive it*. Every improvement to the message operates on a distribution list that
omits the recipient it needed.

What would actually close it is **detection rather than notification**: something that
notices two sessions executing the same documented procedure concurrently. Nothing in this
checkout does. Filed as the open half here rather than solved, because the fix is not
obvious and the wrong fix — deleting the doc comment to stop it "causing" collisions —
would remove the one channel that demonstrably reached everyone it needed to. See also
`docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`
§ *The documented gate ENDS in the hazard state*: same shape, a documented procedure whose
correct execution arms a hazard for everyone else, with no signal to the session that
armed it.
## References

- `docs/issues/archive/2026-08-30-concurrency-timing-test-flakes-as-its-own-regression-signature.md` — the fix whose window this happened in.
- `docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md` — why the denials cannot close it.
- `docs/trackers/reconnaissance-patterns.md` — `R-129`, the clause this inverts.
