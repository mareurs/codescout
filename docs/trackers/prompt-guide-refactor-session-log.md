---
kind: tracker
status: active
title: Prompt + get_guide Refactor — Session Log
owners: []
tags:
  - prompt-surfaces
  - get_guide
  - subagents
  - mcp-channel-caps
---

# Prompt + get_guide Refactor — Session Log

Two-sided observation log for the multi-session effort to refactor the
codescout `server_instructions` surface to fit under Claude Code's ~2 KB
`initialize.instructions` cap (`docs/architecture/mcp-channel-caps.md`),
moving deeper guidance into `get_guide(topic)` with **in-band, runtime
auto-injection** when tools are called without the relevant guide
acknowledged.

This is a session-log tracker, separate from:

- `docs/trackers/codescout-usage-frictions.md` (U-N — pika's project-wide
  tool-usage observations across ALL sessions)
- `docs/trackers/codescout-usage-hookify.md` (H-N — pika's hookify
  candidates)
- `docs/trackers/tool-usage-patterns.md` (T-N — librarian artifact,
  project-wide tool-selection patterns)
- `docs/issues/*` (per-bug files for concrete defects)

This tracker is scoped to **the refactor itself** — does it work? Are
parents complying with Iron Law 6? Does in-band injection deliver
guides correctly? Is the cut to `source.md` causing silent regressions?

## Why this tracker exists

1. **No pre-shipment eval.** Per the brainstorm (hamsa, 2026-05-28), no
   graded set exists for prompt-surface efficacy. Retrospective signal
   from `tools/usage.db`, session logs, and T-N entries IS the eval.
2. **In-band injection is a new mechanic.** Tool response shape changes
   from single-block to multi-block when a first-touch fires. We need
   to observe whether models parse the prefix as "auto-injected guide"
   vs. as part of the answer.
3. **Iron Law 6 is behavioral, not tool-gated.** No hook can prevent a
   parent from underbriefing a subagent. We need observation to confirm
   parents actually comply — and to surface dispatch defects when they
   don't.
4. **Cuts to `source.md` may produce silent regressions.** Removing the
   `@ref` mechanic explainer from the static slice puts it behind a
   `get_guide("buffer-refs")` call. If models don't call get_guide on
   first encounter, they treat `@tool_xyz` as garbage. Needs evidence.

## Design decisions locked (2026-05-28 brainstorm)

<!-- Corrected after F-2 recon (2026-05-28). Storage owner and reset trigger were initially wrong; updated below. -->

| Decision | Value | Source |
|---|---|---|
| Injection variant | **V2 — in-band**, multi-block tool response (guide block + real response block) — OR soft pointer-only `_guide_hint` if compliance is high (see open key Q) | User answer + W-1 finding |
| Eval baseline | `tools/usage.db` + T-N tracker + existing session logs (retrospective). Schema confirmed: `tool_calls(id, tool_name, called_at, latency_ms, outcome, overflowed, error_msg, codescout_sha, project_sha, session_id, input_json, output_json, cc_session_id)` at `src/usage/db.rs:5-73` — `output_json` capture makes `_guide_hint` emission retroactively queryable. | User answer + recon scout |
| New get_guide topics | Expand: `buffer-refs`, `workspace-state`, `iron-laws-detail`, possibly `search-edit-cheat` | User answer |
| Deployment | **Guide content in compiled binary** — no per-project re-onboarding required | User constraint |
| Subagent gating mechanism | **Option Z (no runtime gating on subagents) — confirmed by substrate**: ledger is per-MCP-session, shared via Arc across parent + subagents. Subagents structurally cannot receive `_guide_hint` for topics parent triggered. See W-2. | User answer + F-2 recon |
| Subagent enforcement | **Iron Law 6** — parent passes context. Substrate-validated by F-2/W-2 finding: this is the only channel for parent-known topics to reach subagents. | User answer + W-2 |
| Iron Law 6 wording | "Subagents see only what you brief them with. Pass: which `get_guide(topic)` to call (or the content itself), prior tool results, file paths, symbol names. Applies at every spawn boundary. A subagent re-discovering what you knew is a dispatch defect — yours, not theirs." | Hamsa pick |
| **Per-session ledger storage** | **`CodeScoutServer.guide_hints_emitted: Arc<Mutex<HashSet<String>>>` at `src/server.rs:61`.** One per MCP session. Cloned into every per-request `ToolContext` via `build_context()` at `src/server.rs:220-234`. Reset on `workspace.activate` (`src/tools/config/mod.rs:121`). `/mcp` reconnect resets implicitly via fresh server construction. | F-2 recon (corrected from initial wrong lock) |
| Eviction handling | None in v1. Model re-calls `get_guide(topic)` as the signal. | Hamsa proposal |
| Trigger table location | **Each Tool impl's own `relevant_guide_topic() -> Option<&str>`** — already a trait method on `Tool` at `src/tools/core/types.rs:529`. Today 3 tools declare: `librarian/adapter.rs → "librarian"`, `run_command/mod.rs → "progressive-disclosure"`, `symbols.rs → "progressive-disclosure"`. Extension is per-tool, in-code, no central table. | F-2 recon (corrected from initial wrong lock about a "static map") |

### Existing partial implementation discovered (2026-05-28)

The injection mechanism is **partially implemented today** via the Tool trait. See W-1 for the recon story.

| Surface | Status today | Refactor work |
|---|---|---|
| `Tool::relevant_guide_topic() -> Option<&str>` | Exists | Add to more tools |
| `ctx.guide_hints_emitted` per-session set | Exists | Reuse as-is |
| `_guide_hint` field injection in `call_content` | Exists | Reuse OR upgrade to V2 in-band content block |
| Workspace-reset clears emitted set | Exists | Reuse as-is |
| Tools declaring a topic today | `librarian/adapter.rs`, `run_command/mod.rs`, `symbols.rs` | Extend to: all `artifact*`, `workspace.activate` (when foreign), `@ref`-emitting tools |
| Hint payload | `"_guide_hint": "First call this session for topic 'X'. Run get_guide('X')..."` (string nudge — relies on compliance) | OPEN: keep soft, or upgrade to full guide content as a separate text block (V2) |

