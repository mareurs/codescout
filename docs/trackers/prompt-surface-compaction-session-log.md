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
entry_high_water_F: 6
entry_high_water_W: 6
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
| F-4 | 2026-08-18 | med | librarian-api | open | `anchor_heading` inserts only *before* a heading, so a ledger that appends at the end cannot use the server-writes path |
| F-5 | 2026-08-19 | high | self-friction | fixed-verified | A guard that names the invariant but never exercises it — forcing the gate open left 62/62 passing; "mutation-tested" is a per-ASSERTION claim, not a per-commit one |
| F-6 | 2026-08-19 | med | substrate-drift | fixed-verified | CAP-7 says check 3 needs no design — but `doctor` cannot reach the `[[project]]` list at all; two same-named `WorkspaceConfig` types, and a gitignored config that a worktree silently inherits from main |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-18 | med | Scout the generator before editing a generated surface | Would have shipped a "free mechanical dedup" with no valid implementation | validated |
| W-2 | 2026-08-18 | high | Do the cache arithmetic before scoping byte-shaving: a 100% cache_read surface costs its cache_read price, not its byte count | Would have sunk dozens of evals into `artifact`'s 51-param long tail to recover ~$0.0002/request, while the axis that might move (does the prose change behaviour?) went unmeasured | validated |
| W-3 | 2026-08-19 | high | Name the substrate before quoting a verdict — which tree, which binary, which index actually produced the number | Would have published 4222/1 as a gate result without knowing whether it described `HEAD` or a concurrent session's uncommitted mutant | validated |
| W-4 | 2026-08-19 | high | Calibrate a hand-built instrument against a known-good one on the overlapping population before extending it | Six instrument defects, each producing a plausible number and no error — 80% of scanned files were worktree duplicates, and the first patch-id test silently did nothing while exiting 0 | validated |
| W-5 | 2026-08-19 | high | When N records share a defect, fix the surface that instructed them before repairing any of them | Would have caveated 3 bug files while leaving 6 live instruction surfaces teaching the same rule to the next author | validated |
| W-6 | 2026-08-19 | high | Diff a tool's full report against the baseline a document recorded; account for every delta before reading the field you came for | Verification had already SUCCEEDED at its stated purpose — stopping there would have missed an unguarded write into a concurrent session's worktree, and left CAP-8's load-bearing "3 artifacts" figure standing while wrong | validated |
---

## Baseline measurement (2026-08-18)

Ground truth from driving `target/release/codescout start` over stdio with a real
`initialize` + `tools/list` handshake (XDG dirs redirected to a scratchpad so the
probe could not touch the real guide ledger). Recorded here so later compaction
passes have a comparable baseline.

| Surface | Size | Frequency | Size gate |
|---|---|---|---|
| `tools/list` (27 tools) | 58,882 chars — **over-counted, see correction below** | every request | descriptions only (14%) |
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

**Correction (same day) — take the tool-surface number from the harness, never from a
scratch probe.** The 58,882 above came from a Python probe that re-serialised the payload
with `json.dumps` at its default `ensure_ascii=True`, which expands every em-dash into a
six-character ASCII escape. The over-count therefore tracked prose density, and every
per-tool delta came out a multiple of 5. The authoritative figure comes from
`tool_surface_report_lengths` in `src/server.rs`, which reads the same `serde_json` bytes
the wire carries: **58,572** at the time of this baseline. The per-family splits in the
paragraph above inherit the same inflation and should be re-read from the harness rather
than trusted here.

