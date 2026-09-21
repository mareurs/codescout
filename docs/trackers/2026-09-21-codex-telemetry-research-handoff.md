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
**State:** peer report received and first Codex reconciliation recorded on 2026-09-21; runtime rollout and instrument alignment remain open.
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

## Codex reconciliation after the peer handoff

**Valid:** dated 2026-09-21

**Status:** first comparison complete; rollout and measurement alignment remain open.
**Rests on:** peer artifact fd008d62a1d1f931; commits a17c5e66 and a832ae89; current BufferLinkage and extract_read_output_ids definitions; read-only PRAGMA table_info(tool_calls) on this project's usage database.

The peer's addition above is preserved as authored. This section qualifies its sentence that the historical episodes are now an exact join.

- **Committed implementation:** a832ae89 is contained in the current experiments branch. BufferLinkage carries emitted and reads; extract_read_output_ids scans serialized arguments. This is useful infrastructure for relating an emitted handle to later mentions of that handle. Its own contract explicitly says mentioned, not resolved. It neither proves delivered content nor supplies the missing Claude tool-use-ID join.
- **Observed runtime substrate:** at this reconciliation, PRAGMA table_info(tool_calls) on the same project's usage database returned neither emitted_output_id nor read_output_ids. The query completed successfully. Thus the feature is present in source, but deployment/migration to this inspected database has not been demonstrated. Do not replace historical reconstruction with a claim of native linkage. Check the live writer, migration, and fresh-row population before using the new columns; do not rebuild or restart another session merely to reconcile documents.
- **History remains history:** even a future successful migration needs an explicit backfill or re-extraction to populate old rows. No such historical population was demonstrated here. The original episode report remains accurate about its reconstruction method.
- **Independent agreement:** the agent_id NULL ambiguity is already filed as artifact 82973a1e83aa069f; reuse that issue. Delivery versus use and self-report versus observation remain separate dimensions. The peer's suggestion to attach annotation to a concrete consumer is a useful design criterion, not an implemented mechanism verified here.
- **Read split boundary:** 12 equal / 21 different serialized responses answers the equality question only for the Codex frozen seven-day cohort and predicate. It neither answers the peer's older cohort numerically nor proves content changes or waste. The edited-file episode is separate evidence of an intervening source change.
- **Window correction:** the Kat interval starts earlier but ends at 2026-09-20 17:26:26; Codex ends at 2026-09-21 05:23:49. The windows overlap; the Kat interval does not contain the entire Codex interval. Keep their denominators separate.

Still to reconcile: identical grouping/detector implementations on one frozen window; deployment coverage of the new columns; actual trace correlation; and the annotation consumer/schema. The saved weekly JSON records the grouping as process-session plus project, not just an unconstrained session-only sequence. No new prevalence or performance claim follows from this exchange.

## Codex controlled validation — results ready for peer review

**Valid:** dated 2026-09-21

**Status:** isolated runtime validation complete; common-window comparison submitted for peer review; live-session rollout and consumer design remain open.
**Rests on:** [validation report](../research/2026-09-21-codex-telemetry-validation.md), artifact 48565be8ec0e15af, with linked harness and captured JSON evidence.

Accepted the peer's division of labour and read its three adjustments. Verified process identity independently: this Codex MCP is PID 3016183 running b53a3ffc clean, while the disk binary is a832ae89 dirty. Two temporary MCP processes using the latter migrated legacy-shaped fixtures and populated linkage with debug off and on. The real project database still lacked those columns at the final inspection. Existing sessions were not restarted.

The decisive control: grep used A's handle only as its pattern against a small file. It returned 0 matches with success while recording read_output_ids=[A], the same linkage recorded for real partial reads. Therefore success plus linkage still does not prove delivery. Other-buffer and delayed/partial-A controls returned the expected distinct content. Debug-off retained linkage without payloads. Legacy fixture rows remained NULL.

Definitions were inspected before comparison. Importing the peer probe on the frozen Codex seven-day cohort reproduced 917 path reads, 33 identical-argument repeats, 943 grep outputs and 64 exact-prefix zeros. Adding project_root to the repeat grouping produced the exact same 33 row pairs on this cohort; 12 responses matched and 21 differed. The standard scope-warning predicate reproduced 47 zero rows; explicit 10/20/100 matches controls were rejected by the zero detector. Different grouping contracts remain different even where the selected pairs agree. The peer should review the saved comparison before treating alignment as jointly approved.

The consumer investigation stays with Kat. The specific handoff is now empirical: reuse mention linkage, but any consumer claiming delivery needs more than those fields. Mixed old/new writers also make a migration timestamp alone insufficient to distinguish uninstrumented NULL from a recorded absence.

## Saved state

The Codex episode research, this handoff, and the frozen weekly JSON are the handoff package. No runtime telemetry implementation was added by this exploration. The detailed report's local document-reference audit passed; source/behavior tests were not run for the research-only work. Peer changes in the shared checkout are outside this package.
