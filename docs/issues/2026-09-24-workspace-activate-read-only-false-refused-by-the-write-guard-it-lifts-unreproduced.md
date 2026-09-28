---
id: '652abef29e8d8c45'
kind: bug
status: open
title: 'BUG: workspace(activate, read_only=false) is refused by the read-only write guard it exists to lift (Workspace::is_write doubles as the read-only gate)'
tags:
- cluster/unclassified
---

# BUG: workspace(activate, read_only=false) refused with the write-guard's own error, the remedy that error names (unreproduced on retry)

## Summary

After a `/mcp` reconnect on 2026-09-24, `create_file` in the home checkout was refused with the `WriteBlockCause::ActivatedReadOnly` message (`src/util/path_security.rs:677`). That message names two remedies: pass `workspace=<abs path>`, or call `workspace(action='activate', path=…, read_only: false)`. **The second remedy was itself refused with the byte-identical write-refusal text.** Minutes later the same call, with the same arguments, succeeded.

**Classification checked 2026-09-25 — stays `cluster/unclassified` until reproduced.** A class tag is a
claim about the mechanism, and the mechanism is unknown. The nearest candidate is `IC-12` (transient shared
state lies to readers — the refusal came minutes after a `/mcp` respawn, which is known to drop a session's
activation), but tagging it would assert a cause nobody has observed. Checked by sessionId
`e4fbc7ef-27b7-4707-8469-ccdffa8e4e92`.

## Symptom (Effect)

The refusal points the reader at a call that the same guard refuses. A reader who follows the error's own instruction gets the error again, with nothing to indicate why. This is the "remedy text sends you somewhere useless" failure (`CLAUDE.md` § Testing Discipline, loudness bullet), and here the remedy is not only useless but refused.

## Reproduction

