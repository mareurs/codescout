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

**The read instruction and the write instruction live on surfaces with different
audiences, and only one of them is unavoidable.**

| half | surface | reached by |
|---|---|---|
| READ `taken` before starting | `CLAUDE.md`, activation bootstrap | every session, at startup, unavoidably |
| WRITE `taken` when you start | `get_guide("tracker-conventions")` § *Claiming a bug* | a session that opens that guide |

The write protocol is complete and correct where it is documented — it gives the exact
`doc(action="update", …)` call, says to store the sessionId and not the name, and
specifies release to `investigating`. It is simply on a surface a session reads only if it
goes looking, while the read half is served to everyone at activation. Readers therefore
outnumber writers structurally, and the enforcement layer only ever gets an empty
population to check.

This is `CLAUDE.md` § *Observer Blindness*, "the third position", applied to a protocol
rather than to a number: **a bound published to an audience that does not read it.** The
remedy named there is to move the requirement to the read surface, not to publish it
again.

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
and before this file was committed — `codescout-92` (sid `59112612-…`) claimed
`docs/issues/2026-09-05-doc-update-body-appends-a-trailing-blank-line-every-write.md`:
`status: taken`, `claimed_by: 59112612-…`, `claimed_at: 2026-09-08`, written through the
catalog. So the corpus's **second** write of the field landed inside the window this record
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

Not started. Options:

- **A — move the write step to the read surface.** The activation bootstrap and
  `CLAUDE.md` already tell every session to *check* `taken` at Phase 0; have the same
  sentence tell it to *claim* what it picks up. This is the § *Observer Blindness* remedy
  and the only option that changes who receives the instruction.
- **B — make the zero name its own scope.** Per
  `docs/adrs/2026-08-27-negative-results-name-their-scope.md`, a suspicious zero should say
  what it examined. A `taken` result of 0 across a repo with live peers is exactly such a
  zero, and the triage surfaces could say so rather than returning silence.
- **C — document the clause as decorative** and route collision-checking entirely to the
  socket enumeration.

**Recommendation: A, plus B.** A fixes the audience problem; B degrades honestly in the
interim and while adoption is partial. C is listed because it is defensible — the socket
route is strictly more reliable — but it discards a working, tested mechanism over an
adoption gap, and gives up the cross-machine case that sockets cannot reach.

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
