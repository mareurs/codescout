---
id: '4578968cbf719dfb'
kind: review
status: draft
title: Tracker archiving architecture review
owners:
- marius
tags:
- tracker-archiving
- architecture
- review
topic: Architecture review of tracker archiving scaffolding
---

# Tracker archiving architecture review

Review of [the draft design](2026-10-09-tracker-archiving-scaffolding-design.md), requested on 2026-10-09. Status: recommendations for design revision; no implementation approval, archive execution, or policy ratification.

## Verdict

Keep one preservation service, with explicit requests and future automatic selection using the same integrity checks. Narrow the first working implementation to declared prose entry transfer and whole-tracker retirement. Build and verify the read-only planner before enabling apply. Augmented entry transfer needs a durable row-storage contract; generated trackers need producer participation.

The draft currently combines the foundation with support for additional tracker formats. Its preservation requirements should remain binding when apply is introduced; reducing the first supported formats does not justify weakening identity, citation, counter, or recovery checks.

## Findings

### Archived structured rows need a committed payload

The draft transfers complete params rows and retains an entry_collection, but does not identify their durable on-disk representation or reconstruction path. Existing [augmentation sidecars](../../../src/librarian/augmentation_sidecar.rs), to_row at lines 132–156, deliberately restore params as an empty object. write_through at lines 246–317 projects shape rather than row data. [AugmentationRow](../../../src/librarian/catalog/augmentation.rs), lines 8–33, holds params as catalog JSON.

Consequently, same-catalog row equality cannot establish preservation after catalog reconstruction. This is a gap in the proposed archive contract, not a bug in the deliberate sidecar contract. Require a committed payload and restore path before supporting structured entry transfer. Reconnaissance record: tracker-archiving-session-log:W-2.

### Recovery needs a lifecycle gate

The proposed manifest and SQLite marker explain archive retry but do not specify how ordinary append, update, move or reindex behaves after interrupted publication. [WriteGuard acquire](../../../src/agent/write_guard.rs), lines 176–255, acquires mutex/flock and records the holder; it performs no archive recovery.

Specify the common entry point that checks pending operations under the lock before a subsequent mutation, and the reader behavior while recovery is pending. A recovery conflict must prevent affected writes from compounding the partial operation. Define catalog replacement explicitly: absence of a machine-local commit marker after catalog loss is not sufficient evidence that publication never committed. The exact state machine and integration point remain design work.

### Family identity needs a creation and adoption contract

The draft's seed combines artifact identity with inspected content. [artifact_id_from_abs](../../../src/librarian/ids.rs), lines 17–23, hashes the normalized absolute path; references include create and move. Independently initialized checkouts can therefore choose different family IDs, and different revisions also alter the seed.

A persisted opaque ID can remain stable after declaration; that does not make independent declarations converge. Specify which declaration is adopted, how legacy companions join it, and how conflicting declarations are handled. Keep physical artifact identity distinct from logical ledger identity. No migration or independent-clone experiment was run.

### Share citation semantics without importing scanner orchestration

[resolve](../../../src/librarian/tools/link_scan/resolve.rs), lines 320–451, resolves qualified citations through physical file stems and gives bare local definitions precedence. Splitting siblings across files changes both assumptions.

The resolver imports extraction types and collections; [scanner orchestration](../../../src/librarian/tools/link_scan/mod.rs), lines 24–32, imports catalog entry citations, catalog find/links, scope and ToolContext. Share concrete parsing and family-resolution functions between scanning, planning and retrieval. Keep those functions independent of tool orchestration. The existing slug/params citation path also requires compatibility treatment; a scanner-only change is not a complete citation contract.

[entry_sections](../../../src/librarian/tools/link_scan/extract.rs), lines 67–121, uses the shared fence-aware heading parser but normalizes section text with lines().join("\n"). The archive payload needs original byte spans, as already recorded in tracker-archiving-session-log:W-1.

### In-place retirement has a restart tradeoff

In-place retirement preserves the physical path and avoids relocation repairs. It also keeps that path occupied when a distinct new stream wants the same name.

The [ratified policy](../../trackers/archive-cadence-policy.md), amendment beginning at line 145, uses dated moves partly to permit repeated archival of one name. The serving hygiene recipe still expects dated relocation. The draft already acknowledges that in-place retirement amends this policy; the review does not treat that acknowledgment as approval.

