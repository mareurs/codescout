---
id: a48d9c355aec013c
kind: tracker
status: active
title: Codex — telemetry exploration and research handoff for peer synchronization
owners:
- codex
tags:
- codex
- handoff
- telemetry
- deep-agent
topic: deep-agent-telemetry
---

# Codex — telemetry exploration and research handoff

**Valid:** dated 2026-09-21

**Author:** Codex, session 01a0be18-827e-7a72-b5c9-1bcbebeedac2.
**State:** exploration saved; awaiting comparison with the parallel agent's independent report.
**User request:** preserve this session's exploration/research for synchronization, with codex in the tracker name. The user will pass the pointer to the peer. No peer contact or implementation is implied.

## Start here

1. Read [the reconstructed episodes](../research/2026-09-21-usage-trace-reconstruction.md), artifact a15ff70c1118a9af. This is the detailed evidence: usage row IDs, Claude tool-use IDs, timestamps, builds, observed results and explicit unknowns.
2. Use [the frozen weekly measurement snapshot](../research/2026-09-21-codex-telemetry-week-snapshot.json) for this session's passive baseline. It was copied intact from the exploration's scratch output so it no longer depends on /tmp.
3. Compare these findings with the peer report before choosing an instrumentation change. Keep differing windows, builds, principal coverage and definitions visible.

## Why we explored this

The conversation started with Jev/TypeSafe and System 1, then moved to what codescout actually needs to observe before choosing a classifier or autonomous workflow. Existing background: [local semantic evaluator review](../research/2026-09-18-local-semantic-evaluator-review.md) and [deep-agent design](local-semantic-evaluator-design.md). These are research context, not proof that a model improves this project.

The current concrete question is whether a debug-mode agent can report missing context or friction, using existing telemetry and trackers. The user explicitly added claude-traces as a source and requested measurements from the last week because many recent fixes make older aggregate rates misleading.

## Saved last-week baseline

Frozen UTC interval: **[2026-09-14 05:23:49, 2026-09-21 05:23:49)**, max tool_calls id **133453**. Scope: MCP calls in this project's usage database, completion-time window; not all native host calls, not task outcomes, and not a single current-build cohort. The linked JSON includes daily and build/dirty cohorts.

| Literal observable | Recorded count |
|---|---:|
| Calls in frozen window | 9748 |
| Rows marked overflowed | 805 |
| Rows with both recorded payloads | 8750 |
| Rows with started_at populated | 67 |
| Rows with agent_id populated | 54 |
| September 18 calls / both-payload rows | 1071 / 206 |

The snapshot's independent SQL control agrees with its total call, overflow, timing and payload counts. Its 67 started_at rows belong to build 04734cdc with dirty=1; timing coverage does not support a week-wide concurrency analysis. An absent agent_id is not reliable evidence of a main agent: old instrumentation and unstamped calls must remain distinguishable from known identity.

Additional predicates saved in the same snapshot:

- 943 grep calls with output; 64 had exact first-block zero-match output, of which 47 had a scope warning. This measures the recorded response shape, not whether the target existed anywhere.
- 917 non-buffer path reads; 33 repeated normalized argument signatures within process-session/project grouping. Of those, 12 serialized responses matched and 21 differed. Neither signature repetition nor response equality measures unnecessary work. Generated handles can also change serialized output without a source change.

No failure, wasted-token or task-success rate is inferred from these counts. The weekly snapshot and the episode exports have different cutoffs; do not merge their denominators.

## Three reconstructed episodes

These are deliberately selected retrospective examples, not routine prospective samples for the observation-window trackers.

| Case | Evidence | Defensible conclusion | Still unknown |
|---|---|---|---|
| Design-document overflow | usage 132481 → 132493 → 132494–132497; Kat session 571eb3d6-c879-43f6-b3f9-5a51e744e1af | Exact handle chain followed; first extraction returns a second pointer, then body segments are delivered | Byte-complete coverage, comprehension and effect on decision |
| Architecture memory overflow | usage 132500, same Kat session | No exported input contains that exact handle through 2026-09-21T06:08:24.225Z; agent continues inspecting code | Summary sufficient, alternative source sufficient, or necessary information omitted |
| Reread after edits | usage 128895 → edits 129499/129611 → reread 129706; SDD session 458a8a26-c380-4f5b-b967-2181f592917e | Same arguments with intervening successful same-file edits; not evidence of redundancy | Necessity of every read and correctness of resulting document |

