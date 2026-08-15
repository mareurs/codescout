---
id: '01291679a5ee4707'
kind: tracker
status: active
title: Capability Proposals — codescout (CAP-N)
owners:
- marius
tags:
- capability-proposal
- design
- reflective
- backlog
topic: capability-proposals
---

## Why this exists

A **pre-plan** home for proposed codescout capabilities: an idea with enough grounding to be
judged, but not yet a spec or a plan. `docs/TAXONOMY.md` had no slot for this — a feature idea
was told to go to "`docs/trackers/` or `docs/plans/`" with no named destination, so ideas either
got wedged into a dated landscape doc (`mcp-integration-ideas-2026-04.md`, scoped to an April
read of the Claude Code source) or lived only in a conversation. This is the named destination.

**What belongs here:** a capability codescout does not have, with the substrate check already
done — what exists today, what is actually missing, and what the first decision is.
**What does not:** bugs (`docs/issues/`), tool frictions (U-N), hook designs (H-N), or anything
already scoped enough to be a spec (`docs/superpowers/specs/`) or plan.

**Graduation.** A CAP entry leaves by becoming a spec + plan, or by being marked `rejected` with
the reason kept. It is not a wishlist: an entry with no substrate check is not ready to be here.

## Index

| ID | Opened | Status | Size | Title |
|----|--------|--------|------|-------|
| CAP-1 | 2026-08-15 | proposed | small–medium | Session artifact-touch ledger — expose what this session read and wrote |
| CAP-2 | 2026-08-15 | brainstorm | large | Second arm — a codescout-issued LLM controller that challenges unverified claims |
| CAP-3 | 2026-08-15 | research | medium | Background / async tool execution — surface what already works, then decide on MCP `ext-tasks` |

## CAP-1 — Session artifact-touch ledger

**Ask.** Within a session, record every tracker/artifact the agent touched, classify each as
read-only or read-write, keep a lightweight diff and some statistics, and expose a method that
inspects it. The driving use case is **compaction prep**: at the moment context is about to be
lost, answer "what did I touch, what did I only read, and what did I change?" without
reconstructing it from memory — the least reliable source available at exactly that moment.

