---
id: c23d86ebd07cffc2
kind: bug
status: fixed
title: a full disk truncates a live session's registry row, so every provenance instrument reports it dead
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
closed: 2026-09-17
opened: 2026-09-17
related: []
severity: high
---

## Summary

A Claude Code session registry row (`$CLAUDE_CONFIG_DIR/sessions/<pid>.json`) is rewritten
periodically by its own process. If a write lands while the filesystem is full, the file is
left **zero bytes**. The session keeps running and its socket stays open, but every
provenance instrument that reads that file — `scripts/file-provenance.py`, `ListAgents`,
and any hand-rolled socket walk — now reports the session as absent.

The instruments treat a missing or empty row as **"not live"** rather than **"unknown"**,
so the failure is silent and points the dangerous way.

## Symptom (Effect)

A live session becomes invisible to authorship and liveness queries. Concretely:

- `file-provenance.py` cannot attribute files it wrote, so a peer reading the result
  concludes the work is unowned.
- A liveness enumeration undercounts, and any decision of the form *"this resource belongs
  to a session that has exited, so it is safe to delete"* is wrong precisely about the
  session that is still using the resource.

## Reproduction

Observed 2026-09-17, not constructed:

1. `/home` (btrfs) reached 100% full — `Free (estimated): 64.00KiB`.
2. `/home/marius/.claude-kat/sessions/1523228.json` was truncated to **0 bytes** at 17:05.
3. `/proc/1523228` present, `/run/user/1000/cc-socks/1523228.sock` present, session
   actively executing tool calls.
4. A socket-walk enumeration classified its own session id as dead, alongside six genuinely
   dead ones.

Exactly **one** of the registry files across all profiles was zero-length at the time, which
is why this reads as a freak event rather than a population.

## Environment

codescout `experiments`, Linux, btrfs at 100%, Claude Code 2.1.273/2.1.274, several
profiles sharing one checkout.

## Root cause

Two independent facts that only compose into a hazard together:

- The registry row is a **derived cache** of state the process already holds, rewritten in
  place. A truncating write under `ENOSPC` destroys the record without the process noticing.
- Every reader maps *absent row* to *absent session*. There is no third value for *the
  instrument could not tell*, so a corrupted input is indistinguishable from a true negative.

**The failure is self-amplifying.** Disk exhaustion is exactly when an operator goes looking
for reclaimable per-session state — and it is the same condition that makes live sessions
look reclaimable. The fuller the disk, the more live sessions read as dead.

## Evidence

CLAUDE.md § *Observer Blindness* already states the governing rule — *never close an
authorship question by elimination; identify positively* — and names the positive route as
resolving the session's registry row from the socket a message arrived on. **That route
terminates in this same file**, so the documented safe method inherits the defect.

`CLAUDE_CODE_SESSION_ID` was checked as a disk-independent substitute and is **not** present
in the Claude process's own `/proc/<pid>/environ` (verified across all 18 live sockets: 0
carried it). It is injected for child processes only, so there is no in-memory source a
reader can fall back to.

## Impact

Caught before any damage, by a dry run. The proposed action was deleting build caches
belonging to "dead" sessions; the classifier marked this session's own 29G cache and would
have marked any other truncated-row session's cache the same way. Deleting a live session's
`CARGO_TARGET_DIR` mid-build corrupts that session's in-flight work.

## Root-cause candidates for the fix

Not settled; recorded so the next session does not re-derive them.

- **Distinguish the three states in `file-provenance.py`.** A zero-length or unparseable
  registry row should report `UNKNOWN`, never fall through to "not live". Cheapest, and it
  fixes the dangerous direction without needing a new source of truth.
- **Cross-check against the socket + `/proc` pair before concluding absence.** A live pid
  with an open `cc-socks` socket is evidence of a session regardless of what its row says —
  it just cannot say *which* session.
