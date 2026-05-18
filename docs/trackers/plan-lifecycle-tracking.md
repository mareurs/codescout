---
title: Plan Lifecycle Tracking
status: draft
kind: tracker
---

# Plan Lifecycle Tracking

**Started:** 2026-04-21
**Status:** scoping — concern identified, no mechanism chosen yet.

## Why this exists

Plans under `docs/superpowers/plans/*.md` are written as TDD task lists with `- [ ]` checkboxes, but they drift silently: tasks ship, plans don't get updated, post-ship fixes land with no back-reference, and the next reader can't tell what's done or what came after.

Surfaced on 2026-04-21 while reviewing the `artifact_get` preview feature. 13-task plan fully shipped, but plan file untouched. Follow-ups (I1/I3/M2) and bug fixes (deadlock, summary-empty) landed as loose commits with no plan link.

## The underlying problem

Two orthogonal gaps:

1. **Commit ↔ plan linkage is implicit.** Nothing in the commit tells you which plan task it completed. `git log --grep` doesn't help — you'd need to remember the plan title.
2. **Plan files are write-once.** No convention, skill, or hook updates checkboxes, appends a History section, or records post-ship fixes. Once a feature ships, the plan ossifies as a historical artifact instead of becoming a living record.

Consequence: librarian indexes the plan as `kind: plan, status: draft` forever. Plan preview shows `tasks.done: 0` even when the feature is live.

## Options being weighed

### A. Codescout-as-enforcer
Pre-commit or PreToolUse hook detects `docs/superpowers/plans/*.md` in context; before allowing a commit, requires a checkbox flip in the plan file and a plan reference in the commit message.

- Pro: hard guarantee, no drift possible.
- Con: rigid. Noisy for small tasks, refactors, or work that doesn't map 1:1 to a plan task.

### B. Librarian-as-reviewer
Post-commit skill runs against the last N commits: greps referenced plan files, suggests checkbox flips, appends History entries with commit SHAs. Human reviews and applies.

- Pro: advisory, low friction.
- Con: still needs a trigger. Easy to forget.

### C. Commit-trailer convention + periodic audit (leanest)
Adopt a `Plan: <path>#task-N` trailer convention (like `Co-Authored-By`). Separate skill audits the last N commits against referenced plan files on demand (e.g. at end of `experiments`-branch sessions, before ship).

- Pro: no new tools. Trailers are scannable with `git log --grep`. Uses existing `artifact_links` for plan ↔ commit edges.
- Con: relies on author discipline. No enforcement.

### D. Something else
- Plan `status` transitions (draft → in_progress → shipped) tracked via frontmatter. Librarian surfaces `days_since_status_change`.
- Per-plan auto-tracker: every plan spawns a sibling tracker file that captures progress as commits land.
- Superpowers skill: `/close-plan` command that reads a plan, greps for related commits via trailer or diff, proposes the full close-out edit.

## Immediate action (independent of mechanism)

- [ ] Close the loop on `docs/superpowers/plans/2026-04-20-artifact-get-preview.md`:
  - [ ] Flip checkboxes on all 13 tasks
  - [ ] Add "Post-ship fixes" section covering deadlock fix (`ae03dd5`), summary-empty fix (`a1f5f73`), follow-ups I1/I3/M2 (`1561661`)
  - [ ] Update frontmatter status to `shipped`
- [ ] Backfill the same for any other plan that shipped but wasn't closed — audit needed.

## Decision deferred

No mechanism chosen yet. Leaning toward **C (trailers + audit)** as the cheapest thing that could work, but want to see how much friction it adds in practice before locking in.

Revisit after: closing the preview plan, watching how the next 2–3 plans ship, deciding whether drift is frequent enough to warrant tooling.

## History

- 2026-04-21 — created. Concern identified while reviewing preview feature ship.
