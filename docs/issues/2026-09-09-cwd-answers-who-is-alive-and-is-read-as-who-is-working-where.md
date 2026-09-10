---
kind: bug
status: open
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-09
owner: marius
related: []
severity: high
---

# BUG: the peer table's CWD column answers "who is alive" and is read as "who is working where"

## Summary

`/codescout-companion:reaching-peer-sessions` Step 1 prints a `CWD` column, read from
`readlink /proc/<pid>/cwd`. A session's **active project** is moved by
`workspace(action="activate")`, which does not change its process cwd — so a session can
be working inside a worktree while its row reports the main checkout. The column answers
*which directory the process was launched in* and is universally read as *where this
session is working*. On 2026-09-09 that misreading was one message away from licensing
`git worktree remove` on an occupied worktree.

## Symptom (Effect)

A cwd walk of all 13 live sockets at 06:10Z returned **zero** sessions inside
`.worktrees/doctor-per-project-isolation`. `b80a27d4-9729-40ef-8c28-ad8982df6d13`
(`codescout-87`) self-reports working in that worktree; its row reads:

```
872862  profile=.claude-sdd  cwd=/home/marius/work/claude/codescout
     sid=b80a27d4-9729-40ef-8c28-ad8982df6d13  name=codescout-87
```

Both facts are true simultaneously. Re-walked independently at 06:13:27Z by
`bf6a6925-f207-4a2f-8135-95e7563e859f`: same zero.

## Reproduction

Have a session `workspace(action="activate")` a worktree, then read
`readlink /proc/<its pid>/cwd`. It reports the launch directory, not the active project.
No error, no warning — the walk simply omits the session from that worktree's occupancy.

## Environment

Shared checkout with `.worktrees/`, several profiles, `cc-socks` per-user sockets.

## Root cause

**A proxy for an event outside the observing process's boundary.** "Which project is this
session working in" is *session*-scoped state held by the harness. A peer walking `/proc`
cannot see it, so it substitutes the one process-scoped fact that correlates —
`cwd` — and the substitution fails in this class's signature way: it returns a plausible,
well-formed directory rather than an error.

**The correlation is real, which is what makes it durable.** A session launched inside a
worktree does report that worktree, so the column is right often enough to be trusted, and
wrong exactly when `activate` has been used — which is the case where the answer matters.

**Widening does not fix it.** The natural repair on discovering a miss is to loosen the cwd
match from exact to prefix. That keeps the predicate and fixes nothing: the failing rows do
not have a cwd under the worktree at all, at any match strictness. There is no cwd-shaped
answer to this question.

## Evidence

### The near-miss it produced

`5399543d-22d6-4ed9-9ebb-876be459989f` was instructed to reclaim disk by removing merged
worktrees, and excluded `doctor-per-project-isolation` on the grounds that `b80a27d4` was
live in it. This session then told them, from the socket rows, that *"the one worktree you
are protecting has no session in it"* — a confident wrong conclusion drawn from a true
premise, sent to the party holding the destructive command. Retracted within the hour, and
only because the worktree's own occupant supplied the mechanism.

### Confirmed from the inside by an affected session

The mechanism above was inferred from `b80a27d4`'s self-report and a cwd walk that
disagreed with it. It is now confirmed directly:
`b0015a98-e290-46de-8ed1-3c94bc73a987`, working in `.worktrees/result-cap-marker-gate`,
states that `workspace(action="activate")` moved their **active project** to that worktree
and did not move their process cwd — *"a cwd walk classifies sessions by a property that
codescout's own workspace model decouples from the thing you are asking about."*

That upgrades the finding from *"two walks disagreed with one self-report"* to *"a session
inside the affected set confirms the decoupling."* Worth separating, because a self-report
about one's own state is what the walk was being used to overrule, and the two are not the
same claim: the first is a session saying where it is, the second is a session saying that
the instrument cannot see where it is.

### Deeper than "the cwd is stale": a session has no single cwd

The account above treats a session as having one cwd that `activate` fails to move. That
understates it. Reported by `bf6a6925-f207-4a2f-8135-95e7563e859f` from inside the affected
set: their own process chain is **four processes** — `sh` → `codescout` → `claude` → `bash` —
each with an independently settable cwd, and `workspace(action="activate")` moves none of
them.

So a cwd-keyed check does not read *the* session's directory; it reads **whichever process in
that chain the instrument happened to resolve**. The socket walk resolves the process holding
the socket, which is one specific link and not necessarily the one whose cwd anyone means.
That is why widening the match cannot help: the answer is not too narrow, it is a different
process's answer.

**Independent confirmation by a positive identifier, which is the part worth copying.**
`b80a27d4-9729-40ef-8c28-ad8982df6d13` established occupancy of
`.worktrees/doctor-per-project-isolation` — the tree a cwd walk twice reported empty — with a
**git object**: commit `fe7b6658`, trailer `Session-Id: b80a27d4-…`, contained in that branch
and no other. A commit trailer is minted by the session itself and lives in the object graph,
so it neither decays like a pid nor answers a different question like a cwd. `5399543d` has
retracted "unoccupied" on that worktree.