The surface has moved twice since, both times deliberately and both times with the budget
constant ratcheted to the new total rather than left slack: 58,572 → 57,148 (declaring
`anchor_heading`, +808; compressing the injected `workspace` description, −2,232), then
57,148 → **56,266** (hamsa A-27's cut of five per-field restatements, −882). Run the
harness for the current number.

See [[W-2]] for why the byte total is the *wrong axis* to scope compaction on in the
first place — the surface is 100% `cache_read`, so all 56,266 characters cost ~$0.0043
per request.

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

## F-4 — `anchor_heading` inserts only *before* a heading, so a ledger that appends at the end cannot use the server-writes path

**Observed:** 2026-08-18, pre-registering A-27 into
`docs/trackers/prompt-hamsa-audit-log.md` — the same ledger A-26 went into, using the
`title` + `body` + `anchor_heading` path shipped in `01194e21` (the F-1 fix).

**Expected:** the server writes the `## A-27 — <title>` section itself, which is the
whole point of that path — a hand-written heading missing its dash-and-title defines
no token under `link_scan`, so every citation of the entry dangles.

**Got:** no usable anchor. `anchor_heading` inserts the new section **before** the named
heading (`append_entry.rs:23`), and `## A-26 — …` is the *last* heading in the file
(line 594 of 701; `awk 'NR>594 && /^#{1,4} /'` returns nothing). An append-only ledger
with no trailing section has nothing after the insertion point to name.

**Probable cause:** not a defect in the safety reasoning — `append_entry.rs:106-110`
refuses to infer placement on purpose, citing
`docs/adrs/2026-07-10-repair-and-continue-input-handling.md` ("a write accepts an
explicit target and never infers one"). The gap is that *append at end* is an
**explicit** target too, and the API has no way to say it. The trio-or-nothing guard
then makes the path all-or-nothing rather than degrading.

**Workaround:** the two-step path — `append_entry` with `entry_collection` only (row +
id + high-water mark), then `artifact(action="update", patch={body_edits: [{heading:
"## A-26 — …", action: "insert_after", content: "## A-27 — …"}]})`. This is what A-26
itself used, and what A-27 used. It works, but it hand-writes the heading, which is
exactly the failure mode F-1's fix existed to remove.

**Severity:** med — the safe path is unavailable on precisely the ledger shape that
needs it most (append-only, no trailing template). `prompt-hamsa-audit-log.md` has 26
entries and no trailing section; `prompt-surface-compaction-session-log.md` has
`## Template for new entries` and is therefore fine, which is why F-1's fix verified
green against one ledger and not the other.

**Status:** open

**Fix idea / Pointer:** accept an explicit end-of-body sentinel rather than inferring —
e.g. `anchor_heading: "$end"`, or a sibling `anchor_position: "end"` — so placement
stays caller-stated and the ADR's law is honoured. Cheaper alternative with no schema
cost: none, since the trio guard is what forces the anchor. Note the schema line
already spends 466 characters on this field, so a sentinel is documentable in ~40 more.
Contrast [[F-1]], which shipped this path.

## W-2 — Compression cannot be justified on cost — the surface is 100% cache_read, so the case is legibility and must be measured like one

**Observed:** 2026-08-18, deciding what to compact after the budget gate shipped. The
obvious next move was "the surface is 57,148 characters, cut the biggest tools" — and the
cost arithmetic says that is close to pointless.

**Pattern.** Before scoping any byte-shaving work on a cached surface, do the cost
arithmetic first and state it out loud:

| quantity | value |
|---|---|
| whole tool surface | 57,148 chars ≈ 14,300 tokens |
| observed cache hit rate | **100.0%** across 4 sessions / 3 models |
| cache_read price | $0.30/M |
| **cost of the entire surface** | **~$0.0043 per request** |
| cost of the 882 chars A-27 cut | **~$0.00006 per request** |

The same measurement that refuted *relocation* (moving schema prose into a `get_guide`
topic — break-even K ≈ 12.5 turns, so a cached surface always wins) also undercuts
*compression for cost*. A 100%-cache_read surface is the cheapest place bytes can live.

So the case for cutting is **legibility**, not economics — and legibility is a claim
about how a reader behaves, which is exactly the class of claim that needs an eval rather
than an assertion. That reframing is what made A-27 worth running: not to save 882 bytes,
but to answer a question that generalizes to every schema in the repo — *does per-field
restatement of a global rule improve parameter selection, or is it cargo cult?*

**Counterfactual.** Without this arithmetic the natural plan was a long-tail sweep of
`artifact`'s 51 parameters (12,727 chars, no single fat target), each cut needing its own
arm under P-4's inverted burden. That is dozens of evals to recover a few thousand
characters worth ~$0.0002 per request — effort spent on the axis that does not move,
while the one that might (does the prose change behaviour?) goes unmeasured. A-27 got a
transferable answer out of one tool instead.

**Confirming data points:**
1. A-27 (2026-08-18) — 882 chars cut; the byte win is explicitly disclaimed in the
   pre-registration, and the finding that shipped was the mechanism, not the bytes.
2. The refuted relocation proposal, same session — same cache measurement, opposite
   intervention, same conclusion.

**Impact:** high — it redirects the whole work stream from byte-shaving to
behaviour-measuring, and it is the reason [[F-3]]'s remaining 19.2% question should not
be answered by deletion.

**Promote-when:** a third compaction decision is scoped by this arithmetic rather than by
byte count. At that point promote to `docs/PROGRESSIVE_DISCOVERABILITY.md` as a sizing
rule: *a cached surface's cost is its cache_read price, not its byte count — justify cuts
on legibility and measure them.*

**Status:** validated — two datapoints, both this session, both pointing the same way.
Awaiting the promotion criterion.

## F-5 — A guard that names the invariant but never exercises it — "I mutation-tested it" is a per-ASSERTION claim, not a per-commit one

**Observed:** 2026-08-19, reviewing `b3161def` (the A-29 pin notice) before running any
arm. The commit shipped four assertions and its message reported "mutation-verified …
3 of 3 killed".

**Expected:** `switch_away_hint_carries_no_pin_notice_while_the_gate_is_shut` guards the
default-off invariant — that an unmeasured intervention cannot reach production.

**Got:** it guarded nothing. It checked the gate parser and that the suffix string was
non-empty; it never touched the composed hint. **Forcing
`workspace_pin_notice_enabled()` to return `true` unconditionally left all 62 tests in
the module passing.** The neighbouring `activate_hint_shows_switched_when_away_from_home`
cannot catch it either — it asserts `contains`, which an appended suffix does not disturb.

**Probable cause — and the part worth carrying forward.** The commit *did* mutation-test
three assertions, honestly and successfully. The fourth was the one whose mechanism I had
not thought through, and it is exactly the one that got a coverage claim instead of a
mutation. **Mutation-testing is a claim about an ASSERTION, not about a commit**; reporting
"3 of 3 killed" alongside a fourth untested assertion reads as full coverage and is how a
green bar acquires authority. The tell was available and I did not look: the three tested
assertions each named a *string*, and the untested one named a *behaviour* — and only the
behaviour needed a real call to observe.

**Second defect, found by the same fix.** Once the guard drove a real activation, its
failure message printed the composed hint — which read *"remember to `workspace(...)` when
done"* immediately followed by *"do not activate"*. A flat contradiction, invisible in
source because the two halves sit ~200 lines apart and each is fine alone. It would have
shipped into A-29's arms and measured a muddled instruction while still returning a number.
**Reading the artifact the user actually receives is a different act from reading the code
that builds it**, and only the first one finds this class.

**Workaround:** none needed — both fixed in `b8f6200a`. The guard now drives a real
activation and asserts on the returned hint; the intervention text conditions both
instructions instead of appending one after the other.

**Severity:** high — not for the bug's blast radius (the gate was shut, so nothing
shipped) but for the *method*. An unmeasured prompt intervention reaching production with
a green suite is the precise failure the inverted guards on A-25/A-26/A-27 exist to
prevent, and the guard that was supposed to prevent it was itself the hole.

**Status:** fixed-verified — mutation re-applied after the fix; the new guard fails,
naming the leaked marker.

**Fix idea / Pointer:** two habits, both cheap:

1. **Mutate the invariant, not the helper.** If a guard's name says "X cannot reach
   production", the mutation is *make X reach production* — not *break the parser X reads*.
   If the whole suite still passes, the guard is decorative.
2. **Assert on the artifact the caller receives.** For anything composed from parts
   (hints, prompts, rendered surfaces), a test over the parts cannot see contradiction
   between them. Related: [[W-2]] on measuring the wrong axis, and the same lesson at
   harness level as `prompt-engineering:OP-5`, where a checker missing its exec bit
   reports a clean `0/N` that is character-identical to a genuine floor.

## W-3 — Name the substrate before quoting the verdict — which tree did the gate actually run against?

**Observed:** 2026-08-19, discharging the archived truncation bug's gate-1 condition
(full `cargo test` green on `experiments`).

**Pattern:** Before quoting a test result, a tool report, or a diagnostic count as
*verification*, establish which **substrate** produced it — which tree, which binary,
which index. Check it before *and* after, and read the run's own build lines rather than
assuming continuity across two commands.

**Counterfactual:** A concurrent session held an uncommitted mutation in
`src/tools/config/mod.rs` (hoisting `Agent::activate` above the `switched` computation,
which would pin `switched` false forever). It was present when `cargo test link_scan`
compiled, and reverted before `cargo test --no-fail-fast` ran. Both runs returned
clean-looking numbers and neither mentioned the other's existence. Without the
before/after `git status` plus reading `Compiling …; Finished in 1.42s` out of the run's
own buffer, I would have published **4222 passed / 1 failed** as a gate result while not
knowing whether it described `HEAD` or a mutant.

Note that the two most natural wrong claims were both *available* and both *plausible*:

- *"gate green on my fix"* — right number, unverified tree;
- *"M8 survived the whole suite"* — a coverage claim about someone else's work, drawn
  from a run that never contained their mutation.

The second is the more dangerous, because it is generous-sounding, concerns a colleague's
code, and would have been offered as a favour.

**Confirming data points:**

1. This session — the mutation was present at run 1's compile and absent at run 2's; only
   the recompile line distinguishes them, and nothing surfaces it unprompted.
2. The archived truncation bug's own gate-2 condition exists for exactly this reason:
   `link_scan` executes *inside* the MCP server, so which binary answered had to be
   established before its output could count as evidence in either direction. The bug file
   warns in as many words against reading a stale row as a failed fix.

**Impact:** high — the failure mode emits a **number, never an error**, and the number is
perfectly correct about a tree nobody has. Nothing downstream can detect it, because
there is no defect in the artifact to find.

**Promote-when:** a third instance where a verification's substrate (tree, binary,
database, index) turns out to differ from the one assumed. At 3 datapoints, promote to
`CLAUDE.md` alongside the mutation-apply discipline, as: *name the substrate before
quoting the verdict.* The two rules are the same rule seen from opposite ends — one says
a green bar proves nothing until you mutate it, the other says a measurement proves
nothing until you know what it measured.

