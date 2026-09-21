---
id: a15ff70c1118a9af
kind: research
status: draft
title: 'Recent usage.db and Claude trace reconstruction: delivery, omission, and rereads'
tags:
- telemetry
- deep-agent
- retrospective-evidence
---

# Recent usage.db and Claude trace reconstruction

**Valid:** dated 2026-09-21

Retrospective enrichment, deliberately selected examples; not a prospective sample, prevalence estimate, or evaluation of the latest binary. These episodes occurred on 17 and 20 September, inside the last-week window. No counterfactual intervention was run. No implementation is authorized by this report.

## Evidence and joining method

Read-only SQLite queries against the project's usage database supplied recorded inputs, outputs, outcomes and build stamps. The [claude-traces reader](../../.claude/skills/claude-traces/scripts/cc.py), through its `tool-calls --format json` and `trace --slim` commands, supplied tool-use IDs, arguments, timestamps and public assistant statements. Raw conversation JSONL was not parsed separately. Langfuse was not used to verify these episodes; system-prompt completeness and API-side ingestion remain unverified.

The join is reconstructed from Claude session, tool name, exact parsed arguments and compatible timestamps, with sequence as a cross-check. It is **not a native shared-ID join**. The current tool_calls schema has none of the columns tool_use_id, request_id, trace_id or tool_call_id. Identical reread arguments produce multiple transcript candidates; timestamps disambiguate the selected pair. Usage timestamps in these historical rows are second-resolution completion times, not reliable subsecond ordering keys.

The Kat export ends at 2026-09-21T06:08:24.225Z. Its usage export contains that principal's rows through id 133528 (06:07:05.635 UTC); absence claims below explicitly concern exported inputs, not an ended session. The reread candidate search used 2026-09-14 00:00:00 <= called_at < 2026-09-21 06:10:00 UTC. This is a case study, not a recomputation of weekly rates.

Local scratch exports in /tmp are disposable; the table anchors and sanitized observations below are the durable evidence summary. Retention of the underlying database/transcripts limits later replay.

## Retrieved overflow: fetching a handle is not yet receiving its body

**Principal:** Claude Kat, session 571eb3d6-c879-43f6-b3f9-5a51e744e1af. Codescout process 697cb24c-9812-4a30-b722-3a3d1276cfba; recorded binary b53a3ffc, project b53a3ff, codescout_dirty=0.

**Task context:** the user asked about Jev and then a first-party System 1 architecture. The assistant publicly announced that it would pull the existing design and inspect code. This establishes the declared task, not hidden reasoning or correctness of the eventual design.

| usage row | UTC, 20 September | Transcript tool-use ID | Observed event |
|---|---|---|---|
| 132481 | 10:03:01 | toolu_01KQA9sEmcvLR4Wuye7WCmU4 | doc get, full=true, design artifact d16552e9981f521e; emits @tool_be44e73b |
| 132493 | 10:06:17 | toolu_01Vx78SZbVTtQiSucs1tjUo7 | Extract $.body from that handle; response names @file_be47e558 and 101 lines, without delivering the body |
| 132494 | 10:06:21 | toolu_01NqrpoWZfsQik9UHZ4TBq4K | Requests lines 1–101; response says 27 of 101 shown and next start 28 |
| 132495 | 10:06:26 | toolu_01T1Phecq6et8EpQ4EzG7hpN | Requests 28–101; response says 36 shown and next start 65 |
| 132496 | 10:06:29 | toolu_019tfRMrjWN4qz39cqAQvjHY | Requests 65–101; response says 19 shown and next start 85 |
| 132497 | 10:06:32 | toolu_01QMo3wP3WEBgj44KdxMcgKa | Requests 85–101; receives concluding design text |

**Verdict:** verified same-handle retrieval chain and multiple delivered body segments. The first matching handle call alone delivered a pointer. Requested line ranges also overstate what individual responses delivered. No byte-for-byte completeness oracle was run; do not turn this into a claim that every line was seen, understood or used. All these calls recorded success, which is transport/tool outcome only.

**Missing debug evidence:** delivered segment identity and source revision, explicit continuation/completeness state, and an optional agent report identifying what was needed for the decision. A retrieval edge should distinguish pointer extraction from content delivery.

## Unretrieved handle: omission is observable, necessity remains unknown

**Same Kat principal and build.** Row 132500, 20 September 10:07:01 UTC, memory read of architecture, corresponds to toolu_018fyTzTLVhreKvm1khohDtg at 10:07:00.443Z. The response emits @tool_be488ee8 and an architecture summary.

