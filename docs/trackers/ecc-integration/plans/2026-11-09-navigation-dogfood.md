---
id: '0ea82264703c00fe'
kind: plan
status: active
title: 'ECC dogfood: navigation completeness and recoverable failures'
tags:
- ecc
- dogfood
- navigation
topic: ecc-integration
---

# Navigation completeness dogfood — 2026-11-09

> User authorized committing/integrating the first correction and moving to this next case. Execute inline; ECC spec-miner and silent-failure-review remain the applied workflows.

**Goal:** Determine whether references/call_graph let the consumer distinguish a trustworthy negative from unavailable or incomplete analysis, and correct one confirmed bounded defect if present.

**Baseline commit:** `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`; tree `34977f2047694714e5f852ee427c2dfca435d9e8`.

**Workspace:** `/home/marius/work/claude/codescout/.worktrees/ecc-navigation-truth`, branch `feat/ecc-navigation-truth-20261109`. Native managed-worktree creation failed transport; Git fallback was used after checking registrations and ignored directory. Initial tracked source is clean; shared parent has unrelated edits/staging.

**Contract:** ADR 2026-08-27 negative-results-name-their-scope requires a suspicious negative to name its scope, a trustworthy negative to stay quiet, and no unproved cause to be asserted.

## Observed boundary

Code inspected this session: CallGraph::call, CachedResolver::lookup_pos and one_hop, BFS, resolve_one_hop, resolve_via_lsp, resolve_via_ts, resolve_callees_via_ts, format_output/format_compact/attach_scan_incomplete. Actual references and call_graph traced resolve_one_hop callers. Existing scan-cap/unreadable-definition warnings are already implemented; those are not new findings. references::call delegates to call_with_settle, which has dedicated completeness/settling tests; that surface is not yet fully audited.

Potential distinct gap: CachedResolver::one_hop converts RecoverableError from resolve_one_hop into an empty edge vector without recording that skipped hop. The outer tool discloses definition-walk failures only. A lower-level unsupported/unreadable/non-function callee fallback error could therefore become a clean empty/partial graph. This is a code-inspected conditional finding, pending a concrete consumer-boundary reproduction. Caller fallback also skips unreadable reference files; that separate path remains a lead, not an additional confirmed finding.

## Approach

Start with real positive/empty/unsupported controls and an isolated catalog. If the suspected hop-error suppression is reproduced, preserve best-effort partial graphs while disclosing the failed symbol, direction and actual reason through the existing completeness_warning output path. Do not conflate resolver errors with definition-scan caps, warn on every zero, or change unrelated resolution semantics. A proposed runtime failure trigger is a non-function symbol with no LSP call hierarchy, alongside an actual leaf function control; fixture viability must be observed rather than assumed.

## Steps

- [x] Freeze baseline commit/tree and inspect enforcement/callers/contract/known bugs.
- [x] Finish post-integration verification before source edits: exact frozen baseline gate passed, FMT/CLIPPY/LEAN/DEFAULT/PYTHON_LIGHT all 0, with external scratch and `CODESCOUT_GATE_TMPFS_BASE=off`. Earlier cut-off and quota-failed runs are retained in the first report.
- [ ] Reproduce the navigation suspicion through real MCP or a deterministic end-to-end tool test. Preserve no-warning leaf and actual edges as controls.
- [ ] Record/claim a distinct bug after checking related archived scan bugs; preserve the exact starting code and task inputs for future replay. No replay runs now.
- [ ] Observe a failing regression before production edits; implement the smallest scoped correction using edit_code.
- [ ] Execute focused tests, independent review with actually applied diagnostic mutations and exact restoration, real MCP checks and the required full gate.
- [ ] Record result/limits and index artifacts. Do not conflate this new cycle's baseline with the first cycle's fe965888 commit. Integration of a second correction is a later step only after verification.

## Review focus

The warning must reach compact/buffered consumer output, retain any existing scan warning, preserve useful other-direction/other-node edges, name the true failed scope/reason, and remain absent on a fully analyzed leaf. Do not count a lower-level helper test as proof that the outer Tool/MCP output carries the failure. Mutations must target the changed disclosure call site, not just an already-tested shared formatter.

## Execution ledger

- First correction source commit `561c945d72a3997c972d5ef055e2c9d38e922e67`; integrated on experiments as this baseline `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`. Foreign staged path set preserved; no push or server deployment.
