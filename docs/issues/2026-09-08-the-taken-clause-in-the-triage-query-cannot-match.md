---
id: '2f2aec31efcaf47a'
kind: bug
status: open
title: 'BUG: the triage query''s `taken` clause cannot match — an enforced field nobody writes reads as a collision check that performs none'
owners:
- marius
tags:
- cluster/assertion-that-cannot-fail
topic: bug claiming and cross-session collision avoidance
closed: ''
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
`claimed_by: 59112612-…`, `claimed_at: 2026-09-08`. **Cited as commit `e1e79b89`**, which
is immutable and holds that exact frontmatter — deliberately not by path, because that bug
is being fixed and archived within the hour, and `docs/issues/…` → `docs/issues/archive/…`
would stale a path citation on the move. The artifact id (`2b61de99742ee1d3`) is no better:
`id = sha256(abs_path)`, so it re-keys on the same move. The commit is the only one of the
three that survives. So the corpus's **second** write of the field landed inside the window this record
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

Not started. The options below were rewritten once § *Root cause* was corrected — the
original recommendation was a documentation move, which the correction falsifies.

- **A — ~~move the write step to the read surface~~. FALSIFIED, do not retry.** The
  claiming protocol is already delivered unprompted, in full, to sessions running the
  triage query. Two of them then did not claim. Republishing it on a second surface changes
  the one variable already shown not to matter.
- **B — a mechanism at the point of claiming.** The behaviour that worked was *addressed,
  specific, at the moment of picking work up*. The tool-side analogue is for the act that
  begins work on a bug to carry the claim, or to ask — e.g. `doc(action="update")` moving a
  bug toward an in-progress state offering the `claimed_by` stamp, or the triage query's
  own response naming unclaimed rows in the second person. This is SKF-22's remedy shape:
  replace a trigger the model must notice with one it cannot miss.
- **C — make the zero name its own scope.** Per
  `docs/adrs/2026-08-27-negative-results-name-their-scope.md`, a `taken` count of 0 in a
  repo with live peers is a suspicious zero and should say what it examined. Weaker than B
  and complementary to it: it repairs the *reader's* inference without changing the
  *writer's* behaviour.
- **D — document the clause as decorative** and route collision-checking to the socket
  enumeration, which is the instrument that actually worked. Defensible, and it discards a
  built, tested mechanism over an adoption gap; it also gives up the cross-machine case
  sockets cannot reach.

**Recommendation: B, with C in the interim.** B is the only option addressing the corrected
root cause. C degrades honestly while B does not exist.
## Tests added

None. Note that the obvious test — asserting the ledger has at least one `taken` — would
be a test of the corpus rather than of the code, and would red whenever the repo happened
to be quiet. Any guard here belongs on the *instruction surfaces*, in the family of
`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`.

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