**Status:** validated — two datapoints, both in this work stream.

## W-4 — Calibrate a hand-built instrument against a known-good one before believing any number it produces

**Observed:** 2026-08-19, sizing the payoff of the content-addressed-identity proposal
(CAP-8) across 10 repos in 2 umbrellas.

**Pattern:** When you build an ad-hoc measuring instrument (a script, a regex, a walk) and
a **known-good** instrument already covers part of the same population, run both on the
overlap and compare *before* extending yours to the part it does not cover. A ratio near 1
licenses the extension; a large ratio localises what you got wrong.

**Counterfactual:** My scan reported codescout at 7261 ambiguous citations against
`link_scan`'s 423 — **17×**. Every intermediate number I had produced up to that point was
wrong, and none of them looked wrong. Six distinct instrument defects, each found only by
checking:

1. `.claude/worktrees/` not excluded — **80% of codescout's 6858 markdown files were 5–6
   copies of the same content**. This alone accounted for most of the gap.
2. The noise-prefix regex included single-letter prefixes (`T-`, `M-`, `N-`), which ate
   *genuine* ledger tokens. "9% noise" was really **1%**.
3. Self-references counted as breakage — **35% of all mentions**. `link_scan` has a test
   named `self_citation_wins_even_with_other_definers`; a file citing its own entry
   resolves fine.
