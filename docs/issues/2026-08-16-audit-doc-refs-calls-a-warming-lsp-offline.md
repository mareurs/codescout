---
id: '49e562fcc9ab125b'
kind: bug
status: open
title: audit_doc_refs reports lsp_languages_offline:[rust] while the LSP is up and answering
tags:
- audit_doc_refs
- lsp
- misleading-error
- tool-quirk
---

## Summary

`librarian(audit_doc_refs)` sets `scan_meta.degraded = true` and
`scan_meta.lsp_languages_offline = ["rust"]` on mid-size scans while rust-analyzer is
running and answering queries. The field asserts the server is *offline*, which is false.
An agent reading it discards a legitimate audit as untrustworthy.

## Symptom (Effect)

Three calls, same session, minutes apart, same MCP server:

```
# ~40 files
{"n_refs_found": 4097, "n_refs_broken": 1091,
 "scan_meta": {"degraded": true, "lsp_languages_offline": ["rust"]}}

# references() against the same server, immediately after
references(symbol="resolve_sqlite_dir", path="src/retrieval/config.rs")
→ 9 results returned from the LSP, plus a warming caveat:
  "LSP returned 0 references outside the definition file ... the reference
   index may still be warming after a reindex"

# ~3 files
{"n_refs_found": 116, "n_refs_broken": 12,
 "scan_meta": {"degraded": false, "lsp_languages_offline": []}}
```

The server that is "offline" for the 40-file scan answers a symbol query seconds later,
then is "online" again for a 3-file scan.

## Reproduction

Restart the MCP server (`cargo rb` + `/mcp`), then without waiting for the LSP reference
index to warm:

1. `librarian(action="audit_doc_refs", paths=["docs/trackers/*.md", "CLAUDE.md"])`
   → `degraded: true`, `lsp_languages_offline: ["rust"]`
2. `references(symbol=<any rust fn>, path=<its file>)` → returns results
3. `librarian(action="audit_doc_refs", paths=["CLAUDE.md"])` → `degraded: false`

## Environment

Linux, Claude Code, stdio transport, codescout `experiments`, immediately after a
`cargo rb` rebuild and `/mcp` reconnect. Observed 2026-08-16 during a tracker-hygiene sweep.

## Root cause

Unknown — not yet traced to a line. The *behaviour* is measured (three calls above,
2026-08-16); the mechanism is inferred, not read. Working hypothesis: the flag is set when
one or more symbol lookups fail to resolve inside the scan's time/attempt budget, and a
warming-but-alive rust-analyzer produces exactly that. Whatever the mechanism, the reported
state ("offline") is not the observed state (up, answering, index warming), and the
threshold is scan-size-dependent rather than server-state-dependent.

Note this contradicts a prior in-repo belief that scoped scans stay non-degraded while
repo-wide ones degrade at ~276+ files: here a ~40-file scan degraded and a ~3-file scan
did not, minutes apart, with no change in server state. Scan size is a proxy, not the cause.

## Evidence

The discriminating probe is the middle call. `symbols()` is tree-sitter-backed and succeeds
whether or not the LSP is up, so it cannot distinguish the states; `references()` requires
the LSP and therefore can. Any future triage of this bug should use `references()`, not
`symbols()`, as the liveness test.

## Hypotheses tried

1. **Hypothesis:** rust-analyzer really was down after the rebuild.
   **Test:** `references()` against the same server between the two audit calls.
   **Verdict:** rejected — it returned 9 results from the LSP.

## Fix

Not implemented. Two parts, independent:

1. **Rename or re-scope the field.** `lsp_languages_offline` should report liveness, or be
   renamed to what it measures (e.g. `lsp_languages_unresolved` / `degraded_reason`). A
   field that says "offline" about a live server is a false statement in a machine-readable
   surface, and this class of misleading self-report is what the repo's own R-89 warns about.
2. **Distinguish warming from down.** If the LSP answers at all, the scan is warming, not
   degraded — either wait, retry, or label the result accordingly.

## Tests added

None yet.

## Workarounds

Probe the LSP directly with `references()` before believing `scan_meta`. If it answers,
treat `degraded: true` as "this scan's symbol resolution was incomplete", not as "the
language server is down" — and re-run the scan scoped smaller, which resolves cleanly.

## Resume

Read the `scan_meta` / `degraded` construction site in the `audit_doc_refs` implementation
under `src/librarian/tools/audit_doc_refs/` and determine what actually sets
`lsp_languages_offline`. Confirm whether it is a liveness probe or an unresolved-symbol
tally before choosing between the rename and the retry fix.

## References

- Surfaced by the tracker-hygiene sweep 2026-08-16 (`docs/trackers/tracker-hygiene-log.md`)
- `docs/trackers/reconnaissance-patterns.md` § R-89 — "a tool's output is evidence about the
  code only if the running build contains it"; this is its sibling for tool self-reports

