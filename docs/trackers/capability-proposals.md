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
entry_high_water_CAP: 11
entry_prefix: CAP
expects_augmentation: docs/augmentations/docs-trackers-capability-proposals.yaml
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
| CAP-4 | 2026-08-16 | proposed | — | Cross-session collision hint — tell a session when another one just touched this file |
| CAP-5 | 2026-08-17 | proposed | medium | Server-assigned entry ids for prose trackers — make allocation atomic instead of advisory |
| CAP-6 | 2026-08-17 | proposed | small–medium | Derive TAXONOMY's append-recipe column from `entry_prefix` declarations — it drifted twice in one day |
| CAP-7 | 2026-08-19 | proposed | small–medium | Make record decay detectable — three doctor checks so corrections travel (Layer 2) |
| CAP-8 | 2026-08-19 | proposed | large | Content-addressed identity — a "gram" for entries, stored-not-derived ids for artifacts (Layer 3) |
| CAP-10 | 2026-08-20 | proposed | medium | Practice rules — a curated, agent-agnostic rule set injected at the moment it applies |
| CAP-9 | 2026-08-20 | proposed | medium | Friction observability — fix attribution, then **S-A only** (S-B falsified 2026-08-20) and an in-band `friction()` self-report |
| CAP-11 | 2026-08-26 | proposed | small–medium | Reconcile memory files against memory points — a doctor check, because only doctor can see both projects |

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


### Evidence, 2026-08-16 — the missing primitive is *await*, not *background*

Measured over the live-DB corpus (21,638 calls, `user_version >= 2`); full working in
`docs/trackers/2026-08-15-tool-usage-investigation.md` § History → 2026-08-16.

**This proposal is narrower than it looks.** Latency is one tool: of 1,455 calls over 10s,
**1,425 are `run_command`**, and all 31 calls over 120s are. No other tool reaches 30s more than
once. So there is no general async problem to solve — and `run_command` **already has
`run_in_background`**.

**A reading to avoid.** `run_in_background` shows 140 uses, *every one* under 10s, which invites
the conclusion "it is never used where it is needed." That is wrong — a backgrounded call returns
immediately by design, so `<10s` is precisely where it must appear. The feature is working.

**What is actually missing** is the other half of the pair. Backgrounding a job creates a need to
*wait for it*, and there is no primitive for that — so the wait becomes a **second, foreground,
blocking call** that hand-rolls a polling loop:

```
for i in $(seq 1 60); do grep -qE '...' @bg_00000011 && break; sleep 5; done
gh run watch "$RUN" --interval 20
```

| >60s `run_command` calls | Calls | Blocked time |
|---|---|---|
| All | 95 | 15,707s |
| Hand-rolled wait loops | **35 (37%)** | **10,075s (64%)** |

One third of the calls consume two thirds of the blocked time. **18 of them (2,371s ≈ 40 min) were
spent polling a `@bg_` buffer for a job the agent had already backgrounded** — the exact shape of
"background exists, await does not." The single longest call in the corpus is 25 minutes of
`gh run watch`.

**Implication for the spec.** Specify an await/wait-for-completion surface over existing background
jobs — something the client can hold open or poll cheaply — rather than a new async execution
model. Concretely: a way to block on `@bg_<id>` with a timeout, returning the job's status and
buffer handle. That converts 35 blocking foreground calls into 35 cheap waits and removes the
incentive to write `sleep` loops.

**Scope note:** the remaining 90% of the >10s band (1,287 calls, ≈9.1h) is `cargo`
build/test/clippy. Those are genuine long work, not hand-rolled waiting — but they are the primary
*consumer* of an await primitive, since they are what a caller would background first.
## CAP-5 — Server-assigned entry ids for prose trackers

**Status:** shipped · **Opened:** 2026-08-17 · **Updated:** 2026-08-26

**Valid:** dated 2026-08-26

> **Closed 2026-08-26 — the conditional fired.** Defect class 2, the last one open, is
> closed, and in the exact shape this entry argued for: *"extend `append_entry` rather
> than add `append_section`, so a storage distinction is not encoded as an API one."*
>
> `append_entry(anchor_heading, title, body)` now writes the section itself — `## <ID> —
> <title>` at the ledger's own level, inserted at an anchor, in the same file write that
> assigns the id — which is the *"server-side body writer on top of the one that exists"*
> named above as the remaining scope. The format is pinned by
> `a_written_section_gets_a_def_re_conformant_heading_at_the_ledgers_own_level`, so
> conformance is by construction rather than by validation-after-the-fact, which is
> stronger than this entry asked for. `append_entry` and `update_entry` additionally
> report `undefined_in_body` for an entry nothing can cite
> (`src/librarian/catalog/augmentation.rs`).
>
> Confirmed by use, not only by reading: F-63, W-52 and W-53 in
> `docs/trackers/bug-fix-session-log.md` were each written by one `append_entry` call
> that allocated the id and wrote a conformant heading in the same write.
>
> Found by `librarian(action="doctor")`'s `entry_conditional_past_due`, which is CAP-7's
> own check reporting on CAP-5 — the mechanism this file proposed catching the staleness
> this file had. The note below is left intact: it is the analysis that produced the
> shipped design, and deleting it would erase the reasoning while keeping the conclusion.

**The problem.** Entry ids in prose ledgers are allocated by the agent: scan the file for
the max `PREFIX-N`, add one, write the entry. That read-then-write is not atomic, and it is
the source of every id defect measured on this repo on 2026-08-16/17:

- **9 ids allocated twice** in `reconnaissance-patterns.md`, because the scan used
  `grep '^## R-'` and was blind to the file's second entry format (`| R-N |` rows).
- **A tenth collision missed by about four minutes** on 2026-08-17: a scan returned max 96,
  a peer session wrote `## R-97` into the working tree, and the re-scan immediately before
  writing returned 97. Recorded as R-98.
- **The repair made it worse.** The collision fix appended `a`/`b` suffixes — but the
  resolver's token grammar is `\b[A-Z]{1,3}-\d+\b`, so `R-72b` is not a valid token at all
  and can never be defined *or* cited. The convention produced ids the link graph cannot
  represent.

**Substrate check.**

- `artifact(action="append_entry", id_prefix=…, entry_collection=…)` **already does exactly
  the right thing, atomically** — it computes the next id from the live max across *both*
  existing params entries and ids the markdown body already claims (headings / index rows),
  and returns a `warning` when params lags the body. The primitive exists.
- **It is unavailable to every tracker that broke.** `append_entry` requires an augmentation
  with a declared `entry_collection`. `reconnaissance-patterns.md` (58 entries),
  `tracker-hygiene-log.md` (9) and this file (5) all have `entry_collection: null`; two of
  the three have `augmentation: null` outright. For these, the allocator is unreachable and hand
  allocation is the only option the tool surface offers. `update_entry` has the same
  precondition.
- Nothing in the action set appends a **body section**: `find | get | create | update | move
  | delete | graft | link | graph | state_at | append_entry | update_entry`.

**Substrate check, corrected 2026-08-17 after reading the code rather than the tool
description.** The first version of this check overstated the gap, and the correction makes
the proposal smaller. Verified in `src/librarian/catalog/augmentation.rs::append_entry`:

- **The allocator is already cross-process safe.** It runs inside a single `IMMEDIATE`
  transaction — the doc comment states the read-max-write is safe under *both*
  intra-process and cross-process concurrency, paired with `busy_timeout`. CAP-5's open
  question about the concurrency guarantee is therefore **answered**: it is exactly the
  guarantee the two-sessions-one-checkout case needs.
- **The allocator already reads the markdown body.** `body_claimed_indices(body, id_prefix)`
  folds in the ids the body claims via *both* `## PREFIX-N` sections and index-table rows,
  and `next = params_next.max(body_max + 1)`. The both-formats scan the id-suffix note
  prescribes by hand is already implemented server-side. A `warning` fires when params lags
  the body. Prior art: `docs/issues/archive/2026-07-20-append-entry-id-drift-params-vs-body.md`.
- **The documented flow already puts the body first** — the comment names it as "body
  section → index row → `append_entry`", and calls the markdown body "the canonical
  human-readable surface". The substrate already voted for headings-as-canonical; a
  params-canonical redesign would be arguing against it.

**So the defect is coupling, not absence.** Id allocation — a general, body-aware,
transactional service — is welded to a params write. A caller must possess an
`entry_collection` to obtain an id, which means the one thing a prose tracker needs is
gated on the one thing it does not have. That is the wall in the wrong place.

**Revised proposal: invert the dependency, do not add a sibling action.** Extract the
allocator (transaction + `body_claimed_indices` + `next_index`) so that *both* the params
writer and a body writer depend on it. Adding `append_section` alongside `append_entry`
would encode a **storage** distinction as an **API** distinction — forcing every caller to
know whether a tracker is augmented, which is exactly the knowledge the abstraction should
absorb. Two actions for one concept is the smell; one allocator with two writers is the
shape.

**Why a `get_next_index` would not fix it.** A query that returns the next free id moves the
race rather than removing it: the agent still does read-then-write, and a peer can take the
id in between — which is R-98 verbatim. **Atomicity is the property, not the lookup.** An id
must be assigned by the same call that writes the entry, or it is just a snapshot with a
shorter shelf life.

**Proposal — `artifact(action="append_section")`** (or an `append_entry` that can also write
a body section, which is one mechanism rather than two).