4. Fenced code blocks not stripped, so quoted examples counted as citations.
5. The archived-loses-to-active tie-break unmodelled, overstating ambiguity.
6. In the *fix* for a stale count, I nearly wrote 16 where recounting gave **17** — and the
   recount also revealed my checking regex had false negatives (`S-NN` and `HY-10` do not
   match a `PREFIX-N` pattern).

After the fixes: self-cites 1.27×, dangling 1.19×, ambiguous 1.73×. **Two of three
converging is what licensed reporting the third** — a uniform 5× gap would have meant a
population mismatch, but two matching while one stays high localises the residual to
something specific (link_scan's dangling is prefix-gated to declared ledgers, and it scans
1072 catalogued artifacts rather than the filesystem).

**Confirming data points:**

1. This session — six defects, zero found by re-reading the script, all six by comparing
   its output to something independent.
2. The `git patch-id` rebase-invariance test: the first run silently did nothing because
   `git cherry-pick` has no `-q` flag, and `exit_code` was **0** because the last command in
   the chain succeeded. The result — "patch-id ALSO CHANGED" — was a broken instrument, not
   a finding, and would have killed a correct design.

**Impact:** high — every one of these produced a *number*, never an error. A wrong number
is publishable, citable, and compounds: it becomes the denominator of the next decision.

