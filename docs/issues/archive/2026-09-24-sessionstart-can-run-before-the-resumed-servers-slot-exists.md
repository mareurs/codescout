---
id: '798f69a248d72298'
kind: bug
status: archived
title: 'BUG: SessionStart can run before the resumed session''s codescout server publishes its slot, so that SessionStart never stamps it'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-24
opened: 2026-09-24
owner: marius
related:
- f6a748bcbeee1652
- '92deba12cd82aaf0'
severity: high
---

# BUG: SessionStart can run before the resumed session's codescout server publishes its slot, so that SessionStart never stamps it

## Summary

The companion's `SessionStart` stamp assumes the new server's rendezvous slot already exists. The comment at `claude-plugins:codescout-companion/hooks/session-start.mjs:51-52` says "MCP initialize runs before SessionStart". On an interactive `--resume`, that assumption failed on **2 of 2** observed resumes: the slot was never stamped by the SessionStart that should have stamped it.

The session id still arrives on resume, because the new server inherits it from its predecessor's slot. The **source** does not, so `f6a748bcbeee1652`'s `"kept"` branch is likely unreachable after a resume.

## Symptom (Effect)

Reported by sessionId `09093108-1425-4f6d-9695-a9e3bb98ea0d` (`~/.claude-sdd`), from its own resumes, 2026-09-24:

- **Resume 2, 11:16:51Z, companion 1.20.14.** Server slot `servers/1196086.json` published at 11:16:51.394Z, carrying the inherited `hook_at` 11:05:25Z and no `hook_source`. The `SessionStart:resume` attachment was recorded at 11:16:51.499Z. The slot was never stamped; its next write was the liveness refresher at 11:17:05, which moves `hook_at` only.
- **Resume 1**, earlier the same morning: same outcome, slot `servers/3415327.json` (no `hook_source`).

## Root cause (not proven)

The peer ruled out two alternatives:
- **An empty `source`:** it was `resume`, and `session-start.mjs:77` stamps then.
- **Old code:** 1.20.14's `session-start.mjs` contains the stamping.

What survives is "the hook's scan found no slot in its ancestry". That's consistent with the hook scanning before the server wrote its slot. It is **not proven**, because the hook's start time isn't recorded, only the attachment time (.499Z, after the slot's .394Z).

## Not measured

**Fresh interactive startup: observed once, and it's the severe case.** Reported by the same peer, then read independently by sessionId `774ba049-d97c-443a-b31d-f662a9cb6a1e` at ~11:30Z:

- Session `ebf651ec-5ab7-42d9-a526-dcf9758692e1` started at 11:21:10Z (`~/.claude`, `"kind":"interactive"`, `"entrypoint":"cli"`, parent `comm` `claude`, alive).
- Its slot `servers/1689248.json` reads `{…,"session":"ebf651ec…","hook_at":null}`. The server wrote `session` itself at construction (the keyed tier's env id). `hook_at: null` means no `SessionStart` stamp ever landed.
- Since `Rendezvous::poll` opens the gate only on a non-null `hook_at`, and `refreshLivenessStamp` never opens it (its invariant 1), **that session runs without the rendezvous until its first compaction or `/clear`.** The peer reports the slot was still byte-unchanged at ~11:35Z, although a subagent of that session made a codescout call at 11:29:07Z. So the gate held closed for at least ~14 minutes of activity. Without the rendezvous it gets the blunt clear on activation, a `/clear` it cannot see, and no `SessionStart` source.

**The observations split cleanly by launch mode.** This is a sample, not a rate:

| launch | stamped | observations |
|---|---|---|
| `claude -p` startup | **5 of 5** | all by `774ba049`, 10:51–11:27Z; slot-to-stamp margins 51–86 ms |
| interactive (`cli`) | **0 of 3** | two `--resume`s (`~/.claude-sdd`) and one startup (`~/.claude`), by the peer |

The split is consistent with interactive Claude Code (2.1.281 here) connecting MCP servers asynchronously, after `SessionStart` has fired. `claude -p` would then be the mode that waits for them. That's a hypothesis, not measured. `session-start.mjs:51-52`'s premise, "MCP initialize runs before SessionStart", may have held for an older Claude Code.

**Still not measured:** the hook's own scan time. Instrumenting it (log scan time and a no-slot result) is the step that turns this into a measurement.


## Tests added

In `claude-plugins:codescout-companion/hooks/session-start.test.sh`:

- **The late-slot case.** An intermediate bash plays the Claude, identified by a registry row, and runs the hook while no slot of its own exists. Its server publishes only after the hook has returned.
  - It must be stamped, with the session and the `startup` source. Observed RED (never stamped), then GREEN.
  - A second late slot, owned by another pid, must stay unstamped. It is written FIRST, so a too-broad stamper would reach it in the same scan. The positive half is awaited first, so the negative half cannot pass merely because nothing has run yet.

**Mutations, on scratch copies:**

| mutation | result | caught by |
|---|---|---|
| never spawn | KILLED | the late-slot positive |
| stamper accepts any `ppid` | KILLED | the other Claude's slot |
| shared rule drops the source clause | KILLED | the existing compaction re-stamp test, so the refactor kept that coverage |
| always spawn | SURVIVED | as predicted: inert, it costs a process, not correctness |

**Suites:** all hook suites are green. `tests/run-all.sh` passed in a worktree with the change applied.

**Verified live 2026-09-24, both paths:**

- **Interactive startup.** Hook at +725 ms, slot published at +926 ms with `hook_at: None`. **Stamped at +984 ms** with session `ecf3eb7f…` and source `startup`. No stamper process remained afterwards: 0 node stampers (a `pgrep -f` hit was the query's own shell).
- **Interactive `--resume`** of a seeded `claude -p` session `51912dc7…`. Its slot was published at +1000 ms, carrying the INHERITED source `startup`. It was **re-stamped `resume` at +1050 ms**, with `hook_source_at` 11:43:57.361Z. That is earlier than the slot's own publication (11:43:57.557Z), an ordering only the late stamper can produce.
- Both runs left this session's own slot on `774ba049…`.

## Fix

**Root cause measured 2026-09-24**, on a real interactive startup under a pseudo-terminal (`setsid -f script -qfec "timeout 30 claude"`, cwd = this repo), with 1 ms polling:

| t from launch | event |
|---|---|
| +710 ms | registry row for the Claude pid |
| +763 ms | SessionStart hook runs (it writes `.codescout/cc_session_id` at its start) |
| +982 ms | server publishes its slot, with `hook_at: null` |
| +38 s | still unstamped |

**Implemented 2026-09-24:** `claude-plugins:3a069d5dfcfc5128be4321e58f340459981311ac` (branch `main`, not pushed), patch-id `1eed0be6acbd9b020cb261e603fa10307813cf33`.

- When `session-start.mjs`'s scan finds **no** slot of ours, it spawns `hooks/rendezvous-late-stamp.mjs` detached. The stamper is handed the Claude pid that the ancestry walk ended at (from `92deba12cd82aaf0`'s fix), because a detached process is reparented and cannot walk our ancestry.
- The stamper polls every 100 ms, for at most 30 s, for a slot whose `ppid` is that pid. It stamps it and exits.
- The per-slot rule moved into `lib.mjs` `stampSlotIfStale`, shared by the scan and the stamper, so the two cannot drift. That includes `a5054d13`'s source clause.
- The stamp carries the SessionStart's own time, so `hook_source_at` still records when that start happened.
- **Why not wait inside the hook:** interactive Claude Code may not start its MCP servers until SessionStart returns, so a synchronous wait could only delay startup.

## References

- `f6a748bcbeee1652`: its `"kept"` half depends on this (see its Resume, known limit).
- `92deba12cd82aaf0`: same stamping loop, different defect (it matched too much; this matches too early).
