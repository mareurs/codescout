---
id: '03464a8808345846'
kind: tracker
status: active
title: Prompt Surface Compaction — Session Log
tags:
- prompt-surfaces
- session-log
- compaction
topic: prompt-surfaces
entry_prefix:
- F
- W
entry_high_water_F: 3
entry_high_water_W: 1
---

> **Work stream:** auditing codescout's four prompt surfaces (`tools/list`,
> `server_instructions`, the `get_guide` corpus, onboarding) for correctness and
> byte cost, then compacting the largest.
>
> **This is a guarded ledger** — `entry_prefix: [F, W]` is declared in frontmatter,
> so `edit_markdown` is refused. Append via
> `artifact(action="append_entry", id_prefix="F"|"W", title=…, body=…,
> anchor_heading="## Template for new entries")` and let the server write the
> heading. Status vocabulary is the one in `docs/templates/session-log.md`.
>
> Predecessors (both archived): `archive/prompt-guide-refactor-session-log.md`,
> `archive/mcp-prompt-redesign-session-log.md`.

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-08-18 | high | prompt-surface | fixed-verified | `append_entry`'s `anchor_heading` is implemented but not advertised in the `artifact` schema |
| F-2 | 2026-08-18 | med | self-friction | fixed-verified | Read wire duplication as source duplication — the `workspace` param is injected once, not authored 24× |
| F-3 | 2026-08-18 | med | prompt-surface | open | 19.2% of the tool surface is bought for 38 calls, and the data cannot say whether those tools are dead or unrouted — field experiment **superseded** by hamsa A-26's controlled arms (0/10, 0/10, control 10/10); routing reverted in `89d32048`, evidence points at *substituted* not *unrouted* |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-18 | med | Scout the generator before editing a generated surface | Would have shipped a "free mechanical dedup" with no valid implementation | validated |
---

## Baseline measurement (2026-08-18)

Ground truth from driving `target/release/codescout start` over stdio with a real
`initialize` + `tools/list` handshake (XDG dirs redirected to a scratchpad so the
probe could not touch the real guide ledger). Recorded here so later compaction
passes have a comparable baseline.

| Surface | Size | Frequency | Size gate |
|---|---|---|---|
| `tools/list` (27 tools) | 58,882 chars (62.5 KB JSON) | every request | descriptions only (14%) |
| `server_instructions` | 1,827 chars live | once per conversation | `source_md_under_cap` = 1900 ✅ |
| `get_guide` corpus (10 topics) | 90,485 B; 62,451 B auto-triggered | once per topic per session | **none** |
| `onboarding_prompt` + draft | 19,122 B + 2,128 B | once per project | none needed |

Within `tools/list`: descriptions 8,521 chars, schemas 50,361 chars (86%), of which
30,512 chars (61% of schema bytes) is param-description prose. The librarian family
(`artifact`, `librarian`, `artifact_augment`, `artifact_event`, `artifact_refresh`)
is 28,788 chars — 48.9% of the whole tool surface. `artifact` alone is 12,102 chars
across 51 params.

Guide corpus growth, measured with `git cat-file -s` per commit:
`tracker-conventions.md` went 6,804 B (2026-07-07) → 10,377 B (2026-08-16) →
23,252 B (2026-08-18) — 3.4× in six weeks, 2.2× in the last three days. The
byte-budget reasoning in `src/librarian/adapter.rs:197` and
`src/prompts/mod.rs:386` still cites the 10.4 KB figure, which was correct the day
it was written.

Both auto-injects were observed first-hand this session: `tracker-conventions`
(23.2 KB) fired on an `artifact(action="find")` whose results named `docs/trackers/`
paths, and `librarian` (20.5 KB) fired on the following `append_entry` — 43.8 KB,
~11k tokens, for one session that touched trackers.

---

## F-1 — `append_entry`'s `anchor_heading` is implemented but not advertised in the `artifact` schema

**Observed:** 2026-08-18, prompt-surface audit — reserving R-106 in `docs/trackers/reconnaissance-patterns.md`.

**When:** Called `artifact(action="append_entry", id=…, id_prefix="R")` to reserve an id, intending to hand-write the `## R-106 — …` section.

**Expected:** The `artifact` tool schema documents `append_entry` thoroughly — `entry`, `entry_collection`, `id_prefix` and `cites` are all described at length (the `entry_collection` description alone is 496 chars and walks through the prose-ledger reservation flow).