**Promote-when:** a third work stream where a hand-built measurement diverged from a
known-good instrument on the overlap. At 3, promote to `CLAUDE.md` next to the
mutation-apply discipline — the three are one rule: **a green bar, a measurement, and a
tool's output all need an independent check before they carry weight.** Pairs with
`prompt-surface-compaction-session-log:W-3` (name the substrate before quoting the verdict);
that one is about *which world* was measured, this one about *whether the ruler is straight*.

**Status:** validated — 2 datapoints this session, both with the wrong answer available and
plausible.

## W-5 — When several records make the same mistake, fix the generator — the records were obeying it

**Observed:** 2026-08-19, repairing three bug files that sat unarchived waiting on a
master-side SHA a fast-forward promotion would never mint.

**Pattern:** When N records share a defect, **read the surface that told them to do it
before repairing any of them.** Repairing the instances leaves the generator running; the
next author reproduces the defect faithfully, and the repair looks like carelessness on
their part rather than a working instruction.

**Counterfactual:** I had already caveated the three files and was about to move on. Marius
said *"lets fix taxonomy first"*, and the generator was there in two places:

- `docs/issues/_TEMPLATE.md` **contradicted itself** — its comment block carried the correct
  cherry-pick/fast-forward table, while its `## Fix` section, the prose an author actually
  reads *while writing that section*, said unconditionally to "list **master-side** commit
  SHAs".
- `docs/TAXONOMY.md`'s BUG row said the same thing with **no condition at all**.

The three authors were not careless. They read the instruction and followed it. Repairing
only the three files would have left both surfaces teaching the same thing to the next
author, and this session's other measurement says nothing re-reads a bug file once written.

The sweep found more than expected: **six** live instruction surfaces carried the rule
(`CLAUDE.md`, `docs/RELEASE.md`, `docs/TAXONOMY.md`, `_TEMPLATE.md`, the
`tracker-conventions` guide, and memory `gotchas`, which `RELEASE.md` names as the rule's
concise home). Two leaks appeared *inside my own edits*: a RELEASE.md code block left
contradicting the paragraph I had just rewritten above it, and a TAXONOMY section
(`## SHA-citation rule`) I had never opened, which taught the old rule and cited a CLAUDE.md
heading I had renamed forty minutes earlier — which then broke two further live citations.

**Confirming data points:**

1. This session — 3 instances, 6 generator surfaces, 2 self-inflicted leaks during the
   sweep itself.
2. `docs/trackers/reconnaissance-patterns.md`'s standing law that a declared consolidation
   leaks at call sites nobody swept: *declaring the law is cheap, the whole-tree sweep is
   the actual work, and the sweep is what gets skipped.* Same shape, arriving from the
   documentation side rather than the code side.

**Impact:** high — instance-repair on a live generator has a *negative* half-life: the pile
regrows while the repair makes it look handled.

