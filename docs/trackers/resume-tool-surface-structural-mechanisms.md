---
id: '25633146506bd8b3'
kind: tracker
status: active
title: Tool Surface — Structural Mechanisms (SM-N)
owners:
- marius
tags:
- prompt-surfaces
- resume-queue
- tools-list
- mcp
topic: prompt-surfaces
entry_high_water_SM: 4
entry_prefix: SM
---

# Tool Surface — Structural Mechanisms (SM-N)

> **Currency: measured 2026-09-03.** Every byte figure below was derived that day against
> `experiments` @ `88311708`. **Re-derive before relying on any of them** —
> `python3 scripts/probe_tool_surface.py` and
> `cargo test --lib tool_surface_report_lengths -- --nocapture`. Three recorded values for
> `librarian.fix` (1,133 / 987 / 980) existed simultaneously in this repo on 2026-09-02.

## Why this queue exists

A 2026-09-03 session set out to compact the `tools/list` surface and produced **four
proposals, three of which died to a fact in a neighbouring subsystem**. The survivor is
worth ~830 chars (1.5%). External research then showed the whole framing was wrong: every
mechanism with a published number is **structural**, and hand-trimming prose is two orders
of magnitude below all of them.

This queue holds the structural mechanisms, in the order the evidence supports.

## Established, do not re-derive

| fact | value |
|---|---|
| `tools/list` surface | **55,519 chars** (schema 48,522 / desc 6,997), 21 tools |
| Authorship split | prose **36,715 (66.1%)** / machine 18,804 (33.9%) |
| Parameter descriptions | 29,718 chars = **53.5% of the whole surface** |
| `doc` + `librarian` | **50.2%** of the surface (17,740 + 10,204) |
| Cache behaviour | ~100% cache_read ⇒ ~$0.0043/request. **Cost is not the case; context/attention is.** |
| Guide emission, warmed | ~11,721 B vs `CEILING = 12,244` — ~523 B headroom |
| Guide emission, **unwarmed** | **15,115 B measured** = 2,871 B OVER. First real measurement of a condition `docs/PROBES.md` and `src/server.rs:9222` both label *"a conservation model — not a measurement taken here"*. Model predicted 15,351 (1.5% apart). |

**Three refuted directions — do not re-propose without new evidence:**

1. **`oneOf` / per-action schema narrowing** — arithmetically bounded regression. 17 minimal
   branches cost 794 chars against the 616 chars of `action:` prefixes they remove; mean
   action fan-out is **1.29**, so the flat union is already nearly a partition.
   Full derivation: `prompt-surface-compaction-session-log:W-16`.
2. **Relocating schema prose into `get_guide` topics** — refuted twice. Break-even at
   K ≈ 12.5 turns and `librarian` auto-injects on the first `doc` call
   (`docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` § *Rejected*). And
   there is nowhere to land it: the guide corpus is already over its ceiling.
3. **Adding `serves:` markers to unserved sections** — `## Body Editing Surfaces` is
   already in `SECTION_WAIVERS` (`src/prompts/mod.rs:534-546`) for exactly this reason;
   marking it puts the p50 session ~1,671 B over `CEILING`, and the test's own failure
   message says *"Raising CEILING is a spec amendment, not a fix."*

## The constraint that shapes everything

`docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md`
(**open**): a `serves:` section is injected into the **response** of the first matching
call, so guidance that would prevent a destructive first call arrives attached to the
result of having made it. The discriminator that follows from it:

> *Can a first call, made without this text, destroy something or silently produce a wrong
> result? If yes it stays inline; if it merely produces an error or a dry run, it can move.*

~2,155 B is pinned by this today (`doc.patch` 1,068, `edit_markdown.action` 578,
`doc.new_rel_path` 509). **SM-2 is the mechanism that unpins it.**

## External research — 2026-09-03, with source quality flagged

**Mechanisms with published numbers.** Anthropic Tool Search (`defer_loading: true`):
~77K → ~8.7K tokens (~88%), *accuracy rising* — Opus 4.5 79.5% → 88.1%, Opus 4 49% → 74%
(anthropic.com/engineering/advanced-tool-use). Programmatic Tool Calling: 37%. Code
execution with MCP: 150K → 2K (98.7%) — **not comparable to us**, it bundles deferred
definitions with intermediate-result suppression. Counter-evidence: an independent test at
4,000 tools measured tool-search retrieval accuracy at **56–64%**, failing to surface
obvious tools (growthmethod.com) — the failure mode becomes a silent wrong answer.

