---
id: ed2a2e472dba693d
kind: spec
status: draft
title: doc(create) finalization — bounded observed behavior
tags:
- ecc
- observed-behavior
- librarian
topic: ecc-integration
---

# doc(create) finalization — observed behavior

> Draft from a bounded ECC-assisted inspection/probe, not a declaration that the old implementation is intended. Baseline038fffb3447b55900e617a251b6472486259e07c; final candidate source hash a6febbcb5bada021e0384cfeda9ccd4e527e840c35c9639ed9d043389af27e49. Read the [dogfood report](2026-11-09-create-finalization-dogfood.md) for final verification/integration state.

## Scope and provenance

Owned feat/ecc-create-finalization-20261109 branch; only src/librarian/tools/create.rs changes. Inspected create::call, AugmentSpec, artifact::upsert_and_mint_slug/ensure_slug, augmentation::upsert, worktree::ensure_registration and upsert_active. Exact references establish registration consumers; the sibling resolve_write_target transaction was read as a working pattern. A shared older-runtime call_graph on duplicated Rust call names contradicted the inspected body and was excluded as evidence; follow-upe5016b19fb0f75da is separate and now archived after the later identity correction.

### Requirement: successful augmented create persists the requested artifact

- Behavior: WHEN normal creation succeeds, THEN the file, artifact/slug and augmentation exist and can be read; a linked-worktree create also has a durable registration.
- Enforcement: create::call and direct catalog helpers above.
- Evidence state: code-inspected; existing48 module tests executed-pass; new helper's clean retries executed-pass in51-test GREEN run. Private real-MCP confirmation is recorded separately in the report.
- Contract status: existing success behavior, preserved.
- Unknowns: full augmentation schema/entry-collection validation semantics are outside this finalization scope.

### Requirement: catalog-stage failure precedes user-visible file creation

- Baseline behavior: failures during slug or augmentation insertion leave an artifact row for a missing file; registration is later than disk write and can fail after those earlier effects. Three real SQLite-trigger tests observed extra row2 vs expected1 after an actual injected SQLite cause.
- Candidate behavior: a catalog transaction includes row/slug/augmentation/registration, committing before directory/file creation; an error rolls the catalog phase back.
- Enforcement: create::call transaction boundary; actual failure points are ensure_slug, augmentation::upsert and worktree::ensure_registration.
- Evidence state: code-inspected; three independent REDs at baseline followed by executed-pass51-test module. Each test checks the SQLite cause, no new parent directory, unrelated keeper preservation and clean retry after removing only its trigger.
- Contract status: desired atomic creation with augmentation is documented on AugmentSpec; catalog-stage atomicity is the bounded correction, not a promise of cross-filesystem/database crash atomicity.
- Unknowns: process crash/power loss between the committed catalog phase and disk write is not simulated.

### Invariant: disk failure retains best-effort cleanup and its original cause

- Behavior: WHEN directory/file writing fails after catalog commit, THEN cleanup is attempted and the fatal error retains its original IO source while warning that rows/partial files may remain.
- Enforcement: existing create::call disk-error branch and undo_failed_create; not changed by this correction.
- Evidence state: existing failed_disk_write_reports_best_effort_cleanup_without_promising_rollback executed-pass within both48-test baseline and51-test GREEN.
- Contract status: current intentional best-effort cleanup, not guaranteed rollback.
- Unknowns: cleanup failures are reported truthfully; this correction does not make filesystem/catalog commits atomic.

## Open questions and coverage limits

Transaction-commit/storage failure and process-crash behavior need evidence distinct from statement-abort triggers. Independent applied-mutation review found a worktree fixture coverage gap. After observing the survivor, both downstream failure tests were extended across regular/worktree contexts; exact M7 was killed49/2 and restored module51/0. Production prefix unchanged. Shared catalog/state/profile was not fault-injected. This is dogfood, not a no-ECC repair replay or effectiveness comparison.


Final evidence: private actual-MCP5 failure/retry/read controls and the final frozen gate passed. Source/integration identity and review populations are in the final report. No cross-store crash atomicity claim was added.
