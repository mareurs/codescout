---
id: '4266d09da90acb5e'
kind: bug
status: open
title: 'BUG: ListAgents omits live cross-profile sessions writing the same checkout, and reports the short count as complete'
tags:
- harness
- listagents
- cross-session
- misleading-completeness
- not-codescout-source
closed: null
opened: 2026-08-30
owner: marius
severity: high
unverified: Root cause (profile-scoped discovery over a machine-global socket dir) is inferred from the evidence below, not read from harness source — ListAgents is Claude Code, not codescout, so the code is not in this repo. The symptom, the population, and the cross-profile delivery are all directly measured.
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
- **b. Never present the list as the population.** The response should name its scope
  — *"2 peer sessions in profile `~/.claude`; other profiles not searched"*. This is
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

