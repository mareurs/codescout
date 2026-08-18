# Guide Ledger — Session Identity and Re-arm Policy

**Date:** 2026-08-18
**Status:** Draft — awaiting review
**Component:** `src/tools/guide_ledger.rs`, `src/tools/config/mod.rs`, `src/tools/core/types.rs`, `src/server.rs`, `src/prompts/guides/workspace-state.md`, `codescout-companion/hooks/session-start.mjs`

## Problem

codescout injects guide bodies (`librarian` 19.9 KB, `tracker-conventions` 19.6 KB,
`progressive-disclosure` 5.7 KB, …) into tool responses the first time a session touches
the relevant topic. A `GuideLedger` records which topics have been delivered so they are
not re-sent. It is failing in both directions at once: re-sending guides the conversation
already holds, and — since 2026-08-18, confirmed — withholding guides from conversations
that have never seen them.

### Measured waste (`.codescout/usage.db`, 14 days to 2026-08-18)

| cc_session | MCP procs | activates | guide injections | distinct topics |
|---|---|---|---|---|
| `2c518eb6` | 27 | 49 | 189 | 9 |
| `b8bb058f` | **1** | 38 | **100** | 4 |
| `8a62140a` | 17 | 8 | 52 | 7 |
| `44c01c0f` | 9 | 7 | 42 | 4 |

`b8bb058f` is the diagnostic case: **one** MCP process, so persistence is irrelevant, yet
100 injections over 4 topics. Per-topic totals × body sizes come to **~4 MB of guide text
in 14 days against a ~0.37 MB ideal** — roughly 900K redundant output tokens. (Counts come
from a `LIKE` over `output_json`, which false-positives when a session reads codescout's
own source; the 16 hits for a literal `{topic}` are exactly that. Order of magnitude holds.)

### Three independent defects

**D1 — `activate` wipes the ledger unconditionally.** `src/tools/config/mod.rs:139` is the
first line of `ActivateProject::call`, before parameter validation. Every activate re-arms
every topic; a malformed activate (missing `path`, non-directory) does too. The adjacency
trace is unambiguous — `workspace ACTIVATE → INJECT`, then the next tool call → `INJECT`,
repeating every few minutes.

**D2 — the ledger is bound to the project tree and to Claude Code.** The directory is
`<project_root>/.codescout/guide_hints/`, resolved once at construction from
`agent.project_root()` (`src/server.rs:263-265`). `claude mcp list` shows `codescout start
--debug` with no `--project`, so the root is cwd-derived: a non-project cwd yields `None`
and the ledger goes **silently ephemeral**. In a git worktree it works only because the
companion plugin symlinks `<worktree>/.codescout` → `<main>/.codescout`
(`session-start.mjs:262-274`), which is Claude-Code-only, best-effort, and swallows its own
errors. The session key is `CLAUDE_CODE_SESSION_ID` → `.codescout/cc_session_id` (a shared
per-project file) → random uuid; both non-random branches are Claude Code artifacts.

This violates the project's own **Agent-Agnostic Design** convention (memory
`conventions`): *"the server itself must not depend on it… would a Copilot user lose
access?"* — twice, for the key and for the location.

**D3 — `/clear` leaves the session id stale.** Claude Code's `/clear` mints a new session
id and does **not** respawn the stdio subprocess, so the new conversation is served under
the previous conversation's id and inherits its ledger. Filed as
`docs/issues/2026-08-18-clear-leaves-mcp-session-id-stale.md` (severity high) with a live
trace. This is the dangerous direction: silent starvation, not loud waste.

**D1 currently masks D3.** Because a post-`/clear` conversation is normally re-activated,
D1's unconditional wipe re-arms the guides and accidentally rescues it. Fixing D1 alone
would convert an occasional token cost into systematic starvation after every `/clear`.

## Lifecycle matrix (measured 2026-08-18)