| | |
|---|---|
| **Inputs** | `id`, `id_prefix`, `title`, `body`, `anchor_heading` (insert before), optional index-row fields |
| **Server does** | compute the next id across every entry format the file uses → format the heading as `## <ID> — <title>` → insert before the anchor → optionally emit the index row / params row → return the assigned id |
| **Works on** | any tracker, augmented or prose — no `entry_collection` required |

Four defect classes become structurally impossible rather than matters of discipline:

1. **collisions and the stale-max race** — allocation and write are one operation;
2. **non-conformant headings** — the server emits `## <ID> — <title>`, which is exactly
   `def_re`'s shape (`^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`), so an entry can never be born
   undefined and dangling;
3. **orphaned index rows** — written in the same call, or not at all;
4. **suffixed ids** — never generated, because the server owns the format.

**Evidence that discipline alone does not hold.** Each of three independent ledgers carries a
written instruction telling authors to keep the index in sync, and each violates it:
`reconnaissance-patterns.md`'s template said "Also update the Index table row at the top" and
13 bodies had no row; `tool-usage-patterns.md` avoids the problem only by having no index at
all; and **this file has 5 CAP sections and, until this entry, 3 index rows.** A rule that
fires only when the author remembers to consult it is not yet a rule.

**Shipped, 2026-08-18 — the dependency inversion is done; the body writer is not.**

The substrate check above is now **out of date in its central claim**, and this note is the
correction. Read it before acting on anything above it.

> ❌ *"It is **unavailable to every tracker that broke.** `append_entry` requires an
> augmentation with a declared `entry_collection` … the allocator is unreachable and hand
> allocation is the only option the tool surface offers."*

That was true when written and is false now. `augmentation::allocate_entry_id` was added in
`540c29c3` and **is wired to the MCP surface** at `src/librarian/tools/append_entry.rs:91`:
`append_entry` takes a prose path precisely when `entry_collection` is *omitted*, reserving
the id under the same `IMMEDIATE` transaction and handing it back. It reaches exactly the
three files this entry named as unreachable — `reconnaissance-patterns.md` (now
`entry_prefix: R`, `entry_high_water_R: 104`), `tracker-hygiene-log.md`, and this file.

So **this entry's own "Revised proposal" — *invert the dependency, do not add a sibling
action* — shipped.** Three of its four defect classes are closed:

1. **collisions and the stale-max race** — closed. Allocation is transactional, and the
   counter is the max of three inputs (body, machine-local reservation, and the committed
   `entry_high_water_<PREFIX>` frontmatter mark), so no single input can walk it backwards.
3. **orphaned index rows** — closed for the params case by `update_entry`'s `snapshot_stale`
   report; still hand-work where a tracker keeps a body index.
4. **suffixed ids** — closed. The server owns the format of the id itself.

**Defect class 2 is still open, and it is the remaining work.** `allocate_entry_id`
*reserves* an id and returns a hint saying "then write the section yourself with the id it
returns" — the agent writes the heading, and nothing validates what it wrote. `link_scan`'s
`def_re` is `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`, so an agent that writes `## R-105` without
` — title` creates an entry that **defines no token**, and every citation of it dangles.
That is the mechanism `get_guide("tracker-conventions")` records as having produced ~30 of
39 sampled dangling tokens in this repo. Discipline is currently the only thing preventing
it, and this entry's own *"Evidence that discipline alone does not hold"* section argues why
that is not enough.

The remaining scope is therefore narrower than the table above: not a new allocator, but a
**server-side body writer** on top of the one that exists — format `## <ID> — <title>`,
insert at an anchor, in the same call that assigns the id. This entry already argues the
right shape for it (extend `append_entry` rather than add `append_section`, so a *storage*
distinction is not encoded as an *API* one), and that argument is unaffected by what shipped.

One open question above is **answered** by the shipped code: *"Concurrency: … worth
confirming whether that is process-level or file-level."* It is file-level — a single
SQLite `IMMEDIATE` transaction paired with `busy_timeout`, which is the guarantee the
two-OS-processes-one-checkout case needs. A two-thread test against a ledger with no params
collection ships alongside it.

