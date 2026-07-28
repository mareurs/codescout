---
kind: bug
status: fixed
opened: 2026-06-14
closed: 2026-06-14
severity: low
owner: marius
related: []
tags: [get_guide, guide_hints_emitted, mcp-restart, session-scope, token-efficiency]
---

# BUG: auto-injected get_guide bodies re-fire on every MCP restart within one CC conversation

## Summary
The `guide_hints_emitted` ledger that suppresses repeat guide injection is a
bare in-process `HashSet<String>` on `CodeScoutServer`. It is reborn empty on
every MCP **process restart** (and cleared on every `workspace(activate)`), with
no link to the disk-persisted `cc_session_id`. Because codescout development
restarts the server constantly (`cargo build --release` + `/mcp`), the same
auto-injected guide (`librarian`, `progressive-disclosure`) re-injects its full
~1–2k-token body into a Claude Code conversation that — across a pure `/mcp`
restart — still holds the earlier copy. Pure context waste.

**Distinct from the FIXED `2026-06-11-get-guide-no-session-dedup.md`** — that bug
was `GetGuide::call` being *stateless w.r.t. the ledger* (explicit re-calls
within one process). This bug is the ledger's *lifetime*: it tracks the OS
process, not the model's context.

## Symptom (Effect)
The same guide topic is auto-injected more than once in a single CC
conversation. Each restart re-arms it; the first triggering tool call after the
restart re-emits the full body + the "don't re-call this session" hint.

**Live evidence (this session, `c38fc7f3…`):** the pre-compaction segment ran a
heavy `artifact`-driven legibility campaign (so `librarian` was emitted), yet
this turn `librarian` AND `progressive-disclosure` both re-injected as *"First
call this session for topic X."* The turn's first action was
`workspace(action="status", post_compact=true)` — which routes to
`ProjectStatus::call`, **not** the activate path, so it did **not** clear the
ledger. The only remaining cause is a fresh `CodeScoutServer` (resume = new
process = new `Uuid` session_id = empty ledger).

## Reproduction
1. In a running CC conversation, trigger an auto-inject (e.g. call `artifact(...)`
   → `librarian` guide injects once).
2. `cargo build --release` then `/mcp` to restart the codescout server (the
   documented codescout dev loop). The CC conversation context is **unchanged**.
3. Call `artifact(...)` again → `librarian` guide injects a **second** time,
   labelled "first call this session", though its body is still in context from
   step 1.

