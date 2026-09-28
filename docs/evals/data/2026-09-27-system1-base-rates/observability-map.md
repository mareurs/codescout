# Observability map -- scratch-2026-09-28-task14

A map from any corpus other than the Task 12 freeze is provisional; Task 12 regenerates this file from the frozen real corpus.

## Provenance

- corpus_id: scratch-2026-09-28-task14
- freeze instant (manifest created_utc): 2026-09-28T17:29:48Z
- rendering code: git HEAD ab6bebfe0f46fa340d6dfdd0959a0d6b13854b67 at render time; scripts/measure clean
- repos at freeze (manifest repos): codescout at e22b56403662ae7d9fcef4336750980e1b22a759
- earliest kept top-level entry ts: 2026-08-03T20:49:16.290Z (the minimum over the kept top-level entries that carry a parseable ts)
- 137903 of 594159 kept top-level entries carry no ts field (absent, null or empty) and 0 carry an unparseable ts; the no-ts entries by entry type: last-prompt 26538, mode 26420, permission-mode 26420, atis-latch 26365, ai-title 26248, agent-name 5519, cost-state 292, artifact-autoreact-ledger 54, bridge-session 29, artifact-comment-monitor 15, agent-setting 3
- usage DBs: 1 file(s) in the manifest's files, holding 75210 usage rows (manifest counts)

Transcript sources, from the manifest's files:

| profile dir | project dir | top-level transcripts | subagent transcripts |
|---|---|---|---|
| 00-.claude | -home-marius-work-claude-codescout | 45 | 289 |
| 01-.claude-sdd | -home-marius-work-claude-codescout | 59 | 313 |
| 02-.claude-kat | -home-marius-work-claude-codescout | 34 | 155 |

## Windows

Both windows are half-open, [start, end).

- retained: 2026-08-03T20:49:16.290Z to 2026-09-28T17:29:49Z
- decision: 2026-09-21T17:29:49Z to 2026-09-28T17:29:49Z

Data span observed across turns, tool_events and deliveries: 2026-08-03T20:49:16.531Z to 2026-09-28T09:07:54.699Z.

Rows skipped from every window, day and span for a NULL or empty ts -- turns: 0, tool_events: 0, deliveries: 0.
Rows skipped from every window, day and span for an unparseable ts -- turns: 0, tool_events: 0, deliveries: 0.

## mistakes

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 3439 | 47251 | all | decision points = assistant_text turns (of which top-level -- decision: 3240, retained: 38840) |
| signal/request | needs adjudication | 355 | 2805 | all | prompt + interrupt rows are the candidate population; whether each is a correction is judged (of which top-level -- decision: 355, retained: 2797) |
| delivery/action | needs adjudication | n/a | n/a | n/a | deliveries before the decision exist, but their relevance to the mistake is judged, not counted |
| observed use | needs adjudication | n/a | n/a | n/a | whether the assistant's next action aligned with the correction is judged (Amendment 2(c)) |
| checked outcome | needs adjudication | n/a | n/a | n/a | the Codex judge, plus the operator's 25-item spot-check, decide this |

## context

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 3439 | 47251 | all | assistant_text turns (of which top-level -- decision: 3240, retained: 38840) |
| signal/request | measurable now | 10910 | 114110 | all | tool_use blocks (tool_events total) |
| delivery/action | measurable now | 671 | 7826 | all | deliveries by source, counted as delivered items: one per output_json marker match, one per deliveries_json ledger key (block rows not counted), one per transcript hook injection (delivered items carrying a tool_use_id -- decision: 242 of 671; retained: 3719 of 7826) |
| observed use | needs adjudication | n/a | n/a | n/a | whether the delivered context was used in the assistant's next action is judged |
| checked outcome | needs adjudication | n/a | n/a | n/a | the judge, plus the spot-check, decide this |

## transfer

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | needs adjudication | n/a | n/a | n/a | lesson applicability is judged |
| signal/request | needs adjudication | n/a | n/a | n/a | the lesson inventory is scripts/measure/lessons.py, which this map does not join, so there is no candidate population to count here |
| delivery/action | measurable now | 438 | 4116 | all | deliveries whose engine_or_hook names a transfer-carrying engine (TRANSFER_DELIVERY_MARKERS: get_guide, guide-sections, operator-rule, operator-rules, session-opener), counted as delivered items: one per output_json marker match, one per deliveries_json ledger key (block rows not counted), one per transcript hook injection |
| observed use | needs adjudication | n/a | n/a | n/a | whether a transferred lesson was applied or missed is judged |
| checked outcome | needs adjudication | n/a | n/a | n/a | the judge, plus the spot-check, decide this |
| rediscovery | needs adjudication | n/a | n/a | n/a | always: it needs semantic matching (A1.4) |

