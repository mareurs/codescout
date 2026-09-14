---
id: df0c18734b20fddd
kind: bug
status: taken
title: An armed mutation is a deliberate red, and no observer can distinguish it from a broken test
tags:
- cluster/transient-shared-state-lies-to-readers
topic: shared-checkout mutation testing
claimed_at: 2026-09-14
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
---

# BUG: an armed mutation is a deliberate red, and no observer can tell it from a broken test

## Summary

Mutation testing requires putting a **known-bad** version of the code into the tree, running the
suite, and reading the failure as evidence. On a shared checkout that failure is published to every
other session's `cargo test`, where it is **byte-identical to a real regression**.

Observed 2026-09-08. `59112612`'s default lane went red at 14:13Z on
`stripping_a_fixture_root_covers_every_rendering_not_just_the_native_one`. That `... FAILED` line
was not a defect — it was the evidence I was collecting, from a mutation I had armed deliberately,
expected to fail, and restored within a minute.

## Why this is not the same bug as `df517af91b43a5f7`

That one is a **routing** defect: uncommitted WIP reds the build, the author is knowable
(`claimed_by` in committed frontmatter), and nothing that observes a red consults it. Fix the
routing and the observer learns whose it is.

**Perfect routing does not help here.** Knowing the red belongs to `5399543d` still does not say
whether the FAILED line is a broken test or a working one under measurement. The missing bit is not
*whose*, it is *what kind* — and it exists nowhere on disk.

## The observer did everything right and still concluded wrongly

`59112612`'s own account, and it is the reason this earns a file:

- ran `git grep HEAD` for the test name → **0 hits**
- correctly concluded *"not in the committed tree, not mine to fix"*
- declined to stash, repair or commit it, per the shared-checkout sequence's step 6
- reported rather than repaired

Every step sound. The conclusion — *broken WIP* — was still false, because **the evidence available
to an outside observer cannot separate `broken` from `deliberately red right now`.** That is the
`OB` admission test passing cleanly: not carelessness, a party holding a parameter nobody else can
read.

## Why "be careful" is not available as a remedy

The usual mitigation for a peer-visible red is *don't leave the tree broken*. It does not apply,
and the reason is structural rather than a matter of degree:

| | `df517af91b43a5f7` | this |
|---|---|---|
| the red is | accidental | **intentional** |
| the author wants it | no | **yes — it is the measurement** |
| "compile before you step away" | conceivable | **destroys the evidence** |

Removing the failure removes the finding. The window cannot be shortened below the time it takes to
run the suite, and CLAUDE.md mandates mutation on the production path for exactly the guarantees
this project relies on — so the practice generating the hazard is the practice the testing
discipline requires.

## Fix

**Still unbuilt — but the two directions below are no longer symmetric, and the first one is now
known to BACKFIRE.** Measured 2026-09-09 across two announced mutation windows (03:22Z, nine
mutations in an untracked file; 04:16Z, three in tracked files), four peers announced to each time.

### Announcement was tried. It labels, and it SUBTRACTS WITNESSES.

The entry above called announcement *"cheap, and a policy the arming session must remember"* — a
`SKF-22` weakness. That understated it. Announcement is an **intervention on the population it
needs as instruments**:

> The prescribed response to an announcement is *do not investigate, stand down*. Complying is
> what removes the observer whose build log would have resolved the arming session's own anomaly.
> **The better the announcement works, the less it can observe.**

Observed directly. My 04:16Z window produced a reading I could not explain — one mutation reported
a real E-coded compile error batched and none in isolation, same one-character patch. A concurrent
build log would have settled it. `c9ab2c8d` **held off building because I announced**, and their
held-back build was that log. The anomaly is still unexplained; both offered causes were falsified.

And the 03:22Z window looked well covered only **by luck** — two peers happened to be mid-gate. I
cited that coverage as though the announcement had produced it. It had not.

Promoted to `observer-blindness:OB-23` (*a notification that changes the recipient's behaviour
cannot also measure it*), three-way attribution: `c9ab2c8d` supplied the framing and complied with
it, `59112612` retired it, this file's windows made the cost concrete.

**The split that survives:** *"is anyone building?"* is a **query** — it leaves the population
intact and returns a count. *"Here is what you will see"* is an **intervention**. They are
sequential rather than alternative (`ad379a7c`): query first for the witness count, then announce
for the labelling, which is real — without the 03:22Z announcement one peer's next move was to
bisect their own commits.

