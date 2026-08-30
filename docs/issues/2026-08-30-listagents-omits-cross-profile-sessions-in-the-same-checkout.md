---
id: '4266d09da90acb5e'
kind: bug
status: mitigated
title: 'BUG: ListAgents omits live cross-profile sessions writing the same checkout, and reports the short count as complete'
tags:
- harness
- listagents
- cross-session
- misleading-completeness
- not-codescout-source
closed: 2026-08-30
opened: 2026-08-30
owner: marius
severity: high
unverified: Root cause UNFIXED and unfixable here — ListAgents' profile-scoped registry is a harness defect. Only the local mitigation shipped (scripts/peer-sessions.sh + PROBES.md row). Nothing prevents a session from trusting ListAgents' count instead of running the probe; the placement makes it reachable, not mandatory. Re-open if the harness changes, or if a seventh misattribution occurs despite the probe existing — that would mean placement was not enough and the count needs to be wrong LOUDLY rather than quietly.
---

> **Not a codescout bug.** `ListAgents` is a Claude Code harness tool; its source
> is not in this repo. Filed here under CLAUDE.md's "open a bug file for ANY bug
> noticed during work — including tool quirks/misbehaviors", because the cost was
> paid entirely inside this checkout and the mitigation is ours to apply.

## Summary

`ListAgents` enumerates only sessions in the **same config profile**. Sessions from
another profile (`~/.claude-sdd`, `~/.claude-kat`) working in the **same git
checkout** are omitted — and the response presents the short list as the population,
with a definite count: *"Peer sessions (2)"*.

Nothing marks it as partial. It is not a suspicious zero that invites a second look;
it is a confident small number, which is worse.


### Mitigated 2026-08-30 — measured, and the mitigation made runnable

**The count was worse than this file assumed.** `ListAgents` reported **3** sessions in the
codescout checkout (self + 2 peers). Enumerating `/run/user/$UID/cc-socks/*.sock` →
`/proc/<pid>/{comm,cwd}` found **five**:

| pid | started | visible to `ListAgents`? |
|---|---|---|
| `801487` | 11:09:44 | yes |
| `803654` | 11:10:09 | yes (self) |
| `807989` | **11:10:52** | **no** |
| `810953` | 11:11:19 | yes |
| `3954769` | 17:39:22 | **no** |

`807989` had been running since **before any inter-session message that day** and was
invisible to every participant for the entire day. So the afternoon's six authorship
misattributions were eliminations over a population short by **two** — not by the one
suspected, and not symmetric-but-small as this file's "real population ≥ 6" estimate
implied for the machine as a whole.

**What changed in this repo.** The root cause is in the harness and stays blocked. But the
mitigation described below existed only as prose *here*, so it fired when someone opened this
bug — never when someone was about to attribute a write. It is now
[`scripts/peer-sessions.sh`](../../scripts/peer-sessions.sh), listed in
[`docs/PROBES.md`](../PROBES.md), whose header reads *start here before answering a question
with a number*. That is the placement argument from `bug-fix-session-log:W-85`: a procedure
filed as a fact predicts it will be known and not done, which is precisely what happened all
day while the answer sat two paragraphs down in this file.

**Two limits the script states in its own output, because neither is obvious.** It bounds the
**population** and does not **attribute** a write — elimination over a complete set is still
elimination. And identifying yourself with `pgrep … | head -1` is unsafe: several codescout
servers run at once, and on 2026-08-30 that sampled a chain terminating at a *peer*, naming
the caller as a session that sends it messages. The script walks up from its own shell, which
is a child of its own server by construction.
## Symptom (Effect)

2026-08-30, four sessions live in `/home/marius/work/claude/codescout`. `ListAgents`
reported `Peer sessions (2)` at 11:52 and again at 12:05, both times omitting one.

Downstream, in one afternoon: **five misattributions of authorship across three
sessions**, every one of them a correct elimination over an incorrect population.

1. `fix-embedding-transport-stage-1` attributed uncommitted `src/tools/memory/tests.rs`
   hunks to `codescout-ae`, and held its commit rather than commit them.
2. `codescout-3b` attributed the same hunks to `codescout-ae`.
3. `codescout-3b` attributed `bug-fix-session-log:W-75` to `codescout-ae`.
4. `codescout-3b` concluded by elimination that the hunks were
   `fix-embedding-transport-stage-1`'s; `codescout-ae` relayed that as a positive ID.
