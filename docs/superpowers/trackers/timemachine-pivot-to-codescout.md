---
id: null
kind: tracker
status: active
title: TimeMachine pivot — artifacts → unified docs+code KG
owners: []
tags:
- librarian-mcp
- timemachine
- pivot-tracker
topic: timemachine-pivot-watch
time_scope: null
---

# TimeMachine pivot to docs+code KG

This tracker accumulates evidence for whether the artifact-only TimeMachine
(scope A, shipped 2026-04 / 05) needs to grow into a unified docs+code KG
(scope B). See spec §12 of
`docs/superpowers/specs/2026-04-28-librarian-timeline-design.md`.

## Pivot signal table

| Signal | Pivot weight |
|---|---|
| Users repeatedly ask "what code existed when this spec was written" | high |
| `mutates` edges frequently point at conceptual code modules with no librarian artifact | high |
| Freshness drifts because code changed but no markdown event captures it | high |
| `external_signal` events outnumber file-change events | medium |
| Workspace `as_of` queries used >2×/week per active project | medium |
| Tracker accumulates >10 "wish I could query code at commit X" observations | medium |

## Observations

(Append observations as `note` events on this tracker via
`artifact_event_create`. Re-evaluate at 2026-08-01 or when ≥3 high-weight
signals fire — whichever first.)

The inaugural `intent` event id (set after the first reindex picks this file
up + a manual `artifact_event_create` writes the intent) will be recorded
here as the first observation.
