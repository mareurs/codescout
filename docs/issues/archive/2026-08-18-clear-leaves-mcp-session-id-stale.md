---
kind: bug
status: fixed
tags:
- session-identity
- guide-ledger
- usage-db
- claude-code
- cluster/blast-radius-exceeds-visibility
closed: 2026-08-18
opened: 2026-08-18
owner: marius
related: []
severity: high
---

# BUG: `/clear` mints a new Claude Code session id without respawning the MCP subprocess, so codescout serves the new conversation under the old session id

## Summary

Claude Code's `/clear` starts a new conversation with a new session id but does **not**
restart stdio MCP subprocesses. codescout reads `CLAUDE_CODE_SESSION_ID` once, at server
construction (`src/server.rs:250-262`), so after a `/clear` every call from the new
conversation is served under the **previous** conversation's id. Two consequences: the
guide-hint ledger for the old conversation is applied to the new one (guides the new
context has never seen are suppressed), and `usage.db` misattributes the new
conversation's calls to the old session.

## Symptom (Effect)

A conversation created by `/clear` receives no guide injection for topics the *previous*
conversation consumed, despite holding none of them in context. The new session id never
appears in `usage.db` or in `.codescout/guide_hints/`.

Measured live on 2026-08-18 in `/home/marius/work/claude/claude-plugins` (user ran
`/clear` and re-activated the project on each side):

```
06:22:59  workspace      cc_session=ad86daef…  INJECT     <- session A opens
06:23:06  artifact       cc_session=ad86daef…  INJECT     <- librarian delivered to A
          ---------- /clear -> new session eea6e33a… ----------
06:23:42  artifact       cc_session=ad86daef…  (none)     <- B gets NO librarian guide
06:23:46  run_command    cc_session=ad86daef…  (none)
06:23:52  artifact       cc_session=ad86daef…  INJECT
06:23:52  read_markdown  cc_session=ad86daef…  (none)
06:24:09  workspace      cc_session=ad86daef…  INJECT
```

All 8 calls carry codescout's per-process id `fac51532-b856-4a78-a768-cee51889df84` —
**one** MCP process spanning **two** conversations.

## Reproduction

1. Open Claude Code in a project with codescout configured.
2. `pgrep -x codescout` — note the pid. Make a codescout call that triggers a guide
   (e.g. `workspace(action="activate", …)`).
3. `/clear`.
4. `pgrep -x codescout` — **same pid**.
5. Make the same kind of call again. No guide is injected, and
   `tr '\0' '\n' < /proc/<pid>/environ | grep CLAUDE_CODE_SESSION_ID` still shows the
   pre-`/clear` id while `~/.claude/projects/<slug>/` has a new transcript file named
   with the post-`/clear` id.

Verified against `codescout` on branch `experiments`, Claude Code 2.1.234.

## Environment

Linux, stdio MCP transport, Claude Code 2.1.234 (`AI_AGENT=claude-code_2-1-234_harness`),
codescout `experiments`. Three CC profiles on this host (`~/.claude`, `~/.claude-sdd`,
`~/.claude-kat`) — reproduced in `~/.claude`.

## Root cause

`CLAUDE_CODE_SESSION_ID` is captured once into `ServerEnv` at process start
(`src/server.rs:73-76`) and resolved once into `CodeScoutServer::cc_session_id`
(`src/server.rs:250-262`); nothing re-reads it. The environment of a running process
cannot be mutated by its parent, so a session-id change that does not respawn the
subprocess is unobservable to the server by construction.

measured 2026-08-18: `pgrep -x codescout` + `/proc/<pid>/environ` + the `usage.db` rows
above — one codescout pid, one `cc_session_id`, two Claude Code transcript files.

Not a regression: the same single-read design is what
`docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md` introduced, and it
is correct for every lifecycle event that *does* respawn the subprocess (`/mcp` reconnect,
`claude --resume`, `--fork-session`). `/clear` is the one event that changes the id
without respawning.

## Evidence

### Lifecycle matrix (measured 2026-08-18)

