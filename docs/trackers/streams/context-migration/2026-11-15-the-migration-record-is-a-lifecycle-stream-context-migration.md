---
kind: entry
status: open
title: The migration record is a lifecycle stream, context-migration
created: 2026-11-15
entry_kind: decision
status_since: 2026-11-15
---

Decision (2026-11-15, the operator, in the session that built the hot view): the record of the tracker migration is a lifecycle stream named `context-migration`, not a flat tracker. Reason: we dogfood. A flat tracker would add one more file to the 81 that must be migrated, and a stream is the thing we are building; its session-start hot view is the first live test of the hot path.

Cost if wrong: a collection key cannot be renamed until the identity line's Plan E, so a different name later means a move by hand.

How to use it: one entry per fact; kinds `lesson`, `friction`, `script`, `decision`; statuses `open`, `blocked` (needs `revisit_by`), `done`.
