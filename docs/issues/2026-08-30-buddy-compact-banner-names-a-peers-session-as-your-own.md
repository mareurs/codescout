---
kind: bug
status: open
tags:
- cluster/authorship-unrecoverable-after-the-fact
- companion-plugin
- buddy
- attribution
- misleading-instrument
- multi-session
closed: null
opened: 2026-08-30
owner: marius
related: []
severity: medium
unverified: 'Root cause not established — the hook source has not been read. What IS established (2026-08-31): the cited sid is a live session in a DIFFERENT config profile, so the hook is resolving ''from'' against something not scoped to the reader''s profile. Still reproducing.'
---

# BUG: the buddy compact banner's `from=<sid>` names another live session, reading as "your own pre-compaction transcript"

## Summary

On reload-after-compaction the buddy hook prints
`<!-- buddy:reloaded sid=<mine> from=<other> source=compact -->`. Measured
2026-08-30, `from=` named a **different, concurrently live session**, not this
session's predecessor. The line reads as "here is your own prior transcript",
so an agent investigating what it did before the compaction will attribute that
session's writes to itself. It nearly converted a correct denial of authorship
into a false confession.

Fix lives in `claude-plugins` (buddy), not this repo — filed here because it
bit a codescout session and this is where this session's bugs go, following the
precedent of `open-issue-work-queue:BL-51`.

## Symptom (Effect)

Emitted into this session's context at SessionStart:

```
<!-- buddy:reloaded sid=428b66b8-0b8c-48e5-84c6-754008e3ccc7 from=7114cb0d-4685-46db-a740-572dbe68113a source=compact -->
```

`7114cb0d` is not this session's predecessor. It is the live session
`fix-embedding-transport-stage-1`, which was running concurrently and writing to
the same checkout throughout.

There is no error and no hedge. `from=` in a banner whose subject is
*this* session's reload reads as provenance for *this* session.


### Verified still reproducing 2026-08-31 — and the mechanism is CROSS-PROFILE

A verify-open sweep caught this live, in the sweeping session's own startup banner, which
makes it reproducible rather than recalled:

```
<!-- buddy:reloaded sid=2f584bf5-ded1-4094-8443-14a6e2b3edc1
                   from=428b66b8-0b8c-48e5-84c6-754008e3ccc7 source=compact -->
```

The reader was `codescout-f0`, pid 807989, `CLAUDE_CONFIG_DIR=/home/marius/.claude-sdd`.
Searching all three profiles for the cited transcript:

| profile | `428b66b8…jsonl` |
|---|---|
| `~/.claude` | **FOUND**, last modified 10:35:25 — ~20 s before the check |
| `~/.claude-sdd` (the reader's own) | not present |
| `~/.claude-kat` | not present |

So the banner is not merely naming *another* session — it names a session in a **different
config profile**, one that was being actively written at the moment of reading, and it labels
it `from=` on a `source=compact` line. The natural reading is *"your own pre-compaction
transcript"*; the file is another profile's live session, and is not in the reader's profile
at all.

**Why the profile detail matters more than "another session".** It says the `from` value is
resolved against something that is **not scoped to the reader's config dir** — which is the
same axis that partitions `ListAgents` visibility
(`docs/issues/archive/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`,
where `CLAUDE_CONFIG_DIR` was measured as the discriminator 7/7). A same-profile mix-up would
be a selection bug; crossing the profile boundary suggests the lookup has no profile scope in
it at all.

Not diagnosed: the hook source has not been read, so this names the shape of the defect and
not its cause.

### Third measured occurrence, 2026-09-01 — and this one did NOT stop at a near-miss

The entry above says the banner *"nearly converted a correct denial of authorship into a false
confession."* On 2026-09-01 the same read went further, and the escalation is the part worth
adding rather than the recurrence.

**What happened.** This session's banner read `from=c0ab9bc4-…`. I read it as *"the session this
one was compacted from"*, and — investigating a commit-trailer mechanism — passed that to a peer
session **as fact**, in support of a possible defect. `c0ab9bc4` is not this session's
predecessor; it is the concurrently-live hooks session's own id, exactly as this bug describes.

**Three checks would each have closed it alone, and none was run.** `CLAUDE_CODE_SESSION_ID`
reads `bcc98c22-…`; the commits carrying `c0ab9bc4` touch `scripts/install-hooks.sh` and
siblings, which this session never edited; and every stamped commit postdates the 02:00 hook
install, hence postdates the compaction, so it *could only* carry `bcc98c22`. The last is
decisive on its own — the id and the install window are mutually exclusive.

**Why it is worse than the 2026-08-30 occurrence.** That one was caught inside the session that
held it. This one **crossed a party boundary**. The receiving peer did everything right —
verified against real commits, reasoned carefully, reached the *correct* conclusion — and
attached a **wrong attribution** to it, because the premise came from me and was not among the
things it could check. It reported the mechanism from its own side afterwards: the premise
arrived *labelled as verified*, was *unfalsifiable from there* (a peer cannot read my
`CLAUDE_CODE_SESSION_ID`), and slotted in as *background rather than as claim*, so it was never a
candidate for checking at all.

**Containment:** the wrong attribution never reached a committed artifact — verified, zero
matches for `pre-compaction`, `c0ab9bc4` or `bcc98c22` in the peer's bug file. It lived in chat
and was corrected within three messages, by the only party who could falsify it.

**Severity argument this adds.** The `## Why this one is worse than its siblings` section below
reasons from a *self*-misattribution. The failure mode is larger than that: the banner's output
is **quotable**, and a false premise sourced from it is laundered by a diligent recipient into a
conclusion they can defend. The blast radius is not one session's self-model; it is every party
that session talks to, and their verification effort lands on the checkable parts beside it.

Cross-filed as the fourth instance of `observer-blindness:OB-1` § *Sub-pattern — resolve the
referent from the artifact, never from the sentence pointing at it* (`a552cf83`, receiving-end
mechanism at `da379e1a`). Note the relation: `OB-1` records the **reader's** failure to check,
this bug records the **banner's** manufacture of a claim that does not invite checking. Both are
real and neither excuses the other — but a reader is being asked to distrust a line that is
formatted as provenance metadata, which is why the remedy belongs upstream in `buddy` and not in
a resolution to read more carefully.
## Reproduction

Not reduced to steps. Observed once, on this machine, with four concurrent
Claude sessions across two profile directories (`~/.claude`, `~/.claude-sdd`).
The banner is emitted by the buddy PreCompact/SessionStart hook pair; the
minimal repro to try first is a compaction while at least one other session is
live in the same project directory.

## Environment

Linux; codescout `experiments`; buddy plugin under
`/home/marius/work/claude/claude-plugins/`; four concurrent sessions, three in
`~/.claude` and one in `~/.claude-sdd`.

## Root cause

**Not established — mechanism not read.** The hook source was not opened. What
is measured is only the effect: `from=` carried a session id belonging to
another live session rather than to this session's own prior transcript.

The plausible mechanism, stated as a hypothesis to check first rather than a
finding: buddy resolves "the session to reload specialists from" by recency
across the profile's project directory, so with concurrent sessions the
most-recently-written transcript is a peer's rather than one's own. If that is
right, `from=` is accurate about *where the specialists came from* and
misleading only because the banner's subject is `sid`, which invites reading the
whole line as one session's history.

## Evidence

Two independent identifications of `7114cb0d`, neither relying on the other:

1. **Write-history**, from parsing the session JSONL for `tool_use` entries:
   `7114cb0d`'s most-written paths are `src/retrieval/embedder.rs` (23),
   `docs/TAXONOMY.md` (20), `src/retrieval/client.rs` (15),
   `src/retrieval/search.rs` (11), `src/agent/mod.rs` (5) — the
   embedding-transport work, and visibly not this session's.
2. **Process table**, independently produced by the peer session `codescout-ae`:
   `claude 801487 --resume 7114cb0d… = fix-embedding-transport-stage-1`, live.

## Hypotheses tried

1. **Hypothesis:** `7114cb0d` is this session's own pre-compaction transcript,
   so the `NoCodeSearch` / W-75 writes found in it are this session's, forgotten
   across the compaction.
   **Test:** listed its written paths and compared against this session's known
   work; separately, grepped this session's own transcript for write calls whose
   payload contains `NoCodeSearch`, `CodeChunkSearch`,
   `set_code_search_for_test`, `set_memory_embedder_for_test` or
   `Network-free stubs`.
   **Verdict:** rejected. Zero such writes in this session; `7114cb0d`'s write
   history is another work-stream entirely.

## Why this one is worse than its siblings

It belongs to the family of instruments that return a plausible value instead of
an error — a `--stat` that cannot show whose lines it counted, a touched mtime
read as a build time, a cached `Finished in 0.48s`, and `ListAgents` reporting
"Peer sessions (2)" in a four-session world (that last one being filed
separately by `codescout-ae`).

This one inverts the direction. `ListAgents` **understates who else is writing
your files**; this banner **overstates what you yourself wrote**. And a session
cannot refute it from the inside by introspection, because the hypothesis it
plants — *you did this and do not remember* — predicts exactly the absence of
memory that a compacted agent actually has. Only external evidence breaks it.

## Fix

Not implemented, and the shape matters more than the patch.

Do not print a bare `from=<sid>` on a line whose subject is `sid=<mine>`. Either
label the relationship (`specialists_from=`, plus `predecessor=` when it really
is one), or omit `from=` entirely when the resolved source session is not this
session's own predecessor — which the hook can determine, since it knows both
ids.

## Tests added

None — not fixed, and the fix is in another repo.

## Workarounds

Do not treat the banner's `from=` as provenance for your own history. To
establish what this session actually wrote, parse its own transcript for
`tool_use` entries with a write tool (`edit_file`, `edit_code`, `create_file`,
`edit_markdown`, `artifact`) and match on a distinctive symbol from the diff in
question. That answers "who wrote this line" directly, rather than by
elimination over a candidate set no instrument reports completely.

## Resume

Open the buddy hook that emits `buddy:reloaded` (search `claude-plugins` for the
literal string) and read how the `from` session is selected — that is the
unread premise this whole file rests on. Confirm or refute the recency
hypothesis under *Root cause* before proposing the labelling change under *Fix*.

## References

- `docs/issues/archive/2026-08-30-cli-artifact-drops-time-scope-and-extra.md` — sibling
  filed the same day, also a silent-default class
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the standing
  rule this violates: an instrument's answer must not read as a complete
  population when it is one scope's view
- `open-issue-work-queue:BL-51` — precedent for recording a `claude-plugins` bug
  in this repo's ledger