Why this note exists rather than a rewrite: the analysis above is what produced the shipped
design, and deleting it would erase the reasoning while keeping the conclusion. Third stale
artifact found on 2026-08-18 by the same mechanism — a fix landed and nothing re-read the
document that asked for it. The other two:
`docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`
(said "NOT yet called from any MCP tool" of code already wired) and
`docs/issues/archive/2026-08-18-tracker-conventions-guide-recommends-reverted-id-stamping.md`
(fixed and archived the same day, `d3c1e6ed` — and chasing it one layer down found the real
origin in `tracker_design`'s archetype defaults, not the guide).

**Open questions.**

- Should `append_section` write the params row too when the tracker *is* augmented, or should
  `append_entry` gain a `body_section` option? The latter keeps one mechanism.
- Concurrency: the write needs the guarantee `append_entry` has today — worth confirming
  whether that is process-level or file-level, since the failing case is two OS processes
  sharing one checkout.
- Missing anchor heading: refuse, or append at end of body?
- Does this subsume the id half of CAP-4, or complement it? CAP-4 warns about collisions
  between sessions; CAP-5 removes one class of them at the source.

**Kin:** R-98 (the race, measured), R-99 (the entry template as root cause), HY-9 / proposed
detector D12 (headings are what define tokens), and the entry-level standard now in
`get_guide("tracker-conventions")`.

## CAP-6 — Derive the TAXONOMY table from `entry_prefix` declarations

**Status:** proposed · **Opened:** 2026-08-17

**The ask.** `docs/TAXONOMY.md` is the one-page index CLAUDE.md tells every agent to
start from when deciding where an observation belongs. It is hand-maintained, and it
drifted **twice on 2026-08-17 alone**:

- its T-N and WIN-N rows prescribed `artifact_augment(merge=true, params={…})` — the
  call CLAUDE.md documents as having taken the T-N queue from 19 entries to 1
  (`9943164e`);
- its R-N, U-N and H-N rows prescribed `edit_markdown`, which those ledgers now
  refuse (`fddb7408`, `87f55156`).

Both rows were true when written. Both became false when the substrate moved, and
nothing connected the two. The drifting column is always the same one: the append
recipe — the only column that is a *fact about the tool*.

**Substrate check (verified 2026-08-17, by reading the code).**

- `entry_prefix` now lives in each ledger's **frontmatter** (`ffd3b432`), so the
  prefix→file mapping is machine-readable straight from the repo. Five ledgers
  declare today: R, U, H, HY, CAP.
- `frontmatter::parse` already surfaces it via `Frontmatter.extra`, and
  `allocate_entry_id` already reads it on every call — so nothing new is needed to
  *read* a declaration.
- **But it is not catalog-indexed.** `extra` is documented as "written verbatim to
  YAML and round-trip-safe … NOT catalog-indexed — NOT filterable via find". So
  generating the table today means walking files, not querying. That is the one
  genuinely missing piece.
- `augmentation.entry_collection` already distinguishes params-backed ledgers (T,
  WIN, PV, A) from prose ones, which is what decides the recipe.

**What is and is not derivable.** Derivable: prefix, file, artifact id, storage kind,
and therefore the correct append recipe — exactly the column that drifted. Not
derivable: "Captures" and "Promotes to", which are editorial judgement. So the right
shape is a **generated block inside a hand-written page**, never a generated page.

**Proposal, cheapest-first.**

1. **A cross-check test.** Walk `docs/trackers/*.md` for `entry_prefix` and fail when
   a declared prefix has no TAXONOMY row, or a TAXONOMY row names a prefix nothing
   declares. No new tool, no schema change — and it would have caught both of
   today's drifts.
2. **Index `entry_prefix`** in the catalog (a column, or a filterable projection of
   `extra`) so the mapping is queryable rather than requiring a file walk.
3. **Render the block** — `librarian(action="taxonomy")` or a `doctor` check emitting
   prefix / file / id / storage / recipe.

(2) and (3) only earn their cost if something besides the doc needs to query the
mapping. (1) closes the drift class on its own.

**Open questions.**

- Does the generated block belong *in* TAXONOMY.md, or should TAXONOMY link to the
  rendered output and stop restating it? Restating is what drifts.
- Should a TAXONOMY row with no declaration be an error or a warning? **F/W are
  deliberately undeclared** pending HY-10's namespace decision, so at least one
  legitimate row has no declaration — the check needs an explicit exemption or it
  fires on a state we chose on purpose. A check that flags a deliberate decision is
  how gates lose their authority.

**Kin:** HY-10 (the 3:27 ledger/tracker ratio, and the F/W blocker), CAP-5 (the
allocator this builds on), R-99 (a convention documented anywhere other than where
authors look is not a convention).

## CAP-7 — Make record decay detectable — three doctor checks so corrections travel

**Status:** **COMPLETE — all three checks shipped 2026-08-19.**

| check | commit | patch-id |
|---|---|---|
| 1 `archived_fix_sha_unresolvable` | `b34bf10e` | `cdb2b89d91f336f7b9af25675fd6d1803a79ca4b` |
| 2 `terminal_status_with_caveat` | `067ced2c` | `4315447d80b5bed845e5911dd8178adaf3b72a9a` |
| 3 `declared_root_missing` | `f632e7ef` | `757f9d606e2ad65c1e38b685b3f18e2ee3a227e2` |

Layer 2 is done. The entry stays here rather than graduating to a spec because there is
nothing left to design — it is a record of what shipped and why.

### The ask

Three new checks in `librarian(action="doctor")` so that a record's decay becomes a
**query** instead of something a human has to notice. This is "Layer 2" of the
record-legibility work; Layer 1 (the `unverified:` caveat field) shipped 2026-08-19 across
`CLAUDE.md`, `docs/TAXONOMY.md`, `docs/issues/_TEMPLATE.md` and
`src/prompts/guides/tracker-conventions.md`.

1. ~~**`archived_fix_sha_unresolvable`**~~ — **SHIPPED 2026-08-19**, `b34bf10e` / patch-id
   `cdb2b89d91f336f7b9af25675fd6d1803a79ca4b`. Reports the file, the dead SHA, and the
   recorded patch-id **with** the finding, so the remedy travels instead of being looked up
   afterwards. Live: scanned 54, skipped 324, unresolvable 0 — a true negative, since those
   54 pointers were written the same day from live commits; the check earns its keep at the
   next rebase of `experiments`.
2. ~~**`terminal_status_with_caveat`**~~ — **SHIPPED 2026-08-19**, `067ced2c` / patch-id
   `4315447d80b5bed845e5911dd8178adaf3b72a9a`. A bug file whose `status` is terminal
   (`fixed`/`mitigated`/`wontfix`) *and* whose `unverified:` field is non-empty — the
   population the canonical triage query hides by construction. Archived files are included
   deliberately (nothing re-reads `archive/`, so a caveat there is *more* hidden). Live on
   this repo it reports **8**, and the list is immediately actionable: three of the eight
   say their fix SHA is missing, obsolete, or promised-but-absent, which is exactly the
   population check 1 exists to resolve. Four mutations applied and run, four killed, all
   load-bearing.
3. ~~**`declared_root_missing`**~~ — **SHIPPED 2026-08-19**, `f632e7ef` / patch-id
   `757f9d606e2ad65c1e38b685b3f18e2ee3a227e2`. For each `[[project]]` in the workspace
   config, `<config_owner_root>/<declared_root>` must be a directory. Open decision 4 was
   settled as its leaning said: only the checkout's OWN config is read, and every skip
   (linked worktree, absent / unreadable / unparseable config) is stated in
   `catalog_health.declared_roots.note` rather than passing silently. Absolute roots are
   rejected separately — `Path::join` discards the base for an absolute path, so they would
   otherwise be validated against themselves. Six mutations applied and run; one survived
   (`is_dir()` → `exists()`, 5/5 green) and was closed with a sixth test.

### Substrate corrections found while implementing (2026-08-19)

Three, all found by scouting before designing, and all in the same class — a figure or a
reachability assumption stated without opening the thing it described. This entry's
augmentation prompt says the substrate check is its load-bearing part; on this entry it was
wrong **four times out of four checks attempted**, which is worth more than any single
correction below.

1. **Check 3:** `doctor` could not reach the `[[project]]` list at all — `ctx.workspace` is
   a *different type with the same name*. Logged as
   `prompt-surface-compaction-session-log:F-6`.
2. **Check 1, corpus size:** the archived corpus is **350 files, not 63**. The 63 was a
   subset, quoted here as if it were the population.
3. **Check 1, parseability:** only **54** carry a structured fix pointer. The other 296
   mention commits in freeform prose — including `a45f1bd7` naming a *reproduction* commit
   and `12707fe` appearing as "suspected the recent refactor" then "**Refactor 12707fe is
   INNOCENT**". Sweeping prose for hex would have reported a dead fix SHA about a commit the
   file explicitly exonerates. The check parses the structured line only and **reports its
   coverage**, so a clean result cannot be read as "every declared fix resolves".

### Substrate check (2026-08-19)

- `librarian(action="doctor")` **exists** and is already a named-checks drift scanner:
  `src/librarian/tools/doctor.rs`. A live run this date returned 8 check kinds
  (`abs_path_outside_managed_roots` 310, `entry_without_definition` 1,
  `frontmatter_id_is_not_a_catalog_id` 3, `frontmatter_id_mismatch` 3,
  `ledger_defines_nothing` 2, `missing_file` 2, `params_behind_body` 1,
  `worktree_scoped_row` 2). Adding a check is idiomatic, not novel.
- `scan_undefined_entries` (`src/librarian/tools/doctor.rs:1501`) is the nearest sibling in
  shape — it already emits a per-artifact finding citing the conventions guide.
- **Check 3's ASSERTION is fully specified already**, by the bug it would have caught:
  `docs/issues/archive/2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md`
  names the assertion, the report shape, and says to site it next to
  `abs_path_outside_managed_roots`. **Its substrate is not** — corrected 2026-08-19 after a
  pre-implementation scout, logged as `prompt-surface-compaction-session-log:F-6`. This
  entry previously read "Nothing needs designing", which was wrong twice:
  - **`doctor` cannot reach the `[[project]]` list.** `ctx.workspace` is
    `crate::librarian::workspace::WorkspaceConfig` (`src/librarian/workspace.rs:9`) and
    carries `.roots`. The `[[project]]` entries live on a **different type with the same
    name**, `crate::config::workspace::WorkspaceConfig` (`src/config/workspace.rs:4-12`),
    whose `projects: Vec<ProjectEntry>` holds the `{id, root}` pairs
    (`src/config/workspace.rs:39-46`). Nothing threads it into `ToolContext`
    (`src/librarian/tools/mod.rs:84-103`). So check 3 must locate, read and parse
    `.codescout/workspace.toml` itself — precedent at `src/tools/config/mod.rs:511-515`,
    path via `crate::config::workspace::workspace_config_path` — or the plumbing changes.
  - **The worktree case is a decision, not an edge case.** The config is gitignored, so it
    does not travel into a linked worktree; when absent there, discovery falls back to the
    **main** checkout's settings, a state the code already names `topology: "inherited"`
    (`src/tools/config/mod.rs:939-948`). "The active workspace config" is therefore
    ambiguous in exactly the situation this repo is usually in. See open decision 4.
- **Genuinely missing:** no check reads bug-file frontmatter at all, and nothing resolves a
  git SHA. Check 1 needs a git handle inside doctor — `resolve_head_sha` and
  `probe_has_git_remote` already use `git2`, so the dependency exists; do not shell out.
- Rule-of-three is satisfied: `audit_doc_refs`, `link_scan` and `doctor` are three existing
  instances of "continuous mechanical re-check", so a fourth predicate under the same
  chokepoint is earned duplication rather than a speculative abstraction.

### Why it matters (measured 2026-08-19)

- **10 of 63** archived bug files had already lost their fix SHA; the objects are absent
  from the object database, and subject-keyword recovery returns 2–153 candidates.
- **14 of 16** terminal-but-unarchived bug files stated a blocker in prose that no query
  could reach. One `fixed` record whose body read *"Tests added: None"* and *"does not
  prevent recurrence"* was invisible to `find(kind="bug", status="open")`.
- The correction problem is **propagation, not knowledge**: on 2026-08-18 one bug file
  independently worked out the fast-forward rule and wrote it down, while three siblings sat
  waiting on a SHA that would never exist. Nothing re-reads a bug file once written. These
  checks are what would make such a correction travel.

### Open decisions

1. **Severity and gating.** Should any of these fail CI, or report only? Leaning
   report-only: the measured cause of the unarchived pile is an unsatisfiable gate, and
   adding a gate is how that recurs. Visible absence beats mandatory verification.
2. ~~**Where check 2 draws the line.**~~ **RESOLVED 2026-08-19: report every caveat, no
   severity split.** Not deferred silently — the reason is that no leading-marker convention
   exists, so introducing one at implementation time would leave every caveat written before
   that moment unmarked and force them all into whichever bucket the default picks. An
   undifferentiated list of 8 beats a differentiated one whose categories are an artifact of
   when each entry happened to be written. Revisit only if the population grows enough that
   reading all of it stops being cheap.
3. ~~**Cost of check 1.**~~ **RESOLVED as suggested:** the SHA is resolved via `git2`
   (`revparse_single`, ~54 calls); the patch-id is *reported, never resolved*. A history
   scan for a patch-id is exactly the expensive thing the suggestion avoided, and the
   finding hands the reader the redirect-form recipe instead of running it for them.
4. **Which root check 3 resolves against, in a worktree** (added 2026-08-19 with the
   substrate correction). Three sub-questions, and answering only the first is how this
   ships wrong-but-green: (a) when the worktree has no `.codescout/workspace.toml` of its
   own, does the check read main's, or report nothing? (b) if it reads main's, does it
   resolve declared roots against the worktree root or the main root? (c) should an
   inherited config be checked at all, given the operator cannot fix it from here? Leaning:
   check the checkout's OWN config only, and when absent emit nothing rather than a
   finding — an inherited config is main's to repair, and reporting it from five worktrees
   turns one defect into five. State the skip in the report so the silence is legible.

### Resume

1. Read `scan_declared_project_roots` (`src/librarian/tools/doctor.rs`) — check 3, shipped.
   It is now the nearest sibling for checks 1 and 2 in every respect that matters here:
   finding shape, registration point in `call`, a `catalog_health` sub-object that reports
   what was NOT checked, and a hint that surfaces the count.
2. ~~Next: check 2~~ — done. Its substrate prediction held this time: the `artifact` table
   carries `kind` and `status` columns, so it is a SQL narrow plus a frontmatter parse
   (`unverified:` lands in `extra`, which is NOT catalog-indexed). Read
   `scan_terminal_status_with_caveat` as the nearest sibling for check 1.
3. **Next: check 1 (`archived_fix_sha_unresolvable`)** — needs `git2`, and check 2 just
   handed it a target list. Three of the eight caveats check 2 reports are *about a missing
   or obsolete fix SHA*, so check 1 has a ready-made validation set: it should fire on those
   files and not on the other five. Scout whether `resolve_head_sha`'s `git2` handle is
   reachable from `doctor` before designing — that is the assumption class that was wrong
   twice on this entry.
4. Each check needs tests that observe a *planted* violation, and then **N mutations
   actually applied and run**, with the observed surviving count reported. Not a coverage
   argument. See `CLAUDE.md` § mutation-apply discipline — and note that on check 3 the
   discipline paid: the one mutation that survived was on the branch the check's own spec
   sentence names ("exists **and is a directory**"), which reasoning had marked as covered.

## CAP-8 — Content-addressed identity — a "gram" for entries, and stored-not-derived ids for artifacts

**Status:** open — proposed 2026-08-19, measured and validated, **not** implemented. This is
a migration, and should be built after CAP-7.

### The ask

Every identity in this system is **positional**, and every one has a measured failure:

| Identity | Derived from | Measured failure |
|---|---|---|
| artifact id | `sha256(abs_path)` | re-keys on move — archiving is the normal end state |
| entry id | per-file `PREFIX-N` counter | 33% of cross-file entry citations are ambiguous |
| citation qualifier | file stem | truncated past 31 chars (fixed 2026-08-19, `6d0145e1`) |
| fix pointer | git SHA on a rewritable branch | 10 of 63 archived files already dead |

Replace positional identity with **content** identity. Marius's framing: *"identity is by
content, not moving space."*

### Two halves, two mechanisms — do not conflate them

This was got wrong once mid-analysis and is the most important thing on this entry.

**Entries (the gram).** A tracker entry gets an id derived from `hash(title + description)`.
Measured across 10 repos in 2 umbrellas (3263 markdown files, worktrees excluded, fences
stripped):

- **Title churn: 0.7%** — 4 of 598 entries ever had their title changed. The cascade
  ("regenerate and update every reference") would have fired four times in project history.
- **Collision rate:** `hash(title)` 4.8% → `hash(title+description)` **3.4%**. Of the 16
  residual colliding groups, **15 are deliberate repeats** (the same eval case re-run across
  `nav-eval-round-2/3/4/5`, archive duplicates). **Genuine accidental collisions: 1 group,
  2 headings, 0.10%.**
- Prose churn : heading churn is **55 : 1**, so the hash must cover the **title only** —
  never the body.

**Artifacts.** A title-hash **cannot** serve artifacts: **222 files carry `title: null`**,
and the real title collisions are live-ledger/archive-companion pairs (`Prompt Hamsa — Audit
Log` ×2, `Reconnaissance patterns` ×3) — which `get_guide("tracker-conventions")` *mandates*
creating. The fix here is not a hash but an **inversion**: make the stored frontmatter `id:`
authoritative instead of derived.

### Substrate check (2026-08-19)

- Only **359 of 3263** files carry a 16-hex frontmatter `id:` (11%), and **356 of those are
  exactly `sha256(current path)`** — the field is a cached derivation, not identity.
- The **3** that differ are the only artifacts in the corpus carrying evidence of a prior
  identity. `doctor` classifies them as `frontmatter_id_mismatch`, and ships
  `fix=repair_frontmatter_id` **to overwrite them with the path-derived value**. The system
  has a slot for stable identity and a maintenance action whose purpose is to erase it.
  Inverting that is likely a larger share of the work than any new code.
- **CORRECTION 2026-08-19 (same day, hours later): that population is 4, and it is
  CONTAMINATED.** A live `doctor` run after an unrelated rebuild returned
  `frontmatter_id_mismatch: 4`. The new row is a backend-kotlin plan in a *linked worktree*
  whose frontmatter names its main twin — the overlay's fork-on-first-write copying the
  main file's frontmatter, which is correct. Its `worktree_scoped_row` detail reports
  `collision_with` equal to the very id the frontmatter declares. So the check conflates
  **stale-after-move ids** with **live worktree shadows**, and `fix=repair_frontmatter_id`
  — whose only filter is `containing_root` — would rewrite a file in another session's
  active working tree. Filed as
  `docs/issues/archive/2026-08-19-repair-frontmatter-id-rewrites-files-in-registered-worktrees.md`
  (fixed same day, `f772b8fe`).
  **Two consequences for this entry:** (a) do not cite "3 artifacts carry evidence of a
  prior identity" — the true figure is unknown until the check is de-contaminated, and it
  is the number this entry's whole *inversion* argument rests on; (b) the count moved
  within hours of being measured, so treat every population figure here as a fact about an
  instant. The 2026-07-17 ↔ 2026-08-19 growth comparison below is unaffected — it compares
  citation-resolution counts, not this one.
- **RESOLVED same day, `f772b8fe`.** The check now abstains for a worktree shadow declaring
  its main twin, so the population is de-contaminated. A live run after the fix returns
  `frontmatter_id_mismatch: 3`, and all three are genuine post-move stale ids in
  `docs/issues/archive/` and `docs/trackers/` — none in a worktree; `worktree_scoped_row`
  stays at 3, so nothing was dropped, only re-attributed. **The original figure of 3 is
  therefore correct again, and this entry's inversion argument stands** — but it stands on
  a measurement that has now been checked rather than on one that happened to read 3 while
  counting two different things. Keep consequence (b): the figure is still a fact about an
  instant, and re-measuring it is one `doctor` call.
- `link_scan` already resolves entry tokens, artifact ids, rel_paths and md links
  (`src/librarian/tools/link_scan/`), with a pinned tie-break where archived definers lose to
  active ones. A gram would be a fourth citation kind in an existing resolver.

### Payoff, measured

- **Artifact ids:** of 574 distinct 16-hex ids cited in markdown, 151 are dead (26%), and
  **73 of those died purely by re-keying** — the document is still on disk and readable.
  Content identity fixes **48% of dead ids / 45% of dead instances** by construction.
- **Entry tokens:** of 6321 cross-file citations, 43% resolve, **33% are ambiguous**, 24%
  dangling. The gram fixes the ambiguous share; it does **not** fix dangling (missing
  headings), and `doctor` says that population is already largely migrated.
- **Caveat on the two figures above, added 2026-08-19.** They are **upper bounds**.
  `docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md`
  (mitigated 2026-08-21 with a per-source breakdown in `link_scan`'s own report; the caveat
  below is unaffected since extraction itself is unchanged) records that `link_scan` has no
  "mention" mode: a token written to *teach* the syntax is
  extracted identically to one written to *cite*. Every document explaining how citations
  work therefore injects its own examples into the graph — and those land preferentially in
  the **ambiguous** and **dangling** buckets, because a teaching example usually has many
  definers or none. That is precisely the share this entry's argument rests on.
  **Magnitude: small, and bounded by a mechanism worth knowing.** `link_scan` skips fenced
  code blocks and scans only inline and prose tokens, so the majority of examples — which
  live in ``` fences — never enter the graph. A hand-picked sample of four teaching
  artifacts yields 79 tokens (`docs/TAXONOMY.md` 8; the bug file about this problem itself
  **0**). Treat 79 as a floor from an unsystematic sample, not a count: nobody has
  enumerated every teaching artifact, and the share falling in the ambiguous/dangling
  buckets specifically is unmeasured. The correction does not overturn the case for CAP-8 —
  it means the headline percentages should be quoted as "at most", and that the
  2026-07-17 ↔ 2026-08-19 growth comparison is only sound if the inflation is roughly
  constant across both surveys, which is plausible but unchecked.
- **Not a codescout pathology.** 7 repos with meaningful entry counts show 26–72% ambiguity;
  `backend-kotlin` (different umbrella, different language, 722 entries) sits at 34%, and
  codescout at 28% is among the healthiest. This is a property of the per-file `PREFIX-N`
  convention wherever it is used.

### Open decisions

1. **Add the gram, or replace `PREFIX-N`?** Strongly prefer *add*. Git's own model is
   content-addressed objects plus human-readable refs; `bug-fix-session-log:F-33` is legible
   and `gram:a3f9c2` is not, and this project's moat is the LLM-facing surface.
2. **Whether identical entries in different ledgers SHOULD share a gram.** Under content
   addressing they do, and for the eval-round repeats that is arguably correct. Decide
   before implementing, because it determines whether the ledger is part of the key.
3. **Migration order.** Additive first: compute and store grams, re-key nothing, leave every
   existing citation working. Big-bang re-keying is the rewrite trap.
4. **The declared-vs-undeclared F/W contradiction** (see `docs/TAXONOMY.md` § Main taxonomy)
   should be settled first — it changes what the gram is being asked to fix.

### Resume

1. Settle open decision 2, then 1 — neither needs code.
2. Prototype gram computation over the existing corpus and re-run the collision measurement
   before writing any resolver change; the numbers above are reproducible from
   `docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md`'s method.
3. Treat the `repair_frontmatter_id` inversion as its own change with its own decision — it
   changes what `doctor` considers a defect.

### Substrate CORRECTION 2026-08-19 — an entry-id scheme already exists, and partly ships

Found after the entry above was written, by a semantic query for prior art that should have
run first. **The substrate check above is incomplete. Read this before acting on CAP-8.**

`docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md` is a design for
exactly this problem, with an *Identity model* section, closing TMR-1 ("entries, not files,
are the graph nodes, with globally unique entry IDs") and TMR-7 ("edges captured at write
time") of `docs/trackers/tracker-management-redesign.md`.

And it is **not just a design**. The live `artifact` tool schema shows `append_entry`
accepts a `cites` parameter whose refs may be "a 16-hex artifact id, a `<slug>:<local>`
entry id, or a unique rel_path", creating `entry_cite` edges atomically; `update_entry`
refuses to change an entry `id` precisely because "entry ids key `entry_cite` rows". So a
per-tracker frozen **slug** plus `<slug>:<local>` entry ids is an existing, shipped identity
scheme.

**What this changes:**

- The gram would be a **third** entry-identity scheme unless it supersedes `<slug>:<local>`.
  Decide that explicitly — it is now the first question, ahead of the two listed above.
- The Stage-2 spec **explicitly scopes out** re-keying artifact ids (`sha256(abs_path)` →
  `sha256(rel_path)`) as a catalog-wide migration, which is the same territory as this
  entry's artifact half. Read its *Backward compatibility* and *Move durability* sections
  before re-deriving that analysis.
- Its motivation survey is a **directly comparable earlier measurement**, and the
  comparison is the argument for doing something:

| | 2026-07-17 (spec survey) | 2026-08-19 (this session) |
|---|---:|---:|
| artifacts scanned | 821 | 1073 |
| citations | 2144 | 3858 |
| resolvable edges | 487 | 922 |
| **ambiguous** | **248** | **423** |
| **dangling** | **372** | **534** |

The design was specced a month ago and both loss populations grew by roughly 70% and 45%.
That is the case for scheduling it, and it is stronger than anything in the entry above.

**Method note for whoever picks this up:** this correction exists because a semantic query
for prior art was run during an unrelated cleanup, not during the proposal. Query the
catalog for existing specs *before* writing a capability proposal — the substrate check is
the load-bearing part of a CAP entry, and mine missed a shipped feature.

## CAP-9 — Friction observability: fix attribution first, then a two-predicate detector and an in-band `friction()` self-report

**Ask.** In the requester's words: *"what do we need more to measure frictions with the
agent, maybe a column in the db, maybe a feedback tool, more logs... I want to be able to
easily spot frictions from logic/systemic/tool usage/etc."*

This entry is the surviving design after a five-agent reconnaissance (2026-08-19/20) killed
five of six things originally proposed. It is deliberately ordered: **items 1-2 are
prerequisites, items 3-5 each still have an open decision.**

### The ask, in five items

1. **Fix attribution.** Resolve `cc_session_id` per *call* rather than per process, and add
   `agent_id` + `is_sidechain`. Everything else reads off these numbers.
2. **Two detector predicates, replacing three that failed.** `S-A` = consecutive-error run
   in one session with no intervening success; `S-B` = `err_family IS NULL`. Fire an in-band
   nudge once per session per key through `GuideLedger::notice_once`.
3. **A `friction()` tool** writing typed observations (kind/subkind/severity/notes, bound to
   the triggering `tool_call_id`), so the agent can record logic friction and user
   corrections that leave no mechanical trace.
4. **Ingest `toolDenialKind`** from the transcript as three separate counters:
   native-redirect, codescout-tool-misuse, `user-rejected`.
5. **Offline narration analysis** — high-precision first-person-error regexes over
   `assistant:text`, as a report, not a nudge.

### Substrate check (2026-08-20)

| Piece | What exists | What is missing |
|---|---|---|
| Per-call session identity | `ServerEnv::from_env` reads `CLAUDE_CODE_SESSION_ID` once (`src/server.rs:115-125`); `CodeScoutServer::cc_session_id` doc says *"Resolved once on purpose"* (`src/server.rs:169-177`); cloned per call at `src/server.rs:974-977`. The **rendezvous already polls per call** and re-keys the ledger — `session_key` is documented as *"Distinct from `cc_session_id`, which is usage-correlation only"* (`src/server.rs:178-180`) | Telemetry reading that resolved identity. The machinery is built; it is simply not wired to `usage.db` |
| `agent_id` / `is_sidechain` | **Nothing.** `grep("agent_id\|is_sidechain\|agentId", "src/**/*.rs")` → 0 matches | The whole column pair |
| `S-A` (consecutive-error run) | `outcome`, `cc_session_id`, `called_at` all present on `tool_calls` | Write-time bookkeeping, or a query — no new column strictly required |
| `S-B` (`err_family IS NULL`) | `err_family` + `normalize_err_family` + fingerprint backfill (`src/usage/db.rs:225`, `:444-483`, `:485-525`) | **Nothing — it is a `WHERE` clause** |
| Nudge channel | `GuideLedger::notice_once` (`src/tools/guide_ledger.rs:324-326`) and `refusal_predicate` (`src/prompts/mod.rs:487-519`), the carrier measured at 96% next-call compliance | A friction-nudge caller |
| Observation storage | `pika_observations` table EXISTS in `usage.db` with a full typed schema (kind ∈ iron_law/tool_bug/misusage/pattern, subkind, predicate, verdict ∈ slip/habit/promoted/rejected, severity, recurrence, `u_id`/`h_id`/`t_id`/`bug_id`). **0 rows, and 0 references in `src/`** — created by a buddy plugin skill (`<profile>/plugins/cache/sdd-misc-plugins/buddy/0.9.1/skills/codescout-pika/sql/v1-bootstrap.sql`) | A write path, an ownership decision, and durability — see the filed orphaning bug |
| `toolDenialKind` | Structural field on `type:"user"` transcript entries, paired with `is_error:true` on 153/153 denials, stable across CC 2.1.233-235 | Any ingest at all — `usage.db` cannot see these by construction |
| Turn / token axes | `promptId` (turn key, 0/126 contiguity violations) and `message.usage` (100% coverage) exist **in transcripts only** | Deliberately NOT proposed as columns — see *Withdrawn* below |

### Why it is worth building (measured 2026-08-19/20, mostly not self-generated)

- **The current instrument mis-attributes most of its own rows.** 72.7% of calls and 68.3%
  of errors originate in dispatched subagents filed under the parent's `cc_session_id`;
  separately, 30.9% of rows (8,980/29,103) sit in pools where one server `session_id` carries
  2+ `cc_session_id` labels. Every per-session friction number in this repo is a blend.
- **The detector predicates were validated against ground truth, not asserted.** Over 840
  matched errors (74.5% of the 30-day corpus), against a 27.0% base friction rate:
  `S-A OR S-B` = **48.0% precision, 1.78x lift, 26.0% recall, ~157 fires/30d, zero new
  columns.**
- **The three originally-proposed detectors all failed.** `repeat_family` 1.16x at best,
  `target_thrash` **27.1% against a 27.0% base rate** (friction-random), `route_around`
  **0.79x** — worse than firing on every error, and all five tightenings went *down*
  (0.55-0.69x). Route-around firings are dominated by agents correctly obeying the gate.
- **`err_family IS NULL` self-retires.** A named family is evidence someone already wrote a
  teaching hint; NULL is the complement of the teaching effort, and measures 1.97x lift. As
  gates get authored, high-friction NULLs migrate into named families and the signal fades —
  the correct behaviour for a friction detector.
- **In-band self-report beats transcript parsing for the human-correction class.** Keyword
  search over user text: 67 hits, **0 genuine**. The agent, which sees both the correction
  and its own call history, is the more accurate instrument — not merely the more convenient
  one. Agent self-reports outnumber human corrections ~54 to ~7 in the same 32 hours.

### Open questions

1. **Historical rows.** 30.9% carry a possibly-wrong label and nothing distinguishes them.
   Fixing the write path leaves a half-trustworthy corpus with no way to tell which half. A
   `session_attribution` column or a documented cutoff date — option 3 of the 2026-08-16 bug,
   still unimplemented after two related fixes.
2. **Who owns `pika_observations`.** Adopt it into codescout's migrations, or leave it
   plugin-owned and make observations self-sufficient? Enabling `PRAGMA foreign_keys` to make
   the declared cascade real would *also* start deleting observations at 30 days, which is
   probably wrong for a durable record.
3. **Does `S-B` survive its own success?** If classifying today's NULL head keeps the bucket
   near 2x lift, the signal is about *newness* rather than those specific messages. Worth
   re-measuring after the first batch of families lands, before wiring anything to it.
4. **Is `agent_id` worth it on detection grounds?** No — friction lift across the
   parent/subagent boundary is 0.98x. It buys **correctness of attribution**, not precision.
   Subagent cost share ranges 0.5%-63.4% by workflow, so the value depends on how common
   delegation is across projects.

### Withdrawn, with reasons (so they are not re-proposed)

- **`resolves_id`** — needs a target on success rows; `is_friction = overflowed || outcome !=
  "success"` (`src/usage/mod.rs:81`) gates extraction off by design. 0 of 25,696
  non-overflowed successes have one.
- **Turn and token/cost columns** — codescout sees one `call_content()` invocation. `promptId`
  is a Claude Code transcript concept never sent to the server; tokens live in the API
  response consumed by the harness. Any server-side field would be unverifiable
  client-supplied data or a transcript parser rewritten in Rust. The 30-day sweep also makes
  `usage.db` the *less* durable side. Fix the transcript tooling instead.
- **Rework-loop density as a proxy** — rework is the normal mode: median repeat-edit fraction
  0.68 across 58 buckets, the five known anchors ranked 12/17/28/30/55 of 58, none in the top
  6, and the two pure-reasoning anchors fell below the edit threshold entirely. The filter
  selects *against* the population it was meant to catch.
- **`thinking`-block detection** — 0 of 4,906 blocks carry text; Opus-5/Sonnet-5 emit an
  encrypted signature only.

### Correction 2026-08-20 — S-B is falsified; S-A survives

Measured a few hours after this entry was written, with `scripts/friction-probe.py`
(calibrated against TU-7's published immediate-repeat figures first: ratios 0.9 / 0.82 /
0.63 on the three families with usable n, ordering preserved). **The 1.97x lift for
`err_family IS NULL` does not reproduce from `usage.db`.**

| Predicate (usage.db only, whole 30-day corpus) | NULL | classified | ratio |
|---|---:|---:|---:|
| immediate repeat (TU-7's discriminator) | **2.8%** (2/71) | ~4-5% avg | NULL is **better** |
| same tool succeeds later | 15.5% | 11.2% | 1.38x |
| any success in session | 0.0% | 0.4% | too lenient to discriminate |
| `calls_to_recovery` | mean **1.13**, 0% unrecovered | mean 1.0-1.67 | mid-pack |
| *(POV1's transcript-joined label, for reference)* | *53.2%* | *27.0%* | *1.97x* |

Under every predicate computable from the database the detector would actually run
against, the NULL bucket lands at or below the corpus average. Only the transcript-joined
label puts it at 1.97x, and that label is not reproducible here.

**Why the story was wrong, not just the number.** The justification was "a named family
means someone wrote a teaching hint; NULL is the complement of the teaching effort". The
coverage table says something narrower: `run_command` is **0.2%** unclassified,
`read_markdown`/`grep`/`references` **0%**, against `artifact` **37.5%** and
`memory`/`symbols` ~50%. The NULL population is **49% librarian/artifact API-shape errors
plus 31% a single worktree-activate write gate** — one uncovered *surface*, not a general
untaught population. Classifying it (see
`docs/issues/archive/2026-08-20-largest-unclassified-error-is-the-worktree-activate-write-block.md`,
fixed in `4c7608ee` — 69 of 73 now classify)
is still worth doing for taxonomy reasons; it is not a friction detector.

**A caveat against over-reading this refutation.** One test run while producing it was
itself invalid and is recorded so the mistake is not repeated: "recovery = a later success
sharing the same `friction_target`" returns ~99% friction for *both* arms, because
`friction_target` is only populated when `is_friction` is true, so a **success** essentially
never carries one. That predicate cannot fire for any population. It says nothing about
POV1's method, which reconstructed targets from `input_json`. The divergence between the
two labels is therefore **unexplained**, not attributed — what is established is only that
no usage.db-only predicate reproduces 1.97x.

**S-A survives and is directly measurable:** 84 runs of length >=2 over 30 days, 181 of
1,133 errors (16.0%) inside one, longest run 5 — consistent in magnitude with the ~69
fires POV1 measured on its subset.

**What this changes in the ask above.** Item 2 becomes **`S-A` only**. The "zero new
columns" framing was hiding that S-B's *evidence* was never reproducible from the store the
detector would query — a detector and its justification must live in the same instrument.
Open question 3 is **answered and closed**: do not wait to re-measure after classification;
the bucket is not high-friction now.

**And a fifth open question, from the same pass.** Before/after comparison on this corpus is
not yet possible: across a 2026-08-18 22:30 cutoff the adjusted effect is 0.86pp against
+/-0.40pp Poisson noise (2.15 sigma), needing ~8,071 clean calls per arm for 80% power
against 1,740 available — **4.6x more data**. Three confounds have to be handled every time
(workload mix, build identity + `codescout_dirty` at 4% -> 22%, and reconnect blending: the
24h window held 15 builds across 23 processes). A fourth is unhandled: **mix confounding is
fractal** — `read_markdown` looked 2.4x worse after the cutoff with its error *composition*
unchanged (`librarian_managed_artifact` 68% before, 67% after), because the within-tool
workload moved. The unit that would fix it is (tool x target-kind), which needs
`friction_target` — NULL on 96.6% of rows. That is a third independent argument for the
`friction_target` bug.

### Resume

Do **not** start at the detector. Start at
`docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md`
and decide its open question 1 (historical rows), because the answer determines whether a
column is needed in the same migration as the per-call resolution fix. Items 1-2 are the
only ones worth specifying until that lands; items 3-5 each depend on a decision above.

Six supporting defects were filed 2026-08-20 in `66654f53` — session-id freezing,
`friction_target` key omissions, `pika_observations` orphaning, worktree telemetry deletion,
the unclassified-error head, and a 2.1-2.6x cost overcount in `cc.py`. Read them before
trusting any number in this entry that they touch.
## CAP-10 — Practice rules: a curated, agent-agnostic rule set injected at the moment it applies

**Status:** open — proposed 2026-08-20 with measured evidence from the same day. **Open decision
1 settled 2026-09-02**: option 2 (inferred from the tool sequence), and the mechanism turned out
to already exist — see § *Open decision 1 — SETTLED*. Decisions 2–4 remain open. Next action is
**one** drafted rule plus its arm, not three.

**Valid:** conditional — until the plan-writing rule's arm reports whether an injected practice
rule changes behaviour

*(The previous condition — "until a delivery mechanism is chosen (Open decision 1)" — **fired**
2026-09-02 and is replaced rather than deleted, so the entry's decay class tracks the next
unanswered question instead of one already settled.)*

**Rests on:** codescout is agent-agnostic by design — a rule that only reaches Claude Code is not a codescout capability. See memory `conventions` § Agent-Agnostic Design.

### The ask

codescout has accumulated a body of **working rules** — how to trust a number, when a green result proves nothing, what makes a claim checkable. They are model-agnostic and hard-won, and they live in three places, none of which reaches the moment the rule applies:

| Where they live | Why it does not fire |
|---|---|
| `CLAUDE.md` | Claude Code only; loaded once at session start, never re-surfaced at the moment of use |
| `get_guide(topic)` | Delivers **tool contracts** (how to call `edit_code`), not **practice** (what makes a claim trustworthy) |
| Session-log ledgers (`R-N`, `F-N`, `W-N`) | Durable and well-evidenced, but nothing surfaces them unprompted |

The surfaces where these rules would fire are **third-party skills** — `superpowers:writing-plans`, `subagent-driven-development`, `brainstorming`. **We cannot edit them.** They live in a plugin cache (`~/.claude-sdd/plugins/cache/superpowers-marketplace/…`); an update overwrites any change, and the change would not travel to another machine, profile, or agent.

So: a **curated, versioned set of practice rules, delivered by codescout, injected when the activity they govern begins.**

### Why it matters — measured 2026-08-20

Executing `docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md` through `subagent-driven-development`, **six of six task briefs contained code defects**, every one from a single cause: the plan's Rust was written from `symbols(path=…)` overviews rather than from function bodies.

| Task | Defect in the plan's code |
|---|---|
| 1 | `def_re` has one capture group, not the two assumed; the draft matched the raw `##` line, which the pattern cannot match |
| 2 | `once_cell` is not a dependency; `RecoverableError::with_hint` returns `anyhow::Error`, not `Self` |
| 3 | hand-rolled `today_iso()` when `chrono` was already used verbatim in two files; `ArtifactRow.abs_path` is `PathBuf`, not `String` |
| 4+5 | the brief's test assumed occurrence-counting; `extract()` deduplicates citations per document |
| 6 | hand-rolled date arithmetic **again**; `s.text` where `declared_section_text` was required; a "Consumes" line naming the wrong function; test helpers that do not exist |

One rule would have prevented all six: **a plan that names a function must have opened it.** It belongs in `writing-plans`, which is not ours.

The per-dispatch mitigation — telling every implementer "the brief is a draft, read the real signature first" — worked six times out of six. That is a workaround applied at the wrong end, by hand, once per dispatch, and it does not survive the session.

### Substrate check (2026-08-20)

Most of the mechanism already exists.

- **`get_guide(topic)` already does just-in-time injection.** A topic auto-injects on the first tool call that touches it, and a per-conversation `guide_hints_emitted` ledger stops it re-sending. That ledger is keyed by **conversation identity**, persists to disk, and survives `/mcp` reconnects (`get_guide("workspace-state")` § Per-session state reset). The delivery mechanism is built and already load-bearing.
- **The rules already exist as prose.** `docs/trackers/reconnaissance-patterns.md` (61 `R-N` entries), the session logs, and CLAUDE.md's Iron Rules. What is missing is curation and a trigger, not authorship.
- **Promotion machinery is in flight.** `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` gives an entry a declared decay class, a durable route to its proof, and (Layer 5) an attestation record. **A practice rule is exactly a promoted Statement**, so that spec supplies the metadata rather than requiring a parallel scheme.
- **The prompt-surface cap is the standing warning.** `src/prompts/README.md` pins a 1900-**character** slice cap precisely because always-resident prose stops being read. Whatever the curated corpus grows to, the injected slice must stay small.

### Open decisions

1. **How is the activity detected?** The real design question:
   - **Explicit** — the agent calls `get_guide("practice:planning")`. Honest and agent-agnostic, but depends on the agent knowing to ask, which is the problem being solved.
   - **Inferred from the tool sequence** — a `create_file` under `docs/superpowers/plans/` is a strong planning signal; a subagent dispatch is a delegation signal. Fires without being asked; risks firing wrongly.
   - **Hook-driven** — Claude Code only. **Rejected on the ask's own terms**: agent-agnosticism is the whole reason this beats editing `CLAUDE.md`.
2. **One namespace or two?** Practice rules inside `get_guide` risk diluting tool contracts, which are a different kind of claim with a different failure mode. A sibling tool costs a slot against `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`.
3. **What stops this becoming the thing it warns about?** An unread wall of prose injected at the wrong moment is strictly worse than nothing: it consumes context and trains the reader to skip.
4. **Curation — who promotes a rule into the injected set?** The `Promote-when` machinery the validity spec describes, pointed at a new target. Do not build a second promotion path.


### Open decision 1 — SETTLED 2026-09-02, and the question partly dissolved

**It was posed as "which mechanism?" and the answer is "partition by trigger reachability" — a
question that is now measurable rather than arguable.** The partition: *is the triggering act a
call to a registered codescout tool?* If yes, it routes today with **no new mechanism at all**.
If no, it is the same blocker as `OP-2` and `OP-3`, and that is one decision rather than three.

**Option 2 (inferred from the tool sequence) is chosen, and it turned out to be free.** Between
this proposal being written and today, the operator-rules engine shipped the whole mechanism it
was going to need:

- a rule corpus with `binding: always | triggered` and a `**Serves:**` selector grammar
  (`shape := tool ["." action] ["(" pred ")"]`, `pred := "path~" substring`);
- routing through the same section-grain matcher, with a once-per-session ledger keyed
  `op:<ID>` — which is the delivery discipline *Open decision 3* worries about, already built;
- `Tool::selector_key`'s default inverted at `30b6fc41`, so **all 21 registered tools** supply a
  selector. Before that 17 of 21 returned `None`, so no inference from tool shape could have
  routed anything — option 2 was **unimplementable** when this proposal ranked it second, and
  nothing anywhere said so;
- `annotate_write_path` at `a6b4fc35`, so a write response names the file it wrote and a `path~`
  predicate has something to match.

`OP-4` is the working proof: `create_file(path~/.claude)` routes end-to-end against a real write,
pinned by `tools::core::tests::a_real_edit_file_write_under_dot_claude_delivers_op_4`. A practice
rule declaring `Serves: create_file(path~docs/superpowers/plans/)` is the same shape and needs no
code.

**And the motivating rule is reachable at a BETTER moment than the one this proposal feared.**
*"A plan that names a function must have opened it"* was measured on six subagent task briefs, so
the assumed trigger was a dispatch — harness-only, unreachable. But dispatch is where the *harm*
lands; the *cause* is plan-writing, which is a codescout write. Delivered at plan-write time the
rule arrives **before** the six briefs are drafted rather than as they are handed over. The
unreachable trigger was the wrong one to design for.

**Option 3 (hook) stays rejected — on new grounds, which matters because the old grounds were
weaker than they read.** It was rejected on agent-agnosticism. That argument has eroded: the
companion plugin is always active in this repo, and `OP-2`/`OP-3` are a demonstrated class of
rule only a hook can reach. It is now rejected for a better reason — **it is not needed for the
motivating case**, so building it first solves the harder half of a problem whose easier half is
already done. Re-open it when a rule that matters is provably harness-only, not before.

**Option 1 (explicit) remains a fallback, never the primary**, exactly as argued above: an agent
that knows to ask is not the agent the rule is for.

### What settling this revealed about *Resume* step 2 — "draft three rules"

Only **one of the three** named candidates is expressible in today's grammar. Checked against
`Selector { tool, action, path_contains }`, which has exactly one predicate kind:

| candidate | trigger | expressible today? |
|---|---|---|
| a plan that names a function must have opened it | a write under a plans path | **yes** — `create_file(path~docs/superpowers/plans/)` |
| name what the predicate literally counts before reporting the number | the *measuring* act (`grep -c`, `wc -l`) | **no** — wants a command-string predicate |
| apply the mutation, do not reason about it | an edit followed by a test run | **no** — wants sequence matching |

So step 2 as written is over-optimistic: two of the three cannot be drafted at all without a
grammar extension, and the extension they want is the expensive one. A command-string predicate
would be evaluated on the hot path for **all 21** tools now that the selector default is
universal — the same cost that killed a peer's parallel proposal for `run_command` guide routing
on 2026-09-01.

**Revised step 2: draft ONE rule — the plan-writing one — and measure it.** Strongest evidenced
(6 of 6), the only one needing no new machinery, and one rule is the right unit for the question
*Open decision 3* asks: whether an injected practice rule changes behaviour at all, or is
decoration. Drafting two more before that answer exists is curation without a denominator.

**Still open:** decisions 2, 3 and 4. Decision 2 is narrowed by the above — practice rules as a
second *corpus* inside the operator-rules engine cost no tool slot, which was that decision's
stated concern against
`docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`.
### Resume

1. Settle Open decision 1. It determines whether this is a small change or a subsystem, and it needs no code to decide.
2. Draft **three** rules only, from the strongest existing evidence, and measure whether injecting them changes behaviour before curating more. Candidates: *a plan that names a function must have opened it* (6/6 this session); *name what the predicate literally counts before reporting the number* (CLAUDE.md's Measurement rule — today's `{"ne": null}`, `split('\n')`, `status='archived'` and fence-toggle instances); *apply the mutation, do not reason about it* (validated at 6 datapoints in `W-4`).
3. The eval harness is `prompt-engineering` (prompt-tdd), the same one that scores the reconnaissance trigger string. **An injected rule that does not measurably change behaviour is decoration.**

## CAP-11 — Reconcile memory files against memory points — a doctor check, because only doctor can see both projects

**Status:** proposed

**Valid:** dated 2026-08-26

**Rests on:** the two `ToolContext` definitions read this session
(`src/librarian/tools/mod.rs:84-103` and `src/tools/core/types.rs:58-85`), doctor's
pre-lock call ordering (`doctor.rs:193-195`), and `verify_memory_coverage`'s existing
signature (`src/memory/mod.rs:209-212`).

### The ask

`librarian(action="doctor")` should reconcile, per project, the memory **files** on disk
against the memory **points** in the semantic store, and — the part that matters — pair
the two halves across projects.

Three violations:

| check | means |
|---|---|
| `memory_missing_point` | file on disk, no point — invisible to `recall` in its own repo |
| `memory_orphan_point` | point, no file — `recall` returns content nothing backs |
| `memory_displaced_across_projects` | the same topic is `missing` in project A **and** `orphan` in project B — one memory, filed under the wrong id |

The third is the real defect. The first two are its halves, and each on its own reads as
ordinary untidiness.

### Why doctor, and not `index(action="verify")`

`index(action="verify")` **already computes the per-project half** and is the instrument
that surfaced this. It cannot compute the third row, structurally: it is scoped to one
project, so it reports `orphan` from one side and `missing` from the other and has no way
to know they are the same memory. Finding the 2026-08-26 instance took two `verify` runs
against two repos and a hand-pairing.

Doctor is the only surface with the right scope — its catalog spans every repo on the
machine (a live run this session: 449 violations, 383 of them `abs_path_outside_managed_roots`
from other repos). Same argument CAP-7 makes for putting decay checks there.

### Substrate check (2026-08-26)

**What exists:**

- `verify_memory_coverage(topics, store, project_id)` — `src/memory/mod.rs:209-212`. The
  per-project comparison is **already written and already filters by bucket**. Doctor
  reuses it; only the cross-project pairing is new.
- `scan_declared_project_roots(ctx)` — `doctor.rs:1186`, called at `doctor.rs:193`
  **before** `let cat = ctx.catalog.lock()` at `:195`, with the comment *"reads
  `.codescout/workspace.toml` off disk and touches no connection, so it runs outside the
  lock."* An async `scan_memory_store_drift(ctx).await` drops into that exact slot — no
  `.await` inside the catalog critical section.
- `pub async fn call(ctx: &ToolContext, args: Value)` — `doctor.rs:159`. Already async, so
  awaiting a store needs no signature change.
- Two field precedents in the librarian `ToolContext` itself:
  `artifact_store: Option<Arc<dyn ArtifactVectorStore>>`, documented *"`None` when no
  backend could be constructed (e.g. the configured Qdrant is unreachable)"* — the exact
  degradation this check needs; and `lsp`, documented as *"the same shared instance the
  core MCP `ToolContext` uses — threaded in at construction (`build_tool_context`), never
  a second independent instance,"* citing
  `docs/issues/archive/2026-07-05-audit-doc-refs-lsp-stubbed-off.md` for why reuse rather
  than duplication is load-bearing. That is the wiring pattern, already justified.

**What is genuinely missing:** the librarian `ToolContext` has **no** handle on the memory
semantic store. It holds `artifact_store` — a *different collection*. The `memory` tool
reaches the store through `ctx.agent.semantic_memory_store()` on the **core** `ToolContext`
(`src/tools/core/types.rs:58-85`), which has an `agent` field the librarian context does
not. So the work is: one `memory_store: Option<Arc<dyn SemanticMemoryStore>>` field
mirroring `artifact_store`, threaded at `build_tool_context` the way `lsp` already is.

**The trap, already solved upstream — do not re-derive it.** Only the `structured` bucket
has disk files. `src/memory/mod.rs:169-177` states it plainly: *"The other buckets have NO
disk file by design, which is why coverage must filter rather than compare everything."*
`memory(action="remember")` upserts straight to the store with no markdown, defaulting to
`unstructured`. A naive files-vs-points diff would report **every remembered memory as an
orphan**. `verify_memory_coverage` already filters to `TOPIC_BUCKET`; any new code path
must too.

### Why it matters (measured 2026-08-26)

|  | on disk | in store | |
|---|---|---|---|
| prompt-engineering | 10 | 7 | 3 missing: `language-patterns`, `onboarding`, `prompt-tdd-skill-eval-confounds` |
| codescout | 23 | 25 | 2 orphan: `prompt-tdd-skill-eval-confounds`, `zz-probe-delete-me` |

`prompt-tdd-skill-eval-confounds` appears in **both** rows. One memory, displaced: the file
is prompt-engineering's, the point is keyed to codescout. Traced to the workspace-pin
cross-embed defect fixed in `0cefd1f3` on 2026-07-13 — the point's `created_at` is
~2026-07-04, nine days inside the window. Full account:
`docs/issues/archive/2026-08-26-cross-embed-pin-fix-left-mis-keyed-memory-points-behind.md`.

Its content is a ledger of three ways a prompt-tdd skill A/B silently measures base-model
behaviour instead of the skill — the class of thing `CLAUDE.md` says to read *before*
running an eval. It sat unreachable by `recall` in its own repo for seven weeks, and
nothing reported that.

The generalisation is the point: **`memory(list)` reads a directory and `recall` reads the
store, and nothing reconciles them.** Each answers confidently in its own substrate, so
they never disagree out loud. That is the same failure shape as
`bug-fix-session-log:F-66` (a retired sqlite store answering plausibly all session) and
the reconnaissance skill's substrate rule.

### Open decisions

1. **Which projects to enumerate.** Resolving an arbitrary store `project_id` back to a
   root is exactly the undecided fork in
   `docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md` —
   two surfaces read two different directories and the fix is a decision about which owns
   a sub-project's memories. **Proposed sidestep:** scan the *resident* workspace projects,
   whose roots are known, and report unresolvable store `project_id`s as a separate
   low-severity count rather than blocking on that fork.
2. **Private memories.** `memory(list, include_private=true)` shows codescout has 23 shared
   + 1 private, but `verify` reported `on_disk: 23`. If the walk excludes the gitignored
   private store while the collection holds its points, every private memory reads as an
   orphan. Not observed — both codescout orphans are explained — so this is a question, not
   a finding. Settle it before shipping, or the check's first run is noise.
3. **No `fix=`, at least initially.** A repair is well-defined per half
   (`codescout migrate-memories --in-place` for missing, `memory(action="forget")` for
   orphan) but the *displaced* case has a mandatory order: restore the losing project
   first, because until then the mis-keyed point is the only embedded copy of the content
   anywhere. Naming both commands in the hint — as `index(action="verify")` already does —
   is safer than automating a deletion whose correctness depends on another project's state.

### Resume

1. Settle open decision 2 (private store) — cheapest, and it gates signal quality.
2. Add `memory_store` to the librarian `ToolContext`, threaded at `build_tool_context`
   alongside `lsp`.
3. `async fn scan_memory_store_drift(ctx)` before the catalog lock, reusing
   `verify_memory_coverage` per project and pairing displaced topics across them.
4. Tests: the two this repo's checks always carry — one that fires, one that must **not**
   (a healthy project, and an `unstructured`-bucket memory with no disk file, which is the
   by-design case a naive diff breaks on).
5. Doctor's numbered module docstring, and a `docs/PROBES.md` row naming the blind spot.

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

## CAP-4 — Cross-session collision hint: tell a session when another one just touched this file

**Ask (from the operator, 2026-08-16).** Track recently-modified trackers/files and, when
edits to the same file are recorded under different sessions, hint the current session
about it. Turn "two agents share a working tree" from something each session discovers by
losing work into something the server says out loud.

**The failure is measured, not projected.** Three annexations happened in one day on this
repo, all recorded in `docs/trackers/reconnaissance-patterns.md` R-90:

- a filed bug file swept into `618acd57`, a commit about benchmark ground truth;
- a guide dedup cut swept into `148aabe6`, a commit about comment-marker parsing;
- and the reverse direction — this session swept a concurrent session's file rename into
  `543086d1`, a commit about tracker policy, *while following R-90's own remedy*.

Nothing was lost from disk in any of the three; what was lost was the commit message, and
with it the reason. `git log -- <path>` now attributes a guide deletion to an
`audit_doc_refs` commit. R-90's rule was also wrong twice before it was right, which is the
argument for a structural gate over a discipline: the discipline was authored, executed,
and corrected by the same session inside an hour, and still failed.

**Substrate check — the data already exists, one field is broken.** `tool_calls` already
records `tool_name`, `called_at`, `cc_session_id`, `project_root` and (under `--debug`,
which is the deployed configuration on this host — see CAP-1) `input_json` carrying the
target path. A prototype is a SQL query, not a feature:

```sql
SELECT COALESCE(json_extract(input_json,'$.path'),
                json_extract(input_json,'$.file_path'),
                json_extract(input_json,'$.rel_path')) AS target,
       COUNT(DISTINCT cc_session_id) AS sessions, COUNT(*) AS writes
FROM tool_calls
WHERE called_at >= date('now') AND input_json IS NOT NULL
  AND tool_name IN ('edit_file','edit_code','edit_markdown','create_file','artifact')
GROUP BY target HAVING COUNT(DISTINCT cc_session_id) > 1;
```

Run 2026-08-16 it returned **12 files written by two sessions the same day**, including
`src/tools/core/types.rs` and `src/prompts/mod.rs`, both of which this session edited. So
the signal is real and present.

**Blocker — CLEARED 2026-08-16 (`06498ed2`).** `src/usage/mod.rs` used to resolve the id
from the shared per-project `.codescout/cc_session_id` file and never read
`CLAUDE_CODE_SESSION_ID`, while `src/server.rs` preferred the env var *because the file
collides across concurrent windows*. Two sessions therefore wrote rows under one id — the
exact case this capability exists to detect. The server now resolves `cc_session_id` once
and passes it to `UsageRecorder`, pinned by the regression test
`record_content_uses_the_passed_cc_session_id_not_the_file` (`src/usage/mod.rs`), which
writes a decoy id into the file and asserts the passed one wins. Full gate green at
`64082e8e` (clippy `--all-targets --features dashboard -D warnings`, 3916 tests). Bug file
archived to `docs/issues/archive/`.

**The cutoff is permanent — state it in any analysis that reads this column.** Rows written
before `06498ed2` under a concurrent-session window carry a collapsed id, and the correct
value was *never written*, so it cannot be recovered from the data. The query above
therefore **under-reports across any window reaching back past `06498ed2`** — the 12
measured earlier is a floor, not a count. A base arm for this capability must either start
after that commit or declare the cutoff explicitly; counting across it silently mixes two
attribution regimes and reads as one. (Relocated here from the bug file's `## Resume` item
2 before archiving — nothing re-reads `archive/`, and this is the surface that consumes the
column.)

**Shape.** Three decisions, none settled:

1. **Signal.** (a) `usage.db` rows — names the other session, sees only codescout writes.
   (b) File mtime against a per-session last-seen map — catches *any* external writer
   (the operator's editor, a rebase, a script) but cannot say who. (c) The librarian's
   `field_patch` events — artifact-only, already carries authorship. Lean **(b) as the
   trigger, (a) as the attributor**: mtime is cheap and universal, usage.db turns "someone
   changed this" into "session X changed this 4 minutes ago".
2. **Where it fires.** On write is too late for the sweep case — the damage there happens at
   `git commit`, not at edit time. Candidates: on the *first* touch of a file per session
   (cheap, early), on `run_command` when the command starts with `git commit`/`git add`
   (precise, narrow), or both. The commit case is the one with measured cost.
3. **What it says.** A hint that names the file, the other session, and the age of its edit
   — and, for the commit case, the concrete remedy R-90 landed on:
   `git commit --only <paths>`, which is the form that actually isolates. A hint that only
   says "be careful" reproduces the discipline that already failed.

**Relationship to CAP-1.** Same substrate, opposite direction. CAP-1 is intra-session and
retrospective (*what did I touch, for compaction prep*); CAP-4 is cross-session and
proactive (*who else is here, before I commit*). CAP-1's proposed always-on `touch_target`
extraction is exactly what CAP-4 needs to work off the shipped default rather than only on a
`--debug` host — so CAP-4 is a second consumer that strengthens the case for that field, and
the two should be scoped together.

**Open question worth answering first.** Would the detector have fired *usefully* on the
three real sweeps, or only noisily? Twelve colliding files in one day is a lot of hints if
every one fires. Replay the day's rows against a candidate rule (first-touch only? only
when the other session's edit is unstaged? only at `git commit`?) and count true positives
against total fires before writing any code. The base arm is a query.

## CAP-N — <title>

**Ask.** What the capability is, in the requester's words where possible.

**Substrate check.** What exists today, cited at path:line. What is actually missing. An entry
without this is not ready.

**Open questions.** Numbered, each one a decision someone has to make.

**Why it is worth building.** Evidence, ideally measured, ideally not self-generated.
-->