`workspace(activate)` is a second, independent reset trigger (see Root cause #2).

**Confirmed live, in-band (2026-06-14, conversation `c38fc7f3`).** The owner ran
`/mcp` ("Reconnected to codescout") mid-conversation — no compaction. The
prediction was stated *before* the trigger: a fresh `CodeScoutServer` ⇒ empty
ledger ⇒ the next auto-inject re-fires. The immediately-following `artifact(find)`
returned `_guide_hint: "First call this session for topic 'librarian'"` **plus the
full ~750-token librarian guide body** — while the identical body from earlier the
same turn was still present in context. Decisive: a controlled restart
reproduced the re-injection on demand, prior copy still live, zero compaction.
## Environment
codescout v0.15.0, MCP stdio transport. Affects every consumer but is most
acute in codescout self-development, where `/mcp` restarts are frequent.

## Root cause
The ledger's scope is the OS process, not the model's live context.

1. **In-process, restart-volatile.** `CodeScoutServer.guide_hints_emitted`
   (`src/server.rs:59-61`) is `Arc<Mutex<HashSet<String>>>`, constructed empty.
   `session_id` (`src/server.rs:181`) is `uuid::Uuid::new_v4().to_string()` —
   minted fresh per process. The ledger itself isn't even keyed by session_id;
   it is process-scoped by construction. **MCP restart → new struct → empty
   ledger → re-injection.**
2. **Cleared on activate.** `ActivateProject::call` (`src/tools/config/mod.rs:121`)
   runs `ctx.guide_hints_emitted.lock().clear()` as its first statement
   (intentional + documented in `src/prompts/guides/workspace-state.md`).
3. **A CC-stable id already exists but is unused by the ledger.** The usage
   recorder reads `cc_session_id` from the on-disk file
   `.codescout/cc_session_id` (`src/usage/mod.rs:88-92`, added by the v0.10
   migration `src/usage/db.rs:64-70`). This id survives MCP restarts. The guide
   ledger does not consult it.

## The design tension (why the obvious fix is wrong)
The ledger *should* answer "is guide X currently in the model's live context?"
It actually answers "was X emitted during this process's life." These diverge in
**both** directions:

| Event | Process | Model context | Current ledger | Desired |
|---|---|---|---|---|
| `/mcp` restart | dies → new | **intact** | forgets → **re-injects** | suppress |
| `/compact` | lives | **summarized away** | remembers → suppresses | re-inject |

A naive fix (persist the ledger keyed by `cc_session_id`) would correct the
`/mcp` row but **worsen** the `/compact` row: `cc_session_id` is stable across
`/compact` too, so a cc-keyed ledger would suppress the re-injection that
compaction *needs*. A correct fix must distinguish "context still holds it" from
"context lost it" — which neither the process boundary nor `cc_session_id`
captures alone. A compaction signal (the SessionStart `source=compact` hook) may
be the missing input. **Left open deliberately — see Iron Law of systematic
debugging: no fix without the design settled.**

## Evidence
- `src/server.rs:59-61` — ledger field + "Reset on workspace(activate)" doc.
- `src/server.rs:181` — `session_id: uuid::Uuid::new_v4().to_string()` (per-process).
- `src/tools/config/mod.rs:49-78` — `Workspace::call` dispatcher: `post_compact`
  ⇒ `status` ⇒ `ProjectStatus` (no clear).
- `src/tools/config/mod.rs:120-121` — `ActivateProject::call` clears the ledger.
- `src/usage/mod.rs:88-92` — recorder reads `.codescout/cc_session_id` (the
  CC-stable id the ledger could use).
- Live: this session re-injected `librarian` + `progressive-disclosure` post-resume.

**Empirical magnitude** (this project's `usage.db`, debug builds — distinct
per-process `session_id`s grouped by the CC-stable `cc_session_id`):

| cc_session (8-char) | distinct MCP processes | tool calls |
|---|---|---|
| f0099ed2 | 21 | 1371 |
| 7575e164 | 10 | 837 |
| c38fc7f3 (this session) | 8 | 895 |

One CC conversation routinely spans 6–21 MCP processes — each a ledger reset.
Worst case, an auto-inject topic re-fires once per process (~8–21× per
conversation). The query:
```sql
SELECT cc_session_id, COUNT(DISTINCT session_id) mcp_procs, COUNT(*) calls
FROM tool_calls WHERE cc_session_id IS NOT NULL AND cc_session_id != ''
GROUP BY cc_session_id HAVING mcp_procs > 1 ORDER BY mcp_procs DESC;
```
## Hypotheses tried
1. **My turn-start `workspace(post_compact=true)` cleared the ledger.** Test:
   read the dispatcher — `post_compact`+no-action ⇒ `status` ⇒ `ProjectStatus::call`;
   the `clear()` is only in `ActivateProject::call`. **Verdict: rejected** — status
   does not clear.
2. **Same as the fixed 2026-06-11 bug.** Test: that bug was explicit `get_guide`
   re-calls (now stateful via the ledger); this is auto-inject re-firing because
   the ledger itself was reset. **Verdict: rejected — distinct facet.**
3. **Confirmed:** in-process ledger + fresh `Uuid` per process ⇒ restart resets ⇒
   re-injection. Decisive: live re-injection this session with no activate call.

## Fix
*Not applied — design open (see "The design tension").* Candidate directions to
weigh with the owner: (a) persist emitted-topics keyed by `cc_session_id`, plus a
compaction signal that re-arms on `source=compact`; (b) accept restart
re-injection as the cost of in-process simplicity and document it; (c) shrink the
auto-inject bodies so a re-injection costs less. No `ONBOARDING_VERSION` impact
(guides load fresh per call).

**Feasibility of option (a) — confirmed (2026-06-14, checked `claude-plugins`).**
The two inputs already exist and are live:
- CC-stable id writer: `codescout-companion/hooks/session-start.sh:21` writes
  `$SESSION_ID` to `.codescout/cc_session_id`. SessionStart does NOT fire on
  `/mcp` restart, so the file is stable across exactly the breaking event.
- Compaction signal: `session-start.sh:167` already branches on
  `[ "$SOURCE" = "compact" ]` (it emits the POST-COMPACT reminder — proven live
  this session). The design-doc `PreCompact`/`PostCompact` hooks were never
  shipped (absent from `hooks/`) and are not needed.

**Critical correctness constraint:** the epoch must itself be **disk-persisted**,
not in-memory — an in-memory epoch resets on the same MCP restart it is meant to
survive. Two shapes:
- **1A (cross-repo, deterministic):** `session-start.sh` bumps `.codescout/guide_epoch`
  when `SOURCE=compact`; codescout keys a disk-persisted ledger on
  `(cc_session_id, guide_epoch)`.
- **1B (codescout-only):** codescout bumps a disk-persisted `.codescout/guide_epoch`
  when it receives `workspace(post_compact=true)` (a signal it already gets —
  returns `{flushed:true}`). Self-contained, but depends on the model issuing the
  `post_compact` call (reliable due to the strong SessionStart reminder).

Either way both `guide_hints` and `guide_epoch` live under `.codescout/` so they
survive `/mcp` restart, re-arm on `/compact`, and reset on new `cc_session_id`.
**[SUPERSEDED — see the CORRECTION further below: `CLAUDE_CODE_SESSION_ID` *is*
available since CC v2.1.154, verified live.]** ~~Hard constraint discovered
(2026-06-14, claude-code-guide, doc-cited): an MCP server cannot self-identify its
CC session.~~ CC sets only `CLAUDECODE=1` in MCP
subprocess env — no `CLAUDE_SESSION_ID`; the MCP `initialize` handshake carries no
session id; hooks receive `session_id` in stdin JSON but have no channel to the
MCP process. CC treats MCP servers as long-lived and session-agnostic by design.

**This kills the clean per-`(cc_session_id, epoch)` ledger.** A per-session path
(`.codescout/sessions/{sessionId}/…`, the right storage shape) has a chicken-and-egg:
codescout can't pick its own `{sessionId}` dir. The single-file bridge
(`.codescout/cc_session_id`, written by `session-start.sh:21`) is **last-writer-wins
under concurrent sessions on one project** — a pre-existing latent bug the usage
recorder already inherits (mis-attributes rows when two CC instances share a repo).

**Revised option set:**
- **A — pointer mitigation (recommended for severity:low).** On auto-inject emit a
  ~25-token pointer ("relevant guide: librarian — `get_guide('librarian')` if not
  already in context") instead of the ~600–750-token body. No persistence, no
  session id, concurrency-safe. Turns ~750×N into ~25×N. Cost: relies on the model
  judging "already in context" (prompt-craft sensitive; avoid negation-only wording).
- **B — ppid-correlation persistence (deterministic, heavier).** codescout derives
  a stable per-CC-instance key by walking `/proc/self` to its `claude` ancestor PID
  (+ `/proc/<pid>` start-time to survive PID reuse), persists the ledger keyed on
  that. Survives `/mcp` restart, distinguishes concurrent instances, no session id
  needed — but `/proc`-only (Linux), heuristic ancestor match, cross-platform gaps.
- **C — CC feature request.** Ask Anthropic to expose `CLAUDE_SESSION_ID` to MCP
  subprocesses (mirrors the hook `session_id`). Clean long-term; out of our hands.

The earlier `(cc_session_id, epoch)` design (1A/1B above) is **withdrawn** — it
assumed codescout could read a per-session id, which it cannot do concurrency-safely.
---

### CORRECTION (2026-06-14): the session id IS available — proper fix is feasible

The "MCP can't self-identify its session" constraint above is **WRONG / stale**.
Claude Code ships **`CLAUDE_CODE_SESSION_ID`** in stdio MCP subprocess env since
**v2.1.154** (2026-05-28). Verified empirically in this very process:
```
printenv CLAUDE_CODE_SESSION_ID  → c38fc7f3-7918-4f76-9f54-2b6d17343d6a
cat .codescout/cc_session_id     → c38fc7f3-7918-4f76-9f54-2b6d17343d6a   (identical)
```
This process was spawned by a `/mcp` reconnect on a resumed session, yet the var
is present — proving it (a) **survives `/mcp` restart** and (b) is **per-process**
(concurrency-safe; each CC window gets its own). The earlier A/B/C/proc-hack
analysis is **superseded**.

**THE FIX (persistent, session-keyed, compaction-aware ledger):**
1. **Session identity** — read `CLAUDE_CODE_SESSION_ID` at server start; fallback
   chain: env → `.codescout/cc_session_id` file → random uuid (graceful degrade for
   CC < v2.1.154 / non-CC clients). Keep the existing process-uuid `session_id`
   for usage.db unchanged; this is a separate cc-session key for the ledger.
2. **Persist the ledger** keyed by cc-session-id — e.g. a `guide_hints(session_id,
   topic)` table in `.codescout/usage.db` (no file proliferation), or
   `.codescout/guide_hints/{session_id}.json`. Load into the in-memory
   `guide_hints_emitted` set at startup; write-through on each insert. Survives
   `/mcp` restart → **no re-injection**.
3. **Re-arm on compaction** — on `workspace(post_compact=true)` (a signal codescout
   already receives), **clear** the session's persisted + in-memory topics so guides
   re-inject after `/compact` (context was summarized away). Backstop: SessionStart
   `source=compact` hook.
4. **Keep V2 full-body delivery** — no compliance regression; the body just stops
   re-firing per restart.
5. `workspace(activate)` continues to clear (now clears the persisted copy too).

**Open verification before shipping:** is `CLAUDE_CODE_SESSION_ID` set on a
**fresh** (non-`--resume`) session, or only on resume? (Changelog ambiguous;
v2.1.163 added it “explicitly on --resume”, implying v2.1.154 covered fresh start —
unconfirmed.) The file fallback covers any gap; verify with a fresh `claude`
session + `printenv CLAUDE_CODE_SESSION_ID` inside an MCP `run_command`.

**Research provenance:** two `claude-code-guide` agents disagreed (one said no such
var; one found v2.1.154). Direct env inspection — ground truth — confirmed it exists.
Related: anthropics/claude-code #25642 (closed dup), #41836 (HTTP-transport session
id, still open — distinct from the stdio env var).
---

### IMPLEMENTED (2026-06-14) — experiments-side; cite master SHA after cherry-pick

Shipped the persistent-ledger design above:
- **New `src/tools/guide_ledger.rs`** — `GuideLedger` newtype (`path` +
  `HashSet<String>`) with `load`/`contains`/`insert`/`clear`; `insert`/`clear`
  write through to `.codescout/guide_hints/<session_id>.json`. `#[derive(Default)]`
  ⇒ ephemeral (no path) so the 30+ internal/test `ToolContext` builders compile
  unchanged.
- **`CodeScoutServer::from_parts`** resolves the session id
  (`CLAUDE_CODE_SESSION_ID → .codescout/cc_session_id file → random uuid`) and
  loads the persisted ledger. `ToolContext.guide_hints_emitted` field type
  `HashSet<String>` → `GuideLedger` (transparent — only `contains`/`insert`/`clear`
  are ever called on it).
- **`ProjectStatus::call`** clears the ledger on `post_compact=true` (compaction
  re-arm); `ActivateProject` clear now also removes the persisted file.
- **Docs updated same-change:** `src/prompts/guides/workspace-state.md`,
  `iron-laws-detail.md` (no `ONBOARDING_VERSION` bump — guides load fresh per
  call). Memory `claude-code-mcp-env` records the env-var fact.

Result: a `/mcp` restart reloads the ledger (no re-injection); `/compact` re-arms;
concurrent CC windows isolate by session id.
## Tests added

Four, all green (2722 lib tests pass; `clippy --all-targets -- -D warnings` clean):
- `tools::guide_ledger::tests::ledger_survives_reload_and_isolates_sessions` —
  persistence across reconstruction + per-session isolation + clear-persists.
- `tools::guide_ledger::tests::ephemeral_ledger_is_in_memory_only`.
- `server::guide_hint_tests::guide_ledger_survives_mcp_restart` — two server
  incarnations on one project with a pinned `CLAUDE_CODE_SESSION_ID`; the second
  reloads the persisted ledger (the regression bar); a different session stays
  isolated.
- `server::guide_hint_tests::post_compact_rearms_guide_hints` — emit → post_compact
  → re-emit (the compaction re-arm wiring).
## Workarounds
In dev, avoid triggering the auto-inject right after a `/mcp` restart if the
guide is already in context. There is no way to suppress it from the model side.

## Resume
Decide whether the `/mcp`-restart waste justifies a persistent, compaction-aware
ledger, or whether (b)/(c) suffice. If fixing: the join key is `cc_session_id`
(`src/usage/mod.rs:88`); the missing input is a compaction signal; the ledger
owner is `src/server.rs:59-61`.

## References
- Sibling (fixed): `docs/issues/2026-06-11-get-guide-no-session-dedup.md`.
- Ledger: `src/server.rs:59-61,181`. Reset: `src/tools/config/mod.rs:121`.
- Auto-inject emitter: `src/tools/core/types.rs` (`Tool::call_content`).
- CC-stable id: `src/usage/mod.rs:88-92`; migration `src/usage/db.rs:64-70`.