| Event | New session id? | Subprocess respawns? | Key correct? | Evidence |
|---|---|---|---|---|
| `/mcp` reconnect | no | yes | ✅ | 67 processes under one id |
| `/compact` | no | no | ✅ | 71,891-record / 137.5 MB transcript, **1** distinct `session_id` over 12 days |
| `claude --resume <id>` | no | yes | ✅ | 12-day session on a 17h-old client process |
| `--fork-session` | yes | yes | ✅ | `44c01c0f` forked from `28ea039a`; both recorded |
| **`/clear`** | **yes** | **no** | ❌ | 8 calls, 1 MCP process, 2 conversations, all attributed to the first |

## Research findings that constrain the design

**No protocol-level session identity exists, and the direction is standardised against it.**
`Mcp-Session-Id` was always Streamable-HTTP-only and is *removed* in the 2026-07-28 spec.
SEP-2822 (client-generated session id, explicitly including `params._meta` for stdio) was
**closed unmerged** 2026-06-08. Spec issue #823 — verbatim this problem — was **closed as
declined** 2026-06-26. The 2026-07-28 spec adds: *"a server must not treat connection or
process identity as a proxy for conversation or session continuity."* One live venue
remains: `transports-wg#36`, open, considering an experimental Conversation ID.

**Claude Code is the only harness in the field that exposes a session id.** Verified by
reading each client's MCP-spawn code: absent in Codex CLI, VS Code Copilot, Gemini CLI,
Cline, Roo Code, Continue.dev, Zed, OpenCode; undocumented in Cursor, Copilot CLI,
Windsurf, Amp, Kiro, JetBrains, Antigravity; Aider has no MCP client. Three families
(Codex, Cline/Roo, Continue) actively **strip** the environment to a 6–11 name allowlist,
so an exported variable never reaches us there.

**For IDE clients, conversation identity is structurally unobtainable.** VS Code, Cursor,
Cline, Roo and Zed spawn one MCP server per window/workspace; Copilot CLI's changelog:
*"Switching sessions no longer restarts MCP servers."* The process outlives the
conversation, so no environment variable could ever carry the answer.

**Prior art: Serena built this feature and reverted it.** Keyed on `id(mcp_ctx.session)`,
disabled with a dead `if …: pass` left at `agent.py:1071` as a regression guard, because
*"some clients (e.g. Claude Desktop) will use the same session across multiple chats."*
Their failure direction is ours: a shared id means chat B inherits chat A's "already sent".

## Design decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | `activate` re-arms **only on a genuine workspace-root change** | Same-project re-activation is routine session hygiene, not a re-orientation |
| 2 | A switch re-arms **only `project-activation-bootstrap`** | The other nine guides describe codescout's own tool contracts and are byte-identical in every repo |
| 3 | Opener predicate becomes `!emitted.contains(SESSION_OPENING_GUIDE)` | Forced by #2 — see *Opener predicate* below |
| 4 | Ledger moves to `$XDG_STATE_HOME/codescout/guide_hints/<session>.json` | Session-keyed storage makes worktrees, project switches and non-project cwds non-cases |
| 5 | Session key is a 4-rank chain, ending in an explicit warning | No universal source exists; degrade visibly, never silently |
| 6 | Companion hook rendezvous refreshes the key on `/clear` | Prerequisite for #1, not an accelerator — #1 removes D3's accidental mitigation |
| 7 | Idle TTL re-arms topics as a harness-independent backstop | The only mechanism available where identity is structurally impossible |
| 8 | **Degrade to re-sending, never to suppressing** | The load-bearing invariant; Serena's revert is the evidence |

### Rejected

- **Parent PID (+ start time) as the key.** Survives reconnect, breaks on `claude --resume`
  (the client process restarts), explicitly discouraged by the 2026-07-28 spec, and
  redundant once the process-per-window clients are handled by the TTL. Recorded as F-53 /
  R-105.
- **A per-harness table selected by `clientInfo.name`.** The table has exactly one row, and
  the 2026-07-28 spec says `clientInfo` *"SHOULD NOT be used to change the behavior of the
  client or server."* Probe an ordered list unconditionally instead.
- **SEP-2567 model-threaded handle in the tool schema.** Standards-blessed and correct for
  stateful tools, but it puts an opaque id argument on every call — inverting the token
  saving this ledger exists to produce.
