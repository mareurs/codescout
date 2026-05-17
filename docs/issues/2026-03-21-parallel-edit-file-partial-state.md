---
status: mitigated
opened: 2026-03-21
closed:
severity: medium
owner: marius
related: ["BUG-033"]
tags: ["edit_file", "parallel", "transaction", "by-design"]
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

No code fix planned. Mitigation is the rule: never dispatch parallel write tool calls.

## Tests added

N/A — by-design behavior; not a regression and not test-coverable without protocol change.

## Workarounds

Sequence writes. If a multi-file change must be transactional, batch via `edit_file(edits=[...])` (single-file atomic batch) or do the writes sequentially in separate tool calls with explicit success-check between each.

## Resume

If a future MCP protocol revision adds transactional write semantics, revisit. Concrete next action: monitor rmcp's transaction proposals; current status as of 2026-05-17 is no such proposal exists.

## References

- Originally tracked as **BUG-021** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Crash-mode fix: rmcp 1.2.0 cancellation-race fix.
