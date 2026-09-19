---
id: '3d7e38fd20fac6b8'
kind: bug
status: open
title: 'BUG: file provenance answers at session grain, so N parallel subagents are one writer and fmt-mine.sh cannot refuse any of them'
owners:
- marius
tags:
- cluster/attribute-derived-at-container-granularity
topic: authorship attribution under subagent fan-out
---

## Summary

`scripts/file-provenance.py` attributes an uncommitted file to a **sessionId**. Every subagent a
session spawns runs under its parent's `CLAUDE_CODE_SESSION_ID`. So when one session fans out to N
parallel agents writing N disjoint file sets, provenance answers *"session 48d1f0c8 wrote it"* for
all of them — true, and useless, because the decision every consumer makes needs to know **which
agent**.

The guard whose entire purpose is *"do not touch a writer you are not"* is therefore blind to
exactly the writers closest to you. Peers on other sessions are protected. Siblings inside your own
are not.

## Symptom (Effect)

Two consumers, two different wrong answers:

- **`scripts/fmt-mine.sh`** reported *"formatted 5 file(s) written by this session"* while the
  invoking agent had written **three**. The other two belonged to sibling agents still mid-edit. Its
  refusal branch — the one that names an owner's sessionId and the socket to ask them on — cannot
  fire, because the sessionId genuinely matches.
- **`attribute-red.py`'s `wip_authors`** line reports *"written by THIS session — likely yours"* for
  a file the reading agent never opened. That is the misattribution class `CLAUDE.md` § *Observer
  Blindness* exists for, arriving through the instrument built to prevent it.

## Reproduction

Observed 2026-09-19 during a six-agent bug-fix campaign in this checkout, session
`48d1f0c8-9f60-43bb-a15e-17ec7995813a`:

1. Dispatch N ≥ 2 subagents, each editing a disjoint set of `.rs` files.
2. Have one of them run `./scripts/fmt-mine.sh`.
3. It formats **every** modified `.rs` in the tree, not that agent's three.

## Environment

Claude Code subagents inherit the parent's `CLAUDE_CODE_SESSION_ID` — confirmed by
`fmt-mine.sh`'s own count exceeding the invoking agent's file set while its refusal branch stayed
silent.

## Root cause

Authorship is derived at **session** granularity and consumed at **agent** granularity. The two
coincided as long as a session had one writer, which was true for every prior measurement of this
mechanism. Fan-out breaks the identity, and nothing in the instrument records that it has.

This is the fleet's own blind spot turned inward. `file-provenance.py` exists because
`git diff --stat` names insertions and names no author; it fixed that at the granularity the
problem then had. A subagent is a new kind of writer the scheme has no column for.

**Note what does NOT fail:** the answer is never *wrong* in its own terms. Every file really was
written under that sessionId. There is no error, no refusal, no anomalous value — the instrument
returns a true sentence at a granularity coarser than the question. That is why it reads as working.

## Evidence

- `fmt-mine.sh` self-reported count (5) vs the invoking agent's actual file set (3), same run.
- The two files it swept — `src/tools/config/mod.rs` and `src/tools/rendezvous.rs` — were owned by a
  different agent and dirty at that moment.
- Harm observed: none. rustfmt is idempotent and semantics-neutral, and a syntactically invalid
  intermediate state fails rather than being rewritten. **The severity here is the lost REFUSAL, not
  a corrupted byte** — the guard's protective branch is unreachable for this class of writer, so
  the next consumer of provenance that is not idempotent inherits an unguarded path.

## Hypotheses tried

- *"fmt-mine.sh's attribution logic is wrong."* FALSIFIED — it is correct at its own granularity;
  the file genuinely belongs to that session. Nothing in the script is misreading anything.

## Fix

Not fixed. Candidate directions, neither costed:

1. **Give provenance an agent column.** Requires a per-agent identifier the writer can stamp;
   whether the harness exposes one is unverified and is the first thing to check.
2. **Make the session-grain answer say so.** Cheaper and strictly honest: when a session has
   multiple concurrent writers, have `fmt-mine.sh` and `wip_authors` report *"session-grain
   attribution; N writers active under this sessionId"* rather than *"likely yours"*. This does not
   restore the refusal, but it stops the instrument asserting a precision it does not have —
   which is the half that actually misleads.

Direction 2 is the one that matches `CLAUDE.md` § *Observer Blindness* position 3: a check that
runs when nobody is worried, rather than a rule someone must remember.

## Tests added

None.

## Workarounds

An agent that wants to format only its own files can run `rustfmt --edition 2021 --check` against
the paths it wrote, bypassing provenance entirely. A dispatcher can assign disjoint file sets — which
this campaign did, and which is why the sweep was harmless rather than a collision.

## Resume

Open. Locus: `scripts/file-provenance.py` (attribution), `scripts/fmt-mine.sh` (consumer),
`scripts/attribute-red.py` (consumer). Check first whether the harness exposes a per-agent id at
all; if it does not, direction 2 is the whole available fix.

## References

- `CLAUDE.md` § *Reaching a Peer Session* — "Never route by adjacency"; the provenance instrument is
  the prescribed alternative, and this is its boundary.
- `CLAUDE.md` § *Observer Blindness* — the party structurally unable to see this is the agent
  holding the sessionId, which is every agent involved.
- `docs/issues/archive/2026-09-09-the-documented-gates-first-command-rewrites-every-peers-uncommitted-rust.md`
  — the incident `fmt-mine.sh` was built for, at peer granularity.