**Got:** The response's `next_step` read: *"Next time, pass `title`, `body` and `anchor_heading` to have the server write it and remove that failure mode entirely."* `anchor_heading` is **not a declared property** of the `artifact` input schema — verified against the live `tools/list` dump, which has 51 properties and no `anchor_heading`. `title` and `body` are declared, but described as `"create: artifact title"` / `"create: markdown body"`, with nothing connecting them to `append_entry`.

The parameter is real and load-bearing: `src/librarian/tools/append_entry.rs:34` declares `anchor_heading: Option<String>`; lines 112–132 require all three fields together and refuse a partial set naming what is missing; `src/librarian/catalog/augmentation.rs:836–1088` writes a `def_re`-conformant `## <ID> — <title>` heading at the ledger's own level, validates that the anchor exists verbatim, and writes nothing at all on a bad anchor (pinned by `a_bad_anchor_writes_nothing_at_all_not_even_the_high_water_mark`).

**Probable cause:** The `d3c1e6ed` / `f19d5296` / `758b37dc` line of work made uncitable entries a first-class concern and added the server-side section writer as the structural fix. The tool schema was not updated in the same commit, so the prompt-surface review that `src/prompts/README.md` mandates for "any change to tool behavior or signatures" did not run.

**Why this is the sharp end:** this is the one path that structurally *cannot* produce an uncitable entry, and it is discoverable only by first doing it the fallible way and reading the follow-up hint. Every agent that reserves-then-hand-writes is executing the exact failure mode the feature exists to remove. Measured cost of that mode, from the same work stream: 13 ledgers across five repos carrying entries their body never defines, the largest at 64 of 68, and one namespace resolving to nothing against 117 live citations.

**Workaround:** Use it regardless — it is accepted despite being undeclared. This entry was written with it, which is also the confirmation that the path works end to end.

**Severity:** high — a correctness feature invisible on the only surface an agent reads.

**Status:** fixed-verified — declared in `01194e21`, pinned by `server::tests::artifact_advertises_the_append_entry_section_writer` (mutation-verified: renaming the schema key makes it fail). Gate green, 4,164 passing.

**Fix idea / Pointer:** Declared `anchor_heading` in `Artifact::input_schema()` and re-scoped the `title`/`body` descriptions to name their `append_entry` role. Bug file: `docs/issues/archive/2026-08-18-append-entry-body-writer-undeclared-in-artifact-schema.md` (archived; the earlier pointer here named a slug that was never created). The +808 chars breached `TOOL_SURFACE_CHAR_BUDGET` and were paid by compressing the injected `workspace` description — see the spec, `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`.

## F-2 — Read wire duplication as source duplication — the `workspace` param is injected once, not authored 24×

**Observed:** 2026-08-18, prompt-surface audit, immediately after presenting the baseline measurement to the user.

**When:** Ranking compaction targets. I had dumped the live `tools/list` payload and run a duplicate-detector over every property description.

**Expected (what I asserted):** The `workspace` param's 225-char description appears verbatim on 24 tools — 5,400 chars, 5,175 of them redundant, "8.8% of all schema bytes, deletable by mechanical means alone". I ranked it as recommendation #2: *"mechanical, no eval needed, no judgment involved"* and *"a free 5.2 KB with zero eval risk"*.

**Got (scouted reality):** There is exactly **one** copy in the source. `src/server.rs:1024-1026` calls `CodeScoutServer::inject_workspace_param` for every tool whose `pinnable()` returns true, and that function (`src/server.rs:496-508`) inserts a single hard-coded `json!` block into the advertised schema at `list_tools` time, idempotently. The source is already DRY; the duplication is a property of the MCP wire format, where every tool carries its own complete schema.

**Probable cause:** I measured the generated artifact and inferred the shape of its generator. The measurement was correct — the bytes are genuinely paid 24× per request — but "duplicated" and "authored 24 times" are different claims, and only the second licenses "dedupe it".

**Consequence for the plan:** the remedy changes character entirely. There is nothing to deduplicate. The only lever is shortening the one shared string, which trades clarity on 24 tools for ~165 chars × 24 ≈ 3,960 chars — a content judgement with an eval cost, not a free mechanical win. A second, better lever the misread hid completely: audit which tools actually need `pinnable()`, since the cheapest byte is the one not injected.

**Workaround:** Re-ranked before any edit was made. `pinnable_tools_advertise_workspace_param` only asserts the property is present, so shortening the string will not break the gate.

**Severity:** med — no wrong edit shipped, but the recommendation reached the user with a confidence ("zero risk", "mechanical") the evidence did not support, and it would have produced a subagent brief with no valid implementation.

**Status:** fixed-verified — corrected before any edit; the generator is read and cited above.