**Promote-when:** a third instance where repairing records without repairing their source
document would have regenerated the defect. At 3, promote to `CLAUDE.md` as: *before
repairing N records that share a defect, find and fix the surface that instructed them —
then sweep every surface carrying the same rule, including the memories.*

**Status:** validated — 2 datapoints, one of them this session's own edit leaking twice
while sweeping for exactly this failure mode.

## F-6 — CAP-7 says check 3 needs no design — but `doctor` cannot reach the `[[project]]` list at all

**Observed:** 2026-08-19, post-compaction scout before implementing CAP-7 check 3
(`declared_root_missing`) in `docs/trackers/capability-proposals.md`.

**When:** Phase 1 reconnaissance, before writing any code — triggered by that tracker's own
augmentation prompt, which says the `Substrate check` line is the load-bearing part and must
be re-verified before acting.

**Expected (CAP-7 substrate check):** *"**Check 3 is fully specified already**, by the bug it
would have caught … names the assertion, the report shape, and says to site it next to
`abs_path_outside_managed_roots`. Nothing needs designing."* And separately: *"**Genuinely
missing:** no check reads bug-file frontmatter at all, and nothing resolves a git SHA"* —
naming git as the only new dependency, for check 1.

**Got (scouted reality):** the *assertion* is specified. The *substrate* is not, in two ways
neither CAP-7 nor the bug file mentions:

1. **`doctor` has no handle to the project list.** Its `ctx.workspace` is
   `crate::librarian::workspace::WorkspaceConfig` (`src/librarian/workspace.rs:9`), which
   carries `.roots` and umbrellas. The `[[project]]` entries live on a **different type with
   the same name** — `crate::config::workspace::WorkspaceConfig`
   (`src/config/workspace.rs:4-12`), whose `projects: Vec<ProjectEntry>` holds the
   `{id, root}` pairs (`src/config/workspace.rs:39-46`). Nothing threads the latter into
   `ToolContext` (`src/librarian/tools/mod.rs:84-103`). The check must therefore locate,
   read and parse `.codescout/workspace.toml` itself — precedent at
   `src/tools/config/mod.rs:511-515` — or the plumbing must change. A same-named type on
   the context field is exactly the shape that reads as "already available".

2. **The worktree case is a live decision, not an edge case.** The config is gitignored, so
   it does not travel into a linked worktree; when it is absent there, discovery silently
   falls back to the **main** checkout's settings, a state the code already names
   `topology: "inherited"` (`src/tools/config/mod.rs:939-948`). So "the active workspace
   config" is ambiguous in precisely the situation this repo is usually in — five linked
   worktrees exist right now. Which root the declared `root` resolves against, and whether
   an inherited config should be checked at all, has to be decided before the check can
   report anything trustworthy.

**Probable cause:** the substrate check was written from the *bug file*, which specifies the
assertion completely and correctly, and not from `doctor`'s context type. The bug is a
config-state defect, so its author had no reason to look at what a librarian tool can reach.
Two structurally different types sharing the name `WorkspaceConfig` is what let
"`ctx.workspace`" read as sufficient without opening it.

**Workaround:** implement check 3 as a self-contained config read (locate via
`crate::config::workspace::workspace_config_path`, parse, iterate `projects`) rather than off
`ctx`, and record the worktree resolution as a fourth open decision on CAP-7 rather than
deciding it silently inside the check.

**Severity:** med — would not have cascaded, but the first implementation attempt reaches for
`ctx.workspace.projects`, which does not compile, and then has to guess at root resolution
with the worktree fallback invisible. Under subagent dispatch that is at least one failed
task the controller absorbs; the worktree half could have shipped as a wrong-but-green check.

**Status:** fixed-verified — CAP-7's substrate check corrected in the same commit as this
entry, before any code was written.

**Fix idea / Pointer:** CAP-7 in `docs/trackers/capability-proposals.md`;
`docs/issues/2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md`
§ Resume. The general lesson is W-4's, one level up: *a name is not a calibration*. CAP-8's
method note asks the next author to query the catalog for prior art before proposing; this
asks them to open the **context type** before declaring a check needs no design.

## W-6 — Diff a tool's report against the baseline a document recorded — the deltas are the findings

