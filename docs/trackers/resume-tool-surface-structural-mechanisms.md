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


## The unit: chars are not tokens, and this repo's own ratio is 2× off for THIS surface

Every figure in this queue is in **characters**, because that is what
`scripts/probe_tool_surface.py` and `tool_surface_report_lengths` measure. The thing the
surface actually spends is **context tokens**, and the conversion is not the one this repo
documents.

**Measured 2026-09-03** from Claude Code's `/context`, on a turn where the Gmail / Google /
MindMap / Uber Eats servers had dropped out, isolating codescout's own surface:

| instrument | unit | total | `doc` | `librarian` | those two |
|---|---|---:|---:|---:|---:|
| `probe_tool_surface.py` | chars | 56,476 | 17,940 | 10,204 | **49.8%** |
| Claude Code `/context` | tokens | ~28,700 | 9.0k | 5.1k | **49.5%** |

**Two conclusions, and the second is the one to carry.**

**1. The composition is corroborated.** Two instruments sharing no code, in different units,
agree on the librarian family's share within **0.3 pp**. Published as a denominator rather
than banked as a catch: the char-side figure had been quoted all session and nothing had
independently checked it.

**2. The ratio is ~2.0 chars/token, not 4.** Adding the tool `name` and JSON envelope the
probe excludes (~21 × 50 ≈ 1,050 chars) gives ~57,500 chars over ~28,700 tokens — **2.00**.
This repo's house estimate is `bytes / 4`: `MAX_INLINE_TOKENS = 2_500 // ~10KB at ~4
bytes/token` and `TOOL_OUTPUT_BUFFER_THRESHOLD = MAX_INLINE_TOKENS * 4` (`src/tools/mod.rs`,
restated in `get_guide("progressive-disclosure")`). **So anyone converting a figure in this
queue with the repo's own documented ratio halves it.**

**The heuristic is not wrong where it lives, which is exactly why this is easy to get
wrong.** It governs *tool output* buffering, and prose, code and markdown really do run near
4 bytes/token. Dense JSON schema — short quoted keys, punctuation, `snake_case` identifiers
— runs near 2. The ratio is a property of the **content**, and the constant is named for a
budget rather than for a corpus, so nothing at the point of use says which corpus it was
calibrated on.

**What this changes: the stakes, not the options.** The surface costs ~28.7k tokens per
request-set, roughly double what a reader of the house ratio would assume — 2.9% of a 1M
context, ~14% of a 200k one. Every compaction route is still refuted (§ SM-1–SM-3), so this
raises the value of SM-4 rather than reopening anything.

**Limits, stated because the instrument is a display.** `/context` rounds to 0.1k above 1k,
so the eight rounded tools carry up to ±400 chars of aggregate error; the per-tool hand-sum
comes to 28,470 against a reported 28.7k, consistent within that. The conclusion separates
2.0 from 4.0 and is nowhere near the rounding band. It is also **one client's tokenizer** —
treat 2.0 as this-client-specific until a second one is measured.

**Valid:** dated 2026-09-03

**Rests on:** Claude Code `/context` output, 2026-09-03; `scripts/probe_tool_surface.py` at
`dcd4b1d0`; `src/tools/mod.rs` `MAX_INLINE_TOKENS` / `TOOL_OUTPUT_BUFFER_THRESHOLD`.
## The constraint that shapes everything

`docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md`
(**open**): a `serves:` section is injected into the **response** of the first matching
call, so guidance that would prevent a destructive first call arrives attached to the
result of having made it. The discriminator that follows from it:

> *Can a first call, made without this text, destroy something or silently produce a wrong
> result? If yes it stays inline; if it merely produces an error or a dry run, it can move.*

~2,237 B is pinned by this today — `doc.patch` 1,160, `doc.new_rel_path` 543,
`edit_file.action` 534 — re-derived 2026-09-03 at `0647a6da` from
`python3 scripts/probe_tool_surface.py` § *TOP 20 SINGLE PARAMETERS*, not remembered.

> **Corrected 2026-09-03, on both halves.** This paragraph read *"~2,155 B … `doc.patch`
> 1,068, `edit_markdown.action` 578, `doc.new_rel_path` 509"* and ended **"SM-2 is the
> mechanism that unpins it."**
>
> **(a) The figures and one of the names were wrong.** `edit_markdown` has not been a tool
> since the surface collapse; the parameter is `edit_file.action`. Writing a retired tool
> name into a tracker trips nothing — `prompt_surfaces_reference_only_real_tools` covers
> the three prompt surfaces and `claude_md_contains_no_deprecated_tool_names` covers
> CLAUDE.md, and `docs/trackers/` is outside both.
>
> **(b) SM-2 shipped, and does NOT unpin this.** An `@ack_*` handle is minted **per call**,
> and the pinned prose sits on the *frequent* actions while the irreversible-rare ones
> carry almost none: over 30 days `update` ran 2,555 times against `delete` 15 and `graft`
> 1. Gating `update` would buy 1,160 chars for ~2,555 extra round-trips a month. **SM-3 is
> the route**, because `server_instructions` is session-scoped — it delivers the same
> guidance once per session with no round-trip at all, which is what SM-2 could never do at
> any scope. SM-2's value is safety on rare irreversible operations, with zero budget
> movement.

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

**Status:** done (narrow scope) 2026-09-03 — `19c0fc09` on `experiments`, patch-id
`4e648b64f6a378e58b0731ab1383bdc1ef804e63`.

**Shipped:** `doc(delete)` and `doc(graft)` are dry runs by default, returning what would
be destroyed, and require `force=true` to apply. Not `write_ack` — see below; the existing
ack machinery is path-keyed and grants a session write root on replay, which is the wrong
mechanism for an id-keyed confirmation. The `librarian(doctor, fix=…)` dry-run convention
fits and costs nothing new.

**Both tests assert the EFFECT, never the flag** — a gate reporting `dry_run: true` and
destroying anyway satisfies a flag-only assertion, so the load-bearing checks are that the
file is still on disk and the row still resolves.

**What the preview reports is the point.** `delete`'s cascade takes the augmentation,
events, links and observations, and those are **catalog-only**: `reindex` rebuilds the row
from the file and git restores the file, but nothing rebuilds an augmentation's params or
an event log. The file is recoverable and the history is not, and nothing in an id shows
that. `graft`'s two ids read symmetrically while only one survives, so the preview names
the row that disappears.

Budget 56_276 → 56_476 (+200) for the `force` description — the discoverability half,
without which a caller gets a dry run and no explanation.

**Left undone deliberately:** the frequent write actions. See the frequency table above.

`src/tools/core/write_ack.rs` **exists** (verified 2026-09-03) and already powers the
`@ack_*` gate for dangerous shell commands and out-of-scope writes. The research names the
two-phase confirm — *server returns a refusal whose payload carries the guidance, model
retries with acknowledgement* — as the practitioner answer to exactly our ordering flaw.
**We invented this pattern independently for shell and never wired it to `doc`.**

Guidance attached to a **refusal** arrives before any effect; guidance attached to a
**result** arrives after. That inversion is the whole of it.

Payoff: the ~2,155 B pinned by the open delivery-ordering bug becomes movable, and the bug
is retired rather than worked around.

**Precondition ANSWERED 2026-09-03 — the pattern works here, measured.** The question was
whether a model reliably retries after a confirmation-required refusal; the external
research found only blog descriptions of the pattern and no production reference. Our own
`@ack_*` gate is that reference, and it was never measured. Over the 30-day `usage.db`
window (71,745 calls, 2026-08-04 → 2026-09-03), keyed per **distinct handle** rather than
per row:

| population | handles minted | replayed | rate |
|---|---:|---:|---:|
| **all** | 258 | 231 | **89.5%** |
| out-of-scope write | 90 | 86 | 96% |
| `run_command` (dangerous command) | 168 | 145 | 86% |
| `edit_file` / `edit_markdown` | 53 | 53 | **100%** |
| `create_file` | 36 | 32 | 89% |

**230 of 231 replays came from the same tool that minted the handle**, which is the
cross-tool rejection working rather than being tested.

Three things this does NOT establish, each of which would inflate the number if ignored:

- **A non-replay is not necessarily a failure.** The caller may have read the refusal and
  correctly decided not to proceed — which is the gate succeeding, not failing. So 89.5%
  is a floor on *"the mechanism is usable"*, never a claim that the remaining 10.5% are
  defects. The two figures are not separable from `usage.db` alone.
- **The reason labels are approximate.** They are keyword-classified from the refusal text;
  the 168-row `run_command` bucket is dangerous-command mints my classifier could not label
  more precisely, not a distinct third reason.
- **Retention truncates both ends.** A handle minted near the window edge can have its
  replay swept. Observed effect is small — exactly 1 replay of a handle whose mint row is
  gone — but the direction is toward under-counting replays.

Derivation: `/home/marius/.claude/jobs/…/ack_retry.py`, re-derivable from the schema above;
not committed, because it is a one-question query rather than an instrument.

**So the design is unblocked — and then the scoping data falsified the entry's own
premise.** SM-2 was written as *"the mechanism that unpins the ~2,155 B floor"*. It is not.
Call frequency over the same 30-day window:

| action | calls | pinned prose it would free |
|---|---:|---:|
| `update` | **2,555** | `doc.patch`, 1,068 chars |
| `move` | **327** | `doc.new_rel_path`, 509 chars |
| `delete` | 15 | — |
| `graft` | 1 | — |

**The pinned prose sits on the two most frequent write actions; the genuinely
irreversible-and-rare ones carry almost none.** An `@ack_*` handle is minted **per call**,
not per session, so gating `update` buys 1,068 chars at the cost of ~2,555 extra
round-trips per month — on a surface that is ~100% cache_read and costs ~$0.0043/request
whole. That is a bad trade in every direction, and it does not improve by making the ack
session-scoped: a session-scoped ack delivers the guidance exactly once per session, which
is **what `server_instructions` already does for free and with no round-trip**. At
session grain, **SM-3 strictly dominates SM-2**.

**Re-scoped.** SM-2's value is *safety on rare irreversible operations*, not bytes:
`delete` (15 calls) and `graft` (1) destroy rows outright and are exactly where per-call
friction is cheap and warranted. Gate those two, expect **no** budget movement, and stop
describing this entry as the route off the floor. **SM-3 is that route**; the § *constraint
that shapes everything* note above should be read with this correction.

Still a runtime behaviour change on a shared checkout, so it needs an explicit decision
before it ships — though at 16 calls per 30 days the blast radius is now known to be small.

**Valid:** conditional — `docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md` stays open

**Rests on:** that bug file's § *Workarounds* discriminator, and the existence of
`src/tools/core/write_ack.rs`.

## SM-3 — Move de-duplicable protective prose into `server_instructions`

**Status:** **REFUTED 2026-09-03** — on both halves, by measurement and by a prior eval.
Do not re-propose without reading § *Why, in two independent parts* below.

`instructions` in `InitializeResult` is a **pre-call** channel: it lands in the system
prompt before any tool call, and codescout already ships it. Crucially it is **shared
across tools**, so protective rules that currently repeat per-parameter can be stated once.

**Constraints, both real:** the slice cap is **1,900 characters**
(`src/prompts/README.md`), and it is not a free win on context — `server_instructions` is
resident for the session just as the schema is. The gain is **de-duplication and
ordering**, not raw bytes. Do not scope this as a byte saving.

Client support is uneven (there is an open request for Claude Desktop to consume it), so
this is a Claude-Code-and-some-hosts channel, not universal.

### Why, in two independent parts — either alone is fatal

**1. There is no capacity.** Measured 2026-09-03 off the live wire (`initialize` →
`result.instructions`): the static slice is **1,696 chars against the 1,900 cap — 204 chars
of headroom**, and production renders 1,827 with the dynamic Project Status block, 221
short of the 2,048 client cliff. The prose this entry proposed to move is **2,069 chars**
(`doc.patch` 1,126 + `doc.new_rel_path` 509 + `edit_file.action` 434). That is a **10×
overflow**. Note the unit: 2,069 is the *description strings*; the same three cost **2,237**
as serialized wire property objects. Both are right, they answer different questions, and
the earlier form of this entry quoted the second under the name of the first.

**2. The premise is false: the surface is not duplicated.** Measured by 8-word shingles
over all **308** description strings (36,540 chars): only **18 of 3,328 distinct shingles
(0.54%)** appear in two or more descriptions. The three blocks above are not copies of each
other — they are three different mechanisms — and `doc.patch` *already* de-duplicates
against `edit_file` by pointing at it (*"edit_file's heading-addressed batch shape
exactly"*) rather than restating it. There is nothing to state once.

### The one real duplicate is settled, and cutting it is REFUTED by eval

The shingle run surfaces exactly one large repeat: the injected `workspace` param, **one
distinct 132-char sentence on 18 of 21 tools** — 2,376 chars of prose, **2,244 of it
redundant**, 3,204 as serialized properties (5.7% of the surface). It is the obvious
compaction target and it must not be cut.

`prompt-hamsa-audit-log:A-28` already ran it: four arms, 10 runs each, sonnet pinned,
against a **live `tools/list` capture** rather than a hand-written fixture. Base (132
chars) **10/10**; treatment (53 chars, routing clause dropped) **8/10**; control-null
(description removed) 9/10; control-positive (directive forbidding the pin) **0/10**, which
is what proves the channel binds and the run is not void. The pre-registered rule required
`treatment ≥ base − 1`; it got 8. **Verdict KEEP** — 1,896 chars deliberately not cut.

**The mechanism is the finding, and it generalises past this string.** Every failure in
treatment and control-null was the same one, and none occurred in base: the model reached
for `workspace(action="activate", path=…)`. Activation is **global**, so with a parent
session concurrently working the active project that clobbers it — the exact condition the
per-call pin exists to prevent. The clause is not *describing* the parameter, it is
**displacing a competing prior**.

> Ask of a candidate sentence not *"is this redundant?"* but **"does this DESCRIBE the
> parameter, or DISPLACE something the model would otherwise reach for?"** The second kind
> is invisible to n-gram redundancy analysis.

That blindness is not hypothetical here. This session reached the `workspace` string **by
running exactly the n-gram analysis A-28 names as the trap**, and would have proposed the
cut had `grep` not surfaced the audit. Recorded as a confirmation, not a catch: A-28 states
that once `workspace` and the A-27 clauses are set aside, remaining cross-tool duplication
is **~300 chars**. Today's independent shingle run puts 396 words inside a repeated span,
of which the 18 `workspace` copies account for ~342 — leaving ~54 words ≈ **300 chars**.
Same number, different instrument, derived without reading A-28 first.

### What survives

Nothing that is an *edit*. The 204 chars of headroom could hold one more line, but
`src/prompts/README.md` rule 1 caps hard rules at 5–8 and the slice already carries 6 Iron
Laws plus a quickref, and this repo does not ship prompt changes without an arm (A-20–A-29
are that discipline). Any residual is therefore a **new pre-registration**, which is what
SM-4 already is. **The ~2,155–2,237 B of pinned protective prose stays pinned**, and the
routing-vs-describing question above is the tool for judging the next candidate.

**Valid:** invariant — the two refuting measurements are structural (a 204-char headroom
against a 2,069-char payload; 0.54% shingle repetition), and A-28's verdict is a
pre-registered eval result, not a snapshot. Re-derive the headroom before quoting it.

**Rests on:** `src/prompts/README.md` § *Surfaces* and rule 8 (`STATIC_SLICE_CHAR_BUDGET =
1900`, `src/prompts/mod.rs`); `prompt-hamsa-audit-log:A-28`; the MCP
`InitializeResult.instructions` definition.

## SM-4 — Run the A/B nobody has run: enum-dispatched `doc` vs split per-action tools

**Status:** **BUILT AND PROBED 2026-09-03 — the power probe FIRED and the matrix was NOT
run.** The pre-registration below was committed at `1f8af4ac` before any arm existed; this
outcome block was appended after. See § *Outcome* at the end.

No published benchmark varies tool **shape** holding operations constant. We are unusually
well positioned: the `prompt-engineering` repo is an eval harness that puts codescout's own
prompts under TDD against headless `claude -p`.

### What decision this informs — stated first, because "produces knowledge" is not enough

**It evaluates a change already shipped.** The 2026-09-02 collapse multiplexed five tools
into action-dispatched mega-tools; `doc` now carries 17 actions and 60 params. If splitting
measurably improves action selection, that is evidence the collapse **cost accuracy**, and
the finding is actionable about a decision already taken rather than a hypothetical one.

**No ship rides on the result, and that is pre-registered too.** Splitting `doc` back out is
refuted on byte grounds independently (§ SM-1–SM-3), so `ΔS > 0` must **not** be read as a
mandate to split. Recording that here is what stops a favourable number acquiring a
conclusion it was never scoped to support.

### Design — stub MCP servers, not a rendered fixture

A-28 rendered the surface as text in a `CLAUDE.md` and asked the model to *name* the call.
That is sound when the unit is one sentence **inside** a description. **It is not sound
here**, because SM-4's unit is the *structure of the tool list*, and a text rendering
replaces "selecting from a tool list" with "reading a document about tools" — destroying
exactly the variable. A-28 logged this as a caveat; for SM-4 it would be the whole result.

`mcp_command` is supported (15 scenario files use it), so each arm gets a **stub MCP
server** whose tools return `{"ok": true}` and whose only job is to be *selected from*:

| arm | surface | role |
|---|---|---|
| **enum** | one `doc` tool, the real 17-action schema | base — P-3 makes it binding |
| **split** | N per-action tools (`doc_append_entry`, `doc_update`, …), each carrying only its own action's params | treatment |
| **control-positive** | the `split` surface + a mandatory directive naming a wrong tool | must drive `S` to ≤ 2/10 |

**There is deliberately no `control-null`.** A-28's null was "description removed" — a
meaningful midpoint on a deletion. A *shape* comparison has no null shape, and adding an arm
for symmetry would cost 10 runs to answer nothing. The positive control alone discharges the
validity gate, which is what the operating guide actually requires.

The split surface is **generated** from the live `tools/list` capture by `gen_fixtures.py`,
which refuses to write unless it emitted one tool per action and the union of the split
tools' params equals the enum tool's params. Hand-writing it would test a manipulation that
never arrived.

### Prior art — surveyed 2026-09-03 BEFORE building, and it falsified the first draft

`prompt-engineering` holds 26 scenarios. Two bear directly on this entry, and reading them
changed the design rather than confirming it. **Read both before writing a line of scenario.**

**`scenarios/ledger-vs-tracker` — the stimulus this entry first proposed is MEASURED
TAUTOLOGICAL.** Its `append-shape` scenario is the same task (seed a session log carrying one
`F-N` heading, ask to add an observation) and it scored **10/10 on all four cells**, with
`--ablate` — guide stripped — *also* 10/10. Recorded verdict: *"tautological for sonnet … the
scenario has NO POWER for a capable model."* The first draft of SM-4 would have spent 30 runs
rediscovering that ceiling.

It also supplies the escape, and it is not "use more runs": an **ambiguous** artifact with no
correct action, where the checker **classifies** each run (append / update-in-place / clobber)
and tallies a distribution. That is the only design in the file that produced a signal —
0/10 vs 2/10 — and its own caveat is honest (Fisher ≈ 0.47, needs n≈30–50).

And it establishes the build path: **two frozen aliased binaries** (`codescout-tracker` /
`codescout-ledger`, differing in tool descriptions and one action name) driven via
`registry: anthropic-mcp`. SM-4's "two real surfaces" is a pattern already executed here, not
a new capability — which makes stub servers the *fallback*, not the plan.

**`scenarios/surface-budget` — the measurement unit is not what § *The unit* above assumes.**
Measured 2026-08-23 with `eval-bins/calibrate_attach.py`, reproducible to the token across
three runs: **Claude Code 2.1.241 DEFERS MCP tool schemas.** Attaching codescout raises the
prompt by **1,175 tokens**, 7.3% of the surface's token weight. Only tool **names** are
injected; the ~85% that is JSON schema arrives later via `ToolSearch`, and only if the model
reaches for it. That README also records the consequence this queue has not absorbed: *"the
K* = 12.5 break-even analysis needs redoing — it assumed the tool surface sits in the cached
prefix and is re-read every turn. It does not."*

**That cuts FOR this experiment, not against it.** Under deferral the resident payload is
exactly the **tool-name list**, and enum-vs-split is precisely a difference in names. SM-4
therefore varies the one thing that is definitely resident. It does mean parameter
construction costs a `ToolSearch` hop, which must be scored as part of `P`, not excluded.

Three more from that README, each already paid for: the tool surface is **project-state
dependent** (23 / 26 / 27 tools by fixture), so every arm must untar the fixture project or it
measures 6.5% under production while silently dropping four tools; prompts must say *"use the
codescout MCP tools"* or the model answers with `Bash` and scores a confident PASS having
measured nothing; and `run_arms.py` **scores every arm with the FIRST arm's checker**, warning
only on stderr — hence one config dir per task.

**An obligation is outstanding there, and it blocks publication rather than work.**
surface-budget's own pre-registration is *owed and unwritten* — it was scoped not to modify
the codescout repo, and its README states the `-base` table **must not be published until the
entry exists**. That entry belongs in this repo. It is a separate task from SM-4 and should
not be bundled into it.

### The stimulus — revised, because the first choice is known to ceiling

**Rejected:** *"add a new entry to tracker X"* as a pass/fail task. Documented-real confusion
(CLAUDE.md's ⚠ that `augment` **replaces** a collection, which took a queue from 19 entries to
1 on 2026-08-16), but `ledger-vs-tracker` measured that exact shape at the ceiling with and
without its guide. A confusion being real in the field does not make it reachable by a
stimulus.

**Two stimuli, scoring the two classes where each can actually move:**

- **`P` — parameter construction, unambiguous, param-heavy.** One operation, a call needing
  `id_prefix` + `anchor_heading` + `title` + `body` together. Selection ceilings for a capable
  model; *building* a five-field call correctly is where the enum/split difference should bite,
  and it is the half the literature predicts consolidation **helps** and splitting **hurts**.
- **`S` — action selection, AMBIGUOUS, classified not scored.** An artifact that legitimately
  invites either an append or an in-place update, and a neutral instruction. No correct answer;
  the checker tallies a **distribution** per arm. Shape matters iff the distribution shifts.

### MANDATORY POWER PROBE, before the matrix — the lesson `ledger-vs-tracker` paid full price for

It discovered *no power* **after** running the whole matrix and the ablation. So:

> Run **base only, n=5**, on each stimulus first. If `P` returns 5/5 or 0/5, or `S` returns a
> single class 5/5, that stimulus **has no power and the matrix must not run on it**. Fix the
> stimulus or pin a weaker model (their own recommended escape) and re-probe. A ceiling found
> at n=5 costs 5 runs; found at the matrix it costs 30 and reads like a tie.

This probe is **not** the validity gate below — it asks whether the *task* can move at all,
before any arm claims the *manipulation* did or did not move it.

### The two scores, and why they must not be blended

- **`S` — action-selection**: did the call name the intended operation? (`action="append_entry"`
  in enum; tool `doc_append_entry` in split.)
- **`P` — parameter-construction**: do the args validate against the real schema **for the
  operation actually invoked**?

`P` is scored against the chosen operation, **not** conditioned on `S` being right. Valid
args for the wrong action is a construction success and a selection failure, and that
decomposition is the whole experiment — the literature's claim is that consolidation trades
one for the other, and a single blended accuracy number cannot express it.

### Pre-registered decision rule

1. **Validity gate, binding and first.** If `enum` and `split` return the same `S` **and**
   the same `P`, the run is **VOID** until control-positive moves `S` to ≤ 2/10. Identical
   arms are equally the signature of a manipulation that never reached the model — which is
   what happened in `A-27`, and the gate is what stopped a tie being published.
2. Non-void: report **both** deltas with their failure classes. **Predicted direction,
   committed now: `ΔS > 0` and `ΔP < 0`** (split helps selection, hurts construction).
   Pre-registering the direction is what stops a confirmation being retrofitted to whichever
   way it lands.
3. **`ΔS > 0` is not a mandate to split** — see § *What decision this informs*.
4. n=10 separates **large** gaps only. Anything under 3/10 apart is **INDETERMINATE** and
   needs n ≥ 30, which is a **new pre-registration, not a re-run** — re-running until a
   threshold flips is fishing.

### Harness constraints this design must honour — each one has already cost someone a run

From `prompt-engineering:prompt-tdd-operating-guide` (22 entries; read it, do not trust this
list to stay complete):

- **OP-22** — checkers must be `script:` files. An **inline** checker makes `run_arms.py`
  print *"No custom checker in any arm's scenario.yaml"* and silently skip re-scoring, the
  per-run log and the distinct-count. Cost 70 runs / $11.50. `grep -c 'script:'` before running.
- **OP-5** — a checker without the exec bit reports a clean `0/N`, character-identical to a
  real floor. `chmod +x`, and mutation-test the checker **before** any arm, in two layers:
  that it runs at all, and that it splits pass / fail / absent.
- **OP-21** — `--disallowedTools` **removes** an MCP tool rather than denying it, so a leak
  leaves no `<tool_use_error>` and is undetectable in the transcript. Each arm must expose
  **only its own server**; never rely on denial to isolate a surface.
- **OP-7** — `mcp_command` is the binary on `PATH`, not the working tree. Absolute paths.
- **OP-20** — one scenario per pytest invocation; two scenario dirs sharing a module basename
  cross-bind imports and report false failures in untouched code.
- **OP-16** — `max_cost_per_scenario` resets **per arm**, so the real ceiling is n× the
  number of arms; the suite total is unguarded.
- **OP-11 / OP-3 / OP-2** — a spend-limited subscription returns a clean `0/N` on every arm
  and reads as a tie; `pass_threshold` defaults to 1.0; `1/1 passed` is the *scenario* count.
  Read the rate from `run_arms.py`, never the PASS verdict.

### Known limits, recorded before the run

- **One task.** This measures the `append_entry` / `update` / `augment` confusion set, **not
  "tool shape" in general**. A second task set is a separate pre-registration.
- **Stub servers are not codescout.** They reproduce the *surface*, not the behaviour, so
  nothing here speaks to what happens after a call lands.
- **The split surface is synthetic** — it has never shipped, so its descriptions are
  generated rather than authored, and an authored split surface might score differently.
- `prompt-engineering` is **outside this session's working directories**; building the arms
  needs write access there, which is a permission decision, not a technical one.

**Valid:** conditional — until the arms run and a verdict is recorded below

**Rests on:** `prompt-engineering:prompt-tdd-operating-guide` (OP-1…OP-22);
`prompt-hamsa-audit-log:A-28` for the arm shape and the validity gate;
`prompt-surface-compaction-session-log:W-2`'s cost arithmetic, whose `Promote-when` this
discharges; the 2026-09-02 tool-surface collapse as the change under evaluation.


### Outcome — 2026-09-03: no power, matrix not run, and one voided result on the way

**Built:** `prompt-engineering:scenarios/tool-shape/` — `fixtures/gen_fixtures.py` (derives
both surfaces from one live capture at `e5307ba2` and refuses to write on drift),
`stub_server.py` (one stdio MCP server, arms differ only in `MCP_STUB_TOOLS`),
`check_append_shape.py` (scoring is arm-agnostic; the surface guard below is not),
`test_check_append_shape.py` (25 layer-0 tests), and per-arm configs.

**Probe result — both arms at ceiling:**

| arm | surface actually served | score | distinct |
|---|---|---:|---:|
| enum | 10 × `mcp__codescout__doc` | **5/5** | 5 |
| split | 10 × `mcp__codescout__doc_append_entry` | **5/5** | 5 |

The pre-registered rule fires: *"if `P` returns 5/5 or 0/5, that stimulus has no power and
the matrix must not run on it."* **It did not run.** Cost ~$1.80 against a ~30-run matrix.
This independently reproduces `ledger-vs-tracker`'s conclusion on a different manipulation:
a capable model ceilings on codescout tool-selection tasks, so the ceiling is a property of
the *stimulus class*, not of either shape.

**A VOIDED RESULT CAME FIRST, and how it was caught is the transferable part.** The initial
probe used ONE shared `prompt_tdd.yaml`. `run_arms.py` applied its single `mcp_command` to
both scenario dirs, so **both arms were served the enum surface** — the split arm called
`mcp__codescout__doc`, a tool absent from its own surface, and returned a clean `5/5 vs 5/5`
that read exactly like the ceiling above.

> **`distinct` did not catch it.** `distinct == 1` is the documented signature of a
> manipulation that never arrived; here it was **5** — five genuinely different answers,
> because the model was doing real work on the wrong surface. Only grepping the trace for
> *which tool names actually appear* separated the two worlds. **A per-arm assertion that
> the served surface is the intended one belongs in the checker, not in a human's habit.**

The structural cause is one `surface-budget` already documented from the other side: its
README records that `run_arms.py` takes the **first arm's checker** and scores every arm with
it. Same sharing, different field, same remedy — **one config per arm**, now in place.

**The habit is now a mechanism** — `prompt-engineering:ff33482`, patch-id
`bb8932cfa9a0b15f5feb496c5e95c345dbff759b`. Each arm's predicate derives the SERVED surface
from the calls the server **accepted** and returns `wrong-surface:<served>` without scoring,
so a mis-served arm now produces a `0/N` whose class names the cause instead of the `5/5`
that read as a ceiling. Three of the new tests check CONFIGURATION rather than code — each
arm points at its own checker, names its own serve script, serves its own fixture — because
the void probe was a config defect that a predicate-only suite stays green through.

Two things that design cost, both worth carrying to the next A/B of this shape:

- **The guard needs arm knowledge and the scoring must not have it.** `expect` reaches the
  guard and nothing else; `score()` still takes no arm. That is not left to convention —
  `both_arms_score_identical_semantics_identically` asserts the two arms return the same
  verdict for the same semantics, so a leak reds instead of shipping a difference
  **indistinguishable from the effect the eval measures**.
- **A DENIED call is not evidence of the served surface — it is the opposite evidence.** A
  split-arm model may guess `doc` and be *refused*; counting that would make a correctly
  configured arm accuse itself. Denials are dropped once, in `normalise()`, which also stops
  a refused guess being scored as the model's selection. Arm identity itself cannot come
  from the environment at all: `run_arms.py` builds one env for every arm and
  `PROMPT_TDD_SCENARIO_DIR`, though set, always resolves to `os.getcwd()`.

Nine mutations, zero survivors, each killed by the test written for it — including the arm
leaking into scoring, `was_denied` blinded to the real refusal text, and the guard removed
as seen through the harness path rather than the predicate.

### What this establishes, and what it does not

**Establishes:** the `append_entry` stimulus cannot discriminate tool shape at n=5 on sonnet;
both shapes reach the correct operation with valid arguments every time. A follow-up needs
the pre-registered escape — an **ambiguous** task scored by classification, or a weaker model
— and that is a new pre-registration, not a re-run of this one.

**Does not establish** anything about `ΔS` or `ΔP`. A ceiling on both arms is *no measurement
of the difference*, and reporting `5/5 vs 5/5` as "no effect" would be the error P-2a exists
to prevent.

### Two findings that arrived from BUILDING the arm, before any model ran

1. **The split surface is 61% larger: 97,438 chars against 60,437**, same capability, 20
   other tools byte-identical. Splitting costs ~37 KB because shared params (`id`,
   `workspace`, `entry_collection`, …) stop being written once and start being written per
   tool. Mechanical, arm-independent, and an independent confirmation of the byte-grounds
   refutation in § SM-1–SM-3 — so `ΔS > 0` would have had to be very large to pay for it.
2. **`doc(action="augment")` derives exactly ONE param.** The `id` param's routing prose
   names eleven actions and omits `augment`, and the `augment` param itself leads with
   `create:`, so the enum surface never binds `id` to that action. Filed against codescout;
   the generator carries a named, asserted `MANUAL_ROUTE_FIXUPS` override because leaving it
   would have made `doc_augment` read as unusable — biasing the split arm away from one of
   the stimulus's tempting wrong answers and **manufacturing the predicted `ΔS`**.

**Valid:** dated 2026-09-03 — the ceiling is a measurement of this stimulus at n=5 on sonnet

**Rests on:** `/tmp/ts-enum`, `/tmp/ts-split` per-run logs (ephemeral — the counts above are
the durable record); `prompt-engineering:scenarios/tool-shape/`;
`prompt-hamsa-audit-log:A-38`'s sibling treatment of the same harness constraints.
## Template for new entries

<!-- Appends land above this line. Use:
     doc(action="append_entry", id="<this artifact id>", id_prefix="SM",
         title="...", body="...", anchor_heading="## Template for new entries")
     The server writes a def_re-conformant `## SM-N — <title>` heading. -->
