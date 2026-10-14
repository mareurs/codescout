---
id: '8ffb0a8b969c5af4'
kind: plan
status: active
title: Albert recovery hardening implementation
owners:
- marius
tags:
- albert-r02
- implementation
source_commit: c773a2ca5b5225010c06b5784a13470319bc4617
---

# Albert Recovery Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Reconcile Albert's recovery safeguards with our existing command and deliver credential-free verification plus a concrete client acceptance packet.

**Architecture:** Keep the explicit current CLI and all raw-record/literal-ID preservation. Add private file/backup guards and consistent snapshots around its transaction; defer the scorer's live client at its first real use. Client examples are isolated and do not alter installed profiles.

**Tech Stack:** Rust/rusqlite/sqlite-vec/same-file; Python standard library/pytest; native CLI help and official client configuration references.

**Spec:** ../specs/2026-10-14-albert-recovery-hardening-design.md

## Global Constraints

All databases used by tests are synthetic or explicit trial copies. No credential reads, model calls or networking from Python collection. Preserve multi-namespace and legacy literal IDs, extra columns, empty-index support and model identity requirements. Use CodeScout structural edits and owned commit pathspecs; do not push or include peer changes.

## Review Focus

- Replacing the source or reserved backup with byte-identical files must fail before commit.
- SQLite sidecar backup aliases and WAL dry-run must leave all files untouched.
- Same-inode backup content corruption and original vector-byte drift must fail verification.
- Real process exit/cancellation after vector DDL/insertion must retain original rows/schema/vector bytes in ordinary and WAL modes.
- Offline imports/collection must not be rescued by another test's global dummy credential setup; actual live scoring still requires credentials.

## Task 1: SQLite recovery safeguards

**Files:** Modify Cargo.toml/Cargo.lock, src/migrate/sqlite_memory_vectors.rs and its guide; create src/migrate/sqlite_memory_vectors_hardening_tests.rs and a small private safety module if needed. Existing tests/rebuild_memory_vectors.rs remains the actual CLI regression.

**Interfaces:** Keep RebuildOptions and rebuild_sqlite_memory_vectors public signatures unchanged. Private publication gains a checked after-insert callback. Snapshot retains every raw SQL value and IDs; backup verification adds original schema/vector bytes.

- [x] Write focused real-file/SQLite regressions for source replacement, sidecar backups, WAL preview, zero/private-error vectors and corrupted backup; observe actual failures.
- [x] Implement minimal guards with pinned handles, consistent snapshot, full backup verification and checked atomic publication.
- [x] Run all old/new reconstruction and CLI tests, with abrupt-exit and WAL/cancellation characterization; record what was already green.
- [x] Run the actual MiniLM CLI/reopened-store test against the verified offline snapshot.
- [x] Record bug evidence and update the supported guide.

Expected verification: scripts/with-slot.sh cargo test --lib migrate::sqlite_memory_vectors; scripts/with-slot.sh cargo test --test rebuild_memory_vectors; the explicit ignored genuine-model test. All selected tests pass; no models are downloaded.

## Task 2: Credential-free Python collection

**Files:** scripts/phase2-score-dp1.py and any directly analogous constructor demonstrated by investigation; tests/test_offline_python_collection.py plus narrowly affected existing tests; dedicated issue/plan evidence.

**Interfaces:** Existing judge/scorer call signatures and _p metadata remain; SubscriptionJudge construction is inert and complete() authenticates immediately before a live subprocess. Derive mine_pairs ROOT from HERE.parents[4] so worktree imports test their own code.

- [x] Reproduce empty-home collection with installed pytest dependencies and no client credentials; record exact offending import chain.
- [x] Write subprocess regressions for pure imports/collection and missing-auth live refusal; observe RED.
- [x] Defer construction and preserve live clean-profile validation; run regressions and a credential-free light lane; import probes forbid subprocess and network access.
- [x] Record original failures and measured final lane outcome.

Expected verification: new offline collection tests and scripts/python-tests.py --lane light pass. Credential-dependent live calls fail locally without starting a subprocess/client.

## Task 3: Acceptance guide, review and integration

**Files:** Existing acceptance plan/guide, portable Downloads follow-up packet, owned source/verification records only.

**Interfaces:** Deliver the current standalone command, pinned snapshot hashes and matching source patch; do not combine Albert's different CLI with ours.

- [x] Check local Claude/Codex/Pi contracts and supported approval settings; create isolated config examples and a native-client acceptance checklist.
- [x] Request one fresh strong whole-branch review; require applied mutation counts and close observed survivors with RED→GREEN tests.
- [x] Format owned changes; run ./scripts/gate.sh --check in its mandatory order and read the result.
- [ ] Verify packet hashes, clean patch application to base223 and exact reconstructed source bytes.
- [x] Commit only owned paths, reconcile owned catalog records, fast-forward experiments and restore main activation.

Expected verification: all five gate exit codes zero; targeted/model results recorded with scope; packet checks pass. Native client acceptance on Albert's actual laptop remains explicitly pending unless exercised there.

## Verification scope and root rulings

The strong review applied nine distinct final mutations: seven initially killed, two initially survived, none inconclusive. One preliminary variant also survived and is excluded from the distinct-candidate denominator. Both Important coverage gaps are now closed: public byte-identical backup replacement and aliased -shm backup show actual mutation RED and restored GREEN27pass/0fail. No unresolved Critical/Important/Minor findings; see [session log W-1](../../trackers/albert-recovery-hardening-session-log.md#w-1--observed-survivors-expose-backup-identity-and-parent-alias-test-gaps).

The final ordered gate passed with FMT0, CLIPPY0, LEAN0, PYTHON_LIGHT0 and DEFAULT0 (normal TMPDIR, RUST_TEST_THREADS=8). The shorthand tracker recipe was corrected and its exact rule passed independently. Root owns the full gate, four ordinary CLI checks, one genuine two-record MiniLM/reopened-recall check, packet reconstruction/hashes, and the authorized local merge. Those are separate from the review's bounded Rust/Python scope.

Local Linux results do not establish Windows/macOS behavior, every scheduler/procfs policy, power-loss durability, hostile races after final checks or async cancellation inside synchronous publication. Process exits exercise post-DDL/insert rollback; cancellation is exercised during embedding. The mutation sample is not exhaustive.

Local client contract/schema/manifest/harness checks are recorded in [native acceptance](../../conventions/albert-client-copy-acceptance.md). Albert's versions, approvals, hooks, wrapper enforcement, live authentication, full corpus relevance and inventory of external/other-profile stores remain operator acceptance. His192-record/146-topic report concerns his candidate; no daily promotion or profile/plugin/configuration update is performed.

Task2 ruling: inert SubscriptionJudge construction plus authentication at complete() is the minimal equivalent of lazy live initialization while retaining _p interfaces; a live auth failure moves to immediately before launch. Deriving the miner ROOT from its checkout prevents wrong-source imports; archived campaigns may still require their separately recorded corpus inputs. No deferred Minor findings.