**Observed:** 2026-08-19, verifying that a newly-shipped `doctor` check (`f632e7ef`) was
live on the rebuilt binary after `/mcp` reconnect.

**Pattern:** when a diagnostic tool reports a *set* of counts and some document already
records an earlier run of the same tool, do not just read the field you came for.
**Diff the whole count vector against the recorded one, and account for every delta.**
The field you came to check is the one you already believe; the deltas are the only part
carrying information you do not have.

Concretely: CAP-7's substrate check had recorded a `by_check` vector from a run earlier
the same day. The verification run — whose only purpose was confirming `declared_roots`
appeared — differed in three entries:

| check | recorded | live |
|---|---:|---:|
| `frontmatter_id_mismatch` | 3 | **4** |
| `missing_file` | 2 | **1** |
| `worktree_scoped_row` | 2 | **3** |

Two of the three were one event. Artifact `aeece182252e710d` fired both new rows: a
backend-kotlin plan created that day in a *linked worktree*, whose
`worktree_scoped_row` detail carried `collision_with: "8dcbd4fcb9fd5ffc"` — **the same
value its frontmatter declared**. The shadow's id was correct; the check calling it *"a
move re-keys the row and this file kept the id it was moved away from"* was wrong.

**Counterfactual.** The verification I set out to run was `declared_roots` present, config
path named, `declared: 1, missing: 0` — and it passed. Stopping there was the natural end
of the task, and it would have read as a completely clean result. Three things would have
been missed:

1. `frontmatter_id_mismatch` conflates stale-after-move ids with live worktree shadows.
2. `fix=repair_frontmatter_id` filters only on `containing_root`, so it would **write to a
   file inside another session's active worktree** — while both sibling fixes
   (`reseat_worktree`, `prune_missing`) carry an active-registration guard and document
   why. The scope comment on the unguarded arm reasons carefully about cross-*repo* blast
   radius, measured from a real 207-file incident; the cross-*worktree* axis was never
   considered. One axis closed, one open, in the same function.
3. CAP-8's substrate check rests on *"the **3** that differ are the only artifacts carrying
   evidence of a prior identity"* — the figure its entire "invert stored-vs-derived ids"
   argument is built on. It is 4, and the population is contaminated, so the real figure is
   unknown until the check is fixed. That claim was written hours earlier and was already
   wrong.

Filed, fixed and archived the same day —
`docs/issues/archive/2026-08-19-repair-frontmatter-id-rewrites-files-in-registered-worktrees.md`,
`f772b8fe`.

**Why this is not just "read the whole output".** The deltas were legible *only* because a
prior run's vector had been written down. A count vector with nothing to compare it to
reads as a description of the world; the same vector against a baseline reads as a set of
events. That is the cheap part of this pattern — CAP-7's substrate check recorded its
`doctor` run in full rather than the one number it needed, and that decision is what made
this findable four hours later.

**Confirming data points:**

1. This session — three deltas, one live write-path defect, one falsified tracker claim.
2. W-3 (same log) — the substrate question, one level down: *which tree produced this
   number*. This is the sequel: *what did the same tree say last time*.

**Impact:** high — a fix that writes into a concurrent session's working tree, found by a
verification step that had already succeeded at its stated purpose.

**Promote-when:** a second instance where diffing a tool's full report against a recorded
baseline surfaces something the targeted check did not. At 2 datapoints, promote to
CLAUDE.md as: *"When re-running a diagnostic a tracker has already recorded, diff the full
report against the recorded one and account for every delta before reading the field you
came for."* Pairs with the verify-open cadence, which is the same discipline over a slower
clock.

**Status:** validated — single datapoint, defect filed and tracker claim corrected in the
same pass. Awaiting the promotion criterion.

## Template for new entries

<!-- Appends land above this line. Use:
     artifact(action="append_entry", id="<this artifact id>", id_prefix="F"|"W",
              title="...", body="...", anchor_heading="## Template for new entries")
     The server writes a def_re-conformant `## <ID> — <title>` heading. Add the
     matching Index / Wins Index row in the same session. -->