**Not reproduced.** The sequence, all from session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`:

1. `workspace(post_compact=true)` after `/mcp` reconnect to a freshly rebuilt server (`git_sha 396f04c4`, dirty).
2. `create_file(path="scripts/…")` → refused, `ActivatedReadOnly` text naming `/home/marius/work/claude/codescout`.
3. `workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)` → **refused with the same text.**
4. `create_file(…, workspace="/home/marius/work/claude/codescout")` → succeeded (the pin works).
5. About 10 minutes later, the same call as step 3 → `status: ok`, `read_only: false`.

A background research subagent was running on the same MCP server between steps 3 and 5, and may have activated the project itself. That is unverified.


### Reproduced 2026-09-24, deterministically (session `09093108-1425-4f6d-9695-a9e3bb98ea0d`)

On a server rebuilt at `c07f71ee`+, with no subagent or peer sharing it, in three calls:

1. `workspace(action="activate", path="/home/marius/work/claude/codescout/crates/codescout-embed", read_only=true)` → `status: ok`, `read_only: true` (a workspace member, activated by absolute path).
2. `workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)` → **refused**: *"File writes are disabled: the active project is …/crates/codescout-embed and it was activated read-only. … To lift the block here instead, call workspace(action='activate', path='…/crates/codescout-embed', read_only: false)"*.
3. The same call as step 2 plus `workspace="/home/marius/work/claude/codescout"` → `status: ok`, `read_only: false`, home restored.

So the lead above holds and is not intermittent: the activate call is gated by the CURRENT activation's read-only state, so a read-only activation guards the only call that leaves it, including a call to a DIFFERENT root. The step-5 success in the original sequence fits the peer-reactivation reading it offered. The per-call pin is the working escape (step 3) and the refusal names it first, so a reader following the text in order recovers; the second remedy it names is the one that cannot work.

### Third observation, 2026-09-28 (session `82cff72e-0245-48cb-ab07-45a1c3d0d388`): the refusal cleared with no pinned activate

Server `git_sha fd0b4181` (dirty), pid 2496171, home project `/home/marius/work/claude/codescout`.

1. Four Opus review subagents shared this server in parallel. One of them reported that its first call, `workspace(activate, codescout, read_only=true)`, came back with the write refusal. So the state may already have been read-only from a sibling, or its own call may have set it; which one is unknown.
2. About 16:10Z, after all four had finished and with no other caller on the server, `doc(action="append_entry")` from the controller was refused with the `ActivatedReadOnly` text naming the home root.
3. `workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)`, with no `workspace=` pin, was **refused with the same text.**
4. The workaround ran next: three writes, each pinned with `workspace="/home/marius/work/claude/codescout"` (two `append_entry` and one `edit_file`), and all succeeded. Then one read (`doc(action="find")`).
5. About 16:25Z, the identical call from step 3, still unpinned, returned `status: ok`, `read_only: false`.

**What this adds to the deterministic reproduction above, and what it leaves open.** Step 3 fits that reading: the activate call is gated by the current read-only state. Step 5 does not. Nothing re-activated the project between steps 3 and 5, as far as this session can see: no subagent was running, and the only calls were pinned writes and one read. Yet the unpinned call succeeded. Either a pinned call resets the process-wide activation, or something outside this session changed it. Neither is verified. The discriminating probe: from a read-only activation, make one pinned write, then an unpinned `workspace(activate, read_only=false)`, and a control without the pinned write.

### Settled by probe, 2026-09-28 18:51–18:56Z (session `82cff72e`): two mechanisms, both at the bytes

This session was the only caller on its server, with no subagent running. Every call below targets `/home/marius/work/claude/codescout`. Writes went to a gitignored scratch dir (`target/probe-652abef/`), removed afterwards.

| # | from state | intervening calls | then | result |
|---|---|---|---|---|
| 1 | writable | — | `activate(path="codescout", read_only=true)`, a bare id as the review subagent used | `ok`, `read_only: true` |
| 2 | read-only | — | unpinned `activate(read_only=false)` | **refused** |
| 3 | read-only | a refused retry | the same | **refused** |
| 4 (control) | read-only | a `date` read, a `doc(find)` | the same | **refused** |
| 5 (treatment) | read-only | one PINNED `create_file` | the same | `ok`, `read_only: false` |
| 6 (control) | read-only | one PINNED `read_file` | the same | **refused** |
| 7 (treatment) | read-only | one PINNED `create_file` | an UNPINNED `create_file`, with no activate at all | **ok**: the guard was gone |
| 8 (control) | read-only | none | an UNPINNED `create_file` | **refused** |

**Mechanism A: why the refusal happens (deterministic).**
- `Workspace::is_write` returns true for `action="activate"` (`src/tools/config/mod.rs:31-33`). The reason is that activation persists `.codescout/libraries.json` and must take the cross-process write lock.
- `call_tool` then runs `check_tool_access(name, is_write, …)` (`src/server.rs:1413`), and under an `ActivatedReadOnly` write block that refuses every `is_write` call, this one included.
- So one flag carries two meanings, "take the write lock" and "refuse under read-only", and the second one catches the only unpinned call that could lift the block. The refusal text (`src/util/path_security.rs:675`) then names that very call as its remedy.

**Mechanism B: why it cleared "on retry" (every observation, 2026-09-24 and today).**
- For any write-tool call carrying a `workspace=` pin, `call_tool` calls `ensure_resident(root, Some(false))` before the access check (`src/server.rs:1404-1411`).
- `Agent::ensure_resident` "upgrades [an already-resident, read-only entry] to writable" by setting `p.read_only = false` on the registry entry (`src/agent/mod.rs:758-786`).
- That entry is the one shared by every caller, and the upgrade persists. So when the pinned root is resident read-only, one pinned write lifts the read-only state process-wide. Row 7 shows it directly: an unpinned write succeeds right after a pinned one, with no activate call in between.
- The original sequence's step 4 (a pinned `create_file`) and this session's three pinned writes at 16:10–16:25Z are what cleared the refusal. So "unreproduced on retry" was the pin, not intermittency.

Mechanism B is a separate defect, and the more consequential one: a pin is documented as per-call, yet it silently lifts a read-only guard that another caller set on purpose. It is filed on its own, `docs/issues/2026-09-28-a-pinned-write-lifts-a-read-only-activation-for-every-caller.md`.

## Environment

codescout server pid 3289089, `git_sha 396f04c4` (dirty), profile `~/.claude-kat`, CLI Claude Code, 2026-09-24.

## Root cause

Unknown. **Lead, not established:** the successful activate response carries `wrote_to` / `abs_path`, so activation runs through the write path. If that path's guard checks the *current* activation's read-only state before applying the new `read_only=false`, then a read-only project guards against the call that lifts it. The success at step 5 argues against a deterministic version of this. It is consistent with a peer having re-activated in between, which changed the state the guard read.

## Evidence

The refusal text returned at step 3 is identical to step 2's, including "To lift the block here instead, call workspace(action='activate', …, read_only: false)".

## Hypotheses tried

None yet: no mutation or code read beyond locating the message.

## Fix

Not started. A fix would need a reproduction first (`CLAUDE.md` § Bug Tracking: run the reproduction before reading the fix plan).

## Tests added

None.

## Workarounds

Pass `workspace=<abs path>` per call. It worked first time and does not flip process-wide state under peers.

## Resume

Mechanism known (§ *Settled by probe, 2026-09-28*). The fix for A: exempt `workspace(action="activate")` from the `ActivatedReadOnly` write block while it keeps the write lock. For example, split `is_write`'s two meanings, or let `check_tool_access` pass the exit call that the refusal itself names. The regression test is rows 2 and 8 of the probe: from a read-only activation, an unpinned `activate(read_only=false)` succeeds and an unpinned write is still refused. Fixing A removes the reason callers reach for the pin, but B still needs its own fix (see the separate file).

## References

- `src/util/path_security.rs:677` (the refusal text)
- `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
- `docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md`