| Event | New session id? | Subprocess respawns? | Ledger correct? |
|---|---|---|---|
| `/mcp` reconnect | no | yes | yes — persisted ledger reloads |
| `/compact` | no | no | yes — id unchanged |
| `claude --resume <id>` | no | yes | yes |
| `--fork-session` | yes | yes | yes — new env, new ledger |
| **`/clear`** | **yes** | **no** | **NO — stale id** |

`/compact` evidence: session `2c518eb6-45d3-415d-aebe-8335b96191da` is a single transcript
of 71,891 records / 137.5 MB spanning 2026-08-06 → 2026-08-18 with exactly **one** distinct
record-level `session_id`. A conversation that size has compacted many times; the id never
changed.

### Ledger state after the reproduction

```
PRESENT ad86daef-036c-4cc5-8a34-e1ea5a79514b ->
        ["tracker-conventions","workspace-state","librarian","project-activation-bootstrap"]
ABSENT  eea6e33a-1e27-459d-8425-7659a69a9f06
```

### Currently masked by a second defect

`activate_project` clears the whole guide ledger on **every** call
(`src/tools/config/mod.rs:139`). Because the post-`/clear` conversation is normally
re-activated, that unconditional wipe re-arms the guides and accidentally rescues it. The
starvation is only visible on calls made **before** the first post-`/clear` `activate` —
rows 3 and 4 of the trace above.

**This matters for the in-flight design work.** The planned change (re-arm only on a genuine
project-root change, so same-project re-activation stops wiping the ledger) removes that
accidental rescue and would convert an occasional token cost into systematic guide
starvation after every `/clear`. Prior art for the consequence: Serena implemented
once-per-session suppression keyed on session identity, hit the same class of failure
("some clients use the same session across multiple chats"), and reverted it, leaving a
dead `if …: pass` at `agent.py:1071` as a regression guard.

### Second consumer affected

`usage.db` attribution. This is the same class as
`docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md`
(fixed 2026-08-16), which moved attribution off a shared per-project file precisely so
concurrent sessions could be told apart. `/clear` re-opens the hole by a different route:
the id is per-process-correct but conversation-stale.

### `/compact` is a SECOND trigger for the same staleness — measured live 2026-08-18 ~10:25

Observed from inside a compacted session, while executing Phase A of the guide-ledger
plan. Not a re-run of the `/clear` trace below; an independent lifecycle event producing
the same stale-file end state.

The companion plugin's SessionStart hook announced the id change explicitly:

```
<!-- buddy:reloaded sid=55515bc5-b26d-47a9-ba77-852f3b0b10e8 from=44c01c0f-dae2-4f97-a01b-dc580f0b2dc8 source=compact -->
```

So `/compact` minted `55515bc5…` from `44c01c0f…`. What the on-disk state showed
afterwards:

```
$ cat .codescout/cc_session_id
44c01c0f-dae2-4f97-a01b-dc580f0b2dc8        # STALE — the pre-compaction id

$ ls -la .codescout/guide_hints/            # both ids have a ledger
-rw-r--r-- 66 10:22:56 44c01c0f-dae2-4f97-a01b-dc580f0b2dc8.json
-rw-r--r-- 57 10:25:11 55515bc5-b26d-47a9-ba77-852f3b0b10e8.json
```

**Why this matters beyond `/clear`.** The construction site at `src/server.rs:250-265`
resolves the key as `env.cc_session_id` **or else** the contents of
`.codescout/cc_session_id`. The env var was correct here, so the live ledger was written
under the right id (`55515bc5….json`, freshly stamped). But the *file* fallback is now
actively wrong rather than merely absent: any process that reaches the second arm — a
server spawned without the env var, a sibling agent, a worktree reading the symlinked
`.codescout/` — would attribute this conversation's guides to the previous one. A stale
value is worse than a missing one, because the missing case falls through to a fresh
UUID and merely re-sends a guide, while the stale case reads *another conversation's*
ledger and suppresses guides that were never delivered here. That is the
"degrade to re-sending, never to suppressing" invariant, violated.

This strengthens the case for the spec's ranked key chain (§1): the file fallback needs
either a refresh on every lifecycle event, or demotion below a source that cannot go
stale.

### Ledger accumulation in the project tree, measured the same moment

