---
id: '54a1a8011bca0358'
kind: bug
status: fixed
title: 'BUG: a nested `claude` session''s SessionStart re-stamps its ancestor session''s codescout server, hijacking that server''s session identity'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-24
opened: 2026-09-24
owner: marius
related:
- a5054d135acacbe3
- c186c45e2ed2a038
- '5201164f55ec16ff'
severity: high
---

# BUG: a nested `claude` session's SessionStart re-stamps its ancestor session's codescout server, hijacking that server's session identity

## Summary

The companion's `SessionStart` hook stamps every rendezvous slot whose `ppid` appears **anywhere** in the hook's process ancestry. A `claude` session started from inside another session's tool call (`claude -p` from `run_command` or `Bash`) has the outer Claude process in its ancestry. So its hook also stamps the **outer** session's codescout server slot with the **nested** session's id.

The outer server polls the slot, sees a new session, and rekeys. It forgets every guide topic and re-sends each one as it next triggers. It also records every later call under the nested session's id, until the outer session's next `SessionStart`. Nothing restores it on its own: the 60 s liveness refresh rewrites only `hook_at`.

## Symptom (Effect)

Measured 2026-09-24 in session `774ba049-d97c-443a-b31d-f662a9cb6a1e` (`~/.claude`). The codescout release binary was built 10:48:41Z and contains `ba3a787e`. `claude-plugins` was at `cf5ea29c` (working tree).

- **10:51:45Z** — `run_command` → `timeout 200 claude -p '…' --allowedTools "mcp__codescout__workspace" --max-turns 5 --model sonnet --output-format json` (probe session `1e4fc97a-de21-438c-80f5-bb8e4e07f7ff`).
- **10:51:46.278Z** — **one** hook run stamped two slots, with identical mtime `13:51:46.277988488` local:
  - `servers/3413459.json`: the probe's own server, ppid 3413308.
  - `servers/3393665.json`: **my** server, ppid 50568, which is my Claude process. Before and after:

  ```
  before: {"pid":3393665,"ppid":50568,…,"session":"774ba049-d97c-443a-b31d-f662a9cb6a1e","hook_at":"2026-09-24T10:48:37.803Z","hook_source":"compact","hook_source_at":"2026-09-24T10:48:37.803Z"}
  after:  {"pid":3393665,"ppid":50568,…,"session":"1e4fc97a-de21-438c-80f5-bb8e4e07f7ff","hook_at":"2026-09-24T10:51:46.278Z","hook_source":"startup","hook_source_at":"2026-09-24T10:51:46.278Z"}
  ```
- My next call (a plain `python3` read) re-injected `project-activation-bootstrap` with "First call this session". In `usage.db`, rows `137810`, `137812` and `137814` are my own `run_command` calls (10:52:21–10:52:55Z), and they carry `cc_session_id` `1e4fc97a…`. Row `137808`, the probe launch itself, started before the stamp and still carries `774ba049…`.
- I restored the slot by hand at 10:53:44Z. Switching back rekeyed again: the bootstrap guide was re-injected a second time, and `guide_hints/774ba049….json` shrank to `{"project-activation-bootstrap": …}`.

## In the session behind the original "arms continuously" report (`571eb3d6`)

`571eb3d6-c879-43f6-b3f9-5a51e744e1af` ran `claude -p` from tool calls **8** times (tool_use `command` fields in its `~/.claude-kat` transcript; mine, as a positive control for that grep, shows 2). Measured against `.codescout/usage.db`, UTC:

- `571eb3d6`'s own rows stop in hour 11 on 2026-09-23, yet its transcript continues into hour 16.
- Four sessions it spawned under its scratchpad (`…-scratchpad-toyproj`, `…-scratchpad-toy2`) have rows in **codescout's** `usage.db` from 13:47 to 14:50: `7a2b1b88`, `ce945a85`, `8f1551dc`, `47b90406`. Their cwd was a toy project, so they have no business writing there.
- **The two records match exactly for 14:40:38–14:49:54.** The parent transcript made **51** codescout tool calls in that window, none of them sidechain: `run_command` 25, `edit_file` 7, `doc` 4, `edit_code` 4, `symbols` 4, `read_file` 3, `create_file` 2, `grep` 2. `usage.db` records the same 51, with the same per-tool counts, under `47b90406`.

So the parent ran under a nested session's identity for at least that hour. Each switch is a rekey, and a rekey forgets every topic. From the parent's side, that looks like its guides arming again.

**What this does not show.** It doesn't show that this explains every re-arm in `571eb3d6`. The 12:00–13:47 and 14:50–16:59 stretches have no rows under `571eb3d6` or these four ids. The two mechanisms fixed alongside (`5201164f55ec16ff`, `c186c45e2ed2a038`) were also present in that session.

