---
id: aec87d061236ec7b
kind: plan
status: done
title: ECC Codex pilot real-code evaluation plan
tags:
- ecc
- codex
- evaluation
topic: ecc-integration
---

# ECC Codex pilot evaluation — 2026-11-09

## Objective and scope

Evaluate the installed two-skill adapter alongside the existing codescout setup. Target src/librarian/tools/create.rs::call, src/librarian/tools/temp_write_guard.rs, nearest dispatcher/error conversion and directly relevant test assertions. Extract a bounded observed-behavior draft and perform read-only silent-failure review.

## Completed setup

- [x] Preserve upstream provenance/license and isolate adapter development in its worktree.
- [x] Verify native loading, real fixture navigation, librarian persistence and regression checks.
- [x] Back up and activate only the pilot in the default profile; verify both skills after user restart.

## Current comparison protocol

Two fresh native Codex CLI evaluations use the same installed default profile/model, source scope, repository instructions and codescout connection. Baseline explicitly avoids reading/invoking the ECC adapter bodies; pilot invokes ecc-spec-miner and ecc-silent-failure-review. Both expose installed metadata. Baseline runs first, pilot second. Neither changes profile config.

The initial attempt to remove the pilot through a per-command enabled=false override failed its native discovery control before model execution. Do not reinterpret the revised pair as plugin absent versus present.

Each arm returns a chat-only spec/review. Preserve raw JSONL, stderr, prompts, native terminal usage records and elapsed subprocess time. Snapshot Rust source/tests, Cargo files and repository instructions before/after, and profile config hashes. Check actual installed skill reads and completed MCP calls rather than agent prose alone.

The parent persists results only after both native evaluations have completed, preventing new findings from becoming evidence for the second arm.

## Acceptance and adjudication

- [x] Confirm actual pilot body reads and native codescout navigation; audit baseline non-use.
- [x] Reconcile proposed findings against real enforcement, caller-visible results and contracts.
- [x] Save observed-behavior draft, review and comparison with source/config stability limits.
- [x] Record actual repository verification outcome and validate tracker links.
- [x] Update index, evidence and issues without claiming an unmeasured effectiveness gain.

No source changes, source mutation probes or automatic fixes are in this evaluation. Distinguish existing fallback contracts and incomplete runtime evidence from confirmed defects. A useful result may be parity or inconclusive benefit.

## Next decision

After this pair, decide whether to retain the adapter as an optional workflow and what additional cases would discriminate its value. Additional paid cohorts, full ECC integration, merge and push are not implied by creating this tracking folder.