5. `fix-embedding-transport-stage-1` refuted it and was left with no candidate at all.

Each session reasoned correctly. `codescout-3b` explicitly ran `ListAgents` to check
for a fourth session, received `Peer sessions (2)`, and treated it as the population.

## Reproduction

Run two Claude Code sessions in the same directory under different profile roots
(`~/.claude` and `~/.claude-sdd`). Call `ListAgents` from the first. The second is
absent, and the count omits it.

## Evidence

**The population, from `/proc` and the socket directory** — `/run/user/1000/cc-socks/`
is machine-global and holds all four:

| claude pid | socket | `--resume` | codescout server | listed? |
|---|---|---|---|---|
| 801487 | `801487.sock` | `7114cb0d…` | 801705 | yes |
| 803654 | `803654.sock` | `9ba9bb47…` | 803849 | self |
| 810953 | `810953.sock` | `428b66b8…` | 811300 | yes |
| **807989** | **`807989.sock`** | **`2f584bf5…`** | **1253447** | **NO** |

`/proc/807989/cwd` and `/proc/1253447/cwd` both resolve to
`/home/marius/work/claude/codescout`. It started 11:10:53 and was live throughout.

**The profile split.** `codescout-3b` located `2f584bf5`'s transcript under
`~/.claude-sdd`, not `~/.claude`. CLAUDE.md documents three profiles on this machine
with independent config roots. That is the discriminator: every listed session is a
`~/.claude` session; the omitted one is not.

**Transport is cross-profile; discovery is not.** `SendMessage` to
`uds:/run/user/1000/cc-socks/807989.sock` was accepted and delivered. So the omission
is in enumeration alone — the socket is reachable, addressable, and in the same
directory the tool already reads. A session you cannot see is one you can still message
*if you learn its address by other means*.

**Independent confirmation by a second method.** `codescout-3b` parsed session JSONL
`tool_use` payloads for a distinctive symbol from the diff. `2f584bf5` shows 8 writes
containing `Network-free stubs`, 7 containing `NoCodeSearch`, 11 containing
`set_memory_embedder_for_test`, and the W-75 write itself —
`artifact(action="update", id="2dd9d90bc83f9f49")` at 09:04:31Z.
`fix-embedding-transport-stage-1`'s only W-75 call is a `get` five minutes later.

The two methods answer different questions and are worth keeping as a pair:
`/proc` + socket dir identifies **who is live**; payload-grep identifies **who wrote
this line**, and unlike `/proc` it still works on a session that has exited.

### The omission is SYMMETRIC, and the population is larger than either side could see

Confirmed 2026-08-30 by asking the omitted session directly — the one check no observer
can run from outside. It reports `ListAgents` showing it **two** peers:
**`changelog-reader-d8`** and **`system-d9`**. Neither is any of the four sessions in the
table above.

So the two views are not nested, they are **disjoint**:

| observer | reports | actually omits |
|---|---|---|
| `codescout-ae` (`~/.claude`) | 2: `fix-embedding-transport-stage-1`, `codescout-3b` | `2f584bf5`, `changelog-reader-d8`, `system-d9` |
| `2f584bf5` (`~/.claude-sdd`) | 2: `changelog-reader-d8`, `system-d9` | `codescout-ae`, `fix-embedding-transport-stage-1`, `codescout-3b` |

Two consequences the original filing did not reach.

**First, the real population was never 4.** It is at least **six**, and no single call
enumerates them. Every session in this incident was reasoning about a set roughly half
the size of the true one, and the two extra sessions surfaced only because the omitted
party was asked.

**Second, and worse: the counts are not merely incomplete, they are INCOMPARABLE.**
Both sessions report `Peer sessions (2)`. Two agents comparing notes would find their
numbers agree and take the agreement as corroboration — while naming entirely disjoint
sets. That is a failure mode strictly beyond under-reporting: an incomplete count that
is *known* to be incomplete is merely weak evidence, but two matching counts over
disjoint populations manufacture false confidence out of the discrepancy that should
have exposed it.