There are **zero exported tool-call input objects containing that exact handle** through the Kat export's final timestamp above. This literal predicate covers the whole exported main-session call list, not merely the next call. It does not exclude indirect access, inherited context, alternative retrieval of the same facts, or activity in another principal.

The immediately following recorded calls inspect ENGINES (132501), run_post_in (132502), and search for the coordination design (132503). The public trace explicitly says the assistant is grounding the design in code, then states a conclusion about the coordinator. That conclusion itself was not independently re-evaluated in this reconstruction.

**Verdict:** no exact-handle retrieval observed within the stated scope; task impact unknown. The sequence is compatible with the summary sufficing, with substituting code evidence, and with a context omission. It does not discriminate among them. Do not label this a failure or a successful optimization.

**Missing debug evidence:** an explicit, optional reason such as summary sufficient, alternative source used (with reference), not relevant, deferred, blocked, or unknown. This would be an agent report requiring adjudication, not proof that nothing was missed.

## Identical reread after edits: not evidence of waste

**Principal:** Claude SDD, session 458a8a26-c380-4f5b-b967-2181f592917e. Recorded binary 5a682c9d and project 37d47fe for the four anchor rows. These are historical build stamps; do not use this case to assert current-binary behavior.

Target: [embedding configuration manual](../manual/src/configuration/embeddings.md).

| usage row | UTC, 17 September | Transcript tool-use ID | Observed event |
|---|---|---|---|
| 128895 | 04:23:38 | toolu_01GGpwofLzhVPC7Qv5vG3UT6 | read_file with only the target path; response previews the old deprecation banner |
| 129499 | 07:56:58 | toolu_01KgMLPkUaEb4ovNcJEMyWT7 | Edit replaces that banner; recorded response status ok |
| 129611 | 08:38:17 | toolu_017tU4z1946EWRGshgS4Wg65 | Edit rewrites Environment Variables; recorded response status ok |
| 129706 | 09:25:03 | toolu_014voi4kARfPrPggLzkom3JT | Identical path-only read; now returns a 318-line heading map |

The trace publicly announces a documentation task at 09:24:52. Row 129707 then reads Choosing a Model and Model Recommendations; row 129712 records a successful subsequent edit to this manual. This supports a fresh navigation/read/edit episode, not a demonstrated post-edit verification of the earlier banner.

**Verdict:** identical arguments with two confirmed intervening same-file edits and different recorded responses. This is a counterexample to equating repeated argument signatures with redundant work. Output inequality alone would not suffice: generated handles and output shape can differ without source content changing. Here the successful edits are the additional evidence. Necessity of every read and correctness of the resulting manual were not evaluated.

**Missing debug evidence:** content revision at read time, intervening write linkage, and optional read purpose (navigation, refresh, verification, other). A git project SHA alone cannot identify an uncommitted file revision.

## What to reuse and what is still missing

The current pika_observations table was inspected live: it has a tool_call_id foreign key, kinds iron_law/tool_bug/misusage/pattern, verdicts slip/habit/promoted/rejected, notes and links to existing trackers. It held zero rows at inspection. That is a measurement of this table, not a claim that the project's friction trackers are empty.

Reuse that audit infrastructure for adjudicated friction observations and existing context/workflow trackers for decision evidence. Its present verdict vocabulary does not directly express summary-sufficient, useful refresh, deferred, or unknown; do not force ordinary positive/unknown cases into defect labels.

The smallest next design should address three demonstrated gaps:

1. **Cross-source identity:** a correlation key visible both in the tool response/trace and the persisted call, with principal and server-process scope. Until then, store join method and ambiguity explicitly.
2. **Delivery lineage:** producer call → handle → extraction/child handle → delivered segment, source revision and censoring boundary. Keep delivery, content coverage and agent use separate.
3. **Optional agent annotation:** link the relevant call/result, state required scope, sufficiency (yes/no/unknown), reason and evidence. Store it as self-report, distinct from observed events and later review. Ask for the needed decision scope, not the unanswerable global assertion that all possible context was extracted.

These are recommendations from three deliberately selected cases, not a finalized schema or implementation plan. They do not establish which interventions improve task outcomes. No costs, tokens saved, or failure rates were inferred.

Related question: [overflow retrieval handoff](../trackers/2026-09-20-handoff-overflow-retrieval-discriminator.md). Related design: [deep-agent integration](../trackers/local-semantic-evaluator-design.md). This reconstruction narrows that question but does not resolve semantic necessity; the handoff remains open.
