---
id: 2d2b25bca5ed153a
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
entry_high_water_F: 2
---

# Albert recovery hardening session log

## Purpose

Measured verification lessons from the 2026-10-14 reconciliation. See [implementation plan](../superpowers/plans/2026-10-14-albert-recovery-hardening.md).

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| W-1 | 2026-10-14 | — | test-design | validated | **Observed survivors expose backup-identity and parent-alias test gaps** |
| F-1 | 2026-10-14 | med | environment | mitigated | **Reviewer LSP scratch filled tmpfs and invalidated gate evidence** |
| F-2 | 2026-10-16 | med | tool-scope | mitigated | **An audit-only paths parameter did not narrow reindex** |

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

## F-2 — A reindex cleanup used an audit-only parameter as a scope selector

**Observed:** 2026-10-16, after integrating Albert's memory-history and audit repairs.

**Expected:** Passing five file paths to librarian(reindex) would limit catalog reconciliation to those files.

**Got:** The call reported project scope, 7 updated artifacts, 2077 unchanged, 1562 embedded and an automatic export of191 pending audit rows. It completed with zero embedding/backfill errors. A subsequent schema read showed paths is labeled for audit_doc_refs only (src/librarian/tools/librarian.rs:95-99). This was operator misuse of an action-specific parameter, not evidence of a new product regression.

**Workaround:** Read the parameter's action label before a mutating call. Reindex supports project scope here; use per-file doctor roots for frontmatter repair. Preserve and review the generated audit export instead of reverting its file behind the catalog's export marker.

**Severity:** med — broadened metadata/embedding cleanup beyond the intended five documents; no source/profile/runtime activation was performed by the call.

**Status:** mitigated — schema scope verified, catalog result inspected, original peer source-file hashes preserved.

**Valid:** dated 2026-10-16

**Rests on:** the observed project-scoped response and the input-schema paths label; reassess if reindex gains an explicit file selector.

## Template for new entries

The librarian appends each entry and its index row in the same write.

## Status vocabulary

Friction: open, mitigated, fixed-verified, promoted-to-bug-tracker. Wins: validated, promoted-to-permanent-docs, archived.
