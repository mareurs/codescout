---
id: '2b9ddd39f99d98cc'
kind: bug
status: fixed
title: 'BUG: the triage query''s `taken` clause cannot match — an enforced field nobody writes reads as a collision check that performs none'
owners:
- marius
tags:
- cluster/assertion-that-cannot-fail
topic: bug claiming and cross-session collision avoidance
closed: 2026-09-08
opened: 2026-09-08
owner: marius
related: []
severity: medium
---

## Summary

The canonical bug-triage query — prescribed by `CLAUDE.md` § *Querying active trackers*,
by `get_guide("project-activation-bootstrap")` Phase 0, and by
`get_guide("tracker-conventions")` § *Querying with the librarian* — asks for
`status in ["open", "taken", "investigating", "zombie"]`, and all three surfaces explain
the `taken` clause the same way: *"a live session holds it — check before starting"*.

Measured 2026-09-08: **79 `open`, 0 `taken`, 1 `investigating`** (the latter an unrelated
2026-08-31 record). The `taken` clause cannot match. A session that follows the documented
procedure exactly is told nothing is held, regardless of how much is held — and the step
reads as a collision check while performing none.

**This is not a missing mechanism, and fixing it as one would waste the work.** The
enforcement half is built, wired and tested: `scan_claim_liveness` is called from the
doctor scan at `src/librarian/tools/doctor.rs:508`, emits four outcomes
(`claim_held_by_live_session`, `claim_held_by_dead_session`, `claim_unresolvable_here`,
`claim_without_claimant`), and the decay branch alone carries five distinct assertions
pinning five different details. What is missing is **writers**.

## Symptom (Effect)

```
doc(action="find", kind="bug", filter={"status": {"in": ["taken", "investigating"]}})
  -> 1 row, and it is `investigating`, from 2026-08-31, unrelated

doc(action="find", kind="bug", status="open")
  -> 79
```

A `taken` set that is empty is indistinguishable, at the point of use, from a `taken` set
that is empty *because nothing is claimed*. Both return zero. The query's answer to "is
anyone on this?" is "no" in every state of the world.

## Reproduction

Live near-miss, 2026-09-07, two sessions in this checkout:

1. `codescout-92` (sid `59112612-5fc8-4b31-8c8c-e19220d99eac`) began a bug-fixing session
   and did the correct thing — asked peers for collisions *and* planned to pick a disjoint
   bug from the ledger.
2. At that moment `codescout-af` (sid `89d91024-…`) was actively fixing **five** bugs:
   `252fe84782103842`, `ceb1f7a11823d71f`, `9a5c069bd5eef463`, `9ea0a90867c85260`,
   `bf5e57977f5b6af9`.
3. All five read `[open]`. Ledger-based disjointness would have selected one of them.

The collision was avoided by a peer message over a socket, not by the ledger.

## Environment

Shared checkout, 5 live sessions across 1 profile at 2026-09-07T15:47, 3 of them in
`/home/marius/work/claude/codescout`.

## Root cause

**The instruction reaches its audience in full, through the documented surface, and is
insufficient. What works is being addressed in the second person with a named next action.**

This supersedes an earlier reading in this file, kept below because it is the theory a
reader arrives with and would otherwise act on.

`get_guide("tracker-conventions")` is **auto-injected** — unprompted, as a full content
block — on a `doc(action="find", kind="bug", …)` call, which is precisely the triage query
every session is told to run at Phase 0. It carries § *Claiming a bug* complete: the exact
`doc(action="update", patch={status: "taken", extra: {claimed_by: …}})` call, the rule to
store the sessionId and not the name, and the release path.

Two sessions received it and neither acted on it:

- `codescout-92` (sid `59112612-…`) — self-reported, and the party's own report on itself is
  the weakest evidence available; they said so first and flagged the guide injection as the
  checkable part.
- **The author of this file** (sid `ad379a7c-…`) — independent corroboration, and the
  reason the point is not resting on a self-report. The same guide auto-injected into this
  session, on the very `doc(action="find")` call that produced the `79 / 0 / 1` measurement
  above. It was read. Nothing was claimed. A bug was fixed and archived the previous day
  without a claim either.

So the audience is not the variable. In both cases the behaviour changed only when a peer
addressed the session directly and named the action — which is
`skill-frictions:SKF-22`, *a trigger the model must notice is a policy, not a mechanism*,
reaching a third subsystem.

### Superseded: the audience-split reading

This file originally located the defect in **where** the two halves are published:

| half | surface | reached by |
|---|---|---|
| READ `taken` before starting | `CLAUDE.md`, activation bootstrap | every session, unavoidably |
| WRITE `taken` when you start | `get_guide("tracker-conventions")` | *— believed opt-in; it is not* |

