# Observability map -- task13-fix1-real

Built: 2026-09-27T20:47:03Z.
Repo HEAD at freeze time: codescout at 66a05f759c2964824f9a921e933f76d5a30ea7d2.

## Windows

- retained: 2026-08-03T20:49:16Z to 2026-09-27T20:46:57Z
- decision: 2026-09-20T20:46:57Z to 2026-09-27T20:46:57Z

Data span observed across turns, tool_events and deliveries: 2026-08-03T20:49:16.531000+00:00 to 2026-09-27T16:03:40.261000+00:00 (0 rows skipped for a NULL or unparseable ts).

## mistakes

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 3904 | 47404 | all | decision points = assistant_text turns (of which top-level -- decision: 3544, retained: 38989) |
| signal/request | needs adjudication | 363 | 2662 | all | prompt + interrupt rows are the candidate population; whether each is a correction is judged (of which top-level -- decision: 363, retained: 2654) |
| delivery/action | needs adjudication | n/a | n/a | n/a | deliveries before the decision exist, but their relevance to the mistake is judged, not counted |
| observed use | needs adjudication | n/a | n/a | n/a | whether the assistant's next action aligned with the correction is judged (Amendment 2(c)) |
| checked outcome | needs adjudication | n/a | n/a | n/a | the Codex judge, plus the operator's 25-item spot-check, decide this |

## context

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 3904 | 47404 | all | assistant_text turns (of which top-level -- decision: 3544, retained: 38989) |
| signal/request | measurable now | 12669 | 114560 | all | tool_use blocks (tool_events total) |
| delivery/action | measurable now | 774 | 7874 | all | deliveries by source (share carrying a tool_use_id -- decision: 272 of 774; retained: 3718 of 7874) |
| observed use | needs adjudication | n/a | n/a | n/a | whether the delivered context was used in the assistant's next action is judged |
| checked outcome | needs adjudication | n/a | n/a | n/a | the judge, plus the spot-check, decide this |

## transfer

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | needs adjudication | n/a | n/a | n/a | lesson applicability is judged |
| signal/request | needs adjudication | n/a | n/a | n/a | the lesson inventory is Task 7, not yet built, so there is no candidate population to count |
| delivery/action | measurable now | 508 | 4162 | all | deliveries whose engine_or_hook names a transfer-carrying engine (TRANSFER_DELIVERY_MARKERS: operator-rule, get_guide, operator-rules, guide-sections, session-opener) |
| observed use | needs adjudication | n/a | n/a | n/a | whether a transferred lesson was applied or missed is judged |
| checked outcome | needs adjudication | n/a | n/a | n/a | the judge, plus the spot-check, decide this |
| rediscovery | needs adjudication | n/a | n/a | n/a | always: it needs semantic matching (A1.4) |

## background-worker

| link | label | decision | retained | population | basis |
|---|---|---|---|---|---|
| opportunity | measurable now | 12669 | 114560 | all | tool calls by A1.5 task family |
| signal/request | measurable now | 94 | 628 | all | Agent tool_uses (delegation turns (separate, never summed) -- decision: 68, retained: 572) |
| delivery/action | measurable now | 9167 | 112220 | subagent | subagent turns (agent_path set) |
| observed use | needs adjudication | n/a | n/a | n/a | whether the parent used the subagent's result is judged |
| checked outcome | unobservable | n/a | n/a | n/a | there is no counterfactual; A1.5 measures opportunity size, not delegability or saving |

## Appendix

### A1.6 -- joins by method (overall)

| method | count |
|---|---|
| exact | 85 |
| heuristic | 59616 |
| none | 26210 |
| not_codescout | 28649 |

Amendment 4(a): tool_use_id was NULL on every usage row until 6f6349ca; it appears per session from that session's /mcp. Usage rows with a non-NULL tool_use_id: 85. First exact join ts (the transcript tool_use ts): 2026-09-27T04:58:01.005Z.

Heuristic joins: 59616 total, of which 51848 matched via called_at (started_at NULL) and 7768 matched via started_at directly.

A1.5's latency share is measurable only for joined calls: 59701 of 114560 tool_events are joined (exact + heuristic); latency_ms is only present on the usage rows behind those.

### A1.6 -- joins by method (by day)

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

### A1.6 -- delivery coverage by source

| source | count |
|---|---|
| transcript_hook | 3712 |
| usage_deliveries_json | 70 |
| usage_output_json | 4092 |

hook_success_only: 318; hook_success_twins_dropped: 3839.

usage rows with no kept session: 11786 of 71692 usage rows read (of which 492 from the spec-excluded session).

### A1.6 -- sessions per project and window

| project | retained | decision |
|---|---|---|
| -home-marius-work-claude-codescout | 118 | 23 |

### A1.6 -- exclusions by reason

| reason | count |
|---|---|
| duplicate-prefix-of | 10 |
| excluded-by-spec | 2 |
| sdk-cli | 7 |

excluded-by-spec, data-wise: sid 3c5b02df-b6ce-45f5-9d03-1194e38465c0 present in 2 profile(s): .claude-kat/3c5b02df-b6ce-45f5-9d03-1194e38465c0, .claude-sdd/3c5b02df-b6ce-45f5-9d03-1194e38465c0

### A1.6 -- turns by kind

| kind | count |
|---|---|
| assistant_text | 47404 |
| assistant_thinking | 70917 |
| delegation | 572 |
| interrupt | 142 |
| meta | 18456 |
| prompt | 2520 |
| tool_result | 114554 |
| tool_use | 114559 |

### A1.6 -- divergent-duplicate unowned uuids

none

### A1.6 -- kept sessions with zero turns

count: 1.

.claude-kat/d8a1f024-ebf2-463b-996e-b7a908b34169: its uuids are owned by .claude-sdd/571eb3d6-c879-43f6-b3f9-5a51e744e1af