- **`.codescout/cc_session_id` as a fallback.** Per-project and shared; it is the mechanism
  behind `docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md`.

## Architecture — two tiers

The clients differ in kind, so the ledger does too.

**Tier 1 — a conversation id is obtainable (Claude Code today).** Persisted ledger keyed by
it, full dedup, re-armed on project switch and on compaction. All measured waste lives here.

**Tier 2 — no conversation identity exists (every other client).** Identity is impossible,
so the ledger is bounded by *time* rather than keyed by *session*: in-process only, never
persisted, topics re-arm after an idle interval. A second conversation in a long-lived
process then re-arms within one interval instead of being starved indefinitely.

Tier selection is a consequence of the key chain: ranks 1–3 give tier 1, rank 4 gives tier 2.

## Components

### 1. Session key resolution

Ranks 1, 2 and 4 resolve **once at construction**; rank 3 is read **per request** and, when
present, overrides the construction-time value for that call's ledger lookup. First
non-empty wins.

| Rank | Source | Notes |
|---|---|---|
| 1 | `CODESCOUT_SESSION_ID` | Ours, documented for any harness. 15 of 17 clients accept a custom `env` entry on an MCP server, so it is broadly settable — **but every such surface resolves it statically at launch.** A value set in MCP config is therefore constant across every conversation in that project and **collides between concurrent windows**, which is the suppression direction. The variable is trusted when set (the operator is asserting it), so it MUST be documented as *unique per conversation* — useful only to someone generating it dynamically, e.g. a wrapper script. Misuse is bounded, not prevented, by the idle TTL (§7). |
| 2 | Known-harness env vars, probed unconditionally in a fixed list | Today: `CLAUDE_CODE_SESSION_ID`. One line per harness to extend. Not gated on `clientInfo.name`. |
| 3 | A custom `_meta` key, e.g. `dev.codescout.mcp/conversationId`, read per request | rmcp 1.3 already exposes `RequestContext.meta`; no client sends it today. ~30 lines, zero payoff now, and the exact shape `transports-wg#36` would land on. Being per-request, it is the only rank that can track a conversation change **without** a rendezvous — which is why it is worth having despite no current sender. |
| 4 | None → tier 2 | Log at info **once**, naming the degradation. Today all three failures are silent. |

### 2. Storage

`$XDG_STATE_HOME/codescout/guide_hints/<sanitize(session)>.json`, falling back to
`~/.local/state/…`. No new dependency (`dirs` is present but librarian-gated). Tier 2 never
writes.

**On-disk shape changes once, now**, from `Vec<String>` to a map carrying a delivery
timestamp per topic:

```json
{ "librarian": "2026-08-18T08:15:03Z",
  "project-activation-bootstrap": "2026-08-18T08:14:57Z" }
```

Timestamps make GC and the idle TTL expressible; without them no age-based policy is
possible without a second migration. A legacy `Vec<String>` file is read as "all topics
delivered at file mtime", then rewritten in the new shape.

Multi-process safety: two server processes can share a session id across a reconnect
overlap. Writes go through `util::fs::write_utf8` (write-to-`.tmp`-then-`rename`), so the
rename is atomic and a reader can never observe a torn file.

**Deliberately not read-modify-write.** A merging RMW would union the on-disk set into the
in-memory one — which silently **resurrects** every topic `re_arm` (§4) or `expire_idle`
(§7) had just removed, defeating both APIs. The in-memory set is authoritative for its
process; last writer wins. The residual lost-update window needs two live processes writing
the same session id simultaneously, and a reconnect is kill-then-spawn, not overlap. A
file lock would not help either — it orders the writes without making the older one correct.

#### Windows uses the same POSIX-shaped path, deliberately

Decided 2026-08-18 during Phase A execution, after the question was raised directly. Task 1's
review had flagged it as a Minor and it was deferred; this records the resolution so it is not
re-litigated.

