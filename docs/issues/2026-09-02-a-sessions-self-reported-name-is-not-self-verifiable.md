---
id: '76bb1a07806f8020'
kind: bug
status: open
title: 'BUG: a session''s self-reported name is not verifiable from inside the session, and the positive-identification procedure does not distinguish that from its sessionId'
tags:
- cluster/authorship-unrecoverable-after-the-fact
closed: null
opened: 2026-09-02
owner: marius
severity: medium
unverified: no fix and no test — the defect is in a prescription in CLAUDE.md, which is heavily contended on this checkout, and no test is obviously available since the observation depends on a session's registry name changing under it. Root cause is established; the remedy is a one-clause doc amendment nobody has made.
---

# BUG: a session's self-reported name is not verifiable from inside the session, and the positive-identification procedure does not distinguish that

## Summary

`CLAUDE.md` § *Observer Blindness* prescribes closing an authorship question by asking the
session, "because the harness makes the session id a path component — so the id is *given*,
not inferred." That is sound for the **sessionId**. It is **not** sound for the **name**: a
name is minted into a per-profile registry, compaction and resume mint a new one, and nothing
re-informs the running context. So the documented positive identifier returns a robust answer
for one field and a decayable one for the other, and the procedure does not say which is
which — while names are what sessions actually quote at each other.

## Symptom (Effect)

A peer replied from PID 3805470 and signed off as **codescout-26**. Measured at the time:

```
$HOME/.claude/sessions/3805470.json
  name=codescout-17
  sessionId=9716a130-c93d-4a65-9ab2-ddc53d6d9cfb
```

No live session was named `codescout-26`. The session that held that name was **PID 3661589
on `.claude-kat`** — a different profile — alive in a 10:03 socket enumeration and gone by
11:40. So a session signed as a dead session on a profile it was not running in, and did so
while correctly owning a separate mis-attribution in the same message.

The peer confirmed from its own registry entry and withdrew the signature. It also reported
that `bug-fix-session-log:F-97` credits "codescout-26" for a catch that was sessionId
9716a130's.

## Reproduction

Not deterministically reproducible on demand — it requires a session whose registry name has
changed under it (compaction, resume, or a re-mint) and which then reports its own name from
context. The observation above is the record.

What **is** reproducible is the asymmetry that makes it possible:

```
# robust: the id is a path component, so the session can read it off its own scratchpad path
/tmp/claude-*/<project>/<session-id>/scratchpad

# decayable: the name lives only here, and the running context is not re-informed on change
$CLAUDE_CONFIG_DIR/sessions/<pid>.json  ->  "name": "..."
```

## Environment

Shared checkout `/home/marius/work/claude/codescout`, three `CLAUDE_CONFIG_DIR` profiles
(`.claude`, `.claude-sdd`, `.claude-kat`), 7 sessions in this checkout at the time of the
observation, membership churning within 40 minutes.

## Root cause

Two identifiers with different provenance, presented by the procedure as one:

- **sessionId** — structural. The harness makes it a component of the scratchpad path, so a
  session can read it off its own filesystem. Cannot drift.
- **name** — registry-derived. Minted into `$CLAUDE_CONFIG_DIR/sessions/<pid>.json`; a
  compaction or resume mints a new one and the running context keeps whatever it last believed.

`CLAUDE.md` § *Observer Blindness* justifies "ask the session" with the path-component
argument, which is true only of the id — and then every session in practice quotes its
**name**, because that is what `ListAgents`, `SendMessage` and the socket table all display.
So the procedure's justification and its actual use are attached to different fields.

Measured 2026-09-02: registry entry read for PID 3805470; absence of any live
`codescout-26` confirmed across all three profiles' `sessions/*.json`; PID 3661589's
`/proc` entry confirmed gone.

## Evidence

### The failure is silent in the direction that matters

Routing is unaffected — the socket PID delivered the message correctly regardless of the
signature, and a `from=` attribute copied verbatim always reaches the right session. What
decays is **attribution**: every claim of the form "session X did Y" that keys on a name
carries this error mode, including three such claims made to an operator during the same
evening and corrected afterwards.

### It is a refinement of IC-10 rather than a new class

IC-10's claim is that on a shared checkout there is no attribution channel, so authorship is
inferred from proximity. This file sharpens it: the **positive** channel the corpus recommends
as the escape from proximity-inference is itself partially decayable. The remedy is not "ask"
versus "infer" — it is *ask for the sessionId, not the name*.

### Adjacent instances the same evening, for base rate