**Fix idea / Pointer:** Cross-cutting lesson filed as R-106 in `docs/trackers/reconnaissance-patterns.md`. Counterfactual recorded as W-1.

## W-1 — Scouting the generator before editing a generated surface caught a recommendation with no implementation

**Observed:** 2026-08-18, prompt-surface audit. Recon invoked after the baseline report was delivered and before any compaction edit.

**Pattern:** When a compaction/refactor target is identified from a **generated** surface (an MCP `tools/list` payload, a rendered template, a compiled schema, an API response), read the code that generates it before proposing the remedy. Measuring the artifact establishes *cost*; only the generator establishes *authorship*, and the remedy follows from authorship.

**Counterfactual:** Without the scout, recommendation #2 — "dedupe the `workspace` param across 24 tools, free 5.2 KB, zero eval risk" — would have gone forward. There are three concrete costs it would have incurred:

1. **No valid implementation.** A subagent briefed to "remove the 23 duplicate copies" would have found one `json!` literal in `inject_workspace_param` and either stalled or invented a change nobody asked for. Per the project's own dispatch rule, that is a controller defect, not a subagent one.
2. **A miscommunicated risk profile.** It was sold to the user as the zero-judgement item to do first. It is in fact the item with the widest blast radius per byte — one string that renders into 24 advertised schemas.
3. **A better lever stayed hidden.** Reading `src/server.rs:1024` surfaced `t.pinnable()` as the actual gate. Auditing which tools genuinely need a workspace pin removes whole 259-byte blocks rather than trimming one shared sentence — strictly better, and invisible from the wire dump.

**Confirming data points:**
1. F-2 (this session) — wire-vs-source misread on `workspace`, caught pre-edit.
2. F-1 (this session) — the same scout pass, run against `append_entry`'s runtime hint rather than its schema, found `anchor_heading` implemented at `append_entry.rs:34` and absent from the advertised schema. Reading only the advertised surface would have concluded the hint was advertising a nonexistent param; reading only the source would have missed that it is unadvertised. **Both surfaces were needed to state the defect correctly** — which is the same lesson from the other direction.
3. Also this session: two auto-injected guides (`tracker-conventions` 23.2 KB, `librarian` 20.5 KB) were observed firing first-hand rather than inferred from `relevant_guide_topic()` source, confirming the 43.8 KB per tracker-touching session that the baseline predicted.

**Impact:** med — prevented one unimplementable recommendation and one mis-stated risk profile; surfaced a strictly better lever.

**Promote-when:** A second session catches a generated-surface-vs-generator inference error pre-edit. At two datapoints, promote to `docs/trackers/reconnaissance-patterns.md`'s seven-laws distillation as a named clause under law A ("Ground truth is the artifact") — specifically that a *generated* artifact is ground truth about cost and about nothing else.

**Status:** validated

## F-3 — 19.2% of the tool surface is bought for 38 calls, and the data cannot say whether those tools are dead or unrouted

**Observed:** 2026-08-18, tool-surface audit.

**When:** Ranking compaction targets after joining the live `tools/list` payload against `usage.db`.

**Got:** Ten tools carry **11,299 characters — 19.2% of the per-request surface — for 38 calls in 30 days**. Four have *zero* lifetime calls: `onboarding`, `approve_write`, `call_graph`, `library`.

**Why this is not actionable as it stands:** zero calls is ambiguous. Either the tool is dead weight, or nothing routes to it. `src/prompts/README.md` rule 7 states the second reading outright — *"if a tool has near-zero calls despite being useful, the prompt isn't surfacing it"* — and trimming on the first reading saves bytes while foreclosing the fix. The usage data cannot separate the two, and no amount of re-reading it will.

**Severity:** med — 19.2% of a per-request surface, but acting on the wrong reading is worse than waiting.

**Status:** open — the 19.2% question is NOT settled, but the field experiment below is **superseded, closed 2026-08-18**, and the reading has changed.

---

### Outcome of the field experiment: superseded before it started

Hamsa audit **A-26** ran the controlled arms the same day, and they answered in an hour
what this two-week field study would have spent a fortnight failing to detect.

| arm | named `call_graph` |
|---|---:|
| base (no quickref lines) | **0/10** |
| treatment (lines shipped) | **0/10** |
| positive control (MANDATORY directive) | **10/10** |

The routing intervention was reverted in `89d32048`. Two consequences for this entry:

**The hypothesis this entry pre-registered was the wrong one.** F-3 named three readings
of a null — unrouted, never-tempted, **substituted** — and tested only the first. The
arms point at the third: on a question where one hop is provably insufficient, twenty of
twenty runs reached for `references`, byte-identically, with and without the routing
line. That is a strong competing prior, not an ignorance of the tool — whose 1,060-char
description ships on every request and already says *callers (blast radius)*.