**On Windows the ledger lands at `%USERPROFILE%\.local\state\codescout\guide_hints\`, not
`%LOCALAPPDATA%`.** `XDG_STATE_HOME` is honoured first on *every* platform, Windows included,
so a user or wrapper can override it; absent that, `crate::platform::home_dir()` resolves to
`%USERPROFILE%` (`src/platform/windows.rs:3`) and the POSIX suffix is appended unchanged
(`src/util/fs.rs:115`).

This is **consistency with codescout's own convention, not an oversight about Windows**.
Measured at the bytes: `%LOCALAPPDATA%` appears twice in the tree and neither is codescout
placing its own data — one locates a user's existing Git Bash install
(`src/platform/windows.rs:83`), the other is a comment. codescout's own per-user data has a
single house style on all platforms, and config already follows it:
`global_config_dir_from` resolves to `~/.config/codescout` (`src/config/global.rs:55`).

Why not switch state to `%LOCALAPPDATA%`:

- **Splitting is worse than either uniform choice.** State under `%LOCALAPPDATA%` while config
  stays under `%USERPROFILE%\.config` puts codescout's data in two unrelated places on Windows
  and matches the platform convention in neither.
- **Moving both is a different change.** It relocates existing users' config and needs a
  migration — out of scope for this design, and worth its own plan if ever wanted.
- **The escape hatch already exists.** `XDG_STATE_HOME` works on Windows, and the Windows
  population most likely to care (MSYS2, Git Bash, Cygwin) is exactly the population that sets
  XDG variables.
- **The failure mode is benign.** With `%USERPROFILE%` unset, `state_dir_from` returns `None`,
  the ledger goes ephemeral, and guides are re-sent — the correct direction under the
  "degrade to re-sending, never to suppressing" invariant.

**The one accepted cost, stated so it is not a surprise later:** `AppData\Local` is explicitly
non-roaming, whereas `%USERPROFILE%` can be roamed under enterprise Folder Redirection. A
session-keyed ledger that roams between machines is meaningless — a session id from one host
will simply never match, so the effect is a stale file that the §8 GC collects after 35 days,
not incorrect suppression. `~/.config/codescout` already carries the identical exposure, so
this design neither introduces nor worsens it; fixing it properly means moving config and state
together.

### 3. `GuideLedger` API

```rust
fn contains(&self, topic: &str) -> bool;         // unchanged
fn insert(&mut self, topic: String) -> bool;     // unchanged, now stamps a time
fn clear(&mut self);                             // unchanged — post_compact only
fn re_arm(&mut self, topics: &[&str]);           // NEW — remove named topics, persist
fn expire_idle(&mut self, ttl: Duration);        // NEW — tier 2 / backstop
fn notice_once(&mut self, key: &str) -> bool;    // unchanged, still ephemeral
```

`is_empty()` loses its second job (see below) and can go.

#### What a stamp means: last delivered, not first delivered

Settled 2026-08-18 during Phase A execution (Task 2 review, Ruling 8), because the
shape change forced the question and the spec had not answered it.

**A topic's stamp is the time it was last delivered to the model.** A repeat `insert`
refreshes it; `insert` therefore persists unconditionally rather than only on a genuine
first insertion.

Both consumers want this reading, which is why it wins:

- `expire_idle` re-arms topics the model has not seen in `ttl`. An explicit `get_guide`
  re-fetch **is** a delivery — the model has the text in front of it — so the clock must
  restart.
- The GC keys on idle age as a liveness proxy. A re-fetch is evidence of life.

The counterfactual is not neutral. `BTreeMap::insert` always overwrites the value, where
the old `HashSet::insert` mutated nothing on a repeat, so a naive `if added { persist() }`
leaves the refreshed in-memory stamp unwritten. That splits the two consumers against each
other: `expire_idle` reads the refreshed in-memory stamps while the GC reads the stale
on-disk ones through `read_entries` — a session actively re-fetching a topic looks fresh
in memory and steadily older on disk, which is exactly the state in which a GC deletes a
live session's ledger.

This path is production-reachable, not theoretical: `src/tools/guide.rs:92` calls
`insert` unconditionally on every `get_guide` and uses the returned bool as its
`first_fetch` signal. (The auto-inject path at `src/tools/core/types.rs:714` guards with
`contains` first, so it is unaffected.)

**The return value keeps its `HashSet::insert` contract** — `true` iff newly added — since
`guide.rs:92` depends on it. Only the persistence condition and the stamp change.

Cost accepted: one extra atomic write per re-fetch of an already-delivered topic. The
ephemeral ledger is unaffected — `Default` has no path, so `persist` early-returns.

### 4. Re-arm predicate

Two paths exist in `ActivateProject::call` and only one needs the predicate. The
bare-project-id **focus-switch** path (`src/tools/config/mod.rs:151-186`) returns early via
`activate_within_workspace`; putting the predicate in the full-activation path alone gives
"a sub-project focus switch never re-arms" for free.

The comparand is **`AgentInner::default_workspace_root`** (`src/agent/mod.rs:106`, already
`pub`), **not** `Agent::project_root()` — the latter returns `focused_project_root()`, so
with focus on `crates/codescout-embed` a re-activation of the repo root would read as a
project change (F-52).

```rust
// after root resolution + canonicalization (config/mod.rs:196), NOT at line 139
let switched = {
    let inner = ctx.agent.inner.read().await;
    inner.default_workspace_root.as_deref() != Some(&root)
};
if switched {
    ctx.guide_hints_emitted.lock().re_arm(PROJECT_SCOPED);
}
```

`PROJECT_SCOPED = &["project-activation-bootstrap"]`. Both sides must be canonicalized;
`Agent::activate:528` carries a comment recording a prior bug from omitting exactly that.
Moving the call off line 139 also retires the malformed-activate wipe.

#### Gated on the rendezvous — the safety mechanism, not a procedural rule

This optimization is only safe while conversation changes are detectable. If the companion
rendezvous (§6) has never reported in for this server, a `/clear` is invisible and the
precise behaviour would silently starve the new conversation (D3). So the precise path is
**conditional on the mechanism that makes it safe**:

```rust
if ctx.session_key.rendezvous_active() {
    if switched { led.re_arm(PROJECT_SCOPED); }   // precise
} else {
    led.clear();                                  // legacy blunt behaviour
}
```

`rendezvous_active()` is true once the server has observed its own slot file written by the
hook at least once. Consequences:

- A Claude Code user **with** the companion gets the full saving.
- A Claude Code user **without** it keeps today's behaviour exactly — wasteful, but never
  starved. No regression is possible.
- Tier-2 clients never satisfy the gate, so they keep the blunt path plus the §7 TTL.

This is why §7 gives tier 1 no TTL: the gate, not a timer, is what makes tier 1 safe.

#### Phase A introduced one narrow suppression that Phase C must close on purpose

Found 2026-08-18 by the Phase A whole-branch review. Recorded here rather than fixed there,
because the fix *is* this section and fixing it in Phase A would contradict § 2.

**The change.** Before Phase A the ledger path derived from the startup project root, so the
ledger was implicitly keyed by *(session, project)*. After Phase A it is keyed by session
alone (`src/server.rs:285-289`).

**The consequence.** Take one conversation whose MCP server restarts against a **different**
`--project`. Previously the new project's directory held no ledger for that session, so
`emitted` loaded empty, `is_empty()` was true at `src/tools/core/types.rs:693`, and
`SESSION_OPENING_GUIDE` fired for the new project. Now the session-keyed file is found, the
previous project's topics are already in it, `is_empty()` is false, and
`project-activation-bootstrap` is **suppressed** for the new project until something calls
`workspace(activate)`.

**Why it is recorded and not patched.** The direction is wrong — it suppresses a guide that
was previously re-sent, against this design's own *"degrade to re-sending, never to
suppressing"* invariant. But the reachability is narrow: it needs the same
`CLAUDE_CODE_SESSION_ID` **and** a changed startup root **across a reconnect**, and it
self-heals on the first `activate`. Any Phase A fix would mean re-introducing the project
into the key, which is exactly what § 2 removed and why the ledger now survives a project
switch at all.

**What Phase C must do.** `re_arm(PROJECT_SCOPED)` on a project switch already closes this —
it forgets the project-scoped topic while leaving the tool-contract guides in place, which is
precisely the surgical behaviour this case needs. Two requirements so it closes *deliberately*
rather than as a side effect:

1. The re-arm predicate must fire on a **startup-time** project difference, not only on an
   in-session `activate` call. The suppression appears at server construction, before any
   `activate` has run, so a predicate that only watches `activate` leaves the gap open.
2. Add a regression test for exactly this shape: one session id, two server constructions with
   *different* roots, asserting the opener fires for the second. Nothing in Phase A tests it,
   because in Phase A it does not hold.

Related: the § 7 "Open for Phase B" note is the inverse case — a ledger that empties and
re-fires the opener. This one is a ledger that carries too much and suppresses it.

### 5. Opener predicate

`call_content` (`src/tools/core/types.rs:693`) fires the session opener on
`emitted.is_empty()`. Since `SESSION_OPENING_GUIDE == "project-activation-bootstrap"`
(`src/prompts/mod.rs:429`), removing that one topic from a set still holding nine others
leaves the set non-empty — so the re-arm would inject **nothing**, and `activate`'s own
`relevant_guide_topic` returns `workspace-state`, not the bootstrap.

```rust
- if emitted.is_empty()
+ if !emitted.contains(crate::prompts::SESSION_OPENING_GUIDE)
```

This also retires a latent bug: today, if a session's first codescout call is an explicit
`get_guide("librarian")`, the insert makes the set non-empty and the opener is suppressed
for the rest of the session.

### 6. Companion rendezvous (Claude Code only)

Ordering is fixed and constrains the direction: MCP `initialize` runs **before**
`SessionStart`, so hook-mints/server-reads cannot work at startup. The server publishes
first; the hook writes into the slot.

1. At construction the server writes
   `$XDG_STATE_HOME/codescout/servers/<pid>.json` = `{pid, ppid, start_time, cwd, session}`.
2. `session-start.mjs` fires on every `source` (it already branches on `source` at line 253).
   It enumerates that directory, selects entries whose `ppid` is on its own process
   ancestry, and writes the current `session_id` into the entry.
3. The server re-reads its own entry when the file's mtime changes — an `fstat` per call,
   not a parse.

**Keyed by server pid, deliberately.** A per-project file is what caused the 2026-08-16
attribution bug, and two concurrent windows on one repo would collide again. A pid is a
valid rendezvous *within one process lifetime* even though it is useless as durable
identity — that distinction is the whole reason this is safe and the PPID key was not.

On a session change the server **re-arms the whole ledger** (the new conversation holds
nothing) and switches to the new key. The old session's file is left for GC.

The server must not depend on this. No hook installed → the key never refreshes → the idle
TTL is what catches `/clear`, one interval late. That is the Agent-Agnostic contract:
companion *adds* enforcement, server *degrades* without it.

### 7. Idle TTL

**Measured 2026-08-18** — 258 sessions, 124,324 inter-call gaps, 50 `usage.db` files.
A TTL fires spuriously whenever a *live* conversation is idle longer than the window,
re-injecting guides it still holds:

| TTL | sessions seeing a spurious re-arm |
|---|---|
| 2h | 34.5% |
| 4h | 28.7% |
| 12h | 19.0% |
| 24h | 11.6% |
| 48h | 5.8% |
| 72h | 4.3% |

**The curve is flat and never cheap.** Reaching 5% costs a 48-hour window, and a 48-hour
starvation bound is worthless. No single value serves both tiers, so the design splits.

**Tier 1 — no TTL.** The rendezvous (§6) is the mechanism; a TTL is a bad substitute at
every value. Safety comes from gating instead (see §4): the ledger keeps its blunt
clear-on-every-activate behaviour until a rendezvous is present to detect conversation
changes.

**Tier 2 — 2 hours idle.** Here starvation is the *default* and is permanent: without a
TTL, every conversation after the first in a long-lived process receives no guides at all,
forever. A spurious re-arm costs one re-injection inside a single process that never
persists anything; silence costs every guide for every later conversation. Per decision #8
(re-send, never suppress), the short window is the correct error.

**Caveat, stated because it is load-bearing:** the 34.5%/28.7% figures come from Claude
Code sessions, the only harness with a session id to group by. Tier-2 clients are IDE-
shaped and their conversation cadence is unmeasured. The value is a config key
(`CODESCOUT_GUIDE_TTL_SECS`), and this table is the method for re-scoring it once tier-2
traffic exists.
#### Resolved 2026-08-18 (Phase B): expiring the *last* topic re-fires the session opener

Surfaced 2026-08-18 by the Task 4 review; not a Phase A defect, and deliberately left
unpinned there. Resolved in Phase B (Task 3 of this plan): **accept** the behaviour.

The chain: if `expire_idle` removes the final remaining topic, `persist` takes its
empty-map branch and **deletes** the file (`src/tools/guide_ledger.rs`, `persist`). The
next `load` therefore yields an empty ledger, and `is_empty()` is the trigger for
`SESSION_OPENING_GUIDE` (`src/tools/core/types.rs`, the `if emitted.is_empty()` branch) —
so the orientation guide fires again.

Decision: **accept**, not prevent. Rationale is the governing invariant that runs through
this whole design — "degrade to re-sending, never to suppressing." For a client idle past
its TTL, re-orienting is the correct behaviour, not a bug to route around: enough time has
passed that the model plausibly holds none of the original orientation anyway, so
re-sending it costs tokens once where suppressing it would fail silently. The rejected
alternative — retaining a placeholder entry, or having `persist` write `{}` instead of
deleting when the map empties — was considered and declined, because either would make
`is_empty()` return false forever and suppress the opener permanently for that session.

Pinned by
`expiring_the_last_topic_deletes_the_file_so_the_session_opener_re_fires`
(`src/tools/guide_ledger.rs`, `mod tests`), which expires a keyed ledger's only topic via
`expire_idle` and asserts both that `persist` removes the file and that the next `load`
comes back empty. `persist`'s own doc comment carries the same rationale at the point of
the code it protects, so a future edit to the empty-map branch is warned twice: once by
the doc comment, once by the test failing.

This interacts with §8's GC, which falls back to file mtime for an empty or unparseable
ledger — unaffected by this decision, since GC only runs on ledgers other than the one
being loaded.
### 8. GC

A global directory accumulates across every project, so GC is part of the design rather
than hygiene. Prune on load, keyed on **idle age** (the entry's newest timestamp), not on
creation — a live long-running session keeps writing, so idle age is what distinguishes
dead from quiet.

**Window: 35 days.** Measured 2026-08-18 over the same 258 sessions — fraction whose ledger
would be deleted while the session is still alive:

| window | by lifespan | by idle gap |
|---|---|---|
| 7d | 3.9% | 3.1% |
| 14d | 3.1% | 2.3% |
| 21d | 1.6% | 1.2% |
| **30d** | **0.0%** | **0.0%** |

Observed maxima: 28.9 d lifespan, 27.0 d idle gap. 30 d is the first zero but leaves only
1.1 d of headroom above the sample maximum; **35 d** buys ~6 d for free, since the files are
~60 bytes each (61 files = 244 KB on this host today).

Mark the `read_dir` as cleanup, not discovery — `src/lsp/mux/mod.rs:68-72` records why
(R-45).
## Sequencing constraint

§4 removes the accidental mitigation for D3, so it must not take effect before the
rendezvous (§6) exists. **That constraint is now enforced in code rather than by
convention** — §4's precise path is gated on `rendezvous_active()`, so shipping it early is
inert rather than harmful.

The phase order below is therefore about verifiability, not safety: each phase is
measurable before the next begins.

### Suggested phasing

This spec is larger than one implementation plan and should be executed as three, each
shippable and independently verifiable:

| Phase | Contents | Ships what |
|---|---|---|
| **A — safety** | §2 storage + shape migration, §3 API (incl. the `expire_idle` mechanism), §8 GC | Ledger becomes session-scoped, agent-agnostic, and durable. Worktrees and non-project cwds stop being special cases. No behaviour change to re-arm |
| **B — identity** | §1 key chain, §6 rendezvous (spans `codescout-companion`), §7 TTL **policy** | `/clear` stops corrupting the ledger. Closes `docs/issues/2026-08-18-clear-leaves-mcp-session-id-stale.md` |
| **C — the fix** | §4 re-arm predicate, §5 opener predicate, doc update, test split | Removes the ~900K-token waste this spec was opened for |

Phase C is the payload; A and B are what make it safe. Running them in this order also
means each phase can be measured against `usage.db` before the next begins.

## Testing

| Test | Asserts |
|---|---|
| `activate_same_project_keeps_hints` | Re-activating the constructed root does **not** re-arm |
| `activate_different_project_rearms_bootstrap_only` | A genuine switch re-arms `project-activation-bootstrap` and nothing else |
| `subproject_focus_switch_does_not_rearm` | The bare-project-id path leaves the ledger intact |
| `malformed_activate_leaves_ledger_intact` | Missing/invalid `path` no longer wipes |
| `opener_fires_when_bootstrap_absent_from_a_nonempty_set` | The §5 predicate |
| `explicit_get_guide_first_does_not_suppress_the_opener` | The latent bug §5 retires |
| `ledger_survives_mcp_restart` | Existing test, new storage path |
| `post_compact_rearms_guide_hints` | Existing test, unchanged |
| `session_change_rearms_everything` | Rendezvous: a new session id re-arms the full set |
| `expired_topics_rearm_after_ttl` | §7 |
| `legacy_vec_file_is_read_and_migrated` | §2 shape migration |
| `gc_drops_entries_past_the_window` | §8 |

**`activate_project_resets_hints` (`src/server.rs:4292-4318`) must be split, not tweaked.**
It activates `dir.path()` — the same root `make_server()` (`src/server.rs:3822`) built the
agent with — and asserts *"activate should reset emitted set"*. Under the new policy that is
a same-project re-activation and the test inverts. It encodes the policy being replaced
(W-45).

## Documentation

`src/prompts/guides/workspace-state.md:51` states the ledger is *"Cleared on every
activation… After a clear, the next of either re-emits."* That file is `include_str!`'d at
`src/prompts/mod.rs:445` and hard-injected into the model's context, so the code change
alone ships a falsehood in the guide whose job is to describe activation semantics. Update
it in the same commit. No test asserts on its content — `prompts/mod.rs:1432` checks topic ↔
body *existence* only.

Also update memory `claude-code-mcp-env` with the lifecycle matrix.

## Out of scope

- **Subagent cardinality.** Subagents share the parent's session id, so they inherit the
  parent's ledger and receive no guides. A session key cannot express "share project state,
  fresh doc ledger"; that needs a second key component. Iron Law 6 (brief your subagents)
  remains the mitigation. Track separately.
- **Fork context carryover.** `--fork-session` carries the parent's messages into a new
  session id, so the forked conversation's context already holds the guides but gets a fresh
  ledger and re-injects them. The ledger keys on session identity while it actually tracks
  context content; they diverge here. Small and real; noted, not fixed.
- **rmcp 1.3 → 3.x.** 3.0.0 is the stateless release. Unrelated to this change, but the gap
  is worth its own decision.
- **Filing the codescout use case on `transports-wg#36`.** Cheap and worth doing; not part
  of this change.

## Open parameters

1. ~~TTL length~~ — **resolved 2026-08-18 by measurement.** Tier 1: none (gating replaces
   it). Tier 2: 2h idle, exposed as `CODESCOUT_GUIDE_TTL_SECS`. See §7 for the curve and
   the tier-2 caveat.
2. ~~GC window~~ — **resolved 2026-08-18 by measurement.** 35 days on idle age; 30 d is the
   first zero-loss value and 35 d adds headroom above the 28.9 d observed maximum. See §8.
3. Whether to ship the `_meta` reader (rank 3) now or when a client sends one. **Still
   open** — ~30 lines, no current sender, forward-compatible with `transports-wg#36`.
