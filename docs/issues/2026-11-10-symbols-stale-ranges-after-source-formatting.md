---
id: '4e7ec7bfc047ac7b'
kind: bug
status: open
title: symbols returns stale ranges and mismatched bodies after source formatting
tags:
- navigation
- cluster/unclassified
closed: null
opened: 2026-11-10
owner: marius
severity: medium
---

# BUG: symbols returns stale ranges after formatting

## Summary

Pinned `symbols` reads on the retained BFS identity worktree label ranges from an earlier, shorter source layout, then extract unrelated current bytes as the named body. A post-compaction workspace cache flush did not clear the observed mismatch. This is parked outside the call-graph repair.

## Class fit

Unclassified until the stale-cache mechanism is established. The measured fault joins a symbol label/range to different current source bytes; it differs from a wrong caller-resolution result because even a direct named body read is misaligned.

## Symptom (Effect)

`symbols(name="call_site", path="src/tools/symbol/call_graph/identity.rs", include_body=true)` returns a function range 136–186 whose body begins `let coverage = budget.coverage_requiring_a_candidate();`. A forced current read at line169 begins the actual `fn call_site(edge: &Edge) -> Option<CallSite>`. The same discrepancy was observed before and after `workspace(post_compact=true)`.

## Reproduction

Use the retained `.worktrees/ecc-bfs-identity-20261110` at base `7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345` plus the immutable initial candidate patch. Run the pinned named body read and compare actual lines169–185. [Exact captured responses](../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/navigation-range-counterexample.json); [source byte manifest](../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/candidate-source.json).

## Environment

Live codescout MCP server, current Codex session, Rust source changed through structural tools and subsequently formatted. Parent and independent reviewer both observed misaligned named ranges in separate retained worktrees.

## Root cause

Unknown — the source lookup and range cache have not been traced. Do not infer which cache owns this from the symptom alone.

## Evidence

The retained JSON contains both tool responses, explicit workspace, source manifest reference and post-compaction flush observation. The earlier similar observation is bug-fix-session-log:F-220; its cache-flush workaround was not sufficient for this current case.

## Fix

Pending; retain as independent navigation work. Use forced bounded reads of actual bytes after named output contradicts them. Structural edit tools validate ranges separately; verify every resulting diff.

## Tests added

None — no navigation fix attempted. Runtime counterexample captured before any subsequent source edits.

## Resume — 2026-11-10

Open, outside the current bounded call-graph task. No exhaustive duplicate audit; targeted catalog title and semantic searches did not return an existing active record.