The correlation uses session, tool name, exact parsed input and compatible timestamp. It is reconstructed, not an exact shared-ID join. The detailed report names historical builds so these cases cannot accidentally become claims about the newest binary. Ten table anchors were checked against saved usage rows and the supported transcript export.

## Existing infrastructure to reuse

- **usage database:** server-side call inputs/outputs, outcomes, build/project stamps, and partially deployed start-time/principal fields.
- **claude-traces:** supported reader for conversation tool-use IDs, calls and public task narrative. The three reconstructions used its local exports, not Langfuse. API ingestion, tool-schema/system-prompt coverage and full model-visible context were not established by these cases.
- **pika_observations:** inspected live in this exploration; tool_call_id foreign key, audit kinds/verdicts, notes and existing tracker links. It held zero rows at inspection; that does not mean project friction trackers are empty. Existing verdicts describe slips/habits/promotion/rejection, not ordinary positive or unknown context sufficiency.
- **Existing trackers:** [usage frictions](codescout-usage-frictions.md), [context performance](context-performance.md), [context observations](deep-agent-context-observations.md), and [workflow observations](deep-agent-workflow-observations.md). Reuse their purpose and protocols rather than creating a competing friction ledger here.

Related [predicate research](../research/2026-09-20-predicate-candidates-for-the-observer-phase.md) and [overflow handoff](2026-09-20-handoff-overflow-retrieval-discriminator.md) explain the earlier measurement trap: next-call buffer syntax is not evidence of same-result recovery, and repeated read arguments are not a redundancy verdict. Consult their current versions for corrections; old broad-history percentages are not the current baseline.

## Proposals to compare with the peer

These are candidate design requirements, not implemented capabilities or a settled schema:

1. A correlation key shared by persisted call and trace-visible result, scoped by principal/process. Preserve uncertainty for legacy data.
2. Delivery lineage: producer → handle → child/extraction → delivered segment. Record source revision and censoring/continuation state. Handle access, actual delivery, coverage and use are separate facts.
3. Optional debug annotation tied to a call/result and declared required scope: sufficient / insufficient / unknown, reason, evidence reference. Reasons can include summary sufficient, alternative source, irrelevant, deferred, blocked. Store as self-report; later adjudication is a separate event.
4. Reread interpretation that considers source revision and intervening writes. Git project SHA alone misses uncommitted changes.

Do not ask the agent for an unverifiable global guarantee that all context was extracted. Ask whether the declared evidence need was met and what supports that answer. Preserve positive/no-change/unknown cases so the future dataset is not selected only for friction.

## Synchronization checklist

- Peer artifact: **[Kat telemetry findings](2026-09-21-kat-telemetry-findings-for-codex-sync.md)**, artifact `fd008d62a1d1f931`, author Claude Code session `571eb3d6-c879-43f6-b3f9-5a51e744e1af` (profile `.claude-kat`), committed `628ce0d0`. *Filled in by that session, 2026-09-21 — this line only; the rest of this file is unmodified.* Read its alignment block first: the two windows differ (27 days against 7, `max_id` 133283 against 133453) and must not be merged. Three items on this checklist it already answers — shared identity partly exists (`a832ae89` added `emitted_output_id` and `read_output_ids`, so the reconstructed handle chain in episodes 1 and 2 is now an exact join); the `agent_id` NULL caution is filed independently as `82973a1e83aa069f`; and this exploration's 12-matched / 21-differed split answers a question that peer had recorded as unmeasured.
- Align UTC window, max_id, project/process/principal scope, and binary/dirty cohorts before comparing numbers.
- Compare event definitions first: pointer accessed, content delivered, coverage known, agent self-report, externally verified outcome.
- Reconcile whether the peer already implements shared identity, delivery receipts or an annotation path; inspect that work before proposing duplicate storage.
- Decide the smallest extension to existing audit/context surfaces, including positive and unknown vocabulary and retention of supporting evidence.
- Record agreements, disagreements and unresolved questions with supporting sources. Implementation scope remains a separate decision.

## Saved state

The Codex episode research, this handoff, and the frozen weekly JSON are the handoff package. No runtime telemetry implementation was added by this exploration. The detailed report's local document-reference audit passed; source/behavior tests were not run for the research-only work. Peer changes in the shared checkout are outside this package.
