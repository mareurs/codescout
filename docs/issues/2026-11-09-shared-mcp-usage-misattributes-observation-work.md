---
kind: bug
status: fixed
title: Shared MCP usage stamps observation work with unrelated native session IDs
tags:
- session-identity
- codescout
- cluster/unclassified
closed: 2026-11-12
moved_from: claude-plugins:docs/issues/2026-11-09-shared-mcp-usage-misattributes-observation-work.md (catalog id 16de4012209e895f), moved 2026-11-09
opened: 2026-11-09
owner: marius
related:
- ce484db75532c7f0
severity: medium
unverified: 'Marked fixed by the operator on 2026-11-12 on the strength of claude-plugins f41d5aa, released as codescout-companion 1.21.5. Reproduced and fixed at the hook level with the real ownAncestry inside a real `codex exec` hook (isolated CODEX_HOME); the new nested-codex test is red on the old code and green on the fix (48/48). Read: the four Claude profile caches hold 1.21.5 with the fix; the Codex plugin cache holds only 1.21.0 without it. Not run: the live check (a Codex probe under a Claude session, then cc_session_id in usage.db), because the Codex hook would still be the old one. Not shown: that this is the cause of the four incidents (their processes are gone); a real Codex app-server tree under a real Claude Code session; a Codex install whose comm is not `codex`.'
---

## Symptom and evidence

During stock-observation implementation in the new isolated Codescout checkout /tmp/codescout-stock-observations-20261109, the foreign-index guard attributed files created in this session to unrelated sessions. Read-only usage.db queries showed cc_session_id 01a1bf68-e650-7f20-911e-b53f86597c27 for Task 1 calls and 01a1bf89-3111-7c70-b934-5f6fd89102a3 for later Task 2 writes. Native exec_command and Codescout run_command both reported CODEX_SESSION_ID / CODEX_THREAD_ID 01a1b59b-676f-7881-b7eb-4d8e1687c3f8. The latest stored edit_file input had only path/workspace/old_string/new_string keys, with no caller argument.

These are observed mismatches, not proof of which fallback or transport branch caused them. Source at ef7fe2ee9a2c452637c0d93fbecc86c84ee17677 prefers asserted/native identities above serving-session and construction-time fallbacks; the running binary's incoming metadata was not captured.

## Class fit

This remains unclassified because the wrong writer stamp is measured but the causal transport or fallback boundary is not. It resembles earlier native-session incidents, while differing in that shell context already exposes the correct native root. Equal symptoms do not establish equal causes.

## Workaround and remaining work

The maintainer approved notifying the first named local thread and using the documented ACK for Task 1. Notification was delivered; Task 1 landed in Codescout c1dd47d09 + 1c1546798, with explicit notes that the acknowledged stamp was incorrect. The guard accepts ACK for all-contested batches but refuses mixed own/contested batches; primitives and wiring were committed separately to preserve that rule.

Later tasks acquired wrong stamps under four session IDs. After source verification and independent review, the operator approved the concrete four-recipient notifications and guarded commits, then manually approved previously denied sends. Codescout support `505e8033a`, wiring `c7b38be29` and caller tests `f89590d31` are committed. No usage rows or peer files were modified; no hook was disabled.

## Related and next verification

The prior fixed incident is Codescout artifact ce484db75532c7f0, docs/issues/archive/2026-11-07-hook-diagnostic-root-misattributed-effort-steering-writes.md. Its incoming-native-root fix does not prove the cause of this incident.

Capture only real client identity and native metadata at the running server boundary; compare to the native task and recorded row. Do not persist tool bodies, prompts or secrets. The new stock observer must keep missing/conflicting identity unknown instead of using startup/rendezvous identity.

## Second occurrence with a candidate trigger (2026-11-10, claude-code:74fdade4, codescout `experiments`)

Four pathspec commits of one Claude Code session were refused by the foreign-index guard, each naming a not-live session that did not write the staged bytes. Every staged diff was read in full and held only lines written by the committing session through `edit_file` and `memory` (a new file in the first case, appended sections in the others).

