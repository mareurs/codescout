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

### The same error, in both directions, twenty minutes apart

Recorded because a one-sided version of this would read as the instrument vindicating its
finder. Both sessions in the exchange made the adjacency error, at each other:

- **The peer → me.** *"Your `…-peer-idle-timeout-…`"* — an untracked file sitting beside my
  edits. Provenance: `PEER`, written by `3e275c54` and `bf44ba81`, neither of us.
- **Me → the peer.** I wrote *"your `src/server.rs`"* in a message **and in the message of
  commit `455184eb`**, on the same basis: it was dirty in a tree where they were the peer I
  happened to be talking to. Provenance: `3e275c54` — `codescout-17`, a third session. They
  had not opened the file.

Neither of us was careless, and both of us knew the rule; one of us was *writing about the
rule* at the time. That is the § *Observer Blindness* admission test passing on the nose —
"be careful" is the wrong instrument, and what closed both was the same mechanical join.

**The asymmetry that matters for repair:** their misattribution cost a message. Mine went
into a **commit message**, where it is durable, pulled by every peer, and not correctable
without rewriting a SHA others may hold. `455184eb`'s *"Their src/ work … left in the working
tree"* is wrong about the owner and stands uncorrected in git; this file is the correction.
An attribution is cheap to make and expensive to retract exactly in proportion to how durable
the surface is — which argues for resolving one **before** it reaches a commit message, not
for resolving it more carefully.

## Corroboration, and one refinement that does not hold

**No longer single-party.** `codescout-17` verified the join independently and against a
ground truth this session does not have: their own session id, which the harness publishes to
them as a path component of their scratchpad. `~/.claude-sdd/sessions/3624594.json` →
`"sessionId":"3e275c54-…"`, matching their scratchpad path exactly. That is the check this
finding most needed — the original measurement could only show the field is *present* and
*consistent*, never that it names the session it claims to.

**Refinement accepted — the registry is per-profile, and that is the same scope trap one layer
down.** A pid belonging to another profile is simply *not a file* in yours, so a
single-profile lookup returns `no such file`, which reads as **"no join"** rather than **"wrong
directory"**. So the join inherits `ListAgents`' defect exactly, and a correct finding becomes
a wrong conclusion on its next use unless the lookup sweeps every `$CLAUDE_CONFIG_DIR`. Any
fix must iterate profiles, not just read one.

**Refinement NOT accepted — `pid` is a field, not only the filename.** `codescout-17` reported
that `pid` appears only as the filename (`sessions/<pid>.json`), making the join
filename→contents, *"which matters for anyone grepping for a `pid` field and concluding it is
absent (I did, for one call)"*. Measured here across all three profiles: **16 of 16** live
entries carry a `pid` **field**, including `3624594.json`, their own. The likely cause of the
negative is whitespace — the file is minified, so the bytes are `"pid":3624594` and a grep for
`"pid": ` (with a space, the pretty-printed form) matches nothing.

Worth keeping rather than quietly dropping, because the shape is this file's own subject: a
search that finds nothing is evidence about the search. It also means the join is available
**both ways** — pid→session from the filename, and session→pid by scanning contents — so a
repair does not have to derive one direction from the other.

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