```
$ ls -1 .codescout/guide_hints/*.json | wc -l
61
$ du -sh .codescout/guide_hints/
244K
```

61 ledger files for one repo, none ever collected. This is the concrete case for the
§8 GC (Phase A Task 5) and for §2's move out of the project tree — both were argued from
reasoning; this is the measurement.

### The legacy `Vec<String>` shape is live on disk, not hypothetical

```
$ cat .codescout/guide_hints/55515bc5-b26d-47a9-ba77-852f3b0b10e8.json
["project-activation-bootstrap","progressive-disclosure"]
```

Phase A Task 2's `LedgerFile::Legacy` migration path will meet this shape in every
existing file on every machine, so the migration is load-bearing rather than defensive.

### Unexplained: a topic re-injected while present in its own ledger

In the same session, `progressive-disclosure` was auto-injected twice with the
`"First call this session for topic 'progressive-disclosure'"` preamble — the second time
while the ledger on disk already listed it (see the `cat` above). Several intervening
tool calls that also produced buffered output did **not** re-inject, so it is not firing
unconditionally.

**Cause undetermined — recorded as an observation, not a mechanism.** No `activate` call
was made in the interval (which would explain it via D1), and a plain MCP restart should
have re-read the file and found the topic present. Worth reproducing deliberately before
anyone designs around it; do not treat this paragraph as a diagnosis.
### Narrowing the re-injection: it proves an in-memory reset, and subagents share the ledger

Third occurrence observed 2026-08-18 ~10:45. Correlates with `run_command` calls that
return a `@cmd_*` envelope — `RunCommand::relevant_guide_topic`
(`src/tools/run_command/mod.rs:96-98`) returns `Some("progressive-disclosure")`
unconditionally, and the fire condition includes `output_id` being present, so every
buffered `run_command` is guide-eligible.

**The narrowing.** `src/tools/core/types.rs:714` guards the second branch with:

```rust
} else if let Some(topic) = self.relevant_guide_topic(&val) {
    if emitted.contains(topic) {
        None
```

So a re-fire is only reachable when the **in-memory** set lacks the topic. The on-disk
file demonstrably had it. Therefore the in-memory ledger was reset between occurrences —
this is no longer "cause unknown", it is "something cleared `emitted`, and the disk file
was not the thing consulted."

`clear()` has exactly two call sites (`grep guide_hints_emitted\.lock\(\)\.clear`):

- `src/tools/config/mod.rs:139` — `ActivateProject::call`, the unconditional pre-validation
  wipe (defect **D1** in the spec).
- `src/tools/config/mod.rs:278` — `ProjectStatus::call`, the legitimate `post_compact` clear.

**Unconfirmed hypothesis, recorded as a lead rather than a diagnosis:** these occurrences
fell in a subagent-driven session, and **subagents share the parent's MCP server and
therefore the same `ctx.guide_hints_emitted`**. Any subagent calling
`workspace(action="activate")` — a natural orienting move for a fresh agent — trips D1 and
wipes the *parent's* ledger. The timing fits (re-fires followed subagent completions), but
no subagent transcript was inspected to confirm an `activate` call, so this is not
established. Cheap test for a later session: run a subagent that calls
`workspace(action="activate")` and watch whether the parent's next buffered `run_command`
re-injects `progressive-disclosure`.

**Design consequence that holds either way, and belongs in the spec regardless of what
caused this instance:** D1's blast radius is not confined to the agent that calls
`activate`. In any multi-agent session sharing one MCP server, one subagent's activation
clears every sibling's and the parent's ledger. That raises the cost of the blunt `clear()`
above what the spec's §4 currently argues, and is an additional reason the re-arm
predicate must be surgical rather than total.
## Hypotheses tried

1. **Hypothesis:** `/compact` also mints a new session id (claimed by
   `JoshuaDavid/mcp-session-id-example-for-claude-code`).
   **Test:** counted distinct record-level `session_id` values across all 152 transcripts on
   the host; inspected the 12-day session directly.
   **Verdict:** rejected — no transcript carries more than one `sessionId`, and the
   71,891-record session holds exactly one.