| Commit | Accused session | Created (UUIDv7, local time) |
|---|---|---|
| `d7ba4de8f` | `01a1c1e7-d5ec-7182-8f87-7a5255244ab3` | 2026-11-09 22:02:38 |
| `b23880195` | `01a1c1fa-06af-7660-8293-95e7f03d9df6` | 2026-11-09 22:22:30 |
| `89794d82c` | `01a1c1e2-122b-7631-bd07-e73d536c3806` | 2026-11-09 21:56:20 |
| `bfb800253` | `01a1c276-0b44-7fc0-8374-a446092fade0` | 2026-11-10 00:37:58 |

- All four ids have the shape of Codex thread ids. Within minutes of each, the committing session had started a Codex app-server thread on the REAL `~/.codex` config as a probe; that config launches `codescout start` as an MCP server for the thread (`[mcp_servers.codescout]`).
- For the last row the accused id is EQUAL to the `session_id` of such a probe thread: it appears as the `session_id` of the PostToolUse event of that probe in the capture log of the `codescout-wake` hook (the log is in the committing session's scratchpad, not kept).
- `scripts/file-provenance.py` on the first file ended with `written by THIS session (claude-code:74fdade4)` after hundreds of coverage gaps, among them `relative write has ambiguous shared MCP workspace`.
- Not a controlled comparison: a commit made while no such probe thread had recently run (`4836fece6`) passed with no refusal. Few cases.
- MEASURED 2026-11-10 in `.codescout/usage.db` (read-only, table `tool_calls`; `session_id` is the server session, `cc_session_id` the stamped identity). On both shared-server sessions of the committing session the stamp starts as its own id, then moves to a Codex thread id and STAYS there for every later call, of every tool, until the server session ends:

  | server session | change at (UTC) | from | to | thread created (UTC) |
  |---|---|---|---|---|
  | `2351ffd0` | 19:56:41 | `74fdade4` | `01a1c1e2` | 19:56:20 |
  | `2351ffd0` | 20:04:30 | `01a1c1e2` | `01a1c1e7` | 20:02:38 |
  | `2351ffd0` | 20:23:19 | `01a1c1e7` | `01a1c1fa` | 20:22:30 |
  | `b97bc1ca` | 22:38:21 | `74fdade4` | `01a1c276` | 22:37:58 |

  Between 22:50 and 23:00 UTC all 39 calls of the Claude Code session on `b97bc1ca` (26 `run_command`, 11 `edit_file`, 1 `grep`, 1 `read_file`) carry `01a1c276`, none its own id. The server session `b97bc1ca`, started by a `/mcp` reconnect at 21:35:44, began with the correct id, so a reconnect clears the stamp.
- Reading: the first call of a Codex thread into the shared server changes the stamp, and later calls of the other client inherit it, so the guard blames the Codex thread for the Claude session's writes. Not verified in code which state holds the stamp (the hook rendezvous slot or the connection) or why the committing session's own hooks do not restore its id. Workaround found: `/mcp` reconnect after any Codex thread on the real config, and no such probes while committing.
- Cost of the ack: the guard adds a bare `Co-Authored-Session-Id` trailer even when the message already carries the `codex:`-prefixed form, so `d7ba4de8f` and `b23880195` (published) carry both, and the hook warns that a bare id reads as a Claude Code session.

## Read from code 2026-11-11: which state holds the stamp (codescout `experiments`)

Read with `grep`, `symbols` and `read_file`; nothing was run. Line numbers are those of 2026-11-11 and move.

- `src/server.rs:2141-2144`: the recorded id is `asserted_session.or(native_root).or(serving_session).unwrap_or_else(|| self.cc_session_id.clone())`. `native_root` is `Some` only for a Codex client (`NativeInvocation::Codex`, `src/server.rs` ~1877-1896).
- `src/server.rs:2091-2098`: `serving_session = asserted_back.or(rendezvous_session)`. The comment at ~2099-2112 says the `_meta` conversation tier "is still `None` on every live call". The principal stamp exists only where the companion composes one for a subagent.
- So a Claude Code call that carries no asserted principal and is not Codex reaches `rendezvous_session`, the result of `poll_rendezvous` (`src/server.rs:1470-1489`), which returns `rv.current()`.
- `Rendezvous` is one slot per server process: `publish` writes `<pid>.json` with a `session` field (`src/tools/rendezvous.rs`, `Entry.session`, `Rendezvous::publish`). `poll` re-reads the file when its mtime changes. The code comment on the Codex path states the known hazard: "several Codex servers share one ppid, so a slot can name ANOTHER thread's session" (`src/server.rs` ~1860-1866).
- This is consistent with the measured table above (the stamp moves to a Codex thread id and stays until the server session ends; a `/mcp` reconnect publishes a new slot and clears it). It is not proof: the code that WRITES `session` into a slot after `publish` is the companion hook, in `../claude-plugins/codescout-companion/hooks/`, which this read did not open.
- Open question that decides the repair: does a Codex hook write into the slot of a server process that a Claude Code session is also calling? Which slot a hook picks (by `ppid`?) is hook-side.

## Hook-level reproduction 2026-11-11 (isolated, detached processes)

What ran: the REAL `codescout-companion/hooks/session-start.mjs` (`../claude-plugins` at `338039d`, no uncommitted change under `hooks/`), on node v26.13.0, with `HOME`, `XDG_STATE_HOME` and `CLAUDE_CONFIG_DIR` pointed at a temporary directory and a probe project whose `.mcp.json` names a codescout server (that is what makes the hook proceed). No real server, no real `~/.codex`, no real slot and no real `usage.db` was touched. A "Claude" is a detached bash process (`setsid`) with a registry row `sessions/<pid>.json`, which is the first signal of `isClaudeProcess`, and one slot whose `ppid` is that bash pid. Scripts: `repro.sh` and `fake-claude.sh` under the session scratchpad (not durable).

| case | hook runs | slot before | slot after |
|---|---|---|---|
| A (control) | inside the fake Claude's tree, `session_id=claude-new-A` | `claude-outer-A` | `claude-new-A`, `source=startup` |
| B (the report) | inside a tree of the same shape, `session_id=codex-thread-B` | `claude-outer-B` | `codex-thread-B`, `source=startup` |
| C (negative) | detached, slot `ppid` NOT in the hook's ancestry, `session_id=codex-thread-C` | `claude-outer-C` | `claude-outer-C`, unchanged |

Reading: `ownAncestry()` (`hooks/lib.mjs`) walks up from the hook, adds each pid to the set, and stops AT the nearest Claude process, which it includes. `session-start.mjs` then stamps every slot whose `ppid` is in that set (`stampSlotIfStale`), whatever client the hook serves. A hook run by a Codex thread that a Claude Code session started from a tool call therefore has the outer Claude process as its nearest Claude, and it stamps the outer Claude's server slot with the Codex thread id. The 2026-09-24 nested-Claude fix, which stops at the nearest Claude, is the same rule that makes case B match. The server side of the chain (`poll_rendezvous` then `recorded_session`, section above) is read from code, not run here.

Limits, all real:

- The payload was Claude-shaped (`session_id`, `cwd`, `source`). No real Codex SessionStart payload was captured, so it is not shown that a real Codex hook delivers the same keys.
- The tree was simulated; no real Codex app-server was started under a real Claude Code session. Codex has the companion's `session_start` hook trusted in `~/.codex/config.toml` (read, not run).
- No `usage.db` row was produced; the link from the slot to the stamped `cc_session_id` rests on the code read above plus the measured table earlier in this file.
- Case B shows the hook CAN overwrite the slot. It does not show that this is what happened in the four incidents; their rows are consistent with it.

## Real Codex SessionStart payload 2026-11-11 (codex-cli 0.167.1)

Captured with `codex exec --dangerously-bypass-hook-trust` in an isolated `CODEX_HOME` (auth.json symlinked, one `SessionStart` hook that only copies stdin to a file). The real `~/.codex` hooks did not run, so no real slot and no `usage.db` row was touched.

Payload keys, values shortened:

```
session_id       01a1cad7-8178-7071-a02d-...   (UUIDv7 shape)
transcript_path  <CODEX_HOME>/sessions/2026/11/11/rollout-<ts>-<session_id>.jsonl
cwd              <the exec cwd>
hook_event_name  SessionStart
model            gpt-6.1-sol
permission_mode  bypassPermissions
source           startup
```

Findings:

- Without `--dangerously-bypass-hook-trust`, Codex did not run the untrusted hook (first run wrote nothing). Real installs run it only because `~/.codex/config.toml` holds a `trusted_hash` for `codescout-companion@sdd-misc-plugins:hooks/hooks.json:session_start:0:0`.
- The cached companion 1.21.0 in the Codex plugin cache has an `ownAncestry()` identical to the repo copy (diff empty).
- The Codex child inherited `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CONFIG_DIR` and `CLAUDE_PROJECT_DIR` from the launching Claude session. The environment alone does not tell the hook which client fired it.
- The payload carries no field that names the client.

Real Claude payload, captured the same way (`claude -p --setting-sources project --settings <capture hook> --no-session-persistence`, claude 2.1.322; the API call then failed with an expired OAuth session, but `SessionStart` had already fired):

```
session_id       01c0cc48-7310-44e4-abbb-...   (UUIDv4 shape)
transcript_path  <CLAUDE_CONFIG_DIR>/projects/<cwd-slug>/<session_id>.jsonl
cwd, hook_event_name=SessionStart, source=startup
(no model, no permission_mode)
```

Side-by-side, one capture each:

| Field | Codex 0.167.1 | Claude 2.1.322 `-p` |
|---|---|---|
| `model` | present (`gpt-6.1-sol`) | absent |
| `permission_mode` | present | absent |
| `session_id` version nibble | 7 | 4 |
| `transcript_path` | `<CODEX_HOME>/sessions/Y/M/D/rollout-<ts>-<id>.jsonl` | `<CLAUDE_CONFIG_DIR>/projects/<slug>/<id>.jsonl` |

Interactive Claude 2.1.322 (tmux, same capture hook), measured afterwards, CORRECTS the table: the payload HAS `model` (`claude-opus-5-5`) and still has no `permission_mode`. So the presence of `model` is not a discriminator; it was an artifact of `-p` mode. Only `permission_mode` (Codex yes, Claude no, one capture each) and the `transcript_path` / UUID-version shapes still differ.

Codex `app-server` (JSON-RPC over stdio, `initialize`, `thread/start`, `turn/start`, same isolated `CODEX_HOME`): the first run wrote NO payload. Cause, measured: the hooks were `trustStatus: "untrusted"` in the `hooks/list` response (both `enabled: true`), and `--dangerously-bypass-hook-trust` is an option of `codex exec`, not of `codex app-server` (it is absent from `codex app-server --help`; passing it to `codex` before `app-server` had no effect). Intervention: writing `[hooks.state."<hooks.json>:session_start:0:0"] trusted_hash = <currentHash from hooks/list>` into the isolated `config.toml` flipped `trustStatus` to `trusted`, and the next run captured both `SessionStart` and `UserPromptSubmit`. The `SessionStart` payload from `app-server` has the same keys as the `codex exec` one (`session_id`, `transcript_path`, `cwd`, `hook_event_name`, `model`, `permission_mode` = `default`, `source` = `startup`). `trusted_hash` alone was enough; `trusted_content_hash` was not needed. Not measured: whether `SessionStart` fires at `thread/start` or at the first `turn/start` (the two payload files were written 9 ms apart, `SessionStart` first).

Limits: one capture per client and entry point, `permission_mode` is `bypassPermissions` under `exec` with the bypass flag and `default` under `app-server`, so its value is not stable but its presence is so far. The fields that differ are candidates, not a contract. A hook test must not assert one until a second Claude payload confirms `permission_mode` is always absent.

Not shown by this capture: that the real `session-start.mjs` stamps a real server slot when it receives this payload (the repro above used a Claude-shaped payload), and that this causes the four incidents.

## Real-tree capture and hook-side fix 2026-11-12

Read-only capture in an isolated `CODEX_HOME` (auth.json symlinked, no MCP servers, no real hooks); scripts `tree.py`, `ancestry-probe.mjs`, `cap-hook.sh`, `run.sh` under the session scratchpad (not durable). The probe imports the REAL `ownAncestry` and `isClaudeProcess` from `claude-plugins/codescout-companion/hooks/lib.mjs`, lists the real rendezvous slots that the real rule `ancestry.has(e.ppid)` would stamp, and writes nothing.

- **Live Codex tree, observed with `/proc`:** Codex threads on the real daemon sit under `konsole > codex > codex app-server-daemon > codescout start`. Their servers (pids 212011, 2719103, 2779796, 2866810) have no Claude ancestor, so a hook there cannot reach a Claude-owned slot.
- **The incident shape, reproduced:** a `codex exec` started from a Claude tool call (`run_command`) has the chain `hook > bash > codex > bash > codescout > claude`. Before the fix, `ownAncestry` walked THROUGH the `codex` process and ended at the Claude process; the real rule listed the Claude session's own server slot (pid 961452, session `2e6121be`) as one it would stamp with the Codex thread id. Control: a hook inside the Claude tree lists the same slot, as it should.
- **Hook environment under Codex:** `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID` and `CLAUDE_CONFIG_DIR` are inherited from the launching Claude session; `CODEX_THREAD_ID` and `CODEX_SESSION_ID` are unset. The payload has `model` and `permission_mode`. No payload discriminator was needed.
- **Fix, in claude-plugins `f41d5aa` (patch-id `614efe5909fc1a93e7585d72b603e09479b04cb1`), COMMITTED, NOT RELEASED:** new `isCodexProcess` (comm `codex`, Linux only); `ownAncestry` in `lib.mjs` and in `session-start.mjs` stops at the nearest Claude OR Codex process. `session-start.test.sh` gains a nested-codex case: red on the old code (`a codex session stamped its ancestor's server`), green on the fix (48/48). `./tests/run-all.sh` passed. After the fix, the same real `codex exec` capture ends at the `codex` process and lists no slot.
- **What the fix does not cover:** an app-server started under Claude (only `codex exec` was captured); a Codex install whose process comm is not `codex` (an npm install runs as `node`); non-Linux hosts. Whether the four 2026-11-09/10 incidents had this tree shape is inferred: those processes are gone.
- **Not live:** the plugin repo bumps versions through `release.sh` only, and a hook edit is committed, not live, until a release seeds the install caches. No release was run. Until then the mechanism is still active here, and `/mcp` after a Codex probe remains the workaround.
- **Server side unchanged:** `poll_rendezvous` still re-keys the guide ledger when the slot's session changes. That a foreign stamp also re-arms the Claude session's guides is read from code, not observed.

## Resume

2026-11-09: source delivery is committed: plugin `381ae57` + documentation `bb8ff30`; Codescout support `505e8033a`, wiring `c7b38be29` and caller tests `f89590d31`. The guard classified 23 wiring paths under four unrelated IDs; three owned caller-test paths were committed separately. All precommit hooks passed.

The original notification to `01a1bf68-e650-7f20-911e-b53f86597c27` was accepted. After the operator manually approved the exact previously denied sends, retries to `01a15ca9-f939-7603-acb5-01c6b02f68e2` and `01a1bf89-3111-7c70-b934-5f6fd89102a3` were accepted at 20:10 UTC. The retry to `01a1c000-a771-79b2-8ebe-a72aeb509c54` passed the approval boundary but failed with thread not loaded (-32600). No reading acknowledgment or successful delivery is claimed for that inaccessible target. The operator's approved scope allowed documenting delivery failure honestly.

The guarded wiring commit records all four mandatory bare stamp IDs and explains that they are disputed usage identities, not peer authorship or review. The hook's warning that bare IDs are interpreted as Claude sessions is another limit of these legacy stamps; changing the required ACK IDs would not repair attribution.

2026-11-12: the cause on the hook side is reproduced with the real code and a real `codex exec`, and the fix is committed in claude-plugins `f41d5aa` (section above). The operator then marked this bug `fixed`, before the companion release and before the live check. That decision, not a measurement, closes it. Still owed: a release of the companion (operator), then a live check on the released build: start a Codex probe under a Claude session and confirm the Claude session's `cc_session_id` in `usage.db` does not move to the thread id. If a stamped row still moves after a release that contains `f41d5aa`, reopen this bug.

## Codex cache re-read, 2026-11-12 (session `5566e8a8`)

Read only; nothing in `~/.codex` was changed.

- `~/.codex/plugins/cache/sdd-misc-plugins/codescout-companion/` still holds only `1.21.0`. `grep -c isCodexProcess` on its `hooks/lib.mjs` prints `0`. So the fix is still not live where a Codex thread runs the hook.
- `~/.codex/config.toml` declares the marketplace as `[marketplaces.sdd-misc-plugins]` with `source_type = "local"` and `source = "/home/marius/work/claude/claude-plugins"`.
- `codex plugin marketplace upgrade --help` says it refreshes configured GIT marketplaces. By that text it does not apply to a `local` marketplace; I did not run it. The other commands are `codex plugin add` and `codex plugin remove` (the latter uninstalls a plugin and removes its local cache). Which of them seeds `1.21.5` into the Codex cache was not run, so it is not established.
- Cost to weigh before any of them: the hook entries in `config.toml` carry `trusted_hash` values for `codescout-companion@sdd-misc-plugins`. A changed hook file may need the operator to trust it again. That was not tested.

Open: the operator's decision on how to refresh the Codex cache, then the live check named under Fix provenance.

## Codex cache refreshed, 2026-11-12 (session `5566e8a8`, on the operator's order)

Ran `codex plugin add codescout-companion@sdd-misc-plugins` against the real `~/.codex`, after a copy of `config.toml` (scratchpad `codex-config.toml.bak`). It printed `Added plugin ... Installed plugin root: /home/marius/.codex/plugins/cache/sdd-misc-plugins/codescout-companion/1.21.5`. `remove` was not needed. `codex plugin marketplace upgrade` was not run.

- The cache directory now holds only `1.21.5` (the `1.21.0` directory is gone). `grep -c isCodexProcess` on its `hooks/lib.mjs` prints `2`, the same as the repo copy.
- `diff` of `config.toml` before and after is empty: no trust hash was added or removed.
- Trust: `hooks.json` changed after `1.21.0` was seeded, in claude-plugins `c93a23b` (2026-11-11): it adds two `wake-missed-jobs.mjs` entries, under SessionStart and UserPromptSubmit. They have no `trusted_hash` in `config.toml`, so by the trust rule seen on 2026-11-11 Codex will not run them until they are trusted. The entry that carries the fix, `session_start:0:0` (`session-start.mjs`), is unchanged in `hooks.json`, and its `trusted_hash` is untouched. Not verified: that Codex still lists that entry as trusted. I did not start a Codex session or call `hooks/list`. Also not verified: whether `trusted_hash` covers the script bytes or only the entry.
- `codex plugin list` timed out after 30 s, so it was not used.

Open: the live check (a Codex probe under a Claude session, then `cc_session_id` in `usage.db`). It uses the real config, so a failed fix would move the probing Claude session's stamp until `/mcp`.

## Live check, 2026-11-12 (session `5566e8a8`, on the operator's order)

One probe, on the real `~/.codex` with the refreshed 1.21.5 cache: `codex exec --skip-git-repo-check -s read-only -C /home/marius/work/claude/codescout "Reply with the single word OK and call no tools."`, started from a `run_command` of this Claude session. Codex thread `01a1d02d-4ec2-78a2-8242-58a0a1d2e103` answered `OK`. Its output lists two `SessionStart` hooks and two `UserPromptSubmit` hooks, each pair with one line `Completed` and one `Failed`, and a `Stop` hook line `Failed`. Which hook failed was not read.

Result, from `usage.db` (read-only): before the probe, this session's rows on server session `0d0fbe15` carried `cc_session_id` `5566e8a8-94fa-481e-82d0-3d2659355a5f`. The probe call recorded at 14:34:01 and the next call at 14:34:06, both after the thread was created, carry the same id. No row of this session carries the thread id. **The stamp did not move.**

What this does and does not show:
- It is one sample. I ran no control with the old 1.21.0 hook, so this run does not show that the old hook would have moved the stamp here. The incident table in this file shows the stamp moved within about 20 s of a thread's creation (19:56:20 to 19:56:41), and that is the interval this run covers.
- The Codex thread called no codescout tool. The check is about the SessionStart hook's effect on the slot, which is what the fix targets.
- Which of the `Completed` and `Failed` hooks was `session-start.mjs` was not read. If the failed `SessionStart` hook is the one that carries the fix, the unchanged stamp would be because it did not run, not because the fix worked. That is not excluded.
- The rendezvous slot itself was not read.

Open: read why a `SessionStart` hook reports `Failed` under Codex (the two new `wake-missed-jobs.mjs` entries have no trust hash, so they are the first suspects, not verified).

## Correction to the live check, 2026-11-14 (session `5566e8a8`)

**The live check above does not test the fix. The hook that carries the fix did not run.** Read the section above as void for the claim "the stamp did not move because of the fix".

What I read on 2026-11-13 and 2026-11-14:
- A second probe (`codex exec`, thread `01a1d1cb-cad0-71f0-9ef2-d47a1643dd7e`) printed the same hook lines: `SessionStart` `Completed` and `Failed`, `UserPromptSubmit` `Completed` and `Failed`, `Stop` `Failed`. The output names no hook. The rollout file of the thread has no hook records.
- buddy's `SessionStart` hook ran in that thread. Evidence: a `source=startup` line for the thread in `.buddy/.session-start-trace.log`, and the directory `.buddy/01a1d1cb-…/`.
- The companion `session-start.mjs` left none of its marks. `.codescout/cc_session_id` was last written at 00:03:51 local time, before the probe at 00:06:18, and holds another id. The thread directory has no `recon-loaded` file. The script itself is sound: `node --check` passes and `lib.mjs` exports `isCodexProcess`.
- `hooks/list` (app-server JSON-RPC, read-only, cwd = this checkout) returns `trustStatus: "modified"` for all 22 `codescout-companion` hooks, including both `session_start` entries. The hooks of buddy, `cs-hint-codex` and `~/.codex/hooks.json` return `trusted`.

Cause, from the 2026-11-07 and 2026-11-09 findings in codescout memory `codex-hook-content-trust-20261107`: Codex binds hook approval to the content of the whole plugin root, so any change in the cache cancels it, and the hooks then fail with `plugin contents changed; hook approval required`. The `codex plugin add` of 2026-11-12 (section "Codex cache refreshed") replaced `1.21.0` with `1.21.5`. The `trusted_content_hash` records in `~/.codex/config.toml` still hold the `1.21.0` value (`sha256:057a03b6…` on `session_start:0:0`). **That refresh made every companion hook fail in real Codex sessions on this host.** The earlier note said the config diff was empty, which is true, but it did not check `hooks/list`.

Not verified:
- That the `Failed` lines in the probe belong to the companion hooks. This follows from the trust status and from the missing marks, not from a hook name in the output. The trust status was read on 2026-11-14, after the probe, and `config.toml` changed in between (2026-11-13 09:57: declaration hashes for the two `wake-missed-jobs.mjs` entries, a reasoning-effort setting).
- Whether the fix works. No run with an executing hook exists yet.

Side effect of the probes: buddy's hook rewrote `.buddy/.current_session_id` with the Codex thread id. That file names the last writer, not this session. Skills read `$CLAUDE_CODE_SESSION_ID` first, so the damage is small.

Open:
- Renew the content approvals for the 22 companion hooks (operator step: edit `trusted_content_hash` as on 2026-11-07, or approve in Codex). I did not change `~/.codex/config.toml`.
- Then repeat the probe, and read `.codescout/cc_session_id` and the rendezvous slot, not only `usage.db`.
- Every plugin refresh of the Codex cache needs the same renewal. This is a cost of the refresh route used here.

## Fix provenance

- **SHA:** `claude-plugins:f41d5aa` (`f41d5aaf34149c84bc619e5468341648a7998610`, 2026-11-12, repo `claude-plugins`, path `codescout-companion/hooks/`)
- **patch-id:** `614efe5909fc1a93e7585d72b603e09479b04cb1`
- **Release state (2026-11-12, after the operator ran `release.sh`):** released as codescout-companion 1.21.5 (`claude-plugins:3a0c275`). Read in the install caches: all four Claude profile caches (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`, `~/.claude-stef`, under `plugins/cache/sdd-misc-plugins/codescout-companion/1.21.5/hooks/`) hold `isCodexProcess`. The CODEX cache does NOT: `~/.codex/plugins/cache/sdd-misc-plugins/codescout-companion/` holds only `1.21.0` (seeded 2026-11-09), and its `lib.mjs:588` and `session-start.mjs:141` still stop only at `isClaudeProcess`. A Codex thread runs the hook from that cache, and a Codex thread's hook is the one that stamps the Claude slot, so the fix is not live where the bug occurs until Codex refreshes its plugin cache. `release.sh` does not seed that cache: Codex's `sdd-misc-plugins` marketplace is the local `claude-plugins` repo, and how Codex refreshes its cache was not determined.
- **What it covers:** the `codex exec` tree `hook > bash > codex > bash > codescout > claude`, reproduced with the real `ownAncestry` and fixed by stopping at the nearest Claude or Codex process. It does not cover an app-server started under Claude, a Codex install whose comm is not `codex`, or non-Linux hosts.
- **Live check:** not run. Until the Codex cache holds 1.21.5 a Codex probe would run the old hook and could only show the bug, and it would move this Claude session's slot. After the Codex cache refresh: start a Codex probe under a Claude session and confirm that session's `cc_session_id` in `usage.db` does not move to the thread id.