## Reproduction

1. In a Claude session with codescout, find your slot: `grep -l <your sessionId> ~/.local/state/codescout/servers/*.json`.
2. From `run_command` (or `Bash`): `claude -p 'Call mcp__codescout__workspace with action="status". Reply OK.' --allowedTools mcp__codescout__workspace --max-turns 3 --output-format json > out.json`
3. `cat` your slot. Its `session` is now the probe's id (see `"session_id"` in `out.json`).
4. Your next codescout call re-injects `project-activation-bootstrap`, and `usage.db` records the call under the probe's id.

**Control, run once and not isolated:** a probe started with `setsid -f env CLAUDE_CONFIG_DIR=/home/marius/.claude-sdd …` is reparented away from the caller. It stamped only its own slot (`servers/3471846.json`, 10:58:10Z), and mine stayed `774ba049`. The profile also differed from the first probe, so this is consistent with the ancestry explanation but doesn't isolate it.

## Environment

Linux 7.2.4-zen2; `claude-plugins` working tree at `cf5ea29c`; codescout `experiments` at `2bfe4972`.

All three profiles use the `sdd-misc-plugins` **directory** marketplace (`/home/marius/work/claude/claude-plugins`). Hooks run the working-tree code: observed for `~/.claude` (the `/compact` stamp above) and for `~/.claude-sdd` (the detached probe wrote `hook_source`, which the cached 1.20.13 copy doesn't contain). `~/.claude-kat` has not been probed.

## Root cause

`claude-plugins:codescout-companion/hooks/session-start.mjs:61-99` stamps a slot when `ancestry.has(e.ppid)`. `ownAncestry()` (`lib.mjs:537-548`) walks up to 10 hops, toward pid 1. The stated intent (`session-start.mjs:52-56`) is to stamp "slots belonging to our own process ancestry … [which] keeps two concurrent windows on one repo from stamping each other's servers".

That predicate excludes **sibling** windows. It does not exclude **nested** sessions. For a nested hook the chain is hook → nested claude → shell → outer codescout server → outer claude, and the outer server's slot has `ppid` = the outer claude.

The predicate is a proxy for "the servers of the Claude process that ran me", which the hook can't observe directly. Nesting is exactly where the proxy and its target diverge, which puts this bug in IC-2.

Server side: `Rendezvous::poll` sees the session change and calls `GuideLedger::rekey` (`src/tools/guide_ledger.rs:290`). Rekey forgets every topic by design, because a new session id normally means `/clear`.

**The same predicate has two more call sites, not audited:** `resolveOwnServerPids` (`lib.mjs:262-272`) and `refreshLivenessStamp` (`lib.mjs:509-515`, which only touches `hook_at`).

## Hypotheses tried

1. **Hypothesis:** the `/compact` → `/mcp` sequence emptied the ledger.
   **Test:** read the ledger after the first post-`/mcp` call.
   **Verdict:** rejected. All 14 entries survived, and only `project-activation-bootstrap` was re-stamped. That one came from the deliberate construction-time re-arm at `src/server.rs:519-532`: one re-send per new server, as its own comment says.
2. **Hypothesis:** the probe's server merely shared my ledger file.
   **Test:** my own slot's `session` field, plus `usage.db` attribution.
   **Verdict:** rejected. My slot changed identity, and my calls were recorded under the probe's id.
3. **Hypothesis (raised by a peer session):** profiles other than `~/.claude` run the cached 1.20.13 hook code.
   **Test:** a detached `~/.claude-sdd` probe.
   **Verdict:** rejected for a fresh startup. The probe wrote `hook_source`, which the cache doesn't contain. The peer retracted.

## Fix

**Implemented 2026-09-24:** `claude-plugins:1cc83fbf6cf46c43fb5d408fb5a57715b1b4cb1e` (branch `main`, not pushed), patch-id `aaac514635807cf51e6270fec2ec8dc1738ea8d5`.

Both ancestry walks now **end at the nearest Claude process**, which is included in the walk. That's `lib.mjs` `ownAncestry()`, which serves `refreshLivenessStamp` and `resolveOwnServerPids`, and `session-start.mjs`'s own copy, which serves the stamp. One predicate, `lib.mjs` `isClaudeProcess(pid)`, recognizes a Claude process by either of two signals, because each covers the other's gap:

- **A registry row, `$CLAUDE_CONFIG_DIR/sessions/<pid>.json`.**
  - Measured present for `claude -p` (`"entrypoint":"sdk-cli"`) as well as interactive sessions.
  - Written at startup, so it races the hooks. Measured once: row at 11:18:54.755Z, SessionStart stamp at .820Z.
- **`comm` `claude`.**
  - Set at exec, so it can't lose that race.
  - Linux-only, and true only of the native binary (an npm install runs as `node`).

**Residual, documented at the predicate.** A stale registry row whose pid was reused by a process in our chain ends the walk early. Our own server is then left unstamped: the pre-rendezvous state, and the safe direction. The row's `procStart` equals `/proc/<pid>/stat` field 22 (verified: 84228601 for PID 50568), which is the discriminator if this is ever worth a third guard.

The weaker alternative (stop at the first ancestor that parents *any* slot) was rejected. It fails whenever the nested session's own server hasn't published its slot yet.

## Tests added

All in `claude-plugins`. Each was observed RED against the unfixed hooks, with its positive control PASSING in the same run:

- **`codescout-companion/hooks/session-start.test.sh`:** two nested cases, each fabricating the nested Claude as an intermediate process that owns the inner slot, with the outer slot's `ppid` one hop above it.
  - **(A)** A plain `bash` with a registry row in a private `CLAUDE_CONFIG_DIR`, so only the registry signal can fire.
  - **(B)** A `bash` exec'd through a symlink named `claude`, with no row, so only the `comm` signal can fire. Its `comm` is checked as a fixture precondition, not assumed.
  - Each case asserts that the inner slot IS stamped (positive control) and that the outer slot keeps its session.
- **`codescout-companion/hooks/cs-liveness.test.sh` case 5:** the same shape through `lib.mjs`'s walk. The inner slot is refreshed; the outer is untouched.
- **Mutations, run on scratch copies only because the working tree is live for every profile.** Each pattern was asserted to occur exactly once, and each mutation was KILLED by exactly its intended case:

  | mutation | killed by | stayed green |
  |---|---|---|
  | drop the stop in session-start's walk | (A) and (B) | cs-liveness |
  | drop the stop in lib's walk | cs-liveness #5 | (A) and (B) |
  | disable the registry signal | (A) and cs-liveness #5 | (B) |
  | disable the `comm` signal | (B) | (A) |

  These kills were measured with a real Claude process above the test run. Without one, as in CI, the unfixed walk has nowhere earlier to stop, so the same assertions would red. That follows from reading the walk; it was not measured.
- **Suites:** all 18 hook `*.test.sh` files and `host-normalization.test.mjs` are green. `tests/run-all.sh` has one red suite, `test-pre-push-guard.sh` (six parity assertions). It is **not caused by this change**: in a worktree of the same repo at HEAD, with the same history and shared `.git/hooks` and with all four changed files applied, that suite shows 0 failures. Whether it was already red in the live tree before this change was not measured.

**Verified live 2026-09-24T11:27Z** with the original reproduction: a `claude -p` run from this session's own `run_command`, not detached. Afterwards:

- My slot (`servers/3393665.json`) still read `774ba049…` with `hook_source` `compact`.
- The probe's own slot (`servers/2568872.json`, `ppid` = the probe's Claude) was stamped `e5864c3d…`/`startup`.
- Exactly one slot carried the probe's id, where the unfixed hook produced two.

