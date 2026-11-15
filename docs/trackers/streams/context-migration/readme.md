---
kind: collection
title: Context migration
entry_kinds:
- lesson
- friction
- script
- decision
exit_when: every flat tracker in this repository is migrated or deliberately left flat
hot_cap: 12
initial_status: open
stale_after: 14d
statuses:
  open: open
  blocked: blocked
  done: terminal
---

# Context migration

The working record of the move of flat trackers into lifecycle collections (Plan L-E, `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`). We dogfood it: we use the tools we build on our own trackers, and every surprise goes in here.

## What goes in an entry

- **lesson**: something we learned while migrating a tracker, written so the next run does not repeat the mistake.
- **friction**: something that cost time or went wrong in `inventory`, `apply`, `check` or the skill. State what happened, what was expected, and the command.
- **script**: a script or command line that helped, with the path where it lives and what it does.
- **decision**: a call we made that the next person could reverse, with the reason.

## Statuses

- `open`: not dealt with. Open entries show in the session-start hot view.
- `blocked`: waiting on something named in the entry. Needs a `revisit_by` date.
- `done`: dealt with. Terminal.

## Rules

- Write in English. One entry per fact.
- A friction that is a bug in codescout also gets a file in `docs/issues/`; link it from the entry.
- This collection ends when every flat tracker in this repository is migrated or deliberately left flat (`exit_when`).
