---
status: wontfix
opened: 2026-03-21
closed: 2026-05-17
severity: medium
owner: marius
related: ["BUG-033"]
tags: ["edit_file", "parallel", "transaction", "by-design"]
kind: bug
---

# BUG: Partial state after parallel `edit_file` calls (by design)

## Summary

Dispatching two `edit_file` (or any write-tool) calls in parallel has no transaction semantics. If one is denied by the permission dialog and the other succeeds, files end up half-applied. The crash mode in rmcp was fixed in 1.2.0 (cancellation-race fix); the partial-state semantics remain by design. Mitigated by rule, not by code: never dispatch parallel write tool calls.

## Symptom (Effect)

Two simultaneous `edit_file` calls; one approved, one denied. Result: half the intended file set on disk modified, half not. No rollback. Subsequent reads see an inconsistent project state until the failed write is retried.

## Reproduction

Dispatch any two write-tool calls in the same parallel batch and deny one in the permission dialog. The accepted one applies; the denied one does not; nothing rolls back the accepted one.

## Environment

- Date opened: 2026-03-21
- Tool: `mcp__codescout__edit_file` (also applies to `edit_code`, `create_file`, `edit_markdown`)

## Root cause

`edit_file` and siblings each issue independent writes to disk with no shared transaction or rollback log. The MCP protocol has no concept of "wait for all denials/approvals to settle before any write fires." Once a write is approved and dispatched, it applies. By design.

## Evidence

Direct observation: parallel batch with one denied + one approved write leaves the approved write applied. No automated test — the behavior is architecturally inherent.

## Hypotheses tried

1. **Hypothesis (the crash mode):** rmcp cancellation race on parallel writes. **Verdict:** Confirmed and fixed in rmcp 1.2.0. **Evidence link:** rmcp 1.2.0 release notes.
2. **Hypothesis (the partial-state mode):** Implement a write barrier that gates each write on all sibling parallel writes resolving. **Verdict:** Rejected — would require MCP protocol changes; the rule-based mitigation is sufficient. **Evidence link:** see Workarounds.

## Fix

No code fix in codescout. The crash mode (rmcp cancellation race) was fixed upstream in rmcp 1.2.0; the partial-state semantics are by-design at the MCP protocol level — there is no "wait for all denials/approvals to settle before any write fires" primitive. A write-barrier protocol change is upstream of rmcp and outside codescout's scope.

**Architectural review 2026-05-17 (M6 closure):** Status flipped from `mitigated` to `wontfix`. `mitigated` falsely implied an unresolved root cause living in codescout's code; the actual root cause lives in the MCP protocol's lack of write transactions. The durable answer is the rule (sequence writes; use `edit_file(edits=[...])` for single-file atomic batches). If a future MCP / rmcp revision adds transactional write semantics, reopen and revisit.
## Tests added

N/A — by-design behavior; not a regression and not test-coverable without protocol change.

## Workarounds

Sequence writes. If a multi-file change must be transactional, batch via `edit_file(edits=[...])` (single-file atomic batch) or do the writes sequentially in separate tool calls with explicit success-check between each.

## Resume

If MCP / rmcp ever ships transactional write semantics, reopen this file: revisit the partial-state mode and decide whether to wire codescout's write tools to the new primitive. Concrete next action: monitor rmcp's release notes for "transaction" / "write barrier" keywords. Current status as of 2026-05-17 is no such proposal exists. If parallel-write violations start recurring in practice (i.e. tracked occurrences in tool-usage-patterns.md), promote the rule to a `codescout-companion` PreToolUse hook that blocks parallel write batches.
## References

- Originally tracked as **BUG-021** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Crash-mode fix: rmcp 1.2.0 cancellation-race fix.
