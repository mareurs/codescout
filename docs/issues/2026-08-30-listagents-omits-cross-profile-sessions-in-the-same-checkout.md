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
unverified: Root cause is now CONFIRMED bidirectionally rather than inferred — the omitted session reported its own ListAgents view and it is reciprocally blind. What remains unread is the harness source, which is not in this repo, so the mechanism (profile-scoped registry) is named from behaviour rather than from code.
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