## Workarounds

- **Start nested sessions detached**, with `setsid -f …`. Observed to work once (above).
- **Repair a hijacked slot** by rewriting `session`, plus `hook_source`/`hook_source_at` from the last genuine SessionStart, via a temp file and rename. That's what I did at 10:53:44Z. It costs one more rekey.

## Resume

Fixed and verified live. Archive in one pass with `c186c45e2ed2a038` and `a5054d135acacbe3`. The move mints a new id, so re-point, in the same pass:

- the four `claude-plugins` citations of this path: `hooks/lib.mjs`, `hooks/session-start.mjs`, `hooks/session-start.test.sh`, `hooks/cs-liveness.test.sh`;
- this id in `issue-clusters/IC-2` Members.

`~/.claude-kat` has not been probed, but all three profiles use the same directory marketplace.

## References

- Same investigation: `5201164f55ec16ff` (re-arm consumed by the wrong principal, archived), `c186c45e2ed2a038` (SubagentStop restore), `a5054d135acacbe3` (post_compact source gate). This bug was found during `a5054d13`'s live check.
- `claude-plugins:codescout-companion/hooks/session-start.mjs`, `…/lib.mjs` (`ownAncestry`, `resolveOwnServerPids`, `refreshLivenessStamp`)
- `src/tools/guide_ledger.rs` (`rekey`), `src/tools/rendezvous.rs` (`poll`)