**Substrate check — most of this already exists.** `src/usage/db.rs` writes a `tool_calls` row
per MCP call (`write_record`, `src/usage/db.rs:107-151`) carrying `tool_name`, `called_at`,
`latency_ms`, `outcome`, `overflowed`, `err_family`, `project_root`, codescout + project SHAs,
and — decisively — both `session_id` (the MCP server's) and `cc_session_id`, read from
`.codescout/cc_session_id` (`src/usage/mod.rs:99-104`). Retention is 30 days. So per-session
correlation is already there and needs no new plumbing.

**The one real gap.** The row that would identify *which artifact* was touched is `input_json`,
and it is written only under `--debug`:

    let input_json = if self.debug { serde_json::to_string(input).ok() } else { None };
    — src/usage/mod.rs:85-89

In an ordinary session `input_json` and `output_json` are NULL, so the target is unrecoverable.
Turning `--debug` on globally is the wrong fix: it persists the full input **and output** of every
call, which is both large and a disclosure surface (outputs contain file contents).

> **Correction, 2026-08-15 — the gap above is real in the code and absent on this machine.**
> Measured while investigating tool usage: **51,164 of 53,916 recorded rows (95%) DO carry
> `input_json`**, because every live server here runs `codescout start --debug` (verified via
> `ps -o args=`, all three Claude Code profiles). So on this host the debug branch is not an
> exception — it is the deployed configuration, and the cost this entry warned about is already
> being paid: full inputs **and outputs** of every call, retained 30 days.
>
> Two consequences, both good for the proposal:
>
> 1. **It is prototypable today with no code change.** The touch ledger can be written as a query
>    over existing rows — `json_extract(input_json, '$.id')` / `'$.path'` keyed by `cc_session_id`
>    — and evaluated for usefulness before anything is built. Do that first.
> 2. **The productionisation is decoupling it from `--debug`,** not adding capture. Users on the
>    shipped default get NULL, so a feature built on `input_json` would work here and silently
>    do nothing for them. That is the argument for the narrow extracted `touch_target` below —
>    it is what makes the capability real off this machine.
>
> Worth raising separately with the operator: running `--debug` permanently means every file's
> contents that any tool returned sits in `.codescout/usage.db` for 30 days. That may be intended
> (it is what made this investigation possible) but it should be a decision, not a leftover.

**Therefore the shape is narrow.** Add an always-on extracted `touch_target` (artifact id or
rel_path) plus an `access` class, rather than storing whole payloads. There is precedent in the
same function: `friction_target` is already an extracted-not-dumped field
(`extract_friction_target`), computed conditionally and stored as a short string. This proposal
is that pattern applied unconditionally to a different field.

**RO/RW classification is derivable, not guesswork.** The tool plus its `action` decides it:
`artifact(find|get|graph|state_at)`, `read_markdown`, `read_file`, `grep`, `symbols` are RO;
`artifact(create|update|move|delete|append_entry|graft|link)`, `artifact_augment`,
`artifact_event(create)`, `edit_markdown`, `edit_code`, `edit_file`, `create_file` are RW.
Worth pinning as a single table with a test, so a new tool added without a class is a
compile-or-test failure rather than a silent RO default — an unclassified write showing up as
"read" is the failure mode that would make the ledger lie.

**Open questions.**

1. **Diff granularity.** "Some sort of diff" spans three very different costs: (a) a byte delta
   from the `field_patch` events the librarian already emits — nearly free, already persisted;
   (b) a per-section list of what changed; (c) real content diffs. (a) is available today via
   `artifact_event(action="list")` and may be enough. Start there and see what is actually
   missing before building (c).
2. **Where does it surface?** A new tool, a `workspace(action=…)` arm, or an argument to the
   existing `usage` tool. Weakly favour extending `usage` — it already owns this table — unless
   the compaction use case wants a distinct, obvious name an agent will reach for under pressure.
3. **Does it need to survive compaction itself?** The ledger is in SQLite, so it does. The open
   part is whether anything should *auto-inject* a summary at compaction time, or whether an
   explicit call is better. Auto-injection has a poor track record here (byte caps, relevance),
   so the default should be explicit.
4. **Scope: session or window?** `cc_session_id` keys the Claude Code session, which survives
   `/compact` — so "this session" is well-defined across a compaction boundary. Confirm that
   holds before relying on it.

**Why it is worth building.** This is the concrete, machine-checkable half of a problem the repo
has already documented from the human side: AB-6 in
[`bistriceanu/agent-behavior-analysis.md`](bistriceanu/agent-behavior-analysis.md), *"the unearned
clean slate"* — the agent starts each session fresh while the human carries every prior mistake,
and the only cure named there is "a damage ledger the next session actually reads". That entry's
ledger is prose the agent must remember to write. This one is a byproduct of doing the work, which
is the difference between a discipline and a record.

## CAP-2 — Second arm: a codescout-issued LLM controller

**Ask.** Let codescout itself issue a separate LLM call (Anthropic) at chosen moments, acting as a
controller that challenges the primary agent — specifically targeting claims it is **not certain**
of, or that rest on code it **did not read**. Explicitly a long brainstorming task, not a plan.

**Substrate check.** codescout makes **no outbound LLM call today** — a search across `src/`,
`crates/` and `Cargo.toml` for `anthropic` / `ANTHROPIC_API_KEY` / `api.anthropic.com` /
`x-api-key` returns zero hits (2026-08-15). Even the librarian's augmentation refresh is
documented as *"run by the agent, not automatic"*. So this would make codescout an **LLM client**
for the first time, not merely an MCP server. That is the single largest thing to weigh: it adds
credentials, cost, latency, non-determinism, and a network dependency to a tool whose current
appeal includes having none of those.

**The evidence this is aimed at is unusually good.** From
[`bistriceanu/agent-behavior-analysis.md`](bistriceanu/agent-behavior-analysis.md):

- **AB-1, the root defect** — a census of one session, verified against the transcript: ten
  corrections, four from user pushback, six from running something, **zero from unaided
  self-review**. The agent's own account: *"I stated the conclusion at the moment I formed the
  hypothesis, instead of at the moment I verified it."*
- **Improvement candidate 3** — *"Institutionalize contradiction, don't wait for it. Self-review
  is worth zero; blind fresh-context review is cheap and effective (5/5, ~30s/agent)."*

That is the case for a second arm stated in measured terms, and it is also the constraint on its
design: the intervention that scored 5/5 was **blind and fresh-context**. A controller that
inherits the primary's context inherits its commitments and will rehearse them — the analysis is
explicit that re-reading one's own reasoning "reads exactly as convincing the second time". The
value is in the blindness, so the hard design problem is *what minimal, unbiased brief* the
controller receives.

**Design axes to work through (none decided).**

1. **Trigger.** What fires it? Candidates: a claim tagged UNCERTAIN; a tool-call pattern implying
   assertion-without-reading (an edit to a symbol never fetched with `include_body`); session
   milestones (pre-commit, pre-compaction); explicit invocation. The tool-call-pattern trigger is
   the interesting one, because codescout is the component that can actually see it — `tool_calls`
   already records the sequence, which is the same substrate CAP-1 wants. **CAP-1 is plausibly a
   prerequisite**, and that dependency is the strongest argument for doing CAP-1 first.
2. **Brief.** Blind by construction (see above). Probably: the claim, the files it concerns, and
   nothing about how the primary reached it.
3. **Authority.** Advisory note, a `RecoverableError`-style block, or an injected instruction?
   Anything that can *block* turns a non-deterministic remote call into a hard dependency of local
   work. Strong prior for advisory-only at first.
4. **Cost and latency.** Per-call price and seconds added, against a measured baseline of how often
   the primary is wrong in a way this would catch.
5. **Offline and restricted hosts.** codescout was just given a fully offline embedding path
   (`local-dir:`, merged `e6484b16`) precisely so a restricted environment can work without the
   network. A controller that silently degrades — or worse, silently *doesn't run* — on those hosts
   would reintroduce the exact failure class that work removed: a guarantee that fails open in
   silence. Whatever this becomes, absence of the controller must be loud.
6. **Who owns the key?** Reusing the host's Claude Code credentials, a separate key, and "no key
   configured" are three different products.

**Anti-goal worth stating early.** Not a general "review my work" pass — the census says
self-review-shaped prompts are dead weight. The target is narrow: *claims asserted without the
check that would settle them.*

**Relationship to what already works.** The maintainer's **Conclude Last** rule and the per-claim
verified-vs-inferred tag attack the same defect at the prompt layer, and AB-1 argues they are two
halves of one fix (the rule names the mechanism; the tag makes compliance observable). A
controller is a third, heavier layer. Before building it, it is worth knowing what the cheaper two
already recover — the natural experiment in that document is pre-registered for exactly this and
has not been run. **Running it is arguably the real first task of CAP-2.**

## CAP-3 — Background / async tool execution

**Ask.** Claude Code now accepts MCP calls that run in the background and can be waited on. It is
visibly working already, but codescout surfaces none of it in `get_guide`. Understand it first,
then decide what to expose.

**Substrate check — there are THREE distinct mechanisms here, and conflating them is the main
risk.** Verified 2026-08-15.

**(1) codescout's own `run_in_background` — exists, works, is undocumented.**
`run_command` takes `run_in_background: true`, detaches, and returns an `@bg_*` handle that is
queryable like any other buffer (`tail -50 @bg_00000003`). Used four times in this session and it
worked every time. But: grepping `src/prompts/**/*.md` for `run_in_background` / `@bg_` /
`background` returns **exactly one hit**, and it is about LSP prewarming — unrelated. So the
feature is real, load-bearing for long builds and test runs, and reachable only by reading the
tool schema. `get_guide("progressive-disclosure")` documents `@cmd_*`, `@tool_*`, `@file_*` and
`@ack_*` and never mentions `@bg_*`, even though it is the same handle family. **This is the
actual reported gap and it is cheap to close.**

**(2) Claude Code's client-side backgrounding of a slow MCP call — codescout is passive.**
Observed directly this session:

    MCP tool "codescout/run_command" is still running after 120s. It was moved to the
    background as task kmtbscucy and keeps running; you'll receive a notification with the
    result when it completes. To stop it, use TaskStop with task_id "kmtbscucy".

The result arrived later as a `<task-notification>` system message. The controls
(`TaskStop`/`TaskGet`/`TaskList`/`TaskOutput`) are **Claude Code's own tools, not codescout's**.
codescout was never told any of this happened — it simply kept running the call. So this is
application-layer behaviour in the client, and codescout needs no protocol change to benefit from
it; a slow call is already survivable.

**(3) The MCP `ext-tasks` draft extension — codescout does NOT implement it.**
`ServerCapabilities::builder().enable_tools().enable_tool_list_changed().enable_resources()`
(`src/server.rs:867-873`) declares no `extensions` block at all, and rmcp is pinned at 1.3.0 with
features `server, macros, transport-io, elicitation, schemars` — no tasks feature. Read from the
spec draft ([`modelcontextprotocol/ext-tasks`](https://github.com/modelcontextprotocol/ext-tasks),
`specification/draft/tasks.md`, 2026-08-15):

- Methods are `tasks/get` (poll status, carries the final `result` when `completed`),
  `tasks/update` (answer a server-issued input request), `tasks/cancel`.
- Negotiated by **capability**, not per-request: the client advertises
  `io.modelcontextprotocol/tasks` under `_meta.io.modelcontextprotocol/clientCapabilities.extensions`;
  the server advertises it in its discover response under `capabilities.extensions`. The **server**
  then decides per request whether to return a task or an ordinary result.
- Lifecycle: `working` → optional `input_required` → terminal `completed` | `failed` | `cancelled`.
- The completed `result` has the same shape as the original call's normal result (`CallToolResult`).
- **Correction to an earlier reading in this entry (2026-08-15).** A first pass here said the draft
  forbids `notifications/progress` for a task outright, and concluded tasks and our existing
  progress code would be mutually exclusive. That is wrong and would have misled an implementer.
  The prohibition is scoped to the **`subscriptions/listen` stream**: request-scoped notifications
  (`notifications/progress`, `notifications/message`) still flow on **the response stream of the
  request they belong to**. So `src/tools/progress.rs` is not in conflict with adopting tasks.
- Status: **draft extension**, not core spec. Revises a `2025-11-25` predecessor.

**Threshold — SETTLED 2026-08-15, no experiment needed.** This entry originally recorded the
backgrounding threshold as unmeasured, because the one observation was ambiguous (the client
backgrounded "after 120s" on a call whose own `timeout_secs` was also 120). The Claude Code
changelog answers it directly:

> **2.1.212** — "MCP tool calls running longer than 2 minutes now move to the background"

So it is a fixed **2-minute client-side rule**, independent of the server's own timeout — the
coincidence with `timeout_secs: 120` was exactly that. Worth noting the discipline paid: had the
ambiguous datapoint been written up as fact, it would have been *right by accident* and for the
wrong reason, and the next person tuning a server-side timeout would have drawn a false
conclusion from it.

**The client has moved much further than this entry assumed — re-scoped 2026-08-15.**

MCP shipped a **2026-07-28 specification** that makes the protocol **stateless** and adds a formal
**extensions framework**. Tasks moved *out of* experimental core and *into* the
`io.modelcontextprotocol/tasks` extension: a server may answer `tools/call` with a **task handle**,
and the client drives it with `tasks/get` / `tasks/update` / `tasks/cancel`. A new
`subscriptions/listen` stream carries opted-in server→client change notifications and **replaces
the HTTP GET endpoint and `resources/subscribe` / `resources/unsubscribe`**; servers may also push
`notifications/tasks` on it, each carrying the full task state.

And Claude Code already implements it. From the changelog:

> **2.1.233** — "Fixed MCP v2 connections endlessly reopening the subscriptions/listen stream"

A bug fix for MCP v2 connections is only possible where MCP v2 connections exist. **So item (3) is
no longer "a draft nobody implements" — the client implements it and codescout does not.** That
inverts the entry's original framing, where (3) looked like speculative early adoption.

**The new gating question is rmcp.** codescout is pinned to rmcp 1.3.0, which predates the
2026-07-28 spec; whether the Rust SDK has shipped stateless/extensions/tasks support, and at what
version, is the first thing to check before any of (3) is scoped. That is a cheap lookup and
nobody has done it.

**Other client-side MCP changes since this repo's notes were written** (codescout memory
`claude-code-mcp-env` was captured at CC v2.1.177; this session ran 2.1.227 with 2.1.233 installed):

| Version | Entry | Why it matters here |
|---|---|---|
| 2.1.212 | MCP calls >2 min move to the background | settles the threshold above |
| 2.1.214 | "Added a periodic progress heartbeat for long-running tool calls" | client-side UI heartbeat; distinct from MCP `notifications/progress` — do not conflate |
| 2.1.219 | `mcp_server_errors` added to the headless stream-json init event | a diagnostic surface for MCP failures in headless runs |
| 2.1.232 | Fixed MCP connections hanging for the full 30-second connect timeout | there is a 30s connect timeout; relevant to slow codescout startup |
| 2.1.233 | MCP v2 / `subscriptions/listen` fix | the client speaks MCP v2 |
| 2.1.233 | "Todo/task-tracking tools (TaskCreate/Get/Update/List, TodoWrite) are no longer available" | **not** the MCP background-task controls; the backgrounding message points at `TaskStop`. Whether `TaskStop`/`TaskOutput` survive the removal is unverified — do not assume either way |

**Still genuinely unknown.** Whether Claude Code sends `_meta.progressToken` on tool calls at all.
codescout builds a `ProgressReporter` only when it receives one (`src/server.rs:977-979`), and
nothing logs the outcome — the live diagnostic logs are tracing output, not raw JSON-RPC, so their
zero hits for `progressToken` say nothing about the client (the only `_meta` matches in them are
`body_meta` fields inside response bodies). Settle it with one temporary `tracing::info!` on the
`get_progress_token()` branch, one rebuild, one tool call — not by reading logs that cannot carry
the field.

**Open questions.**

1. **Scope split.** (1) is a documentation fix worth doing on its own and needs no research; (3) is
   a protocol adoption with real cost. Should this entry graduate as two, so the cheap half is not
   held hostage to the expensive one? Weak preference: yes.
2. **Where does `@bg_*` get documented?** `get_guide("progressive-disclosure")` already owns the
   handle families and is auto-injected on first overflow, which makes it the natural home. Mind
   the 2200-byte cap on the `server_instructions` slice (that cap is why guide topics exist) — so
   the guide, not `source.md`.
3. **Does adopting `ext-tasks` buy anything the client already provides?** Claude Code's
   application-layer backgrounding already makes slow calls survivable with zero server work. The
   case for (3) has to be something that behaviour cannot do — e.g. a task surviving a client
   restart, or `input_required` for mid-call elicitation. Name that before building.
4. **Draft-tracking cost.** `ext-tasks` is a draft that has already revised once. Implementing
   against it means tracking a moving target, and rmcp 1.3.0 does not expose it — so it would be
   hand-rolled JSON-RPC over the extensions block, or an rmcp upgrade.
5. **Progress/tasks exclusivity.** Given the MUST NOT above, which existing progress-reporting call
   sites (`src/tools/semantic/index.rs` is one) would have to change behaviour if their tool ever
   returned a task?

**Why it is worth doing.** Item (1) is a real capability the model cannot discover from the
guidance surfaces — the same failure class as any undocumented affordance: it gets used by accident
or not at all. Items (2) and (3) matter because a long `cargo test` or a full reindex is exactly
the call most likely to exceed a client timeout, and knowing which layer owns that failure decides
whether we fix it here or rely on the client.

**Prior art to check before designing (not yet read).** `docs/trackers/run-command-pipeline.md`
and `mcp-integration-ideas-2026-04.md` §5 ("Lifecycle resilience / kill the `/mcp` restart dance")
both touch long-running-call behaviour and may already record decisions this entry would otherwise
rediscover.

## Anti-goals

- Not a wishlist. An entry without a substrate check ("what exists today, what is missing") is not
  ready to be here.
- Not a bug tracker. A defect goes to `docs/issues/`, even when the fix is a new capability.
- Not a plan. Once an entry has tasks and a file structure, it graduates to a spec + plan and this
  entry becomes a pointer.

## History

### 2026-08-15 — opened

Created with CAP-1 and CAP-2, both raised in session. Substrate checks done at open time rather
than deferred: CAP-1's `usage.db` reading (the `--debug` gate on `input_json` is the whole gap),
and CAP-2's zero-hit search for any existing LLM client. `docs/TAXONOMY.md` gains a CAP-N row in
the same change.

### 2026-08-15 — CAP-3 added

Raised from a live observation (Claude Code backgrounding a slow codescout call) and researched
before writing, per the entry requirement. The research pass found three separate mechanisms where
the question assumed one, which is the entry's main content. Two corrections worth keeping:

- A research summary asserted "no official specification for MCP tasks exists" while **its own
  source list contained the draft spec** (`modelcontextprotocol/ext-tasks`). Read the spec instead
  of the synthesis; everything in CAP-3 §(3) comes from the draft itself.
- The one available observation of the client's backgrounding threshold is ambiguous (the client
  backgrounded "after 120s" on a call whose own `timeout_secs` was also 120). Recorded as
  unmeasured with a specific experiment to settle it, rather than written up as 120s.
## Template for new entries

<!-- Insert new CAP-N entries above the "## Anti-goals" heading. Also add an Index row.

## CAP-N — <title>

**Ask.** What the capability is, in the requester's words where possible.

**Substrate check.** What exists today, cited at path:line. What is actually missing. An entry
without this is not ready.

**Open questions.** Numbered, each one a decision someone has to make.

**Why it is worth building.** Evidence, ideally measured, ideally not self-generated.
-->
