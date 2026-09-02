---
id: '0e0316e9036d7f16'
kind: spec
status: active
title: Tool Surface Budget — bounding the per-request prompt payload
owners:
- marius
tags:
- prompt-surfaces
- budget
- tools-list
- gate
- schema
topic: prompt-surfaces
---

# Tool Surface Budget — bounding the per-request prompt payload

**Author:** architecture-snow-lion, 2026-08-18
**Evidence:** `docs/trackers/prompt-surface-compaction-session-log.md` (F-1, F-2, W-1),
`docs/issues/archive/2026-08-18-append-entry-body-writer-undeclared-in-artifact-schema.md`,
`docs/trackers/reconnaissance-patterns.md` R-106.

## Problem

`tools/list` delivers **58,882 characters** of authored text — 27 tool descriptions plus
their input schemas — on every request of every session. Three facts make that a
structural problem rather than a size complaint.

**Nothing measures the larger half.** `description()` is capped twice
(`tool_descriptions_stay_under_budget` at 300, `every_tool_description_under_cap` at
1800). `input_schema()` is **50,361 of the 58,882 chars — 86% — and no test has ever
measured it.** `all_tools_have_valid_schemas` (`src/server.rs:2299`) asserts `is_object`
and `type == "object"`; it passes on an empty schema.

**Per-item caps do not bound a sum.** Every tool currently passes its description cap —
`librarian` sits at 1,692 of 1,800, 94% — and the surface still reached 58,882 chars.
Two mechanisms: growth moved sideways into the uncapped `input_schema()`, and a per-item
cap permits N items to each sit at their limit. A per-tool schema cap would repeat this
exactly.

**The exemption had no replacement budget.** `tool_descriptions_stay_under_budget`
carves out the librarian family (`is_librarian_tool`, `src/server.rs:1896-1904`). The
carve-out is *correct* — `artifact` dispatches 12 actions and `librarian` 9; 300 chars is
not achievable for a dispatcher. But an exemption from a budget must name a replacement
budget, and this one named none. That family is now **28,788 chars, 48.9% of the whole
surface**.

The live consequence is filed: `anchor_heading` shipped in `5d5ed457` implemented and
**unadvertised**, and no gate in the repository could have caught it. Its cost is not
bytes — it is that the one code path which structurally cannot produce an uncitable
entry is invisible on the only surface an agent reads.

## Measured — 2026-08-18, live wire

Method: drive `target/release/codescout start` over stdio with a real `initialize` +
`tools/list` handshake, XDG dirs redirected to a scratchpad. Script retained at
`scratchpad/probe_tools.py`.

| Surface | Size | Frequency | Size gate |
|---|---|---|---|
| `tools/list` (27 tools) | **58,882 chars** (62,546 JSON) | every request | **none on schemas** |
| `server_instructions` | 1,827 chars | once per conversation | `source_md_under_cap` = 1900 ✅ |
| `get_guide` corpus (10 topics) | 90,485 B | once per topic per session | none |

Within `tools/list`: descriptions 8,521; schemas 50,361; of the schema bytes, **30,512
(61%) is param-description prose**. Largest: `artifact` 12,102 (51 params), `librarian`
8,492, `edit_markdown` 4,753, `artifact_augment` 4,564.

### Why this is a recurring cost, not a one-time one

Four Claude Code sessions (2026-07-16 → 08-18, three models), from session JSONL:

| Session | Turns | cache_read | fresh input | avg prefix/turn | 14,720-tok tool block |
|---|---:|---:|---:|---:|---:|
| `55515bc5` | 1057 | 281.4M | 2,112 | 266,184 | 5.5% |
| `83fe3085` | 837 | 259.3M | 1,674 | 309,790 | 4.8% |
| `88e875db` | 1467 | 429.6M | 2,934 | 292,873 | 5.0% |
| `e423206f` | 322 | 47.5M | 644 | 147,597 | **10.0%** |

**100.0% cache_read in all four.** At $0.30/M cache-read against $3/M fresh, prefix
re-reading is **68% of session cost** ($84 of $123 on `55515bc5`). The tool block is
re-read on every request for the life of the session, and a short session pays double the
share of a long one — a fixed tax against a growing denominator.

### What this measurement does NOT license

