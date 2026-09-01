---
kind: bug
status: open
tags:
- cluster/doc-contradicted-by-code
- peer-sessions
- authorship
- provenance
closed: null
opened: 2026-09-01
owner: marius
related:
- docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md
- docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md
severity: medium
---

# `CLAUDE.md` denies a pid→session join that every live registry entry carries

**Found:** 2026-09-01, while routing a shared-checkout commit question.
**Affects:** `CLAUDE.md` § *Observer Blindness* (final sentence of the authorship
paragraph), and `scripts/file-provenance.py`, which stops one join short of an address.

## Summary

`CLAUDE.md` closes its authorship procedure with:

> Neither `/proc/<pid>/environ` nor the transcript JSONL carries a session id, so there is
> **no pid→session join**; this is a **session→id** join the session publishes on request.

The premise is true and the conclusion is false. Both named sources genuinely lack a session
id — but `<config-dir>/sessions/<pid>.json` carries `sessionId`, `pid`, `messagingSocketPath`
and `name` **in one record**, so the join is not merely available, it is a complete four-way
one, offline, with nothing to ask.

The cost is not cosmetic. The paragraph's own remedy is *"ask the session and have it quote
its scratchpad path"* — a round trip that spends a peer's turn — and it is presented as the
best available identifier precisely because the cheaper one is declared not to exist.

## Symptom (Effect)

`scripts/file-provenance.py` names writers as **session ids**:

```
SHARED    docs/trackers/issue-clusters.md
          window: writes at or after 2026-09-01T12:40:35+00:00
          written by THIS session (b2a50de8)
          written by 6524892b-e096-4c65-b3a5-89cdc9cd49ed
```

A session id is not addressable. `SendMessage` takes a name or a socket path, so under the
documented procedure the id is a dead end and the next step is to broadcast to a candidate
set and ask. `CLAUDE.md` records what that costs: a 2026-09-01 broadcast reached two of five
peers, **both innocent**, while the two real writers sat among the sessions `ListAgents`
omits.

## Reproduction

Measured 2026-09-01 on this machine, 16 live sessions across 3 profiles:

```bash
# the join the doc says does not exist
python3 -c "
import json; d=json.load(open('$HOME/.claude-sdd/sessions/2601241.json'))
print(d['pid'], d['sessionId'], d['name'], d['messagingSocketPath'])"
# 2601241 6524892b-e096-4c65-b3a5-89cdc9cd49ed codescout-b7 /run/user/1000/cc-socks/2601241.sock
```

Universality, rather than one lucky record:

```bash
# live registry entries: 16   carrying sessionId+socket+name: 16
```

Both `/proc/<pid>/environ` and the transcript JSONL were re-checked and do lack it, so the
doc's premise is not the error — only the inference from it.

## Worked demonstration — the same session, 20 minutes later

The routing question that surfaced this produced three unknown writer ids across two files.
All three joined to live, named, addressable peers with **nothing asked of anyone**:

| session id | name | pid | profile | reachable as |
|---|---|---|---|---|
| `3e275c54` | `codescout-17` | 3624594 | `.claude-sdd` | `uds:/run/user/1000/cc-socks/3624594.sock` |
| `bf44ba81` | `codescout-8a` | 3261760 | `.claude` | `uds:/run/user/1000/cc-socks/3261760.sock` |
| `6524892b` | `codescout-b7` | 2601241 | `.claude-sdd` | `uds:/run/user/1000/cc-socks/2601241.sock` |

Two profiles, so a per-profile sweep would have resolved at most two of the three — the join
inherits the scope rule that governs every other peer instrument here, and the enumeration
must span `$CLAUDE_CONFIG_DIR` profiles exactly as the socket sweep does. Enumerated on this
host: `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`.

**And the join earned its keep on the first use.** A peer, coordinating in good faith,
attributed an untracked bug file to this session — *"your `…-peer-idle-timeout-…`"* — on
adjacency alone. `file-provenance.py` returned `PEER`, naming `3e275c54` and `bf44ba81`:
neither this session nor the peer making the claim. Under the documented procedure the
correction stops there, at *"not mine, and I cannot say whose"*. With the join it completes —
both writers are named, live and one message away. That is the difference between refuting an
attribution and repairing one.

## Root Cause

The sentence generalises from two sources to a namespace. Both are **process**-level
artifacts, and the session id lives in the **registry**, which is the one surface the
authorship procedure never reads for this purpose — although the peer-enumeration skill
already opens exactly these files, for `name` and `status`, and simply does not read the
adjacent `sessionId` field.

So this is `IC-11` (*documentation denies a capability the code has since gained*) with an
`IC-18` flavour: the selector (*"process-level sources"*) is narrower than the population it
was taken to cover (*"anywhere a session id might live"*).

## Impact

Routing on a shared checkout is the corpus's most expensive recurring failure, and this
removes the last hop. With the join, the sequence

> enumerate over sockets → intersect with the write-derived set → **ask** the survivors

becomes

> enumerate over sockets → **join** the write-derived id to a live socket → address the writer

Asking survives as the fallback for a writer whose session has exited (no `/proc`, possibly no
registry entry). That is a real residue and the reason this is not a total replacement: the
join covers **live** writers only, which is the common case precisely because
`file-provenance.py` is windowed to recent writes.

## Fix

Two parts, neither started.

1. Correct the `CLAUDE.md` sentence. It should say the join exists in the registry and is
   per-profile, so it must be swept across all profiles like every other peer instrument —
   the same scope rule the surrounding paragraph already establishes.
2. Teach `scripts/file-provenance.py` to resolve each session id it prints to
   `name`, `pid`, `profile`, `socket` when a live registry entry exists, and to say
   *"session exited — ask is unavailable"* when it does not. That converts the script's output
   from evidence into an address, and it is the § *Observer Blindness* mechanism shape: it runs
   whenever provenance is run, with nobody having to remember this file.

## Notes

**Not** filed as a bug in the harness. The registry contents are exactly right; the defect is
that our own documented procedure asserts they are not.

**Observer note.** The blind party is the author of the authorship procedure, who had just
finished measuring that `/proc` and the transcripts lack the id — the strongest position from
which to believe the negative, and the one with no reason to open a third file. Found here
only because a routing question forced a *positive* identification, and the registry was
already open for `name`. That is the paragraph's own law holding on the paragraph.