**It is preserved because it is wrong in the expensive direction.** It prescribes moving
the claiming instruction onto the always-served surface — a documentation edit, cheap,
plausible, and measurably insufficient, since the instruction is *already* served
unprompted to the sessions that then did not claim. Anyone re-deriving the audience theory
would ship that edit and observe no change, with nothing to tell them why.
## Evidence

**The mechanism works end to end — adoption is the whole gap.** Written exactly once in
this corpus's history, on 2026-09-07, by `codescout-92`, at the prompting of another
session rather than by following the docs. `librarian(action="doctor")` then reported:

```
claim_held_by_dead_session:  0
claim_held_by_live_session:  1
claim_unresolvable_here:     0
claim_without_claimant:      0
```

resolving the sessionId to pid, current name and cwd, printing the `SendMessage` address a
peer would need, labelling itself *"informational, not a defect"*, and closing with *"Use
the id, not the name."* The claim was released by completion when the bug was fixed and
archived. So the field went 0 → 1 → 0 within a day, correctly, and the count today is 0
again.

**The count moved while this file was being written, and that is a datum rather than an
embarrassment.** At ~08:28 on 2026-09-08 — minutes after the `79 / 0 / 1` above was taken,
and before this file was committed — `codescout-92` (sid `59112612-…`) claimed a bug through the catalog: `status: taken`,
`claimed_by: 59112612-…`, `claimed_at: 2026-09-08`. **Cited as commit `e1e79b89`, patch-id
`26bfbb2d429c0d52aba8db728a3acb7739829cf5`** — the pair, not the SHA alone. It holds that
exact frontmatter.

The three candidate forms and what each survives:

| form | survives the archive move? | survives a rebase? |
|---|---|---|
| path `docs/issues/…` | no — becomes `docs/issues/archive/…` | n/a |
| artifact id `2b61de99742ee1d3` | no — `id = sha256(abs_path)`, re-keys on the move | n/a |
| commit SHA `e1e79b89` | yes | **no** — positional, dies on the next rebase |
| SHA **+ patch-id** | yes | yes — content hash of the diff |

**The lesson is in how the first three were chosen, and it is this file's own subject
again.** The question asked was *"which form survives the archive move?"*, and the commit
is the right answer to it — correctly reasoned, correctly excluding the other two. It was
then published as though it answered *"which form is durable?"* A rebase of this checkout
was in flight at that moment (`ahead 8, behind 3`), and `CLAUDE.md` says in as many words
that the SHA *"is positional and dies when `experiments` is rebased (which happens after
every ship)"*. **A citation form is durable against a named event, never in general** — and
the narrow answer, published unqualified, reads exactly like the general one.