Three authorship claims were made and corrected in about two hours: a bug file crediting
`codescout-0a` for `codescout-69`'s finding; a citation-ownership claim routed to this session
by adjacency (both files dirty, work nearby, `file-provenance.py` and the socket enumeration
available and unused); and this signature. **Knowing the class prevented none of them** — one
was committed by an author who had written about the class forty minutes earlier, which is
`OB-1`'s point.

## Hypotheses tried

1. **Hypothesis** — a typo in the sign-off.
   **Test** — read the registry entry for the sending PID; searched all three profiles for a
   live session with the signed name; checked `/proc` for the PID that formerly held it.
   **Verdict** — rejected. The name belonged to a real, different, now-dead session on another
   profile.

2. **Hypothesis** — the socket table is wrong.
   **Verdict** — rejected. The PID → profile → registry chain is consistent and delivery worked
   throughout. Only the self-report disagreed.

## Fix

Not implemented. Two parts, and the second is the one that matters.

**Documentation.** `CLAUDE.md` § *Observer Blindness*'s positive-identification sentence
should name the **sessionId** rather than leaving "the id" to be read as the display name, and
say plainly that a name is not self-verifiable from inside the session. The scope table in §
*Reaching a Peer Session* is already correct about routing; this is about attribution.

**A mechanism, per that section's own third position.** Asking a session to quote its
scratchpad path yields the sessionId structurally — that is already the robust form and needs
only to be the *prescribed* form. Whether anything can make a name self-verifying is a harness
question and probably out of scope; the cheaper move is to stop attributing by name.

Do **not** attempt to make names stable — that is the harness's to decide, and the corpus's
own advice (positively identify, do not eliminate) works fine once it points at the right
field.

## Tests added

None, and it is not clear a test is available: the defect is in a prescription, and the
observation depends on a session's registry name changing under it. Recorded rather than
excused.

## Workarounds

**Attribute by `sessionId`. Route by socket PID. Never use a name for either.** Three tiers,
not two — and this file's first version got that wrong, see below.

| identifier | lifetime | good for |
|---|---|---|
| `sessionId` | survives restart, profile change, compaction, resume | **attribution** |
| PID / socket path | dies on restart — shorter than the session | **routing only** |
| name | re-minted by compaction, resume, or a restart under another profile | neither; a display label |

Reply by copying a message's `from=` attribute verbatim. When quoting a peer's identity to a
third party, state the field: `sessionId 9716a130`, not `codescout-17`.

> **Corrected 2026-09-02, within an hour of filing, and the correction is the same shape as
> the bug.** This section first read *"attribute by **sessionId** or **socket PID**, never by
> name"* — pairing the two as if equally durable. A peer supplied the counterexample from its
> own restart, verified here: sessionId `953b5e77-b804-4956-9198-b3ac8696b4c9` moved from
> **PID 2299153, profile `.claude-kat`, name `codescout-00`** to **PID 1720384, profile
> `.claude`, name `codescout-cc`** when its operator restarted the CLI under a different
> profile and the harness resumed the same conversation. PID 2299153 is dead and its registry
> entry is gone; 1720384 is alive with the **same sessionId**.
>
> So a PID is stable only *within a process lifetime*, which is a boundary shorter than the
> session and one this file published without. **The consequence runs the useful way:**
> `.git/session-stage-log` and `scripts/file-provenance.py` are both keyed on sessionId, so
> that peer's rows kept attributing to it correctly across the restart. Only *addressability*
> broke — anyone holding the old socket path or the old name had a dead handle. Attribution
> survived precisely because it does not use the two fields that decayed.
>
> Anything that assumes pid↔session is stable for the life of a sessionId has a live
> counterexample: one sessionId, two PIDs, two profiles, hours apart.
## Resume

Amend `CLAUDE.md` § *Observer Blindness*'s "the id is *given*, not inferred" sentence to name
the sessionId explicitly and to mark the name as decayable — one clause, and the wording is
in § *Fix*. Check whether § *Reaching a Peer Session*'s "a session can quote its own id from
its scratchpad path" needs the same disambiguation; it is arguably already correct, since it
names the path.

Filed rather than fixed because `CLAUDE.md` is heavily contended on this checkout and the
sentence is load-bearing for a procedure several sessions are actively following.

## References

- `docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md` and
  `docs/issues/2026-08-31-cross-account-agents-cannot-see-each-other.md` — the discovery/delivery
  scope split this sits on top of.
- `CLAUDE.md` § *Observer Blindness* (the sentence to amend) and § *Reaching a Peer Session*
  (correct about routing).
- `docs/trackers/observer-blindness.md` `OB-1` — knowing the class prevents no instances.
- Observation and withdrawal by sessionId 9716a130; the field-provenance diagnosis was its
  reply to this session's measurement.
