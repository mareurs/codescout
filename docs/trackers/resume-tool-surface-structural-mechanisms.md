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