**Open key question (replaces prior open-question #1):** is the **soft hint** sufficient, or do we need to upgrade to **hard force-injection of full guide content**? Empirically answerable from `tools/usage.db`:
- count `_guide_hint` emissions per session
- count `get_guide(topic)` calls within N tool calls after a hint
- compliance rate per topic
- compare wrong-tool rate before vs. after hint emission

If compliance is high (~85%+), soft mechanism is sufficient. Refactor becomes: extend `relevant_guide_topic()` coverage + author new topics + cut source.md.

If compliance is low (~<70%), upgrade to V2 (full guide content as separate text block in the multi-block response).

This measurement must precede the V2 decision.
## Open questions

1. **Trigger table seed entries for v1** — start with `artifact* → librarian` only, or ship multiple edges (`workspace.activate → workspace-state`, `@tool_*-producing tools → buffer-refs`, etc.)?
2. **In-band injection header wording** — must explicitly tell the model "you don't need to re-call get_guide for this topic" to prevent wasted confirmations. Exact phrasing TBD.
3. **Authoring order for new get_guide topics** — `buffer-refs` is highest-risk-to-cut (silent failure on `@tool_xyz`). Probably author first.
4. **Static slice target size** — `docs/architecture/mcp-channel-caps.md` says ~2 KB hard cap. Iron Laws 1-6 + nav table ≈ 1.4 KB. Custom Instructions block always loses. Acceptable?

## Append rules — who writes here vs. elsewhere

| Observation | Goes here (this tracker) | Goes elsewhere |
|---|---|---|
| "The new injection didn't fire when it should have" | **F-N here** | — |
| "Parent dispatched subagent without briefing — Iron Law 6 defect" | **F-N here** | — |
| "Model parsed in-band guide as part of the answer" | **F-N here** | — |
| "Cut to `source.md` broke X workflow" | **F-N here** | Also open `docs/issues/<date>-<slug>.md` if it's a discrete bug |
| "New get_guide topic content has a stranger-gap" | **F-N here** | — |
| Model used `Read` on a `.rs` file during this refactor | — | **U-N → `codescout-usage-frictions.md`** (pika's normal channel) |
| Tool-selection pattern observed across many sessions | — | **T-N → `tool-usage-patterns.md`** (librarian artifact) |
| Concrete reproducible bug in `get_guide` tool itself | — | **`docs/issues/<date>-<slug>.md`** + F-N pointer here |
| Counterfactual evidence the refactor reduced wrong-tool calls | **W-N here** | Promote to CLAUDE.md when criteria fire |

### Hamsa's append cadence

- F-N when a prompt-surface edit reads with a stranger-gap that survived self-critique
- F-N when an audit of a new get_guide topic finds a contract not pinned
- W-N when a deletion measurably closed a gap (with counterfactual)
- F-N when an "improvement" was an inspection, not a measurement (unverified flag worth preserving)

### Pika's append cadence

- F-N when a tool-call observation indicates the refactor produced unexpected behavior (e.g. model called `get_guide("librarian")` after auto-injection already delivered it — eviction or parse failure?)
- F-N when a subagent's first tool call is `get_guide(topic)` and the parent should have briefed it (Iron Law 6 defect, observable)
- W-N when a subagent dispatched WITH proper briefing skipped a tool family that previously caused friction (parent compliance evidence)
- Routine U-N entries STILL go to `codescout-usage-frictions.md`, not here. This tracker is refactor-scoped.

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-28 | high | 2kb-cap | pinned-as-eval-baseline | Truncation directly observed in `initialize.instructions` |
| F-2 | 2026-05-28 | med | ledger-state | fixed-verified | Ledger lives on `CodeScoutServer`, not `Agent`; reset is `workspace.activate` |
| F-3 | 2026-05-28 | high | injection-mechanic | pinned-as-eval-baseline | Soft `_guide_hint` compliance is ~1.3% — model nearly never calls `get_guide` after a hint |
| F-4 | 2026-05-28 | med | static-slice-cut | fixed-verified | Brainstorm missed pre-existing `source_md_under_cap` invariant (2200-byte gate); Iron Law 6 cannot ship alone |
| F-5 | 2026-05-28 | med | static-slice-cut | fixed-verified | `extract_surface` uses substring `find()`; any content quoting the marker breaks slice extraction |
| F-6 | 2026-05-28 | med | iron-law-6 | open | Parent should brief subagent on triggered guide topics, not just on file paths |
| F-7 | 2026-05-28 | med | codescout-tool | fixed-verified | `edit_markdown(action='replace')` silently drops `<!-- @surface NAME -->` / `<!-- @end -->` markers from the section body |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-28 | high | Recon-before-build on Tool trait methods | Avoided ~150 LOC duplicate of existing `_guide_hint` mechanism | validated |
| W-2 | 2026-05-28 | high | Recon revealed substrate vindicates Iron Law 6 | Avoided shipping soft-hint extension without compensating discipline; subagents would be structurally blind to parent-triggered topics | validated |
| W-3 | 2026-05-28 | high | Project's `source_md_under_cap` invariant caught the over-cap edit before merge | Iron Law 6 would have shipped malformed (truncated mid-law); silent multi-session subagent-quality regression | validated |
| W-4 | 2026-05-28 | high | Iron Law 6 briefing pattern empirically validated by subagent self-assessment | Without file-and-symbol-anchored brief, subagent would have spent ≥10 exploratory tool calls discovering the architecture | validated |

---

## Category conventions

Extends the project session-log category list with refactor-specific
categories. Existing categories still apply (`codescout-tool`,
`subagent`, etc.); these are additional:

| Category | When to use |
|---|---|
| `injection-mechanic` | Friction with the in-band auto-injection itself (didn't fire, fired wrong, malformed) |
| `iron-law-6` | Subagent dispatch defect — parent underbriefed |
| `static-slice-cut` | Removing a block from `source.md` caused observable misuse |
| `get-guide-topic` | New or existing get_guide topic content has a stranger-gap, contradiction, or missing contract |
| `ledger-state` | Per-session ledger drift — wrong ack state, reconnect issue, subagent inheritance edge case |
| `2kb-cap` | Direct observation of the Claude Code instructions truncation in action |

---

## F-N entry template

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

A win without a **Counterfactual** is marketing — name what would have
happened without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

See `docs/templates/session-log.md` for the canonical vocabulary. This
tracker uses it verbatim.

---

## F-1 — Truncation directly observed in `initialize.instructions`

**Observed:** 2026-05-28, Claude Code session start (Opus 4.7 1M context)

**When:** Investigating the loaded MCP server instructions context at the user's request — comparing the rendered context to the source slice in `src/prompts/source.md`.

**Expected:** Full `server_instructions` output as built by `build_server_instructions` in `src/prompts/mod.rs:27-104` — static slice (~1.7 KB) + dynamic Project Status block (project name, memories list, semantic index status, workspace projects, language warnings, custom system prompt).

**Got:** Static slice intact through "RecoverableError vs anyhow::bail" (end of `## Deeper guidance`). Dynamic Project Status block was cut mid-first-line:

```
## Project Status

- **Project:** code-explorer at `/home/marius/work… [truncated]
```

The `… [truncated]` marker is the literal Claude Code injection at the ~2 KB / ~500 token cap on `initialize.instructions`, documented at `docs/architecture/mcp-channel-caps.md`.

**Probable cause:** Claude Code silently truncates the MCP `initialize.instructions` field at ~2,000 bytes. Server emits ~42 KB end-to-end (verified by direct stdio probe in the channel-caps doc); ~95% never reaches the model. Static slice already nearly fills the cap; ANY dynamic content beyond the first line is structurally lost.

**Workaround:** None today. Per-project memories, custom system prompts, workspace projects table, and Kotlin warnings are all in the cut zone and not reaching the model via this channel. The codescout-companion plugin's SessionStart hook partially compensates by injecting memory names into context via a different channel, but the full per-project system prompt is dead.

**Severity:** high (it's the symptom motivating this whole refactor)

**Status:** pinned-as-eval-baseline

**Fix idea / Pointer:** This entry is the baseline. The refactor's success metric is "F-1's symptom no longer applies to code-explorer's instructions delivery." Specifically: after the refactor, dynamic Project Status, memories list, and (any) custom system prompt must all fit inside the cap, OR be delivered via in-band injection on first relevant tool call. Future W-N entries will reference F-1 as their counterfactual baseline.

---
## W-1 — Recon-before-build caught existing soft-hint mechanism

**Observed:** 2026-05-28, mid-brainstorm with hamsa on the in-band injection design

**Pattern:** Before drafting the runtime injection mechanic (estimated ~150-200 LOC), followed the librarian reindex response thread. The response JSON contained a `_guide_hint` field — a string nudge to call `get_guide("librarian")` because it was the first librarian-family call this session. Grep on `_guide_hint` → `Tool::relevant_guide_topic()` trait method → impl callsites (3 tools today) → `call_content` in `src/tools/core/types.rs:423-517` — revealed that the per-session ledger (`ctx.guide_hints_emitted`), topic dedup, workspace-reset semantics, and response-injection plumbing are **already implemented**.

**Counterfactual:** Without this recon, the brainstorm was about to lock task #3 ("Build runtime in-band injection mechanic, ~150-200 LOC") — which would have been a parallel implementation of an existing mechanism. The new code would have either: (a) conflicted with the existing `guide_hints_emitted` state, (b) shipped alongside it as a confusing second mechanism, or (c) been merged into the existing one only after the duplicate work landed. Discovered via a single librarian reindex call that emitted the hint we didn't know existed.

**Confirming data points:**
1. The librarian reindex response itself: `"_guide_hint": "First call this session for topic 'librarian'. Run get_guide(\"librarian\") for full guidance."` — observed mid-brainstorm.
2. The `symbols` tool call that followed *also* emitted `_guide_hint` for `"progressive-disclosure"` — confirming the mechanism works across tools and topics.
3. `grep` revealed only 3 tools declare `relevant_guide_topic()` — so the mechanism is real but under-deployed. The refactor's job becomes "extend an existing mechanism," not "build a new one."

**Impact:** high — substantially reduces v1 scope (LOC estimate from ~150-200 down to ~50-80, mostly plumbing more tools to declare topics)

**Promote-when:** if recon-before-build prevents another duplicate-mechanism build in a future refactor (≥1 more datapoint), promote to a CLAUDE.md rule under "Design Principles": *"Before designing a runtime mechanism, grep the existing tool trait methods for prior art — particularly the Tool trait's `Option<&str>` returning methods (relevant_guide_topic, etc.) which signal extension points."*

**Status:** validated

---
## F-2 — Plan-vs-reality gap: ledger lives on `CodeScoutServer`, not `Agent`; reset is `workspace.activate`, not `/mcp` reconnect

**Observed:** 2026-05-28, recon pass after the brainstorm's first-pass design lock.

**When:** About to lock task #3 (build mechanism) and task #6 (measure compliance) before scouting the actual lifecycle of the existing `_guide_hint` mechanism.

**Expected (this tracker's "Design decisions locked" table, written mid-brainstorm):** "Per-session ledger storage: In-memory in `Agent` state. Reset on `/mcp` reconnect (matches model's own context loss)."

**Got (scouted reality):**
- Ledger lives on `CodeScoutServer.guide_hints_emitted: Arc<parking_lot::Mutex<HashSet<String>>>` at `src/server.rs:61`.
- Constructed ONCE per MCP session at `src/server.rs:157`.
- `build_context()` at `src/server.rs:220-234` clones the `Arc` into every per-request `ToolContext` — so EVERY tool call in the session shares the SAME underlying mutex.
- Reset is `ActivateProject::call()` at `src/tools/config/mod.rs:121` (`ctx.guide_hints_emitted.lock().clear()`), which fires on `workspace.activate` including the initial activation. `/mcp` reconnect reset is *implicit* (new server instance creates a new HashSet via `Default::default()` at line 157), not an active reset path.
- Subagents that share the parent's MCP server connection share the parent's ledger. There is no per-caller boundary at the substrate level.
- Mechanism is battle-tested: 6 tests at `src/server.rs:2711-2840` cover first-call emission, second-call dedup, cross-tool dedup, workspace reset, conditional firing for `progressive-disclosure` (only on overflow), and emit-once semantics.

**Probable cause:** Brainstorm reasoned about storage from first-principles ("where should the ledger live?") without scouting where it already lived. The user's earlier "force in code, not re-onboarding" comment was a load-bearing hint about deployment shape; the storage hint was buried in `_guide_hint` field name we saw in the librarian reindex response.

**Workaround:** Updated the tracker's "Design decisions locked" table (next edit) to reflect actual storage location, owner, and reset trigger. No code change needed yet — task #3 is now "extend existing tool coverage" not "build new ledger." The architectural reality (shared-per-MCP-session ledger) is exactly the substrate Iron Law 6 was designed to compensate for; see W-2.

**Severity:** med — Would have caused task #3 to build a parallel ledger on `Agent` state. The parallel ledger would either conflict with the existing one (two sources of truth on "is this guide delivered?") or supersede it (breaking the 6 existing tests). Caught pre-code via recon. Compliance measurement task #6 is unaffected — schema scout (see Resume notes) confirmed `output_json` is captured, so retroactive `_guide_hint` detection is feasible against `tools/usage.db`.

**Status:** fixed-verified — design lock corrected mid-brainstorm, no code written under the wrong assumption.

**Fix idea / Pointer:** Tracker design-decisions table updated this turn. Task #3 description amended to "extend existing `Tool::relevant_guide_topic()` coverage, reuse `guide_hints_emitted`, do not build parallel mechanism." Recurrence trigger: if a future refactor proposes new state in `Agent` for cross-tool tracking, re-read `src/server.rs:55-65` first — that's where shared-per-session state already lives.

---
## W-2 — Recon revealed substrate vindicates Iron Law 6 (subagents structurally blind to parent-triggered topics)

**Observed:** 2026-05-28, same recon pass as F-2.

**Pattern:** Scouted the `_guide_hint` mechanism's storage owner before locking the design. Discovered the ledger lives on `CodeScoutServer` (one per MCP session), is cloned into every per-request `ToolContext` via `build_context()`, and is therefore shared between parent and any subagent that uses the same MCP server connection. Subagents inherit the parent's "already-delivered" set — once parent's first artifact call triggers `_guide_hint`, NO subagent's first artifact call will see one. Independently, the user's proposed Iron Law 6 ("Subagents see only what you brief them with...") is the exact compensating discipline that makes this substrate workable.

**Counterfactual:** Three things would have gone wrong without this scout:
1. We would have shipped the soft-hint extension (more tools declare `relevant_guide_topic()`) WITHOUT Iron Law 6, on the assumption that "subagents will receive their own hints when they call new tool families." They won't. The substrate suppresses the hint. Subagents would be uniformly under-informed for any topic the parent had already triggered.
2. We would have built a parallel per-Agent ledger (F-2's friction), creating two sources of truth. The 6 existing tests at `src/server.rs:2711-2840` would have started failing or required duplication.
3. We would have framed Iron Law 6 as a stylistic preference ("good dispatch hygiene") rather than as substrate-mandated discipline. The "dispatch defect — yours, not theirs" line in the law would have read as opinion. With the substrate finding, it reads as architectural reality: there is no other mechanism that delivers parent-known topics to subagents.

**Confirming data points:**
1. `src/server.rs:61` — single `guide_hints_emitted` on `CodeScoutServer`.
2. `src/server.rs:220-234` — `build_context()` Arc-clones the same mutex into every per-request context.
3. `src/server.rs:2749` test `artifact_event_after_artifact_no_hint` proves cross-tool dedup — same mechanism applies cross-caller in production where parent and subagent both go through the same server.
4. F-1 (this tracker) — the 2 KB truncation already removes any "subagent dispatch" guidance from auto-injected instructions, so subagents that DO get fresh server_instructions via SubagentStart hook are the only ones who see Iron Laws — but they still don't get topic hints.

**Impact:** high — recon prevented shipping a half-design that would have left subagents architecturally blind to topics, with no compensating discipline.

**Promote-when:** if Iron Law 6 lands AND post-deployment session logs show that parents are briefing subagents about specific `get_guide(topic)` calls (visible as subagent first-tool-calls being `get_guide(X)` after parent pre-context), promote to CLAUDE.md as: *"Iron Law 6 is not stylistic — the `guide_hints_emitted` ledger is per-MCP-session, shared across all callers including subagents. Once the parent triggers a topic hint, no subagent will receive that hint independently. Briefing subagents about relevant topics is the only compensating channel. See `docs/architecture/mcp-channel-caps.md` for the underlying cap that necessitates the soft-hint mechanism in the first place."*

**Status:** validated — refined 2026-05-28 after live subagent observation (F-6, W-4). The substrate model holds within an MCP session, but the **session boundary is `/mcp` reconnect**, not the wall-clock session. A reconnect creates a fresh `CodeScoutServer` (`src/server.rs:157` instantiates a new `guide_hints_emitted: Arc<Mutex<HashSet<String>>>` via `Default::default()`) and the ledger resets to empty. Subagents are blind to topics the parent triggered **in the same post-reconnect window**, not topics the parent triggered hours ago in a prior reconnect window. In the live verification, my parent had triggered `librarian` post-reconnect (subagent saw no librarian inject ✓) but had NOT re-triggered `progressive-disclosure` post-reconnect (subagent's first overflowing `symbols` call DID receive a progressive-disclosure inject ✓). Both observations match the substrate; the prediction error was about ledger STATE at spawn time, captured separately in F-6 as a briefing-shape friction.

---
## F-3 — Soft `_guide_hint` compliance is ~1.3% — model nearly never calls `get_guide` after a hint

**Observed:** 2026-05-28, task #6 compliance measurement against `.codescout/usage.db` (29,413 tool calls spanning 2026-05-01 to 2026-05-28).

**When:** Empirically measuring whether the existing soft-hint mechanism drives `get_guide(topic)` calls, to decide task #3's path (extend soft mechanism vs. upgrade to V2 hard-injection).

**Expected (working hypothesis from the brainstorm):** unclear — hamsa proposed a threshold (~70% compliance triggers staying with soft, <70% triggers V2). No baseline measurement existed before today.

**Got (queries against `.codescout/usage.db`):**

Hint emissions filtered to the actual payload marker (`'First call this session for topic'`) — the loose `_guide_hint` substring match was contaminated by false positives (docs that *mention* the field, e.g. this very tracker, design plans). False-positive contamination: 286 raw hits → 79 real emissions (72% noise rate when matching `'%_guide_hint%'` instead of `'%First call this session for topic%'`).

**79 real emissions, broken down:**

| tool_name | topic | emissions | sessions |
|---|---|---|---|
| librarian | librarian | 24 | 24 |
| symbols | progressive-disclosure | 20 | 20 |
| run_command | progressive-disclosure | 16 | 16 |
| artifact | librarian | 13 | 13 |
| read_markdown | librarian | 1 | 1 |
| (false positives) | other | 5 | various |

Real topic totals: librarian 38, progressive-disclosure 36. Distributed across 9 days (2026-05-19 through 2026-05-28), peaks on 05-23 (31) and 05-25 (22). Both `_guide_hint` mechanism and `get_guide` tool shipped same day (2026-05-19) — no pre-tool-existence skew.

**`get_guide` tool calls (entire DB):**

| Form | calls | sessions |
|---|---|---|
| `list-topics` (no arg / `{}`) | 3 | 3 |
| `{"topic":"librarian"}` | 1 | 1 |
| any other topic | 0 | 0 |

**Compliance ratio:**
- Hints emitted: 74 (excluding 5 false-positive "other")
- Subsequent `get_guide(topic=<hinted>)` calls: 1
- **Compliance ≈ 1.3%**

**Probable cause:** The `_guide_hint` field is buried inside a possibly-large JSON envelope at a position the model does not visually salience. The hint text is *correct* and *explicit* ("Run get_guide('librarian') for full guidance"), but the model treats the field as inert metadata rather than as an actionable instruction. Hamsa Heuristic 3 applies in reverse: the *real* tool response content dominates the small hint field, and the model copies behavior from the content, not from the metadata.

**Workaround:** None — this is the measurement, not a session blocker. Decision is to ship task #3 as V2 (hard in-band injection) rather than extend the soft mechanism.

**Severity:** high — invalidates the working hypothesis that the existing mechanism is sufficient. Without this measurement, task #3 would have shipped as a soft-only extension and the truncation problem (F-1) would have remained de-facto unsolved (cap'd surface delivers nothing the model acts on).

**Status:** pinned-as-eval-baseline — this measurement is the **baseline** against which task #3's V2 shipment will be evaluated. After V2 ships, re-run the same queries: with V2, the model receives full guide content in-band on first-touch, so the new metric becomes "did the model correctly use the tool family the topic governs in the same session, after receiving the guide?" rather than "did the model call `get_guide` after the hint?"

**Caveat (Hamsa self-critique):** This measurement proves the soft mechanism does not drive `get_guide` calls. It does NOT prove the model is *misusing* the hinted tool families. Two alibis for the low compliance:

1. **"Model already knew."** Model may have internalized library/progressive-disclosure content from prior context and judged the hint redundant. Untestable directly without a wrong-tool baseline per session — but irrelevant for new topics (`buffer-refs`, `workspace-state`, `iron-laws-detail`) that don't exist yet and so the model cannot have internalized.
2. **"Model is silently misusing without surfacing as failed tool calls."** Possible — the misuse manifests as bad output rather than tool errors. Would require qualitative session-log review to falsify.

Either alibi notwithstanding, **V2 is justified for the introduction of new topics in this refactor**: new topics have no prior model knowledge, so the soft mechanism (which empirically doesn't drive calls) cannot deliver their content. V2 guarantees delivery regardless of compliance.

**Fix idea / Pointer:** Task #3 — commit to V2 hard-injection path. Update task description accordingly. Track post-V2 compliance the same way; if compliance increases (more `get_guide` calls in V2-shipped sessions for unhinted topics, or correct use of newly-introduced topic content with no need to call `get_guide`), F-3 graduates from `pinned-as-eval-baseline` to `fixed-verified`. F-1 (truncation) is still load-bearing — V2 lives in tool responses (100 KB cap) not `instructions` (2 KB cap), so V2 is the *delivery mechanism* and F-4 (cut source.md) is still needed to restore the dynamic `Project Status` block to the 2 KB instructions channel.

---
## F-4 — Brainstorm missed pre-existing `source_md_under_cap` invariant test (2200-byte gate)

**Observed:** 2026-05-28, while trying to ship task #5 (Iron Law 6) alone.

**When:** After the `edit_markdown` adding Iron Law 6 landed and `cargo test --lib prompts::tests::prompt_surfaces` succeeded (with snapshot regenerated). Widened test scope to `cargo test --lib prompt` and the unrelated test `prompts::redesign_invariants::source_md_under_cap` failed: `server instructions are 2295 chars; cap is 2200. Cut content or move it to get_guide.`

**Expected (this tracker's earlier brainstorm):** "Iron Law 6 can ship today. Edit `src/prompts/source.md`, ~250 bytes added. No code logic. Bumps no version (server_instructions surface). Once shipped, gates on subagent context-passing start to land." — from the "What's actionable right now" wrap-up I wrote after the recon pass.

**Got:** Iron Law 6 cannot ship alone. The `redesign_invariants::source_md_under_cap` test at `src/prompts/mod.rs:1037-1046` enforces a 2200-byte cap on the rendered `build_server_instructions(None)` output. Slice was 2013 before (under cap); +282 bytes for Iron Law 6 took it to 2295 (95 over cap). The test message literally says "Cut content or move it to get_guide" — a project discipline gate already encodes the design constraint we were brainstorming.

**Probable cause:** Brainstorm pinned the design constraint conceptually ("must fit under Claude Code's ~2 KB cap") but did not scout whether a programmatic invariant already enforced it. Same root pattern as F-2 (substrate facts unscouted before lock). Recon-before-claim pattern (W-1, W-2) repeatedly applies: the codebase has *already* encoded the discipline; before designing how to enforce it, look for what's enforcing it today.

**Workaround:** Three options for the user:
1. **Bundle Iron Law 6 with a surgical cut.** Drop the `Gate: "..."` quote lines in Iron Laws 2/4/5 (~200 bytes). Net delta: +282 - 200 = +82, takes slice to ~2095, under cap. Pairs naturally — the gate quotes tell the model what error text it'll see; arguably load-bearing for first-encounter learning but redundant after gates fire once.
2. **Revert Iron Law 6, bundle with full task #4.** Keep task #5 in `in_progress` but block on task #4. Both ship in one commit when task #4 completes its full slice cut.
3. **Raise the cap to e.g. 2400.** Defeats the purpose of the invariant. Not recommended.

**Severity:** med — caught by an existing gate; no production drift. Surfaces a brainstorm pattern: third "brainstorm-then-recon" miss in this work stream (F-2 storage owner, F-3 compliance baseline, F-4 invariant test).

**Status:** fixed-verified — user picked workaround (1). Iron Law 6 + surgical cut of gate-quote lines in Iron Laws 2/4/5 landed in the same edit. `cargo test --lib prompt` reports all 80 tests passing including `source_md_under_cap`. Final slice size: see `wc -c tests/fixtures/prompts/server_instructions.md`. The brainstorm-pattern lesson (recon-before-claim) stands; promote it to W-3's CLAUDE.md candidate text.

**Fix idea / Pointer:** Pick workaround (1) or (2). After this turn, the brainstorm should adopt a discipline: **before claiming "X ships independently," grep for `cargo test` invariants on the surface X touches** (`grep -r "assert.*MAX_" src/prompts/` would have caught this; `symbols(path=src/prompts/, query="invariants")` would have surfaced the `redesign_invariants` module).

---
## W-3 — Project's own `source_md_under_cap` invariant caught the over-cap edit before merge

**Observed:** 2026-05-28, same turn as F-4.

**Pattern:** The project ships a `cargo test`-enforced invariant (`prompts::redesign_invariants::source_md_under_cap` at `src/prompts/mod.rs:1037-1046`) that asserts the rendered `build_server_instructions(None)` is ≤ `MAX_INSTRUCTIONS_CHARS` (2200). The Iron Law 6 edit tripped the gate immediately on `cargo test --lib prompt`.

**Counterfactual:** Without the invariant test, the slice would have grown to 2295 bytes and shipped. The 2 KB Claude Code truncation (F-1) would have started cutting INSIDE the static slice — specifically, mid-Iron-Law-6 (since the cap hits at ~2000 from start). Iron Law 6 would have been malformed in production for every session. The defect would have surfaced only as quiet subagent-dispatch quality degradation across many sessions — exactly the kind of slow, multi-session-spanning regression that's hardest to attribute.

**Confirming data points:**
1. Test ran on first `cargo test --lib prompt` after the edit; failed loudly with the byte count and a remediation hint ("Cut content or move it to get_guide").
2. The test's existence + error wording matches the project's CLAUDE.md discipline ("Prompt Surface Consistency" section, "Style guide for prompt surface edits"). Someone — Marius or an earlier session — anticipated this exact failure mode and built the gate.
3. F-4 (same turn) is the brainstorm miss; W-3 is the project discipline win that compensated.

**Impact:** high — gated a quietly-broken Iron Law shipment, exactly the kind of silent-degradation regression that wouldn't surface via any single failed test in production.

**Promote-when:** if a future prompt-surface refactor catches another silent regression via the same invariant — i.e., the gate fires AGAIN on a second occasion — promote to a CLAUDE.md "Verification Patterns" entry: *"The `source_md_under_cap` invariant in `src/prompts/mod.rs` is load-bearing — it catches what no manual review sees. Run `cargo test --lib prompt` before considering any prompt-surface edit ready to commit."*

**Status:** validated

---
## F-5 — `extract_surface` uses substring `find()`; any content quoting the marker breaks slice extraction

**Observed:** 2026-05-28, after the F-4 fix tried to add an editor-facing comment to `src/prompts/source.md` warning about the 2200-byte cap.

**When:** Adding an HTML comment to `source.md` (via `edit_file` insert=prepend) that included the literal strings `<!-- @surface server_instructions -->` and `<!-- @end -->` in its prose (referencing them descriptively).

**Expected:** The HTML comment, placed BEFORE the actual `<!-- @surface server_instructions -->` marker line, would be ignored by the slice extractor since the extractor was assumed to skip to the *actual* marker line.

**Got:** `extract_surface` in `src/prompts/source.rs:36-50` does:
```rust
let open = format!("<!-- @surface {surface} -->");
let marker_end = source.find(&open)? + open.len();
```
`source.find(&open)` returns the **first** occurrence of the substring anywhere in the file — including the literal text of my reference comment. The extractor then took bytes from the middle of my HTML comment, hit the comment's closing `-->` (treated as `<!-- @end -->` partial match), and produced a 7-byte garbage extraction.

Symptoms:
- `prompts::tests::prompt_surfaces_server_instructions_snapshot` failed: "expected 2059 bytes, actual 7 bytes"
- `prompts::redesign_invariants::server_instructions_mentions_get_guide` failed: extracted content didn't contain "get_guide"
- `prompts::source::tests::extracts_server_instructions_byte_for_byte` PASSED — the test fixture in `src/prompts/source.rs::tests` doesn't include a literal marker before the actual one, so the test never exercised this failure mode.

**Probable cause:** Naive substring search for a syntactic marker. The extractor assumes the marker only appears in its intended position; doesn't guard against accidental quotation in comments, prose, or even other surfaces' bodies.

**Workaround (this session):** Moved the editor-facing comment to `src/prompts/README.md` as a new Rule 8 in the "Rules for editing the `server_instructions` surface" section. Rule 8 explicitly warns future editors NOT to put a literal marker-string reference into `source.md` itself, citing this F-5.

**Severity:** med — caught by snapshot test (W-3 win again). Production-shippable workaround is in place. But the root cause (brittle extractor) remains. Future editor who tries the same well-intentioned thing will hit the same wall.

**Status:** fixed-verified — `extract_surface` rewritten with line-anchored matching (commit pending this turn). The new implementation walks lines with `split_inclusive('\n')`, tracking byte offsets, and matches the marker only when a trimmed line exactly equals `<!-- @surface NAME -->` or `<!-- @end -->`. Prose that quotes the marker shape inline no longer matches. Mirrors the editor-side line-anchoring discipline added by F-7. Tests added: `extract_ignores_marker_quoted_in_prose`, `extract_ignores_close_marker_quoted_in_prose`, `extract_tolerates_trailing_whitespace_on_marker`, `extract_requires_marker_on_its_own_line`. All 6 pre-existing tests still pass byte-for-byte (the source.md fixtures round-trip cleanly). Mitigation (README.md Rule 8 "don't quote the markers in source.md") is now belt-and-suspenders rather than the only line of defense — the parser self-defends.

**Fix idea / Pointer:** Tighten `extract_surface` so the marker must appear at line start (anchored regex `(?m)^<!-- @surface {surface} -->$`), OR add an extractor test that includes a marker-mention prefix to lock the desired behavior. Worth filing as a `docs/issues/2026-05-28-extract-surface-substring-match.md` bug file if the root cause is to be addressed in this work stream; otherwise the F-5 mitigation (README.md Rule 8) is sufficient guidance.

---
## F-6 — Parent should brief subagent on triggered guide topics, not just on file paths

**Observed:** 2026-05-28, post-`/mcp` reconnect, after launching a verification subagent to assess V2 hard-injection behavior.

**When:** Spawned a general-purpose subagent with Iron-Law-6-compliant briefing (file paths, symbol names, F-2/W-2 finding pointer). Asked it to explore the prompt surface architecture AND report whether it saw any V2 auto-inject markers in its tool responses.

**Expected (briefing's claim):** Parent had already triggered `librarian` and "very likely" `progressive-disclosure` post-reconnect; subagent would see neither, confirming W-2.

**Got:** Subagent's first `symbols()` call DID fire V2 hard-injection for `progressive-disclosure` — full guide body in second Content block, markers intact. The subagent reported the inject was a surprise relative to my briefing prediction. Substrate model still holds — explanation in W-2 amendment below — but the subagent's actionable critique was: *"I have no idea which tool calls the parent already made this turn, so I can't predict which V2 injects will fire on me. If the parent had said 'I've triggered these topics: [librarian]', I'd have predicted my own injects accurately."*

**Probable cause:** Iron Law 6 (commit 79d5e496) names what to pass — guide topics, tool results, file paths, symbol names — but doesn't explicitly call out **session-state context** like the triggered-topic ledger. The parent has access to its own action history; the subagent has none of it.

**Workaround / fix idea:** Extend Iron Law 6's noun list to include "topics you've already triggered this MCP session" as a recommended brief item. Lightweight — costs ~10 bytes in the spawn prompt typically. Optional substrate support: `workspace(status, include=["guide_hints"])` could surface the ledger, but that's a v2 concern; the immediate fix is behavioral.

**Severity:** med — does not block the V2 mechanism (subagent still got the guide content auto-injected; the mistake was prediction, not delivery). But the gap is a real signal-quality friction for parents diagnosing subagent behavior, AND adds noise to W-2 in this tracker (made my "subagents are blind to topics parent triggered" framing read as falsifiable rather than substrate-derived).

**Status:** open — fix in this turn is documentation (W-2 amendment + this F-6 entry + extending the source.md `## Deeper guidance` topic list to include `workspace-state` + `iron-laws-detail`, which the subagent ALSO caught as a separate discoverability gap). Iron Law 6 wording amendment deferred — needs a brainstorm pass and re-running the source_md_under_cap arithmetic.

**Fix idea / Pointer:** Concrete brief shape next time: `"I've triggered these topics this session: [librarian, progressive-disclosure]. You will not see V2 inject for them."` — 2 lines, 100 bytes. Cite W-2 + F-6 in the spawn prompt for any future subagent.

---

## W-4 — Iron Law 6 briefing pattern empirically validated by subagent self-assessment

**Observed:** 2026-05-28, same subagent dispatch as F-6.

**Pattern:** The Iron Law 6 wording locked in commit 79d5e496 — "Pass: which `get_guide(topic)` to call (or the content itself), prior tool results, file paths, symbol names" — was applied to the dispatch prompt by enumerating specific files (`src/prompts/source.md`, `src/prompts/source.rs::extract_surface`, `src/prompts/mod.rs::build_server_instructions`, `src/prompts/mod.rs::redesign_invariants`, `src/prompts/mod.rs::GUIDE_TOPICS`, `src/tools/core/types.rs::Tool/call_content`, two new guide markdowns, `src/prompts/README.md`) and pointing at F-2/W-2 in this tracker for the substrate finding.

**Counterfactual:** Without the file-and-symbol-anchored briefing, the subagent would have spent time discovering the architecture via grep + symbols search (codescout's tool composition is non-trivial — `build.rs` slicing, runtime `extract_surface` parser, `GUIDE_TOPICS`/`topic_body` pair as single source of truth feeding both `GetGuide` and `Tool::call_content`). The subagent's report (Section C) explicitly says: *"the file:symbol enumeration... and the F-2/W-2 pointer made navigation O(1). Self-discovery cost: roughly zero."* Without the brief, expect ≥10 exploratory tool calls before the subagent could write the same Section A architecture findings.

**Confirming data points:**
1. Subagent Section A — every file:line citation in the architecture findings matches a path/symbol named in the spawn prompt. No discovery work needed for the architecture.
2. Subagent Section C — "missing" list = exactly one item (the `## Deeper guidance` topic-list staleness), which is a real bug the briefing couldn't have known. "Excessive" list = empty.
3. Subagent Section D — chose NOT to call `get_guide()` because file paths were named directly; "`get_guide` becomes redundant unless I want the canonical 'in-prompt' rendering." Briefing made the get_guide pathway optional rather than required.

**Impact:** high — pattern is reproducible; can be promoted to a CLAUDE.md "Dispatch Patterns" section after one more confirming subagent dispatch. The single-shot validation above is sufficient to keep the Iron Law 6 wording as-shipped without amendment for that specific concrete-noun discipline.

**Promote-when:** A second subagent dispatch with the same brief shape produces an equivalent "self-discovery cost ≈ zero" report. At 2 datapoints, promote to CLAUDE.md as: *"When dispatching subagents, the Iron Law 6 nouns are not abstract. Enumerate concrete file paths, symbol names, tracker IDs (F-N/W-N), and finding pointers. The subagent's report should be able to cite back the same paths verbatim — if it can't, the brief was insufficient."*

**Status:** validated

---
## F-7 — `edit_markdown(action='replace')` silently drops `<!-- @surface NAME -->` / `<!-- @end -->` markers from the section body

**Observed:** 2026-05-28, twice in succession while editing `src/prompts/source.md` this turn.

**When:** Used `edit_markdown(action="replace", heading="## Deeper guidance", content="<new topic list>")` to refresh the get_guide topic list. The first attempt's new content omitted the `<!-- @end -->` and following `<!-- @surface onboarding_prompt -->` markers that lived at the tail of the section body. Build panicked: `surface 'onboarding_prompt' not found`. After restoring those markers in the new content, the same shape struck again — the intro paragraph of the `onboarding_prompt` surface (which structurally lives between the `<!-- @surface onboarding_prompt -->` line and the next heading `## THE IRON LAW`) was inside the same replace target body. UPDATE_PROMPT_SNAPSHOTS=1 then regenerated the onboarding_prompt fixture to match the broken-source-md state, masking the bug until `git diff` caught the unstaged snapshot diff.

**Expected:** Replace would only modify the human-readable content I provided, preserving structural markers.

**Got:** Replace overwrites the ENTIRE section body (from line-after-heading to next-sibling-heading) verbatim with the new content. Any structural HTML comment markers in the OLD body that the NEW content doesn't include are dropped silently — no warning, no test failure.

**Probable cause:** `perform_section_edit_ext`'s "replace" arm (`src/tools/markdown/edit_markdown.rs`) computes the section's body range from heading line to next-sibling-heading and substitutes the new content wholesale. Surface markers are body content by markdown's parse model — they're not headings, they're HTML comments — so the section-shape semantics don't know they're load-bearing. Sibling of F-5 (`extract_surface` substring-finds; can match the FIRST occurrence anywhere). Both stem from `source.md` mixing structural markers with prose body content where line-naive parsers/editors can't see them.

**Workaround (used this turn):** Manually restored the lost markers + intro paragraph via `edit_markdown(action="edit", old_string=..., new_string=...)`. Re-ran UPDATE_PROMPT_SNAPSHOTS to refresh the now-correct fixture. Amended the previous commit to bundle the fix.

**Severity:** med — silent data loss caught only by build.rs panic (and only because the broken state failed extract_surface). If the markers had been less load-bearing (e.g. an HTML comment styling hint), the loss would have shipped unnoticed.

**Status:** fixed-verified — `perform_section_edit_ext` now gates `action="replace"` on a surface-marker-preservation check. New helper `find_lost_surface_markers(old_body, new_content)` returns markers present in OLD but not NEW; if non-empty AND `force=false`, the replace returns `RecoverableError` naming the lost markers and pointing at this F-7 entry. `force=true` (already used for the body-shrink guard) overrides the gate for intentional structural change. Tests added: `replace_refuses_when_surface_markers_would_be_dropped`, `replace_with_force_drops_markers_silently`, `replace_preserves_markers_when_new_content_includes_them`, `extract_surface_markers_ignores_marker_shaped_text_in_prose` — the last is the false-positive guard for prose that quotes the marker shape (sibling of F-5's parser-side line-anchoring rule).

**Fix idea / Pointer:** Shipped this turn (next commit on `experiments`). Future deeper fix could move surface markers to dedicated heading-separated blocks in `source.md` so they never live inside section bodies in the first place — but the gate is sufficient for now. The librarian's `artifact(update)` write path (`src/librarian/tools/update.rs:117`) ALSO benefits from the gate — call_site updated to pass `false` (defensive in depth; tracker bodies don't typically contain markers but a future tracker documenting THIS feature might quote them).

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