- **Write the row atomically** (temp file + rename). Prevents the truncation rather than
  tolerating it, but the file belongs to Claude Code, not this repo.

## Fix

`live_sessions()` now returns `(live, unreadable)`. A row that cannot be parsed, or that
parses without a `sessionId`, no longer vanishes: the pid is read from the FILENAME —
which survives the bytes being destroyed, because rows are keyed `<pid>.json` — and if
that pid is alive the row is recorded as a live session whose identity could not be read.
Both callers print it with pid and profile.

In `file-provenance.py` the notice sits **outside** the named-sessions footer,
deliberately: the verdict carrying no session line is `UNKNOWN`, and `UNKNOWN` plus a
live-but-unnameable session is exactly when *"nobody owns this file"* is the wrong
conclusion to draw.

**Scoped narrowly on purpose.** Only a row that cannot say WHO it is counts. A known sid
whose socket is gone is a session we can still name — merely unreachable — and the
existing dead-session path already reports that. Widening to every skipped row would make
every stale file on disk shout and bury the one row that means something.

The chosen shape was already in the file, one function up: `_pid_alive` documents that
`PermissionError` means *alive, not absent*, and refuses to collapse the two. The same
three-state distinction was owed one function down and had not been made.

**Fixed:** `c7c72638`
**patch-id:** `623307653dc7c6b57504013b2202c279cb23a6a6`

## Tests added

Five cases in `tests/file-provenance.sh` (156 → 161), in the registry-join section as
case **I**. **Red observed first**, on all five.

The fixture is a genuinely zero-byte file (`: > "$REG_B/$TRUNCPID.json"`) against a pid
held alive by a backgrounded `sleep` — not a malformed-JSON stand-in, because 0 bytes is
precisely what a truncating rewrite under `ENOSPC` leaves behind and is the state the real
incident was observed in.

**One assertion was rewritten before implementing, and the reason is the point.**
`has "and names the profile holding it" "$out" "regB"` **PASSED in the red state** —
satisfied by the profile label on an unrelated live row elsewhere in the same output. It
would never have discriminated. It was folded into a single unique needle,
`pid $TRUNCPID (regB)`, and per *a red does not survive its assertion being edited* the
red was then re-observed (157/5 → 156/5) rather than carried over.

**The discriminator case is what keeps the others honest:** a corrupt row for a DEAD pid
must stay silent. Without it, every assertion above is satisfied by reporting any
unparseable file, and every stale row on disk would shout.

Four guarded sites, one mutation each, all KILLED against a re-derived green baseline of
161/0:

| site | verdict |
|---|---|
| liveness gate on the pid (`if _pid_alive(f.stem)` → `if True`) | KILLED — 160/1, the dead-pid case alone |
| parse failure keeps the row (`rec = None` → `continue`) | KILLED — 156/5 |
| the unreadable record itself (`unreadable.append(…)` → `pass`) | KILLED — 156/5 |
| the warning is printed (`if unreadable:` → `if False:`) | KILLED — 156/5 |

**`mutation-probe` reports INCONCLUSIVE for every one of these, by design**, because its
parse keys on cargo's `^running N tests` and this is a shell suite. Its own message names
the substitute reading — the runner's summary line against a known-clean baseline, never
the exit code — and that is what the verdicts above are read off. The probe still applied
each mutation exactly once and reverted it, so only the verdict was missing.

## Workarounds

None needed — fixed. Before `c7c72638`, the workaround was to treat any liveness or
authorship result as a lower bound while the disk is near full, and to confirm a session's
absence against `/proc/<pid>` plus its socket before acting on it. The tool now says this
itself, which is the whole repair.

## References

- `scripts/file-provenance.py` — the attributing instrument
- `CLAUDE.md` § *Reaching a Peer Session*, § *Observer Blindness* — both route to the
  registry row as the positive identifier
- Found while reclaiming disk during a 100%-full incident, by a dry run whose output named
  the running session as dead.