`usage.db` spans 30 days across **25 distinct `codescout_sha` values**, 96.7% of calls
from one project and one developer. Therefore:

- **No `chars/call` ratio may be computed.** It divides today's bytes by historical
  counts taken against a substrate that changed 25 times. Bytes are a property of today;
  rates are a property of a workload; their product is a forecast, not a measurement.
- **No trim may be justified by a usage rate** until a fixed-SHA measurement exists.

The cache economics above are exempt from this: they are a property of the Anthropic API,
replicated across four sessions and three models, and involve no codescout substrate.
Every decision below rests only on today's source and today's wire.

## Design decisions

1. **Budget the payload, not the item.** One constant bounding the sum of
   `description() + input_schema()` across all advertised tools. This is the boundary
   where the cost is actually paid.

2. **Measure what `list_tools` builds — not what `input_schema()` returns.**
   `src/server.rs:1017-1028` filters by `availability(&caps)` and injects the `workspace`
   param for `pinnable()` tools *after* calling `input_schema()`. A gate that sums raw
   `input_schema()` would measure a string nobody receives — the exact defect
   `production_render_fits_the_client_channel` exists to prevent (its doc comment: *"the
   green test was measuring a string nobody receives"*). The helper MUST reproduce the
   `list_tools` construction.

3. **Pin the capability set.** Availability is conditional on `has_lsp`,
   `has_embeddings`, `has_git_remote`, `has_libraries`. The budget is measured against
   **all-capabilities-true** — the maximal advertised surface, and the only one that must
   be guaranteed to fit.

4. **Ratchet, not ceiling — default lower, raise on a recorded justification.** The
   constant's first instruction is the same as `STATIC_SLICE_CHAR_BUDGET`'s: *find the
   bytes.* That is the right first move and makes adding a parameter a trade rather than
   an accretion. It is not a prohibition, and this decision used to read "lower-only",
   which its own history had already falsified — the budget was raised twice on purpose
   (2026-08-28 for `memory`'s `force` shrink guard, 2026-09-02 for `workspace`'s
   `read_only` precedence clause), each time because the bytes bought something the
   surface genuinely owed and each time recorded as debt at the constant.

   **Raise it when the addition is owed** — a default documented wrong, an action no
   agent could discover, a guard whose absence loses data. Do not raise it to avoid
   re-reading a paragraph. Two requirements make the permission safe rather than a
   loophole: set the constant to the **exact measured total** from the report test, never
   rounded up, so the ratchet still bites on the very next added byte; and **add an entry
   to the constant's log** saying what the bytes bought. A raised budget is
   indistinguishable from an earned one once the reason leaves the room, which is why the
   constant carries a log rather than a number.

   **The cheapest payback is a description that is long because the tool is wrong.**
   First lowering, 2026-09-02, 56_547 → 56_497: `memory`'s `project_id` description spent
   50 characters documenting an *undocumented* alias — the schema advertised one key while
   the runtime honoured two. Removing the alias removed the sentence. Prose-golf across 26
   tools was never needed.

5. **Fail loudly and specifically.** On breach, print the per-tool table so the failure
   names where the bytes went. A budget that reports only a total tells an author to give
   up rather than to choose.

### Rejected

- **Per-tool schema caps.** Repeats the defect this spec documents: per-item caps do not
  bound a sum, as the description caps already demonstrate.
- **Move `artifact` schema prose into `get_guide("librarian")`.** Refuted by measurement.
  Both surfaces are cache_read at the same rate; the guide arrives as a tool result at
  turn K and costs `X × (N−K) × $0.30/M + X × $3.75/M`, against the schema's
  `X × N × $0.30/M`. Break-even at **K ≈ 12.5 turns**, and the librarian guide fires well
  before turn 12 in any session touching trackers. A wash at best.
- **Narrow `pinnable()` to the 7 tools observed using `workspace`.** All 21 observed uses
  fall in a three-day burst (2026-08-13..15) with none across the 9,310 calls since. That
  is a bursty capability, not a dead one, and removing it wrongly breaks concurrent-subagent
  workspace pinning **silently**. Costs 4,403 chars; not buyable with three days of evidence.
- **Derive schemas from `Args` via `schemars` (already a direct dependency).** This is the
  structural answer to F-1's whole class — doc-comments become descriptions, and
  advertised-equals-accepted holds by construction. Deferred under this project's
  `tool-registration-rule-of-three`. See *Revisit-when* for the live count, **recounted
  2026-09-01 and corrected in both directions at once** — it gained a confirmed instance and
  lost an unverified one, and still stands one short.

## Architecture

Everything lands in `src/server.rs`, beside the gates it joins. No new module: this is a
measurement of an existing construction, and extracting it before a second caller exists
would be the same premature abstraction the spec rejects above.

```
tests::advertised_surface(caps) -> Vec<(name, desc_len, schema_len)>
    └── mirrors list_tools: availability filter → input_schema() → inject_workspace_param
                                                                     when pinnable()

const TOOL_SURFACE_CHAR_BUDGET: usize = <measured>   // ratchet, lower-only
tests::tool_surface_under_budget()                   // the gate
tests::tool_surface_report_lengths()                 // the map, always prints
```

## Components

**1. `advertised_surface(caps)` helper.** Reproduces `list_tools`'s construction and
returns per-tool `(name, description().len(), serialized input_schema length)`. Single
source for both tests below. Serialization must match the wire: compact separators, no
pretty-printing.

**2. `TOOL_SURFACE_CHAR_BUDGET` + `tool_surface_under_budget()`.** Sums the helper's
output and asserts `<= budget`. **Do not hardcode 58,882** — that figure came from the
release binary against the real project. Run component 3 once in the test harness and take
*its* number; the harness's capability set and project fixture may advertise a different
tool set. The doc comment states the unit (characters), the rule (lower-only), and cites
this spec.

**3. `tool_surface_report_lengths()`.** Always-passing companion that prints the per-tool
table sorted by total, plus the sum. Mirrors the existing
`tool_descriptions_report_lengths` (`src/server.rs:1925`), which covers descriptions only.

**4. F-1 fix — `Artifact::input_schema()`.** Declare `anchor_heading`; re-scope `title`
and `body` (currently `"create: artifact title"` / `"create: markdown body"`) to name
their `append_entry` role; state the tri-field requirement — all three or none, a partial
set is refused. Estimated **+450 chars**, which will breach the budget from component 2.
That breach is the design working, not a defect.

**5. `workspace` string trim.** One edit to the `json!` literal in
`inject_workspace_param` (`src/server.rs:496-508`). 259 chars → ~95, injected into 24
tools: **−3,936**. `pinnable_tools_advertise_workspace_param` asserts presence only, so it
does not break. The param set is unchanged — only its prose.

Net after 4 + 5: **−3,486 chars per request**, plus a closed high-severity bug.

## Testing

| Test | Pins | Mutation that must kill it |
|---|---|---|
| `tool_surface_under_budget` | the sum, post-injection, post-filter | add 500 chars to any param description |
| `tool_surface_report_lengths` | nothing (report) | n/a — must never gate |
| `advertised_surface` mirrors `list_tools` | the construction | remove the `inject_workspace_param` call from the helper; the sum must drop by ~6,216 |

The third row is the one that matters and the one most likely to be skipped. Verify it by
mutation before trusting the gate — a helper that drifts from `list_tools` reproduces
precisely the defect named in decision 2.

## Sequencing

Order is load-bearing; each step depends on the previous one having landed.

1. **Components 1–3** — helper, budget, report. Land the ratchet at today's measured value.
2. **Component 4** — the F-1 fix. **Expected to fail the gate.** Do not raise the budget.
3. **Component 5** — the `workspace` trim pays for it. Lower the constant to the new total.

Steps 2 and 3 may be one commit if the intermediate red state is inconvenient, but the
commit body should record that the addition was paid for rather than absorbed.

## Documentation

**Both done.**

- `src/prompts/README.md` — `tools/list` is now listed in § Surfaces (it was absent, which
  is part of how it went unmeasured), and § *The tool-surface budget* carries the rule, the
  two load-bearing constraints, and the schema-to-guide break-even.
- `docs/issues/archive/2026-08-18-append-entry-body-writer-undeclared-in-artifact-schema.md`
  — its `## Fix` proposed paying for `anchor_heading` out of prose duplicating
  `get_guide("librarian")`, on the assumption that guide bytes are cheaper than schema
  bytes. The cache measurement refutes that; the note is kept with the correction beside
  it, because the correction is the useful part. Bug fixed in `01194e21` and archived.
## Out of scope

- Guide-corpus compaction. The cost there is the **1,336 auto-injections across 21
  sessions**, not corpus size, and that is what the guide-ledger session-identity work
  already in flight addresses.
- Any trim justified by a usage rate — including the 10 tools with ≤12 lifetime calls
  (19.2% of the surface). Blocked on the routing experiment below.
- `server_instructions` and `onboarding_prompt`. Already budgeted, or delivered once.

## Open parameters

1. **Budget value.** Derived from component 3's first run in the test harness, not from
   the 58,882 figure in this spec.
2. **Unit.** Characters, not bytes — same reasoning as `CLIENT_INSTRUCTIONS_CHAR_LIMIT`
   (`src/prompts/mod.rs:39`), where a byte comparison over-counted an em-dash-dense
   surface and stayed green while shipping truncated. **Decided: characters.**
3. **Envelope or authored text?** This spec budgets `description + input_schema`
   (58,882), not the full JSON payload (62,546). The 3,664-char difference is protocol
   framing no author controls. **Decided: authored text only.**

## Revisit-when

- **A third instance of advertised ≠ accepted appears.** Then derive schemas from `Args`
  via `schemars` and delete this class of defect. The dependency is already present; the
  argument for waiting is sample size, not feasibility.

  **Live count, recounted 2026-09-01: 2 confirmed, 1 live. The trigger has NOT fired.**
  Both halves of the original count moved, in opposite directions:

  | instance | verdict |
  |---|---|
  | F-1 — `append_entry`'s `anchor_heading` implemented, unadvertised | confirmed; **since fixed** (`anchor_heading` is advertised today) |
  | `artifact(action="graft")`'s `from_id` / `into_id` — **required** by `graft::Args`, absent from the 53 advertised properties | **confirmed, new.** Measured: 1 attempt, 1 failure (`missing_required_param`) in 51,346 recorded calls. `docs/issues/archive/2026-09-01-graft-requires-two-params-the-schema-never-advertises.md` (`2fbb59c9b84a0dcf`), fixed at `6894b67d` / patch-id `3cb9bc68a685c46252388dc21a3dd8d7beff9098` |
  | `query` / `title_contains` / `preview` — previously listed here as the unverified second instance | **retired — wrong class.** Verified 2026-09-01: no `Args` struct under `src/librarian/tools/` accepts any of the three (the sole grep hit is an unrelated fn parameter, `src/librarian/tools/get.rs:35`). They are neither advertised **nor** accepted — agents guessed the names and serde dropped them, because `find::Args` cannot carry `deny_unknown_fields` while the dispatcher passes sibling actions' keys through. That is `IC-15` accepted-parameter-silently-dropped, owned by `system-retrospective-improvements:T-2`, not this class |

  Worth stating plainly, because the correction ran both ways: an **inflated** count fires
  this rewrite early, and a **stale** one never fires it at all. The entry that was costing
  us was the unverified one — it had sat as "origin not yet checked" since 2026-08-18, and
  checking it took one grep.

  **What now detects instance 3 without anyone going looking.**
  `param_probe::assert_required_are_advertised` (`src/librarian/tools/mod.rs`, added
  `6894b67d`) asserts each action's required params against the advertised schema, at all
  four probe sites. Until this trigger fires, that guard is the thing standing in for the
  structural fix — so the next instance arrives as a red test rather than as a user
  tripping over it.
- **The routing experiment resolves.** Add `call_graph` and `tree` to the
  `server_instructions` quickref, note the SHA, and re-read `usage.db` filtered on
  `codescout_sha` after two weeks. `references` is the control — already routed, 128
  calls. If `call_graph` is still zero after routing, it is dead rather than hidden, and
  the 19.2% question becomes actionable.
- **`workspace` usage exceeds ~2% of calls.** Concurrent-subagent work becoming routine
  would reopen the `pinnable()` question from the other side.

**Confidence:** high on the diagnosis and on components 1–5 — all rest on today's source
and today's wire, with no inferential step. Medium on the budget's exact value; it should
be measured, then held. Low on which low-traffic tools are dead, which is why nothing
here touches them.
