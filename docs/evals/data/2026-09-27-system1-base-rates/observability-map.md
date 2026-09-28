# Observability map -- scratch-2026-09-28-codescout-r3

A map from any corpus other than the Task 12 freeze is provisional; Task 12 regenerates this file from the frozen real corpus.

## Provenance

- corpus_id: scratch-2026-09-28-codescout-r3
- freeze instant (manifest created_utc): 2026-09-28T04:02:29Z
- rendering code: git HEAD e099eafe4af3b881480ce843d173bdff2ac866ff at render time; scripts/measure clean
- repos at freeze (manifest repos): codescout at ac35741a8efaff94bddcbddbbbc5ba5ccc41e106
- earliest kept top-level entry ts: 2026-08-03T20:49:16.290Z (the minimum over the kept top-level entries that carry a parseable ts)
- 138774 of 598885 kept top-level entries carry no ts field (absent, null or empty) and 0 carry an unparseable ts; the no-ts entries by entry type: last-prompt 26678, mode 26561, permission-mode 26561, atis-latch 26506, ai-title 26388, agent-name 5688, cost-state 291, artifact-autoreact-ledger 54, bridge-session 29, artifact-comment-monitor 15, agent-setting 3
- usage DBs: 1 file(s) in the manifest's files, holding 72094 usage rows (manifest counts)

Transcript sources, from the manifest's files:

| profile dir | project dir | top-level transcripts | subagent transcripts |
|---|---|---|---|
| 00-.claude | -home-marius-work-claude-codescout | 45 | 289 |
| 01-.claude-sdd | -home-marius-work-claude-codescout | 59 | 296 |
| 02-.claude-kat | -home-marius-work-claude-codescout | 34 | 155 |

## Windows

Both windows are half-open, [start, end).

- retained: 2026-08-03T20:49:16.290Z to 2026-09-28T04:02:30Z
- decision: 2026-09-21T04:02:30Z to 2026-09-28T04:02:30Z

Data span observed across turns, tool_events and deliveries: 2026-08-03T20:49:16.531Z to 2026-09-28T04:02:17.802Z.

Rows skipped from every window, day and span for a NULL or empty ts -- turns: 0, tool_events: 0, deliveries: 0.
Rows skipped from every window, day and span for an unparseable ts -- turns: 0, tool_events: 0, deliveries: 0.

## mistakes

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 3922 | 47422 | all | decision points = assistant_text turns (of which top-level -- decision: 3562, retained: 39007) |
| signal/request | needs adjudication | 364 | 2663 | all | prompt + interrupt rows are the candidate population; whether each is a correction is judged (of which top-level -- decision: 364, retained: 2655) |
| delivery/action | needs adjudication | n/a | n/a | n/a | deliveries before the decision exist, but their relevance to the mistake is judged, not counted |
| observed use | needs adjudication | n/a | n/a | n/a | whether the assistant's next action aligned with the correction is judged (Amendment 2(c)) |
| checked outcome | needs adjudication | n/a | n/a | n/a | the Codex judge, plus the operator's 25-item spot-check, decide this |

## context

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 3922 | 47422 | all | assistant_text turns (of which top-level -- decision: 3562, retained: 39007) |
| signal/request | measurable now | 12731 | 114622 | all | tool_use blocks (tool_events total) |
| delivery/action | measurable now | 746 | 7846 | all | deliveries by source, counted as delivered items: one per output_json marker match, one per deliveries_json ledger key (block rows not counted), one per transcript hook injection (delivered items carrying a tool_use_id -- decision: 269 of 746; retained: 3715 of 7846) |
| observed use | needs adjudication | n/a | n/a | n/a | whether the delivered context was used in the assistant's next action is judged |
| checked outcome | needs adjudication | n/a | n/a | n/a | the judge, plus the spot-check, decide this |

## transfer

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | needs adjudication | n/a | n/a | n/a | lesson applicability is judged |
| signal/request | needs adjudication | n/a | n/a | n/a | the lesson inventory is Task 7, not yet built, so there is no candidate population to count |
| delivery/action | measurable now | 480 | 4134 | all | deliveries whose engine_or_hook names a transfer-carrying engine (TRANSFER_DELIVERY_MARKERS: operator-rule, get_guide, operator-rules, guide-sections, session-opener), counted as delivered items: one per output_json marker match, one per deliveries_json ledger key (block rows not counted), one per transcript hook injection |
| observed use | needs adjudication | n/a | n/a | n/a | whether a transferred lesson was applied or missed is judged |
| checked outcome | needs adjudication | n/a | n/a | n/a | the judge, plus the spot-check, decide this |
| rediscovery | needs adjudication | n/a | n/a | n/a | always: it needs semantic matching (A1.4) |

## background-worker

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 12731 | 114622 | all | tool calls (the A1.5 task-family split is not computed in this map) |
| signal/request | measurable now | 94 | 628 | all | Agent tool_uses (delegation turns (separate, never summed) -- decision: 68, retained: 572) |
| delivery/action | measurable now | 9167 | 112220 | subagent | subagent turns (agent_path set) |
| observed use | needs adjudication | n/a | n/a | n/a | whether the parent used the subagent's result is judged |
| checked outcome | unobservable | n/a | n/a | n/a | there is no counterfactual; A1.5 measures opportunity size, not delegability or saving |

## Appendix

### A1.6 -- joins by method (overall)

Window: retained, as the whole events DB (rows skipped above for their ts included).

| method | count |
|---|---|
| exact | 85 |
| heuristic | 59667 |
| none | 26214 |
| not_codescout | 28656 |

