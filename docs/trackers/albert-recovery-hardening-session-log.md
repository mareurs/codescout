---
id: '3a3c39a7e52ec756'
kind: tracker
status: active
title: Albert recovery hardening session log
owners:
- marius
tags:
- albert-r02
- reconnaissance
- verification
entry_prefix:
- F
- W
snapshot_anchor: '| ID | Date | Severity | Category | Status | Title |'
entry_high_water_W: 1
entry_high_water_F: 1
---

# Albert recovery hardening session log

## Purpose

Measured verification lessons from the 2026-10-14 reconciliation. See [implementation plan](../superpowers/plans/2026-10-14-albert-recovery-hardening.md).

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| W-1 | 2026-10-14 | — | test-design | validated | **Observed survivors expose backup-identity and parent-alias test gaps** |
| F-1 | 2026-10-14 | med | environment | mitigated | **Reviewer LSP scratch filled tmpfs and invalidated gate evidence** |

## W-1 — Observed survivors expose backup-identity and parent-alias test gaps

**Valid:** dated 2026-10-14

**Status:** validated
**Observed:** Fresh strong review actually applied nine mutations: seven killed, two survived. Removing all post-reservation backup identity checks left26 passing tests; removing only backup-parent canonicalization also left26 passing tests. The existing aliased -journal fixture was refused later by SQLite and could not distinguish our guard.

**Counterfactual:** Without applying those mutations, the ordinary green suite would have approved unpinned identity and alias checks. The new public byte-identical backup replacement fixture proves equal bytes and different file handles, then observes mutation RED26pass/1fail. Changing the alias to -shm observes the focused mutation RED0pass/1fail. Restoring both production guards passes27 tests with0 failures; the ignored helper is executed by its parent tests.

**Rests on:** /tmp/codescout-hardening-review-results-20261014/m1.diff; m9.diff; /tmp/codescout-hardening-m1-red-20261014.log; /tmp/codescout-hardening-m9-red-20261014.log; /tmp/codescout-hardening-final-green-20261014.log.

**Generalization:** A content check cannot pin file identity; a generic error assertion cannot pin a guard if a later component produces the same error. Apply the mutation before choosing the fixture.

## F-1 — Reviewer LSP scratch filled tmpfs and invalidated gate evidence

**Valid:** dated 2026-10-14

**Status:** mitigated
**Observed:** The first full gate returned FMT0, CLIPPY101, LEAN101, DEFAULT101, PYTHON_LIGHT1 while logs reported disk quota errors under /tmp. The reviewer's LSP target consumed1.4GiB on tmpfs despite its test builds leasing home-backed slots.

**Resolution:** Reviewer moved only its owned target to home cache, without killing shared language servers. The attempted home-backed TMPDIR rerun caused three unrelated tests to fail: /tmp permission and narrow inline-budget fixtures depend on the normal temporary path. The root restores the normal TMPDIR for final verification after moving the review's owned LSP target; all temporary source mutations are already restored. The failed first gate is not counted as passing verification.

**Rests on:** /tmp/codescout-albert-hardening-gate-20261014.log; /tmp/codescout-albert-hardening-gate-final-20261014.log.

**Remaining limitation:** This mitigates this run; it does not change the general LSP or tmpfs policy.

## Template for new entries

The librarian appends each entry and its index row in the same write.

## Status vocabulary

Friction: open, mitigated, fixed-verified, promoted-to-bug-tracker. Wins: validated, promoted-to-permanent-docs, archived.
