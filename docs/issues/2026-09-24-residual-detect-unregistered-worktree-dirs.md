---
id: '1e153b9a3d5bb333'
kind: bug
status: fixed
title: 'RESIDUAL: Build a detector (doctor/probe) for .worktrees/ entries that are not registered worktrees, since both git worktree list and git status are blind to them'
tags:
- cluster/record-asserts-an-unchecked-completion
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Build a detector (doctor/probe) for .worktrees/ entries that are not registered worktrees, since both git worktree list and git status are blind to them.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` (status `mitigated`):

> Mitigated, not fixed — and the residual is now closed by an UNATTRIBUTED removal rather than by the decision this file parked on. Measured 2026-09-01: `.worktrees/bench` is absent, `.worktrees/` holds only `audit-trail-t1`, and `git worktree list` reports neither — so the orphaned gitdir and the 163M are gone. WHO removed it and WHEN is not establishable: no commit in the last 60 mentions the bench worktree, and the `.worktrees/` mtime (02:24) is equally explained by `audit-trail-t1` being created in it, since a directory mtime records its last entry change and not which entry. No regression guard exists for the class: nothing prevents a record asserting an unchecked completion again, and `docs/trackers/retrieval-benchmark.md:76` agrees with reality today by accident rather than by repair. AMENDED 2026-09-02 — the 2026-09-01 reading closed a MEMBER, not the population, and the population regenerates on its own. `.worktrees/` today holds `audit-shards-t7`: 8K, absent from `.git/worktrees/` and from `git worktree list`, containing only a dead session's gitignored `.buddy/bf44ba81-4cb3-4fdc-a92b-0780646ca7b9/` and `.codescout/cc_session_id`. `audit-trail-t1` is gone and a different unregistered member replaced it inside 24h, so `the orphaned dirs are gone` was true of the instance and false of the class — the same member-vs-population cut CLAUDE.md names under Testing Discipline. The invisibility is doubly-instrumented, which is why nobody trips over it: `git worktree list` reports registrations and cannot see it, `git status` honours `.gitignore:133 .worktrees/` and cannot see it either — two correct instruments whose blind spots coincide, so agreement between them is one blind spot counted twice. Unlike the 2026-08-30 bench case, positive identification IS available and unused here: the residue names its own session id on disk, so the author is given rather than inferred from a directory mtime.

## Fix

Implemented in `src/librarian/tools/doctor.rs` as a new doctor check `unregistered_worktree_dir` (`scan_unregistered_worktree_dirs`, `doctor.rs:3015`). It lists the entries of the MAIN checkout's `.worktrees/` (resolved through `main_root` when a session runs inside a linked worktree) and flags each one that is absent from `.git/worktrees/*/gitdir`. Each finding names the likely author session when the residue carries one (`.buddy/<id>/`, `.codescout/cc_session_id`) and says so when it does not, rather than inferring one from a directory mtime. A missing `.worktrees/`, a missing `.git` or an unreadable directory is reported in `catalog_health.unregistered_worktree_dirs` as "not checked", never as a pass. The check is project grain (`id: None`, no foreign rows to scope-gate) and report-only: there is no `fix=` mode, because deleting a directory that may hold someone's unsaved work is not a repair a report should perform. It is a defect, not informational: `codescout doctor --fail-on-violations` exits 1 on a machine that carries residue. The main checkout carries such residue today: `.worktrees/doctor-per-project-isolation`, which the check flags and whose only content is `.buddy/b80a27d4-9729-40ef-8c28-ad8982df6d13/`. Nobody has removed it; removing it is a separate act.

## Tests added

In `src/librarian/tools/doctor.rs`:

- `an_unregistered_worktree_dir_is_flagged_and_a_registered_one_is_not` (:18393) pins that the unregistered directory fires and the registered one does not, and that the health block counts them.
- `a_dot_worktrees_holding_only_registered_worktrees_is_silent` (:18461) pins the silent case.
- `the_scan_resolves_the_main_checkouts_worktrees_from_inside_a_linked_worktree` (:18479) pins that a session in a linked worktree scans the main checkout's `.worktrees/`.
- `a_project_with_no_dot_worktrees_states_that_nothing_was_checked` (:18511) pins the "nothing to check" note.

The two registry meta-tests were extended to name the new check: `every_declared_check_is_scope_gated_or_a_named_exemption` (:15015) lists it as project grain, and `admits_relevance_exemption_allow_list_stays_exhaustive_over_check_all` (:15140) pins it as excluded from the relevance exemption.

## Fix provenance

- **SHA:** `777dc501` (`experiments`)
- **patch-id:** `52c9d37437bf9e73c6f8ff108d37159d3346c68e`

## Resume

Closed on 2026-10-06 by `777dc501` (local on `experiments`, not pushed at the time of writing). Residual follow-ups, listed and not filed: (1) the live residue `.worktrees/doctor-per-project-isolation` (author session `b80a27d4-9729-40ef-8c28-ad8982df6d13`) is flagged and still present, so `--fail-on-violations` exits 1 on this checkout until someone checks that session is gone and removes it or re-registers it. (2) The parent `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` still carries its `unverified: TRACKED 1e153b9a3d5bb333` caveat; its residual work is now delivered by this file, and the integrator decides how to retire that caveat.

## References

- `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` — parent