**Consequence for anything downstream of `file-provenance.py`**, including
`scripts/fmt-mine.sh`, which the gate now runs first: if a refusal names an owner via a
cwd-derived route, that is the layer to check before doubting the attribution. Provenance's
own authorship path reads transcripts rather than cwd, so it is not implicated — but the
distinction is exactly the kind that gets collapsed under time pressure.

### What the instrument IS good for

The same walk is the correct and only instrument for *who is alive and reachable* — it
found two occupants that `ListAgents` could not see, on a different profile, and their
socket addresses are what let the removal be routed to them as a question. The defect is
the column's reading, not the walk.

## Cluster adjudication — IC-2 over IC-18, PROPOSED and not yet settled

`bf6a6925-f207-4a2f-8135-95e7563e859f` independently drafted this defect under
`cluster/selector-narrower-than-its-population` (`IC-18`), argued by that class's own
discriminator — *could the mechanism, unchanged, report how many it missed?* A cwd walk
cannot, so the test genuinely fires. This file is tagged `IC-2`
(`gate-keyed-on-unobservable-event`). Both readings are defensible and **the risk is that
one fact is counted +1 in two classes, which is what drives promotion thresholds.**

**The discriminator proposed here is NARROWER versus DIFFERENT, with the remedy as the
test.** `IC-18`'s implied remedy is to widen the selector. That provably cannot apply: the
failing rows have no cwd under the worktree at any strictness, because a session's cwd set
is a different set from "sessions working here", not a subset of it. **A class whose remedy
cannot apply is the wrong class, even when its diagnostic test fires** — firing is not
applying.

`b80a27d4-9729-40ef-8c28-ad8982df6d13` checked that against the strongest counter-form
rather than accepting it: the best available cwd predicate, *"any process in the session's
tree has cwd under X"*, **would** find a worktree session via its tool shell — which makes
it look like a narrowing problem, and is presumably why `IC-18` was reached for. But that
shell exists only while a tool call is running, so between calls the property is
unobservable by any cwd at any strictness. The reading survives the strongest form of the
objection.

**Not settled here.** Routed to `bf6a6925` for overrule, since they hold the reproduction
and the four-process finding, which is the load-bearing half under either tag. If confirmed,
`IC-18`'s `**Members:**` should name this and say *"considered, routed to IC-2, deliberately
NOT counted here"* — the emitter/consumer split (the table that PRINTS cwd versus the gate
that DECIDES on it) is real and worth preserving, but as one instance with a cross-reference
rather than two counts. **Whoever resolves this: record the outcome in both files.** A
pending adjudication that is settled in a transcript and nowhere else is the failure this
whole file is about, one level up.

## Hypotheses tried

1. **Hypothesis:** the enumeration missed rows (a stale-socket or permissions gap).
   **Test:** two independent walks, different sessions, three minutes apart.
   **Verdict:** rejected — both returned the same zero, and the session in question is
   demonstrably live. The walk is complete and the column is answering a different
   question.

## Fix

Not applied — the change is to a **skill in another repo**
(`claude-plugins/codescout-companion/skills/reaching-peer-sessions/SKILL.md`) plus
`CLAUDE.md` § *Reaching a Peer Session*, and no codescout gate reaches a sibling checkout.
Options:

1. **Relabel the column** `LAUNCH-CWD`, and state in the skill that occupancy is not
   derivable from it. Cheapest, honest, and leaves the question unanswered.
2. **Ask, do not infer.** The rows carry a socket per session; "are you working in
   `<path>`?" is one `SendMessage` and returns an authoritative answer. This is the same
   shape as `OB-23`'s *ask, do not broadcast*.
3. **Publish the active project in the registry row.** `$CLAUDE_CONFIG_DIR/sessions/<pid>.json`
   already carries `sessionId`, `name`, `cwd`; an `activeProject` field would make the fact
   readable without cooperation. Not in this repo's gift.

**Before any worktree removal, occupancy must be established by (2) regardless** — the
other two are documentation.

## Tests added

None — the defect is in a markdown instrument in another repository, and no test here can
reach it. `provenance_probes_reference_only_real_tool_names` shows the crossing is
maintainable when the assertion is about names; this one is about a column's meaning.

## Workarounds

Ask each live session directly. Never conclude "this worktree is unoccupied" from a cwd
walk; the only safe negative is one a session gave you.

## Resume

Decide between the three options with the companion-plugin owner. If (1), the skill's Step
1 table and `CLAUDE.md` § *Reaching a Peer Session* must change together — the addressing
table there is what sessions actually follow.

## References

- `docs/issues/archive/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`
  — the adjacent defect one layer up: `ListAgents` cannot see the sessions at all, this one
  sees them and mislabels where they are
- `docs/trackers/observer-blindness.md` § OB-23
- Mechanism supplied by sessionId `bf6a6925-f207-4a2f-8135-95e7563e859f`, whose worktree
  was the one at risk
