---
id: e9c57db19df471c2
kind: plan
status: done
title: Backend memory overwrite protection
---

# Backend memory overwrite protection implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan inline. User authorized sequential execution and commits on 2026-10-14.

**Goal:** Preserve exact previous topic contents before every backend overwrite, including forced writes, with coherent cooperative concurrency.

**Architecture:** A store-wide fs4 advisory lease covers reading the existing topic, shrink policy, mandatory backup, atomic replacement and anchor stamping. MemoryStore::write keeps its wholesale replacement policy; guarded_write evaluates shrink from the captured contents under the same lease. History mirrors the normalized destination under .history and stays outside the active topic list.

**Tech Stack:** Existing Rust fs4, tempfile and uuid dependencies; no additional packages.

**Spec:** Authorized task and contract described to the operator in this session: force bypasses shrink only; exact-byte private history and selected-store routing are mandatory.

## Global constraints

- Keep public method and MCP/dashboard request signatures compatible; no revision API or merge feature.
- Backups use UUID.md.bak files, exclusive creation and Unix0600; history directories Unix0700, local .gitignore=*.
- Five-second bounded store lock; aliases share a lease. Cooperating writes/deletes serialize.
- Refuse unreadable/invalid UTF-8 old content, unsafe nested/leaf symlinks and history failures before replacement. Configured memory root may itself be a symlink, pinned by canonicalization.
- No automatic purge or installation. External programs ignoring locks are outside the concurrency guarantee.

## Review focus

- BOM and Unicode bytes must survive without reencoding: integration exact-byte test.
- Forced MCP and dashboard saves still require history: assertions at both call sites.
- Redirected nested and explicit stores retain their own history: routing/symlink fixtures.
- Concurrent processes must preserve each predecessor: real child-process contention test.
- Write staging and history creation failures leave originals intact: filesystem failure tests.

## Task 1: Shared overwrite transaction

**Files:** Create src/memory/write_transaction.rs and tests/memory_write_history.rs; modify src/memory/mod.rs and src/memory/guarded.rs; extend src/tools/memory/tests.rs and src/dashboard/api/memories.rs tests.

**Interfaces:** WriteTransaction::begin(store:&MemoryStore,topic:&str)->anyhow::Result<Self>; existing()->Option<&str>; write(&self,content:&str)->anyhow::Result<()>; delete(&self)->anyhow::Result<()>. MemoryStore::write/delete and guarded_write/delete retain current signatures.

- [x] Write public-API tests for exact backups, selected store, force failures, shrink refusal, aliases, reserved history, symlinks, Git ignore and permission modes.
- [x] Run cargo test --test memory_write_history; observe assertion failures on current code.
- [x] Implement WriteTransaction and wire shared paths; captured strict read precedes policy, backup publication precedes topic publication; unique tempfile staging preserves existing permissions.
- [x] Add real subprocess contention and call-site failure/backup regressions; run focused tests and observe success.
- [x] Independent final reviewer actually applies transaction and call-site mutations; record killed/surviving counts and close Important gaps after observing them.
- [x] Run ./scripts/gate.sh --check as the final broad check plus integration test target. Commit owned files with evidence and publish only cleared commits.

## Rulings

Store-wide locking covers topic aliases and anchor updates without changing client protocols. Internal onboarding regenerations bypass shrink policy while preserving prior bytes. Deletion keeps existing semantics and participates in the lease. Atomic publication uses unique staging paths instead of the shared .tmp filename. History is not a security boundary against arbitrary concurrent filesystem mutation; no such guarantee is asserted.

## Verification evidence

Initial integration tests: 1 pass/8 fail on base86; after implementation all cases pass. Expanded final integration: 15 passed, 0 failed, two ignored helpers explicitly executed by subprocess scenarios. Linux RLIMIT_FSIZE1024 test demonstrated partial final backups RED; unique pending-file publication passed GREEN with EFBIG as the measured failure. Independent review initially killed four of five mutations; nested history permission mutation survived and failed after assertions for all history directories. Parent-directory sync and stale header findings corrected. No power-loss injection or Windows runtime acceptance was performed.

Focused MCP/public-private and dashboard forced-route tests passed with --features dashboard. Fresh final dashboard suite: 7 passed, 0 failed. Independent restored integration: 15 passed, 0 failed, two worker helpers. Initial mutation tally was 4 killed / 1 surviving / 0 inconclusive; only the surviving nested-permission mutation was rerun after the final publication changes, and it was killed. The other four were not rerun. Final full ./scripts/gate.sh --check exited0: FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0, PYTHON_LIGHT=0. Python light: 1223 passed and 424 subtests. Gate does not cover heavy Python, server-stack, real ONNX weights, Windows or laptop activation. No remaining review findings; no deferred minor findings. Only owned feature-branch commits are published, preserving the shared experiments checkout and its uncleared peer commits.