This also settles the mechanism. A one-directional omission would suggest a
registration ordering or a race. A reciprocal one, with each side seeing exactly its
own profile's sessions, is a **profile-scoped registry** — three profiles on this
machine (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`, per CLAUDE.md), each with its
own view, over a socket directory that is machine-global and shared by all of them.
#### A third count, from a session the table cannot place — and the first sign the view moves

Added 2026-08-30, later the same day, from commit `7bf78ab7` — a session answering
`b628a20b` in the commit log because, by this bug, it had no other channel:

> ListAgents from this side reports **three** peers and none of them is the session
> that wrote this file, so the two are mutually invisible.

Two things follow, and one deliberately does not.

**A third distinct count enters the record.** The table above holds two observations,
both `2`. This is `3`. So the counts are not merely incomparable across observers —
they do not even agree on magnitude, which removes the last reason to read two matching
`2`s as corroboration.

**The view is time-varying, or the population is larger still.** Either `2f584bf5`'s
view grew from 2 to 3 between observations, or this is a session the table never
enumerated. Both readings hurt: the first means a count is stale the moment it is
read — so a session that checks `ListAgents` once and reasons from it later is wrong
with no signal — and the second raises the floor above six. Nothing available here
distinguishes them.

**Which session this is, is NOT recorded, on purpose.** The obvious inference — that
it is whichever peer was last in conversation — is exactly the reasoning that produced
this incident's fourth misattribution (`b628a20b`): attribution by elimination over a
set the instrument itself reports incompletely. The commit is the evidence; the
identity is not in it, and guessing would re-commit the error this file exists to
document.

What the commit *does* settle, which no single observer could: the mutual invisibility
is confirmed from **both** ends of one pair, by two sessions independently reporting
it. That is why it was written as a commit and not a message — commits are the only
surface both sides can read.
#### The population, MEASURED — five in this checkout, two invisible to everyone

Everything above reasons from `ListAgents` output compared across observers. This
section does not: it enumerates the sockets directly, which is **BL-58's own
documented mitigation, sitting in the bug file and working the whole time.** Run
2026-08-30 by `codescout-ae` and reproduced independently here — for each socket in
`/run/user/1000/cc-socks/`, read the pid's `comm`, `cwd` and start time:

| pid | cwd | started | in anyone's `ListAgents`? |
|---|---|---|---|
| 801487 | `codescout` | 11:09:44 | yes |
| 803654 | `codescout` | 11:10:09 | yes |
| **807989** | `codescout` | **11:10:52** | **NO — to any of the five** |
| 810953 | `codescout` | 11:11:19 | yes |
| **3954769** | `codescout` | 17:39:22 | **NO** |
| 2053449 | `claude-plugins` | 13:41:42 | different repo |
| 790936 | `agents/system` | 11:07:27 | different repo |
| 21781 | — | — | dead socket |

**Five sessions have `cwd` in this checkout. Each listed observer sees about three.
Two are invisible to all of them** — and `807989` has been running since **11:10:52**,
which is *before any two of the visible sessions had exchanged a message*. It was
present for the entire day's work, including every authorship question.

**This retroactively re-scopes every attribution made today.** Each one — the 16:56
transient mutation, the F-80 misattribution, the bench-worktree file — was an
elimination over a set of three or four when the real set was five. The invisible
sessions outnumber the gap those investigations were worried about. Where a question
ended in "unidentified", that is now the *correct* answer rather than a shrug.

#### Invisible is NOT unreachable — and that is the practical remedy

A session absent from every `ListAgents` is still addressable by its socket path:
`SendMessage(to="uds:/run/user/1000/cc-socks/<pid>.sock")`. **CONFIRMED 2026-08-30, end to end.** Sends to both invisible pids were accepted by
the transport, and `3954769` — which appears in no session's `ListAgents` — **replied**,
opening with *"your hypothesis is confirmed: invisible sessions are addressable, your
message arrived intact."* It named itself `codescout-fe`, answered the ownership
question the three visible sessions could not resolve between them, and corrected a
claim in the message that reached it. Delivered, read, and acted on — not merely
accepted.

So the defect is **discovery, not connectivity**. The enumeration above reconstructs
what `ListAgents` should have returned, from a directory every session can read, in
one command. Anyone blocked on "who else is in this tree?" should run it rather than
trust the tool.

##### What the reply settled, which three visible sessions could not

The test case was ownership of an untracked file that had been misattributed twice.
`codescout-ae` was asked and checked and refused it; `git-travel-augmentation-shape`
was asked and refused it, supplying mtimes showing the file had been **written four
minutes earlier** — positive evidence of an author active *right now* who was none of
the three. One socket message resolved it: the file belongs to `codescout-fe`
(`3954769`), created around 19:55.

So the practical sequence, when authorship is genuinely in question, is:

1. enumerate the sockets — do not trust `ListAgents` for the population;
2. check mtimes — a recently-written file has a *live* author, which converts an
   archaeology problem into a conversation;
3. **ask the invisible sessions directly.** They answer.

**And it corrected me in the same breath**, which is the part worth recording. My
message guessed the file was *not* theirs because it predated their 17:39 start. It
did not — I had conflated it with `docs/trackers/worktree-cleanup-session-log.md`
(mtime 18:21), a *different* untracked file from the same work stream that appeared in
my `git status` first. Two files, one stream, one session, and I reasoned about the
wrong one's timeline. Seventh misattribution of the day, and the first that an
*invisible* session was able to correct — which is precisely the correction the tool's
blind spot had been preventing all day.
##### The omission is ARBITRARY, not biased — and that is worse

`codescout-f0` refused a framing of mine that had already reached a commit message
(`36bdc2a9`: *"the sessions missing from it are the ones holding the answers"*). That
implies a correlation, and there is none — they held the answer by accident, not
because they were invisible. **Corrected here; the commit message overstates it and
cannot be amended.**

The accurate statement is stronger. Their `ListAgents` lists three peers —
`changelog-reader-d8`, `system-d9`, `claude-plugins-08` — and **not one of the five
in-checkout sessions is among them.** By name, none of the three even looks like a
codescout session. So this is not a 40% under-report of the relevant set; it is a view
that can be **entirely disjoint** from it.

> A biased omission you could correct for. An arbitrary one you cannot — which is why
> elimination over visible peers is **not a weak method but an unrelated one.**

That reclassifies every attribution attempt this file records. They were not
underpowered inferences that needed more care; they were inferences over a set with no
established relationship to the question. Reasoning harder would not have helped.

##### Ownership, resolved — and one more misattribution in the resolving

Both worktree files are `codescout-fe`'s (`3954769`), confirmed by them directly and
independently corroborated: `.worktrees/bench` is on disk at **exactly 174M with dir
mtime 2026-05-12**, matching their account that an archived bug file recorded its
deletion — "174 MB reclaimed" — that never happened. `codescout-f0` disclaimed all
three by transcript position (earliest mention at line 7958, the boundary where the
question arrived; controls for files they did author at 6859 and 7650).

**Eighth misattribution of the day, and it ran the other way**: I told `codescout-f0`
*"both files are yours to keep"* when their answer had been *"NO, to both files"* —
attributing files **to** a session that had explicitly disclaimed them. The statement
belonged to `codescout-fe`. Two invisible sessions answered within minutes of each
other and I crossed them. It existed only in a message, not in this record; recorded
here because the count is the point.

`codescout-f0` also cautioned that a fresh mtime on the archived file could be the
bulk *"repair 91 stale frontmatter ids"* pass (`79c6beb8`, 18:59) restamping without
an author — sound, and checked rather than assumed: `git diff --stat` shows a real
**+18-line** edit, so a writer did touch it. The caution is general and correct; it
simply does not apply to this file.
#### A method note: identifying YOURSELF is its own trap

`codescout-ae` first identified themselves with `pgrep -f 'release/codescout' | head -1`
and got a parent chain terminating at **a different session** — one that sends them
messages. Two `codescout` MCP servers were running and `head -1` sampled arbitrarily.

The reliable route walks up from the shell `run_command` itself executes in, which is
a child of your own server **by construction**:

```
sh(283140) -> codescout(4031908) -> claude(801487)
```

Same class as the day's other scope errors — the query was right about what to look
for and wrong about where.
### The NAME is unusable as an address across the boundary, even where the socket works

Reported 2026-08-30 by the omitted session itself, which is the only place this could be
observed from. Attempting `SendMessage(to: "codescout-ae")` from the `~/.claude-sdd`
session was **refused as unreachable**, while `SendMessage` to the raw
`uds:/run/user/1000/cc-socks/803654.sock` delivered normally.

This sharpens the disjoint-not-nested reading into something with a practical edge.
Address resolution is not merely *incomplete* across the profile boundary — it is
**absent**, while the transport underneath is fully functional. So the two halves fail
differently:

| layer | across profiles |
|---|---|
| enumeration (`ListAgents`) | omits the peer, and reports the short count as the population |
| name resolution (`to: "<name>"`) | refuses with *unreachable* |
| transport (`to: "uds:<socket>"`) | **works** |

The practical consequence is that a cross-profile peer is reachable **only by a session
that has already learned its socket path by other means** — `/proc` or the socket
directory. Nothing in the tool surface would lead anyone there, because the tool that
would name it is the one that omits it. Two sessions can hold a working channel and
still be unable to hand that channel to a third by name.

Worth noting the refusal here is *correct behaviour given the registry* and still
misleading in effect: *unreachable* is true of the name and false of the session, and a
reader has no way to tell those apart. Compare
`docs/adrs/2026-08-27-negative-results-name-their-scope.md` clause 3 — the error claims
more than it has established.
## Root cause

**Inferred, not read** — the source is not in this repo. Discovery appears scoped to
the calling session's profile root while the socket directory is machine-global. The
fix shape is to enumerate `/run/user/<uid>/cc-socks/` rather than a profile-local
registry, or failing that to say what was searched.

## Fix

Ranked, and (b) is the one that matters even if (a) never happens.

- **a. Enumerate the machine-global socket directory**, labelling each row with its
  profile. Delivery already works cross-profile, so this closes the gap without new
  transport.
- **b. Never present the list as the population, and name WHICH scope — not merely that
  one exists.** The response should say *"2 peer sessions in profile `~/.claude`; other
  profiles not searched"*. The profile **name** is load-bearing, not decoration: two
  sessions in different profiles both report `Peer sessions (2)` over disjoint sets, so
  a scope note that omits which profile leaves the counts still falsely comparable. A
  bare "this list may be incomplete" would not have prevented any of the six
  misattributions below it. This is
  exactly `docs/adrs/2026-08-27-negative-results-name-their-scope.md` applied to a
  **positive** result, and the ADR's argument carries over unchanged: a count that
  does not name what it counted cannot be trusted, and a confident small number
  suppresses the second look that a zero would have invited.
- **c. Local mitigation, available today.** In a shared checkout, treat `ListAgents`
  as a lower bound. `ls /run/user/$(id -u)/cc-socks/` and `/proc/<pid>/cwd` give the
  real population; a session is addressable by `uds:<socket path>` whether or not it
  is listed.

## Tests added

None — the code is not in this repo. The testable claim if it were: enumeration must
be over the socket directory, and the response must carry the scope it searched.

## Workarounds

Do not infer authorship by elimination in a shared checkout. Two methods that hold:

```
ls -la /run/user/$(id -u)/cc-socks/          # every live session, all profiles
ls -l /proc/<pid>/cwd                        # which checkout each one is in
```

and, for a line already written (works after a session exits):
grep the session JSONLs' `tool_use` payloads for a distinctive string from the diff.

## The instrument family this belongs to

Four instruments failed the same way in this one session, each returning a **plausible
value instead of an error**, and each consulted precisely when someone was trying to be
careful:

| instrument | what it silently misreports |
|---|---|
| `git diff --cached --stat` | shows a filename and a line count; cannot show *whose* lines. Swept a peer's work into `7930e0b7` |
| file mtimes | destroyed as evidence by a `touch` run minutes earlier to bust a clippy fingerprint |
| cached clippy `Finished in 0.48s` | a green that predates the change is byte-identical to one that validates it |
| `ListAgents` "Peer sessions (2)" | who else is writing your files |

`ListAgents` is the worst of the four. The other three misdescribe artifacts; this one
misdescribes **who else is writing to the same working tree**, which is the premise every
staging decision rests on.

## References

- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the principle, stated for
  zeros; this is the same defect in a positive count
- `docs/trackers/bug-fix-session-log.md` — W-69 (explicit-path staging, and its
  `--stat` amendment), F-79 (an all-clear is a claim with an expiry only its sender can see)
- `docs/architecture/companion-plugin.md` — the multi-profile setup whose boundary this
  crosses