Amendment 4(a): tool_use_id was NULL on every usage row until 6f6349ca; it appears per session from that session's /mcp. Usage rows with a non-NULL tool_use_id: 85. First exact join ts (the transcript tool_use ts): 2026-09-27T04:58:01.005Z.

Heuristic joins: 59667 total, of which 51848 matched via called_at (started_at NULL) and 7819 matched via started_at directly.

A1.5's latency share is measurable only for joined calls: 59752 of 114622 tool_events are joined (exact + heuristic), and 59752 of those joined rows carry latency_ms on their usage row.

### A1.6 -- joins by method (by day)

Window: retained, by UTC day of the tool_use ts; the 0 tool_events rows with a NULL or unparseable ts appear in the overall table only.

| day | exact | heuristic | none | not_codescout |
|---|---|---|---|---|
| 2026-08-03 | 0 | 0 | 130 | 6 |
| 2026-08-04 | 0 | 0 | 276 | 23 |
| 2026-08-05 | 0 | 0 | 18 | 1 |
| 2026-08-23 | 0 | 0 | 460 | 47 |
| 2026-08-24 | 0 | 0 | 835 | 63 |
| 2026-08-25 | 0 | 0 | 2930 | 134 |
| 2026-08-26 | 0 | 0 | 6664 | 298 |
| 2026-08-27 | 0 | 0 | 6908 | 735 |
| 2026-08-28 | 0 | 0 | 1775 | 652 |
| 2026-08-31 | 0 | 2803 | 9 | 1446 |
| 2026-09-01 | 0 | 6217 | 39 | 2504 |
| 2026-09-02 | 0 | 11311 | 122 | 4177 |
| 2026-09-03 | 0 | 6194 | 64 | 2241 |
| 2026-09-04 | 0 | 431 | 15 | 341 |
| 2026-09-07 | 0 | 465 | 0 | 401 |
| 2026-09-08 | 0 | 1329 | 6 | 1555 |
| 2026-09-09 | 0 | 4706 | 346 | 1446 |
| 2026-09-10 | 0 | 3805 | 13 | 1745 |
| 2026-09-11 | 0 | 2893 | 34 | 1145 |
| 2026-09-12 | 0 | 1661 | 25 | 643 |
| 2026-09-13 | 0 | 2342 | 30 | 831 |
| 2026-09-14 | 0 | 3561 | 59 | 1877 |
| 2026-09-15 | 0 | 837 | 148 | 1211 |
| 2026-09-16 | 0 | 1431 | 65 | 1734 |
| 2026-09-17 | 0 | 1003 | 210 | 788 |
| 2026-09-18 | 0 | 206 | 0 | 379 |
| 2026-09-19 | 0 | 286 | 1170 | 378 |
| 2026-09-20 | 0 | 231 | 660 | 367 |
| 2026-09-21 | 0 | 186 | 1021 | 170 |
| 2026-09-22 | 0 | 99 | 130 | 162 |
| 2026-09-23 | 0 | 362 | 221 | 109 |
| 2026-09-24 | 0 | 4796 | 1049 | 511 |
| 2026-09-25 | 0 | 1046 | 224 | 227 |
| 2026-09-26 | 0 | 788 | 108 | 206 |
| 2026-09-27 | 85 | 627 | 446 | 96 |
| 2026-09-28 | 0 | 51 | 4 | 7 |

### A1.6 -- delivery coverage by source

Window: retained, as the whole events DB (rows skipped above for their ts included).

| source | delivered items | table rows | unit |
|---|---|---|---|
| transcript_hook | 3712 | 3712 | one per hook injection (a hook_success or hook_additional_context row) |
| usage_deliveries_json | 42 | 84 | one per ledger key of a deliveries_json engine record; its block rows (key NULL) are digests of the same deliveries and are not counted |
| usage_output_json | 4092 | 4092 | one per operator-rule or get_guide marker match in output_json |

hook_success_only: 318; hook_success_twins_dropped: 3839.

usage rows with no kept session: 12137 of 72094 usage rows read (of which 521 from the spec-excluded session).

### A1.6 -- sessions per project and window

Window: both, as columns; a kept session is in a window if any of its turns' ts is.

| project | retained | decision |
|---|---|---|
| -home-marius-work-claude-codescout | 118 | 23 |

### A1.6 -- exclusions by reason

Window: retained, as every session in the corpus.

| reason | count |
|---|---|
| duplicate-prefix-of | 10 |
| excluded-by-spec | 2 |
| sdk-cli | 7 |

excluded-by-spec, data-wise: sid 3c5b02df-b6ce-45f5-9d03-1194e38465c0 present in 2 profile(s): .claude-kat/3c5b02df-b6ce-45f5-9d03-1194e38465c0, .claude-sdd/3c5b02df-b6ce-45f5-9d03-1194e38465c0

### A1.6 -- turns by kind

Window: retained, as the whole events DB (rows skipped above for their ts included).

| kind | count |
|---|---|
| assistant_text | 47422 |
| assistant_thinking | 70960 |
| delegation | 572 |
| interrupt | 142 |
| meta | 18462 |
| prompt | 2521 |
| tool_result | 114616 |
| tool_use | 114621 |

### A1.6 -- divergent-duplicate unowned uuids

Window: retained, as every divergent-duplicate copy in the corpus.

none

### A1.6 -- kept sessions with zero turns

Window: retained, as the whole events DB (rows skipped above for their ts included).

count: 1.

.claude-kat/d8a1f024-ebf2-463b-996e-b7a908b34169: 1187 of 1187 uuids owned by .claude-sdd/571eb3d6-c879-43f6-b3f9-5a51e744e1af