**The measurement design was also wrong in a way worth keeping.** This entry proposed a
naturalistic two-week window with `symbols` as a workload-presence control. That could
never have separated the three hypotheses either — it would have observed another null
and licensed nothing, exactly as the 30-day window before it did. A controlled arm with a
positive control settled it in three runs of ten. **Where a controlled arm is available,
a field window is the weaker instrument, not the more realistic one.**

What remains open is the original question for the other nine tools: 11,299 characters,
19.2% of the per-request surface, for 38 calls in 30 days. Nothing here authorises a trim
— a null still does not authorise a deletion — and the next move for any of them is a
controlled arm on a stimulus that tempts the tool, not another observational window.

---

### Pre-registration (written 2026-08-18, before any result)

**Hypothesis.** `call_graph` and `tree` score ~0 because `server_instructions` never routes to them, not because they are useless. The suggestive correlation: **no tool with zero lifetime calls is named anywhere in `server_instructions`.** `references` is named and has 129 calls; `call_graph` serves the same domain, is arguably more useful for impact analysis, is named nowhere, and has 0.

**Intervention.** Two quickref lines — `call_graph` and `tree` — landed in `ba16b16a`. Static slice 1,654 → 1,747 chars against its 1,900 cap. Live for the next **new** conversation against a rebuilt binary (`cargo rb`), not for a `/mcp` reconnect.

**Baseline, all four active `usage.db` files, to 2026-08-18:**

| project | call_graph | tree | references | symbols | total calls |
|---|---:|---:|---:|---:|---:|
| codescout | 0 | 10 | 129 | 2,614 | 25,826 |
| claude-plugins | 0 | 0 | 2 | 9 | 386 |
| prompt-engineering | 0 | 0 | 0 | 16 | 164 |
| researcher | 0 | 3 | 0 | 26 | 329 |

**`call_graph` is 0 across 26,705 calls in four projects** — including `researcher`, the most navigation-shaped workload available, whose second-most-used tool is `symbols`.

**Controls.** `symbols` is the workload-presence control (high volume: did navigation work happen at all in the window?). `references` is the same-domain control (already routed, so it calibrates what "routed and used" looks like). Neither is touched by the intervention.

**Metric.** Calls per tool with `called_at >= '2026-08-19'` — conservative, excluding the pre-change part of 2026-08-18.

```sql
SELECT tool_name, COUNT(*) FROM tool_calls
WHERE called_at >= '2026-08-19'
  AND tool_name IN ('call_graph','tree','references','symbols')
GROUP BY tool_name ORDER BY 2 DESC;
```

Run it against each of the four DBs; `codescout_sha` is available if the window needs tightening to specific builds.

**Decision rule, fixed in advance:**

| Outcome | Reading | Action |
|---|---|---|
| `call_graph` > 0 | it was **unrouted** | keep the lines; the 19.2% question resolves toward *route, don't trim*, and the other 8 tools get the same treatment before any trim |
| `call_graph` == 0 **and** `symbols` ≥ 200 | routed, navigation work happened, still unused | **evidence toward** dead weight — not a mandate; see the asymmetry below |
| `symbols` < 200 | the window contained no navigation workload | **inconclusive** — extend the window, do not conclude |

**Known weaknesses, stated up front rather than discovered later.** n ≈ 10 sessions over two weeks, one developer, one prompt surface, and 96.7% of all historical calls come from a single project. This design **cannot** separate "the tool is dead" from "this developer does not do impact analysis". It also cannot rule out that a quickref line is too weak an intervention where a worked example would not be.

**The asymmetry is the honest reading, and it is pre-committed:** a positive result is strong (something changed when only routing changed); a null result is weak. **A null must never on its own authorise removing a tool** — the same law as `reconnaissance-patterns` R-3 → R-79, this ledger's most-repeated: a search that finds nothing is evidence about the search, and a negative result does not authorise a deletion. Removal needs a positive finding, measured, and preferably a second independent signal.

**Fix idea / Pointer:** Re-run the query on or after 2026-09-02 and record the outcome against the table above. Spec: `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` § Revisit-when. Intervention commit `ba16b16a`; gate `598b92f2`.

## Template for new entries

<!-- Appends land above this line. Use:
     artifact(action="append_entry", id="<this artifact id>", id_prefix="F"|"W",
              title="...", body="...", anchor_heading="## Template for new entries")
     The server writes a def_re-conformant `## <ID> — <title>` heading. Add the
     matching Index / Wins Index row in the same session. -->