Define reopening the same ledger versus starting a new stream. Reconcile the taxonomy, ratified policy and serving recipe before releasing the changed retirement behavior. Existing documentation conflicts are already filed; no duplicate issue was created.

## Proposed decision 1 — First supported operations

**Decision:** Start with declared prose entry transfer and in-place whole-tracker retirement through one explicit preview/apply service; validate the planner before enabling apply.

**Context:** These cover live-ledger shrinking and completed-tracker retirement while the structured-row durability contract remains unresolved.

**Alternatives considered:** Supporting every tracker format immediately broadens the first implementation before its storage contracts exist. Retirement alone misses live-ledger shrinking. Filtering alone does not reduce the stored live ledger.

**Consequences:**
- now easier: both requested operations establish one preservation contract for future cleanup callers.
- now harder: augmented transfers and producer-managed operations remain explicitly unsupported until their contracts exist; a distinct restarted stream needs explicit naming/lifecycle rules, and in-place retirement requires policy reconciliation.

**Change scenarios absorbed:** A cleanup guide, automatic trigger or aggressiveness configuration changes candidate selection without changing transfer integrity.

**Revisit-when:** A durable structured-row format or producer contract is validated, or users require the former live path for a distinct new stream.

**Confidence:** High on narrowing format support and sharing the service; medium on adopting in-place retirement until restart rules and policy rollout are settled.

## Proposed decision 2 — Evidence and recovery authority

**Decision:** Make archived evidence reconstructible from committed payloads, and require recovery resolution before subsequent affected mutations.

**Context:** File publication and SQLite commit are separate boundaries. Existing sidecars omit row data, and existing locking does not recover pending archive operations.

**Alternatives considered:** SQLite-only evidence does not survive catalog loss. Recovery only when the archive request is retried permits ordinary writers to compound partial publication. Per-entry metadata entirely in frontmatter adds parsing and merge burden as history grows; its storage layout needs an explicit assessment, not an assumed performance result.

**Consequences:**
- now easier: catalog reconstruction and interrupted-operation handling have a declared authority.
- now harder: a durable payload format, explicit recovery state machine, catalog-loss behavior, reader behavior and mutation-gate integration must be specified and tested.

**Change scenarios absorbed:** Process death between publication boundaries, an append after interruption, catalog replacement and increasing archived history.

**Revisit-when:** The catalog becomes authoritative durable storage, or publication and recovery boundaries change.

**Confidence:** High on the preservation and gating requirements; medium on the eventual storage layout.

## Proposed decision 3 — Parsing, identity and policy boundaries

**Decision:** Share concrete raw-span parsing and family-resolution functions across archive planning, citation scanning and entry retrieval; keep selection policy outside preservation.

**Context:** These consumers must agree about defining headings, logical identity and physical proof location. Existing extraction and resolution already separate pure interpretation from catalog orchestration.

**Alternatives considered:** Copying parser/resolver logic allows semantics to diverge. Depending on scanner tool orchestration pulls catalog/context concerns into the archive core. A general adapter interface before concrete requirements emerge adds an unproven abstraction.

**Consequences:**
- now easier: split siblings retain coherent identity, retrieval and citation behavior, while future callers can select different candidates.
- now harder: declaration/adoption conflicts and legacy slug/params citations require explicit contracts; shared legacy behavior and family discrimination need regression guards.

**Change scenarios absorbed:** Moving a sibling to a companion, retrieving it from the root, reconstructing declared families and adding automatic selection callers.

**Revisit-when:** Concrete producers need behavior the shared functions cannot express.

**Confidence:** High on shared concrete semantics; medium on family creation/adoption until the conflict policy is specified.

## Evidence and limits

The review examined the draft and current source contracts. The findings about interrupted transfers, catalog loss and independent initialization are architectural inferences from those contracts, not runtime archive failures. No archive implementation exists; implementation mutation review is inapplicable at this stage.

The repository gate was attempted during the review and exited 2 before tests because CLAUDE_CODE_SESSION_ID is unset. No test-pass or recovery-success claim is made. This review changes documentation only.

Before implementing apply, add observed fixtures for byte conservation, semantic citation identity, durable counter preservation, archived retrieval, crashes followed by ordinary mutations, catalog replacement and foreign edits. Apply candidate mutations to the implemented logic and report observed survivors before writing tests intended to close those gaps.