Re-find it after any rewrite with the documented redirect form (Iron Law 3 blocks the
pipe):

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/ids.txt
grep 26bfbb2d /tmp/ids.txt
```

Patch-id derived here rather than copied from the peer who supplied it; both readings
agree byte-for-byte.

**Confirmed end-to-end by the event it was written against, 2026-09-08T09:56** — published
because a re-derivation that *confirms* is a denominator, and absent confirmations are what
make a population look self-correcting (`CLAUDE.md` § *Testing Discipline*).

The rebase landed roughly forty minutes after the amendment. Measured across it:

| commit | before | after |
|---|---|---|
| the cited claim `e1e79b89` | on branch | **orphaned** |
| this file's four commits | on branch | **orphaned**, rewritten |
| `bd3d0973`, `4012dcd7`, `488a959e` (already pushed) | on branch | **unchanged SHAs** |

So the discriminator is *pushed vs unpushed*, not age. And the recovery ran clean:
`grep 26bfbb2d` over the `git patch-id --stable` index returned **exactly one** hit,
`35674093`, reachable from HEAD, same subject. Zero ambiguity.

**A dead SHA that still `git log`s is the trap worth naming.** `git log -1 e1e79b89`
succeeded *after* the rebase and printed the right subject — an orphaned object resolves
until garbage collection, so the check most people reach for reports success on a commit
that is no longer on any branch. `git merge-base --is-ancestor <sha> HEAD` is the check
that separates them.

**The SHA above is deliberately NOT being updated to `35674093`.** Chasing it is the
follow-up the pair convention exists to abolish — *"record the pair once at fix time…
nothing is owed later"* — and the new SHA would be orphaned by the next rebase anyway. The
patch-id is the half that resolves; the SHA is a human-readable convenience that is
expected to rot. So the corpus's **second** write of the field landed inside the window this record
describes, and the headline number is already `1`, not `0`.

What that does and does not change. It does **not** refute the finding: both writes to date
came from one session that had been told about the gap by a peer, not from a session
following the documented surfaces — which is the audience problem in § *Root cause*, intact.
It **does** establish that adoption is cheap once the instruction reaches someone, that the
read and write halves compose correctly in practice, and that this file's own
§ *Resume* instruction to re-derive rather than cite was worth writing: it paid inside
twenty minutes.

A reader arriving later should expect the number to have moved again in either direction,
and should treat a non-zero `taken` as the question *"did the audience change, or was the
writer told?"* rather than as the finding being closed.

**Cluster tag, and the runner-up rejected with cause.** Tagged
`cluster/assertion-that-cannot-fail` (`IC-16`): the defining property measured here is that
the clause *cannot match*, which is the class's claim exactly.

`IC-2` (`gate-keyed-on-unobservable-event`) was considered and rejected — **do not retag it
there without addressing this.** IC-2's claim is that a gate substitutes a proxy *because
the event is unobservable*. Here the event is perfectly observable: the socket enumeration
in `CLAUDE.md` § *Reaching a Peer Session* answers "who holds this checkout right now" in
one call, and is what actually prevented the collision above. The proxy is not standing in
for an unobservable event; it is an observable event nobody records.

## Hypotheses tried

- *Is the check missing?* No — refuted at `doctor.rs:508` plus five test assertions on
  `claim_held_by_dead_session` (`:14801`, `:14837`, `:15024`, `:15080`, `:15207`).
- *Is the write protocol undocumented?* No — `get_guide("tracker-conventions")` §
  *Claiming a bug* is complete, including the sessionId-not-name rule and the release call.
- *Is `taken` simply unused because sessions do not collide?* No — three sessions shared
  this checkout on 2026-09-07 and five bugs were held concurrently.

## Fix

Fixed at `a31c0197`, patch-id `4fe255c39c131bac6510e28924dc5bbedded8e24` (label:
**`experiments`**), by sessionId `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30`.

**Shipped option B — a mechanism at the point of claiming, and documented in the code as
the SECOND-best shape rather than sold as the best.** `doc(action="find", kind="bug", …)`
now returns `hints.claimable` carrying the caller's **own** sessionId, already substituted,
beside the ids it applies to. `SessionRegistry::resolve_self` resolves that id from the pid
of the server's parent — the server is spawned by the session it serves, so `getppid` names
it. Same registry route `CLAUDE.md` documents for identifying a peer from the socket its
message arrived on, pointed inward, with the same caveat that the row is self-asserted.

Why it is second-best, stated plainly because the distinction is the finding: § *Observer
Blindness* ranks *"make the correct path end in a safe state"* above *"an unconditional
policy tied to a trigger that happens anyway"*. The first rung is **unavailable** — no tool
call means *"I am starting work on bug X"*, so nothing can carry the claim as a side
effect. This is the second rung, and calling it a mechanism would overstate it.

**What it adds over the guide that failed.** `get_guide("tracker-conventions")` already
delivers the complete protocol, unprompted, on this exact call. It asks the reader to
notice a general rule applies, recall their sessionId, and compose a call. This collapses
all three into a literal, pre-filled line beside the specific rows.

**Declining is the load-bearing behaviour.** Zero ppid (`rendezvous::parent_pid`'s Windows
sentinel), no matching row, or two rows disagreeing on a reused pid all yield `None`, and
the hint degrades to a visible `<your-session-id>` placeholder plus the scratchpad route.
A wrong sessionId would be stamped into `claimed_by`, making `taken` name the wrong
session — this defect inverted, and worse, because a false claim stops the next reader
asking the peer directly.

**A — ~~move the write step to the read surface~~. FALSIFIED, do not retry.** See
§ *Root cause*. **C** (name the zero's scope) and **D** (declare the clause decorative)
were not taken; C remains a reasonable complement if adoption stays low.

**Auto-stamping `claimed_by` on any catalog edit was rejected, not overlooked.** It is the
first-rung shape and it fails on a real case: a session annotating a bug it is not working
— adding a cross-reference, correcting a count — would silently claim it. This file's own
author annotated an unrelated archived bug the previous day while fixing something else.
## Tests added

Eleven, all in the default lane, read out by name rather than inferred from a total.

**`src/librarian/session_registry.rs` — four on `resolve_self`,** three of which assert it
*declines*: zero ppid, no matching row, and two rows disagreeing on one pid. The Windows
one pins a contract nothing was checking — `rendezvous::parent_pid` returns `0` there and
its own comment argues that is safe because zero *"degrades to never-matched rather than to
a WRONG match"*, which holds only if consumers honour it. `resolve_self(0)` is now asserted
`None` **even against a row literally storing pid 0**, so the producer's stated reasoning
has a consumer-side proof.

**`src/librarian/tools/find.rs` — five on shape, two on REACHABILITY.** The split is the
point: `cargo build` emitted `function claim_hint is never used` while all five shape tests
were already green. That is `cluster/declared-not-wired`, and § *Testing Discipline*'s
*"an alarm nothing reaches is exactly as informative as no alarm"*. Deleting
`a_bug_page_carries_the_claim_hint_through_the_real_call_path` would let the wiring be
removed with five green tests still vouching for it.

The **negative** reachability test earns its place equally:
`a_non_bug_page_carries_no_claim_hint`. A hint attached to every response is furniture, and
furniture is unread — which reproduces this defect one layer out.

Gate green 2026-09-08 at `a31c0197`: FMT=0, CLIPPY=0, LEAN=0 (3633 tests), DEFAULT=0 (5613
tests, 0 failures, 1760 `librarian::`).

**Verified end-to-end against a running MCP server, 2026-09-08**, by sessionId
`59112612-5fc8-4b31-8c8c-e19220d99eac` after a `cargo rb` + `/mcp` reconnect — which is
the exact re-check condition the `unverified:` field named, so the field is now cleared.

`doc(action="find", kind="bug", filter={status in [open, taken, investigating, zombie]})`
returned `hints.claimable` carrying that session's **real** sessionId, not the placeholder:
`resolve_self` took none of its three declining paths. The verifier cross-checked the id
against two sources independent of this code — their scratchpad path component and
`CLAUDE_CODE_SESSION_ID` — and all three agreed.

**Scope of that evidence, stated because it is narrower than "verified":**

- **Happy path only at runtime.** No case was constructed in which `resolve_self` *should*
  decline, so the three declining branches have unit coverage and no runtime exercise. That
  gap is small by construction — those branches turn on registry state, not on the MCP
  boundary the field was doubting — but it is not zero, and the verifier named it unprompted.
- **One session, not two.** This file's author could not reproduce it: their own server
  process still predates `a31c0197`, and the same query from that session returns no
  `hints.claimable` at all. Which is itself a small confirmation — the hint appears exactly
  where the fix is compiled in and nowhere else.
- **The binary's mtime is NOT the proof.** The release build (10:44:52) postdates
  `a31c0197` (10:33:48), and that is corroboration only: commit time records when someone
  committed, not when the code existed, so the comparison is invalid in general. It was
  invalid earlier the same day pointing the *unfavourable* way, and pointing favourably
  does not repair it. The runtime output is the proof.

Eleven tests, all in the default lane, read out by name rather than inferred from a total.

**`src/librarian/session_registry.rs` — four on `resolve_self`,** three of which assert it
*declines*: zero ppid, no matching row, and two rows disagreeing on one pid. The Windows
one pins a contract nothing was checking — `rendezvous::parent_pid` returns `0` there and
its own comment argues that is safe because zero *"degrades to never-matched rather than to
a WRONG match"*, which holds only if consumers honour it. `resolve_self(0)` is now asserted
`None` **even against a row literally storing pid 0**, so the producer's stated reasoning
has a consumer-side proof.

**`src/librarian/tools/find.rs` — five on shape, two on REACHABILITY.** The split is the
point: `cargo build` emitted `function claim_hint is never used` while all five shape tests
were already green. That is `cluster/declared-not-wired`, and § *Testing Discipline*'s
*"an alarm nothing reaches is exactly as informative as no alarm"*. Deleting
`a_bug_page_carries_the_claim_hint_through_the_real_call_path` would let the wiring be
removed with five green tests still vouching for it.

The **negative** reachability test earns its place equally:
`a_non_bug_page_carries_no_claim_hint`. A hint attached to every response is furniture, and
furniture is unread — which reproduces this defect one layer out.

Gate green 2026-09-08 at `a31c0197`: FMT=0, CLIPPY=0, LEAN=0 (3633 tests), DEFAULT=0 (5613
tests, 0 failures, 1760 `librarian::`).
## Workarounds

Ask. The socket enumeration in `CLAUDE.md` § *Reaching a Peer Session* plus a direct
`SendMessage` is what worked on 2026-09-07, and it is the only instrument that returned a
true answer. Treat a ledger `taken` count of 0 as "unknown", never as "nothing is held".

## Resume

Start at § *Root cause* — the fix is an audience change, not a mechanism. Before doing
anything else, **re-derive the counts**: this file's `79 / 0 / 1` is a fact about
2026-09-08T08:2x, and the whole point of the record is that the population moves. If a
later reading finds a non-zero `taken`, that is the finding, and it changes the fix rather
than confirming it.

## References

- `get_guide("tracker-conventions")` § *Claiming a bug*, § *Status vocabulary*
- `get_guide("project-activation-bootstrap")` Phase 0
- `CLAUDE.md` § *Querying active trackers*, § *Observer Blindness*, § *Reaching a Peer Session*
- `src/librarian/tools/doctor.rs:508` (`scan_claim_liveness`), `src/librarian/tools/create.rs:91-95`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