## background-worker

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 10910 | 114110 | all | tool calls (the A1.5 task-family split is not computed in this map) |
| signal/request | measurable now | 75 | 626 | all | Agent tool_uses (delegation turns (separate, never summed) -- decision: 55, retained: 570) |
| delivery/action | measurable now | 5932 | 111841 | subagent | subagent turns (agent_path set) |
| observed use | needs adjudication | n/a | n/a | n/a | whether the parent used the subagent's result is judged |
| checked outcome | unobservable | n/a | n/a | n/a | there is no counterfactual; A1.5 measures opportunity size, not delegability or saving |

## Appendix

### A1.6 -- joins by method (overall)

Window: retained, as the whole events DB (rows skipped above for their ts included).

| method | count |
|---|---|
| exact | 141 |
| heuristic | 59277 |
| none | 26067 |
| not_codescout | 28625 |

Amendment 4(a): tool_use_id was NULL on every usage row until 6f6349ca; it appears per session from that session's /mcp. Usage rows with a non-NULL tool_use_id: 2674. First exact join ts (the transcript tool_use ts): 2026-09-27T04:58:01.005Z.

Heuristic joins: 59277 total, of which 51848 matched via called_at (started_at NULL) and 7429 matched via started_at directly.

A1.5's latency share is measurable only for joined calls: 59418 of 114110 tool_events are joined (exact + heuristic), and 59418 of those joined rows carry latency_ms on their usage row.

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
| 2026-09-26 | 0 | 732 | 108 | 195 |
| 2026-09-27 | 85 | 344 | 303 | 79 |
| 2026-09-28 | 56 | 0 | 0 | 4 |

### A1.6 -- delivery coverage by source

Window: retained, as the whole events DB (rows skipped above for their ts included).

| source | delivered items | table rows | unit |
|---|---|---|---|
| transcript_hook | 3710 | 3710 | one per hook injection (a hook_success or hook_additional_context row) |
| usage_deliveries_json | 24 | 48 | one per ledger key of a deliveries_json engine record; its block rows (key NULL) are digests of the same deliveries and are not counted |
| usage_output_json | 4092 | 4092 | one per operator-rule or get_guide marker match in output_json |

hook_success_only: 318; hook_success_twins_dropped: 3837.

usage rows with no kept session: 15587 of 75210 usage rows read (of which 1762 from the spec-excluded session).

### A1.6 -- sessions per project and window

Window: both, as columns. Each count is the number of sessions with any turn in the window (descriptive; the go/no-go denominator is sessions with ≥1 decision point, spec § Scope).

| project | retained | decision |
|---|---|---|
| -home-marius-work-claude-codescout | 117 | 22 |

### A1.6 -- exclusions by reason

Window: retained, as every session in the corpus.

| reason | count |
|---|---|
| duplicate-prefix-of | 10 |
| excluded-by-spec | 3 |
| sdk-cli | 7 |

excluded-by-spec, data-wise: sid 3c5b02df-b6ce-45f5-9d03-1194e38465c0 present in 2 profile(s): .claude-kat/3c5b02df-b6ce-45f5-9d03-1194e38465c0, .claude-sdd/3c5b02df-b6ce-45f5-9d03-1194e38465c0; sid 82cff72e-0245-48cb-ab07-45a1c3d0d388 present in 1 profile(s): .claude-sdd/82cff72e-0245-48cb-ab07-45a1c3d0d388

entrypoint_changed (kept sessions whose first and last entrypoint differ; each keeps its first entrypoint's class): 0.

### A1.6 -- turns by kind

Window: retained, as the whole events DB (rows skipped above for their ts included).

| kind | count |
|---|---|
| assistant_text | 47251 |
| assistant_thinking | 70564 |
| delegation | 570 |
| interrupt | 141 |
| meta | 18182 |
| prompt | 2664 |
| rejection | 10 |
| tool_result | 114095 |
| tool_use | 114109 |

### A1.6 -- divergent-duplicate unowned uuids

Window: retained, as every divergent-duplicate copy in the corpus.

none

### A1.6 -- kept sessions with zero turns

Window: retained, as the whole events DB (rows skipped above for their ts included).

count: 1.

.claude-kat/d8a1f024-ebf2-463b-996e-b7a908b34169: 1187 of 1187 uuids owned by .claude-sdd/571eb3d6-c879-43f6-b3f9-5a51e744e1af