### An announcement also under-describes its own blast radius, in two ways

**Radius is what others READ, not what you WRITE.** I announced 03:22Z as *"only
`agent::build_check::tests::*` can move — no tracked file is edited"*. Both halves were wrong:
`src/agent/mod.rs` was tracked and modified (the `mod` declaration), and a concurrent
`cargo test --workspace` reads the **whole worktree** regardless of which files I consider mine.
Scoping your writes bounds nothing about a concurrent build.

**One window carried THREE failure kinds under one exit code**, and I predicted one:

| kind | in that window | announced? |
|---|---|---|
| test failures | the mutations themselves | yes |
| **clippy `-D warnings` errors** | 18 `never used` from a then-unwired module | no |
| a peer's gate red | reached a third session's run | no |

The clippy red was found in **another session's gate log**, not by me. An arming session cannot
enumerate its own blast radius, which is the same structural claim this file already makes about
the `broken` / `deliberately red` bit.

### So the marker is the direction — with one property this file did not know to require

A marker other sessions can read remains the shape `OB-1`'s third position prefers, and it is the
only one that survives the arming session forgetting. Tonight adds a constraint:

**The marker must be PASSIVE — it must label without prescribing.** A marker that reads as *"a peer
is mid-mutation, stand down"* reproduces `OB-23` exactly: it changes the reader's behaviour and
destroys the evidence. The correct shape labels a red the reader **was going to see anyway** and
asks for nothing, so the build still happens and its log still exists.

That is the same *informational, never a request* constraint the author-side build check
(`7168c1f0`) was built under, arriving from the opposite direction — there it protects the peer's
autonomy, here it protects the arming session's own evidence.

**Second property, from `59112612`:** prefer a record the acting party writes **unconditionally, at
the moment it acts**. Their pre-push log has that shape and cannot be dismissed by compliance,
because it depends on nobody else doing or not doing anything. A marker written by the arming
session at arm time qualifies; one that depends on peers reading it does not.

**Not built, and deliberately not built tonight.** Announcement is available and imperfect;
building the marker is a design change to a shared gate surface, which is an operator's call.
### 2026-09-13: the red did not read as BROKEN. It read as a POLICY QUESTION.

Third instance, and it adds a failure kind this file does not name. Armed by sessionId
`f3c594ce-c424-40d3-a603-9693cfef3f63`, observed by `8bd791df` at 21:29 on a `tests/doc_tool_refs.rs`
mutation. **No announcement was made** — not withheld on `OB-23` grounds, simply not thought of,
which is `SKF-22` arriving exactly as predicted.

**The new part.** This file's whole framing is `broken` versus `deliberately red`, a two-way bit.
The mutation here was to a guard that SCANS THE CORPUS, so defeating it did not produce a generic
`... FAILED`. It produced a detailed, correct-looking finding list:

```
45 present-tense document(s) name a tool parameter that does not exist.
  docs/architecture/augmented-artifacts (13), src/prompts/guides/librarian (8),
  librarian-runtime (7), workspace-state (4)
  Sample: `memory(recall` — bare, no `=`;  `doc(update` — bare, no `=`
```

Every file named is real, every form quoted is really in it, and the guard's own message explains
what it thinks is wrong. So the observer did not read *"a test is broken"*. They read **a
disagreement about intended behaviour**, and asked the right question about it: *"if the bare
`doc(update` form is meant to be legal and the test needs the carve-out, that's your call."*

That is a **third** state, and it is worse than the two this file already has, because it is
*actionable*. `broken` invites standing down. A policy question invites deciding — and a peer who
had decided *"yes, the bare form is legal, add the carve-out"* would have written a carve-out into
a guard mid-mutation, against a mutation, on my behalf. Nothing would have looked wrong at any
step. The generalisation: **a mutation to a guard produces a red carrying the guard's own
reasoning, and reasoning is exactly what makes a red look legitimate rather than defective.**

**A better routing instrument was used, and the file's thesis held anyway — which is the value of
the datapoint.** `59112612` reached `git grep HEAD` and inferred. `8bd791df` had
`scripts/file-provenance.py` via `fmt-mine`, which named me **positively**: `PEER, window from
2026-09-12T09:05:58Z, sessionId f3c594ce…, LIVE`. They also checked that none of their own three
touched files appeared among the 45 — a real independent check, not an elimination. Correct
identification, first try, no inference. **And it still carried no information about which kind of
red it was**, because the arming bit is not on disk. § *Why this is not the same bug* predicted
this; it is now measured against the improved instrument rather than the weak one.

**Blast radius reached a THIRD party, one the arming session had no reason to model.**
`05841db2` was preparing to push `experiments` — 80 commits, 5 sessions, 25 mine — and a
second-hand *"the gate is red"* is exactly the signal that should stop a push. It did not only
because they sent a courtesy notice first and I could say the red was stale. That is two
independent courtesies covering for a missing mechanism, and neither was owed. § *An announcement
also under-describes its own blast radius* lists three failure kinds under one exit code; add a
fourth — **a stale red report outliving the red, and blocking an unrelated action**. The red lasted
under a minute; the report of it was still live twenty minutes later.

**What would have helped, and it is not announcement.** The passive marker this section already
prefers would have worked here, and this instance sharpens the requirement: the marker must be
readable **after the fact**, because the damage travelled as a *report* rather than as a build. A
marker present only while the mutation is armed labels the build log and does nothing for the
sentence a peer writes about it afterwards. `59112612`'s *unconditional, written at the moment it
acts* property already implies a durable record; this is the argument for why durability, not just
presence, is the load-bearing half.

Still not built, still an operator's call on a shared gate surface.
### Instance 2 — 2026-09-13, and the cost landed on a session that did everything the file asks

A second instance, reported from **both** sides within one exchange, which is what makes it
worth recording rather than merely counting.

`codescout-aa` (sessionId `05841db2-4ba0-4cb2-a22f-c0bc2f771e20`) was mutation-testing
`written_by_report` in `src/tools/semantic/index.rs` — three mutations, ~30s each, restored
between. Mutation 1 deleted the `reading_binary_dirty` line. `8bd791df` ran the four-command
gate in that window and got:

```
assertion `left == right` failed: the READER's dirty flag …
  left: Null
 right: true
```

Lean 3521/1, default 5468/1, clippy 0. The `left: Null` is the deleted field, so the red was
a faithful report of a deliberate absence.

**What is new here is the observer's conduct, because it exhausts the remedies this file
already proposes and the cost still landed.** The 2026-09-08 instance concluded that asking
beats telling; `8bd791df` asked. It also, *before* writing anything, ran
`file-provenance.py --all` and read `fmt-mine`'s refusal — two independent instruments — and
reported a **positive identification** rather than inferring from the dirty file next to the
red. Adjacency would have returned the same answer on this occasion and is anti-evidence on a
five-session tree (`CLAUDE.md` § *Reaching a Peer Session*, rule 1). None of that shortened
the window: the gate still had to run, still went red, and still cost a triage plus a
write-up.

So the standing conclusion holds and sharpens: **an armed mutation is indistinguishable from a
break by construction, and no amount of observer diligence closes it — only the mutating
session knows, and only in advance.** `codescout-aa` named the gap themselves: they did not
announce the window, and offered no mechanism because none exists here. `f3c594ce` said
exactly the same thing after instance 1. Two authors, independently, reaching *"policy, not
mechanism"* is the strongest evidence in this file that the third position of `CLAUDE.md` §
*Observer Blindness* is unbuilt rather than merely unused.

**A candidate mechanism, recorded because both authors stopped at policy — and it is HALF a
mechanism, which the co-reporter caught.** The mutating session already touches the tree; a
marker file it writes on arming and removes on restore (`.buddy/<sid>/armed-mutation`, naming
the path under test) would let the gate runner distinguish the two states without asking, and
would survive the mutator being busy.

`05841db2` named the defect in it, and the correction matters more than the sketch: **only the
ARMING half is unconditional.** The mutating session always writes the marker, but the reading
half is *another trigger someone has to remember* — and this instance is precisely the case
where the reader was mid-gate with a red in front of them and no reason to go looking. The
sketch moves the remembering from the mutator to the observer rather than removing it. That is
this file's own conclusion holding one layer up, against its own proposed remedy.

**The refinement that follows, and where it lands.** The reading half becomes unconditional
only if it rides an instrument that already fires on the red — which exists: `run_command` runs
`scripts/attribute-red.py` on failure-shaped output (`02e61230`), so a marker read there
reaches the observer without anyone deciding to look. At which point both halves reduce to one
prior question, because that instrument carries the channel gap this same instance established:
a `Bash` gate never reaches it. So *"build the marker"* and *"instrument the second channel"*
are not two tasks — the first is worth little without the second, and the second makes the
first cheap.

Liveness for a stranded marker (crash mid-mutation) is the easy half: the sid in the path maps
to a socket, so `/proc/<pid>` existence makes a dead session's marker ignorable with no timeout
and no clock. Sketched jointly by `8bd791df` and `05841db2`; **not built**, and routed to an
operator queue rather than settled between peers, because it is a new mechanism rather than a
fix to an existing one.

**`wip_authors` does not cover this, and the reason is worth pinning.** `02e61230` (same
morning) widened `run_command`'s red-attribution to failure-*shaped* output rather than only a
non-zero exit, and it was verified on the wire at 09:34. It did not fire here, and the
hypothesis offered — a server predating the 09:27:41 build — was **wrong**: `readlink
/proc/$PPID/exe` returned the live inode, not a `(deleted)` one. The operative cause is the
ceiling `CLAUDE.md` § *Reaching a Peer Session* already states — *"native `Bash` bypasses
`run_command` entirely, so on a `Bash` gate the silence means nothing"* — and the gate had been
run through `Bash`, which the companion plugin permits today while the shell eval is live. So
the hook's new coverage is real and orthogonal: widening the *trigger* from exit code to output
shape does not reach a *channel* that was never instrumented.

Raised by `05841db2-4ba0-4cb2-a22f-c0bc2f771e20`; observed and written up by
`8bd791df-5ff4-40fe-af30-69cc3fefc2f7`. Neither party is at fault in a way a rule would have
prevented, which is the point.
## The window is not bounded by your own process — a shared build lock hands the tree on

Everything above treats the hazard as an OBSERVER problem: a peer cannot tell your deliberate
red from a real one, and the remedy is to announce the mutation before arming it. That remedy
is right and it fixes **attribution**. It does nothing about the **window**, and the window is
wider than the arming session can see.

Measured 2026-09-14, on this checkout, with three `cargo test --workspace` runs contending:

```
your cargo test exits        -> releases the shared build-directory lock
peer's queued cargo acquires -> begins compiling CURRENT source
your revert runs             -> too late; their build already read the mutated file
```

The construction that looks safe is `cargo test ... ; cp original back` — unconditional, same
command, revert guaranteed. It bounds the mutation's lifetime **within the arming process** and
that is not the property that matters. `cargo test` returning is *the same event as* the lock
freeing, so the gap between the test finishing and the `cp` landing is not randomly placed
relative to a queued peer: **it is the precise instant they are waiting for.** Queueing does not
make a peer miss the window, it aims them at it.

The fix costs nothing — revert before the process exits, not after the test does:

```sh
cargo test ...; rc=$?; cp "$orig" "$target"; exit $rc
```

Raised by `8bd791df-5ff4-40fe-af30-69cc3fefc2f7`, who was the queued peer, from their own
process state (`pgrep -c rustc` = 0 while their run had been alive 16 minutes and silent for 13
— proof it had not compiled anything yet and would compile whatever the tree held when the lock
freed). Not inferred from a red they saw; derived before one could happen.


### 2026-09-14: the DIAGNOSIS above is right and the REMEDY under it does not follow

The timeline is correct and the mechanism is correct. The prescribed fix does not address it,
and it "costs nothing" because it changes nothing about the hazard.

| | ordering |
|---|---|
| criticised as merely looking safe | `cargo test … ; cp original back` |
| prescribed as the fix | `cargo test …; rc=$?; cp "$orig" "$target"; exit $rc` |

Those are the **same three steps in the same order**: test runs, test exits — *which is the lock
freeing* — then `cp`. `rc=$?` and `exit $rc` preserve an exit status the caller would otherwise
lose; neither moves the revert relative to the lock. Read the section's own timeline against its
own fix and they are the same sequence:

```
your cargo test exits        -> releases the shared build-directory lock   <- unchanged
peer's queued cargo acquires -> begins compiling CURRENT source            <- unchanged
your revert runs             -> too late                                   <- unchanged
```

**Why the wording hid it.** *"Revert before the process exits, not after the test does"* names
exactly the right property — but the difference between the two snippets is the exit-code
capture, not that property, and a reader checking the prose against the code finds a sentence
that is true of neither sample over the other. The prose describes a fix; the diff delivers a
tidier failure.

**And the property is not reachable by reordering.** You cannot revert before your own process
frees the lock, because `cargo test` returning *is* that event — which the section already says.
So no arrangement of `;` closes it. The window is a property of **mutating bytes in a worktree a
peer's build can read**, and only two things reach that:

- **Do not mutate the shared worktree.** A git worktree or a copy removes the window entirely
  and costs a cold `target/` — real money on this repo, and an operator's call, not a free swap.
- **Accept the window and make the red legible**, which is this file's marker direction and its
  actual answer.

So the honest state is that the window is **open**, not fixed, and the marker is carrying more
weight than the file currently says.

**Instance, this session, arming the mutation described in `61d92a7f`'s record.** I ran the
criticised form — `cargo test`, an `echo`, then `cp` — having read this section, and believed I
was compliant because the revert was unconditional and in the same invocation. Those are the two
properties the prescribed snippet has. Neither is the one that matters. Five peer sessions were
live in this checkout at the time.

**What DID hold, reported as a denominator rather than absorbed as a catch:** the other half of
this file's guidance worked exactly as written. The patcher asserted its pattern occurred exactly
once before writing (`assert t.count(old)==1`), so a silent no-op was impossible; the expected
post-revert count was written down **before** the command ran; and the conclusion rested on an
observed **kill**, not a survival, so by this file's own triage rule (*a mutation's GREEN is two
propositions, a mutation's RED is one*) no applied-ness audit was owed at all. Three prescriptions
followed, three held. The one that failed is the one whose remedy does not follow from its
diagnosis.
## A mutation that never applied is indistinguishable from one that survived

A second failure in the same session, and the one with the worse blast radius, because it
corrupts the *result* rather than a bystander's reading of it.

A patch script stored a literal source string; `cargo fmt` reformatted the target's match arms
between the string being written and the patch being run; the patch no longer matched. The test
run that followed compiled **unmutated** source and reported:

```
AssertionError                <- the patcher, three lines up
test result: ok. 178 passed
[exited with code 0]
```

`178 passed` is exactly what *"the mutant survived, this guard is untested"* looks like. The
conclusion it invites — go write the missing test — is wrong, and the test already exists. With
a bare `str.replace()` instead of an assertion the patcher would have no-opped **silently** and
produced the identical output with no traceback at all.

**The general rule, and it is a triage rule rather than a checklist** (`8bd791df`, whose
formulation this is): **a mutation's GREEN is two propositions, a mutation's RED is one.** A red
cannot be produced by a mutation that never applied, since the unmutated tree was green a moment
earlier. So a conclusion resting on an observed kill needs no applied-ness check, and only a
conclusion resting on a **survival** does. *Audit your greens, not your reds.*

When a survival is load-bearing, verify applied-ness at the bytes, either side, independent of
what the test runner says:

```
patcher asserts the pattern occurs exactly once     (a bare replace is the silent failure)
grep the mutated file   -> expect 0 of the original
grep after the revert   -> expect 1
```

**And the reformat race is specific to this repo rather than incidental**, which is why it
belongs in the record: step 1 of the mandated four-command gate formats Rust, every session runs
it, and `fmt-mine.sh` formats whatever `file-provenance.py` attributes to the caller. So any
mutation script here that stores a literal source string has a live race against **any peer's
gate step 1**, on a practice `CLAUDE.md` actively mandates. Store a pattern that survives
reformatting, or re-read the target immediately before patching.

One more from the same hour, same family, caught the cheap way: a post-revert check grepped a
function *name* and expected 3 call sites, got 16, because `grep -c` counts lines mentioning the
name including doc comments. Harmless only because the number came back too large. It was caught
because the expected value had been written down **before** the command ran — which is the
whole technique, and costs one word.

## Tests added

None, and none is possible from inside the arming session: the state under test is *another
session's reading* of a transient tree. What could be tested is a marker mechanism, once one
exists.

## References

- `docs/issues/2026-09-08-a-claimed-bug-file-names-the-author-of-the-wip-that-reds-the-build.md` —
  the routing sibling. Same substrate, different missing bit.
- `docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md` — the
  write-side of the same shared tree.
- `docs/trackers/issue-clusters/IC-12-transient-shared-state-lies-to-readers.md` — the class.

## Attribution

Observed by sessionId `59112612-5fc8-4b31-8c8c-e19220d99eac`, who reported the red rather than
repairing it and then supplied the outside-view account above — including that every step they took
was correct. Armed by `5399543d-22d6-4ed9-9ebb-876be459989f`, who could not have known it was being
read.

Filed by the arming session because the observer **structurally cannot write it**: the whole
content of the class is that the distinguishing signal exists only inside the session that armed
the mutation, and the outside view is precisely the half carrying no information.