2. **Hypothesis:** sessions that drove codescout but are missing from `usage.db` ("ghosts")
   are evidence of stale env.
   **Test:** dated them; checked `usage.db` retention and the 2026-08-16 attribution fix.
   **Verdict:** rejected — all five ghosts fall between 2026-07-19 and 2026-07-30, i.e.
   before the attribution fix. Zero ghosts after it.
3. **Hypothesis:** two claude-plugins transcripts with no matching codescout process show
   the stale-env case.
   **Test:** counted codescout calls in each.
   **Verdict:** rejected — both have zero codescout calls; that window had no codescout
   server at all.
4. **Hypothesis:** `/clear` changes the id without respawning the subprocess.
   **Test:** the reproduction above.
   **Verdict:** **confirmed.**

## Fix

Implemented in Phase B of the guide-ledger session-identity design
(`docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md`). Summary —
full mechanism in the spec:

1. **Ranked session-key resolution** (rank 1 `CODESCOUT_SESSION_ID`, rank 2 known-harness
   env vars including `CLAUDE_CODE_SESSION_ID`, rank 3 a per-request `_meta` key, rank 4
   none) replaces the single construction-time env read at `src/server.rs:250-262`.
2. **Two-tier ledger.** Keyed tier persists per conversation id (survives `/mcp`
   reconnects, no idle TTL — the rendezvous below is the detection mechanism). Anonymous
   tier is in-process only, never persisted, and re-arms after a 2-hour idle TTL
   (`CODESCOUT_GUIDE_TTL_SECS`) when no identity is obtainable.
3. **Companion `SessionStart` hook rendezvous.** The server publishes a pid-keyed slot at
   `$XDG_STATE_HOME/codescout/servers/<pid>.json` at construction (MCP `initialize` runs
   before `SessionStart`, so the server must publish first); `session-start.mjs` stamps the
   live session id into the matching slot on every `source`, including `"clear"`. The
   server polls the slot **before** consulting the ledger on each tool call and, on a
   detected change, re-arms the **whole** ledger (not just the project-scoped topic) and
   switches to the new key.

**Fix SHA (`experiments`):** `5bdb7f45..feb845aa`.
**Companion hook:** `codescout-companion:b8ffa8b`.

`git rev-list --left-right --count master...experiments` → `0\t1066` (measured
2026-08-18): `0` on the left means `master` is a strict ancestor of `experiments`, so
promotion is a **fast-forward** — the `experiments` SHA range above already is the
eventual `master` range, with no separate master-side SHA to mint or record.
## Tests added

- `session_change_rearms_everything` (`src/server.rs:4821`) — a new conversation id
  re-arms the WHOLE ledger, not just the project-scoped topic.
- `a_tool_call_polls_the_rendezvous_and_re_arms` (`src/server.rs:4934`) — pins that the
  rendezvous poll runs **before** the ledger is consulted, so a detected session change
  takes effect on the *same* response rather than one call late.
- Companion side: `codescout-companion:b8ffa8b` added rendezvous assertions to
  `hooks/session-start.test.sh` — cross-window isolation, full-field round-trip, and the
  `hook_at` wire format.
## Workarounds

Run `workspace(action="activate", …)` after `/clear` — its unconditional ledger wipe
re-arms every guide. This is the accidental mitigation described above, and it disappears
if the in-flight re-arm change ships without item 1 of the Fix.

## Resume

N/A — fixed. Fast-forward promotion applies (`git rev-list --left-right --count
master...experiments` → `0` on the left, measured 2026-08-18), so there is no
pending-master-SHA to record: the `experiments` SHA above already is the eventual
`master` SHA. No further action.
## References

- `src/server.rs:73-76`, `src/server.rs:250-262` — single-read of `CLAUDE_CODE_SESSION_ID`
- `src/tools/guide_ledger.rs` — the ledger this corrupts
- `src/tools/config/mod.rs:139` — the unconditional clear that currently masks it
- `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md` — introduced the
  persisted ledger
- `docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md`
  — same class, different route
- `docs/trackers/bug-fix-session-log.md` — F-52, F-53, W-45 from the same design session
- `docs/trackers/reconnaissance-patterns.md` — R-105 (derived keys and lifecycle events)