**Scale check.** Every catalog mechanism (tool search, Tool RAG, SEP-1821 dynamic
filtering, Cursor's 40-tool cap) targets 100–4,000 tools. **At 21 we are below all of
them.** The one that fits our scale is VS Code Copilot's split: send name + description
only, inject the full `inputSchema` **on demand** after the model picks a tool
(code.visualstudio.com, 2026-06-17).

**Authoring guidance.** No vendor publishes a length limit for descriptions — Anthropic,
OpenAI and the MCP spec are all silent. The only description-side lever with a measured
number is **`input_examples`** (1–5 per tool): parameter-handling accuracy **72% → 90%**.
It says *add examples*, not *remove prose*. Anthropic separately recommends **consolidating
multi-step operations into single tools**, which cuts *toward* our mega-tools.

**The mega-tool question is unresolved and under-measured.** No benchmark isolates tool
*shape* — BFCL, tau-bench, ToolBench and MCP-Universe all vary tool *count*. Reconciled
reading: consolidation trades tool-**selection** error for parameter-**construction**
error, and `doc` (17 actions × 60 params) sits on the wrong side of that trade — but that
is a hypothesis, not an importable finding. **Do not cite** the widely-repeated "16%
accuracy drop per 1,000 tokens" or "near-zero at 740 tools" — both come from SEO-style
blogs with no published methodology.

**Research hygiene note.** One fetched report claimed MCP has no standardised
`instructions` field, citing a GitHub discussion *requesting* one — it mistook a feature
request for an absence, and was falsified against the live system prompt. Treat that
report's other negative claims with suspicion.

## SM-1 — Emit MCP tool annotations so the client can gate destructive calls without prose

**Status:** done 2026-09-03 — `71c827f9` on `experiments`, patch-id
`516fb4cd13a37ef504f2c16ecd935ed33e357341`.

**Shipped:** `Tool::annotations()` with an emit-nothing default; overrides on 17 of 21
tools; `advertised_mcp_tools()` extracted so `list_tools` and the budget gate share one
builder; `advertised_surface` now counts `annotation_chars`; gates
`annotations_agree_with_is_write` and `annotations_reach_the_advertised_payload`. Verified
on the live wire: **17 annotated, 4 bare**. Cost **757 chars**.

**`memory`, `doc`, `librarian` and `run_command` emit nothing** — the MCP defaults
(destructive, non-idempotent, open-world) already describe them exactly. The
counter-intuitive result: the *read-only* tools are the expensive ones, because
`openWorldHint` defaults to **true**, so a local-only reader must spend bytes asserting
`false`, while the four most destructive tools are free.

**The funding plan failed, and that is this entry's most reusable finding.** SM-1 was to be
paid for by trimming `librarian.fix` — which § *Established* above called pre-cleared. The
trim was written, measured at net −17, and **reverted**:
`doctor_results_route_away_from_librarian_so_fix_modes_stay_in_the_schema`
(`src/librarian/adapter.rs`) reds on it. **A `serves:` marker proves a section is DECLARED,
not that a call REACHES it** — `relevant_guide_topic` picks the topic from the RESULT, and
a real doctor result names tracker paths (128 of 138, measured 2026-08-31), so
`### doctor repairs` is never consulted on a doctor call. The identical move shipped once
(`d94dd53d`) and was reverted (`c7d66f94`). Budget raised 55_519 → 56_276 instead, with the
account in the constant's log.

**So the discriminator in § *The constraint that shapes everything* is necessary and NOT
sufficient.** *"Is the first call harmless?"* clears prose to move; *"does the call reach
the section?"* decides whether it lands anywhere at all. **SM-2 and SM-3 must check both** —
and the second is not visible from the marker, only from `relevant_guide_topic`.

**Filed en route:** `docs/issues/2026-09-03-workspace-activate-writes-libraries-json-outside-the-write-lock.md`.
Classifying every tool forced a per-tool *"does this modify its environment?"* and found
`is_write` answering `false` where the disk says otherwise.

MCP defines per-tool `annotations`: `title`, `readOnlyHint`, `destructiveHint`,
`idempotentHint`, `openWorldHint`. Codescout emits **none** — verified 2026-09-03,
`grep` over `src/**/*.rs` for all four returns 0 matches.

They are a few bytes of booleans and let the **client** shape confirmation UX and
auto-approval policy without a byte of protective prose in any description. Unannotated
tools may be treated as destructive by default, so emitting them also *reduces* friction on
the read-only majority.

**Known limit, from the sources:** they are **untrusted hints, not security guarantees** —
a malicious server can label a destructive tool `readOnly`
(blog.modelcontextprotocol.io, sunpeak.ai). They shape UX; they do not enforce.

The `Tool` trait already carries `is_write(&self, input) -> bool`
(`src/tools/core/types.rs:833`), which is **per-call** — annotations are **per-tool** and
static, so this is a related but distinct axis. Do not assume one derives from the other.

**Valid:** conditional — MCP keeps `annotations` in the tool schema and clients keep acting on it

**Rests on:** the MCP specification's tool-annotations definition; the verified absence of
any `destructiveHint`/`readOnlyHint`/`idempotentHint` in `src/`.

## SM-2 — Wire `write_ack` to destructive `doc` actions, retiring the pinned-prose floor

**Status:** open

`src/tools/core/write_ack.rs` **exists** (verified 2026-09-03) and already powers the
`@ack_*` gate for dangerous shell commands and out-of-scope writes. The research names the
two-phase confirm — *server returns a refusal whose payload carries the guidance, model
retries with acknowledgement* — as the practitioner answer to exactly our ordering flaw.
**We invented this pattern independently for shell and never wired it to `doc`.**

Guidance attached to a **refusal** arrives before any effect; guidance attached to a
**result** arrives after. That inversion is the whole of it.

Payoff: the ~2,155 B pinned by the open delivery-ordering bug becomes movable, and the bug
is retired rather than worked around.

**Open question before building:** does the model reliably retry after a
confirmation-required refusal? The research could not find a production reference
implementation — only blog descriptions of the pattern. Our own `@ack_*` gate is the
in-house evidence; measure its retry rate from `usage.db` before committing to the design.

**Valid:** conditional — `docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md` stays open

**Rests on:** that bug file's § *Workarounds* discriminator, and the existence of
`src/tools/core/write_ack.rs`.

## SM-3 — Move de-duplicable protective prose into `server_instructions`

**Status:** open

`instructions` in `InitializeResult` is a **pre-call** channel: it lands in the system
prompt before any tool call, and codescout already ships it. Crucially it is **shared
across tools**, so protective rules that currently repeat per-parameter can be stated once.

**Constraints, both real:** the slice cap is **1,900 characters**
(`src/prompts/README.md`), and it is not a free win on context — `server_instructions` is
resident for the session just as the schema is. The gain is **de-duplication and
ordering**, not raw bytes. Do not scope this as a byte saving.

Client support is uneven (there is an open request for Claude Desktop to consume it), so
this is a Claude-Code-and-some-hosts channel, not universal.

**Valid:** conditional — the 1,900-char `server_instructions` slice cap stands

**Rests on:** `src/prompts/README.md` § *Surfaces*; the MCP `InitializeResult.instructions`
definition.

## SM-4 — Run the A/B nobody has run: enum-dispatched `doc` vs split per-action tools

**Status:** open — the highest-value item, and the only one that produces new knowledge

No published benchmark varies tool **shape** holding operations constant. We are unusually
well positioned: the `prompt-engineering` repo is an eval harness that puts codescout's own
prompts under TDD against headless `claude -p`.

Score **two** error classes separately, because the literature says consolidation trades
one for the other:
- **action-selection** error (did it pick the right operation?)
- **parameter-construction** error (did it build valid args?)

A design that improves the first and worsens the second is the predicted outcome, and a
single blended accuracy number would hide it.

This is also the eval `prompt-surface-compaction-session-log:W-2`'s `Promote-when` has been
waiting for — *"a third compaction decision scoped by this arithmetic rather than by byte
count"*.

**Before running:** read `prompt-engineering:docs/trackers/prompt-tdd-operating-guide.md`.
It is a ledger of ways that harness does what you asked in a way that reads as something
else, and the failure mode is a **number**, not an error.

**Valid:** invariant — the absence of a shape-isolating benchmark is a fact about the
published literature as of 2026-09-03; a new benchmark would supersede this entry rather
than falsify it

**Rests on:** the 2026-09-03 research pass recorded above; `W-2`'s cost arithmetic.

## Template for new entries

<!-- Appends land above this line. Use:
     doc(action="append_entry", id="<this artifact id>", id_prefix="SM",
         title="...", body="...", anchor_heading="## Template for new entries")
     The server writes a def_re-conformant `## SM-N — <title>` heading. -->
