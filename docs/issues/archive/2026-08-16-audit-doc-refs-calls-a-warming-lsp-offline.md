---
id: '49e562fcc9ab125b'
kind: bug
status: fixed
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

Traced (was "Unknown — not yet traced to a line" when filed). The working hypothesis was
right in shape and understated the problem.

`scan_meta.lsp_languages_offline` was fed from exactly one writer,
`resolver.rs::note_degraded`, called from three sites — and only the first is anything
like "offline":

1. **`ctx.lsp` is `None`** — no language server wired for the scan at all. The one case
   the word described honestly.
2. **The LSP answered, but without a symbol tree-sitter finds in the same file** — the
   server is mid-index. The branch's own comment says so: *"The server ANSWERED, and
   tree-sitter disagrees with it — so the symbol exists and the server is behind its own
   index."* This is a **provably live** server being reported as offline.
3. **No answer within `LSP_FIRST_CALL_BUDGET`** — a cold start, indistinguishable from a
   hung server without probing further. Not offline either.

`note_degraded` took only `(ctx, lang)` and pushed a bare language name, so all three
collapsed into one list under a name that asserted the rarest of them about all of them.

This also explains the scan-size dependence the filing could not account for. Scan size
is a proxy for *how many chances the run had to hit a mid-index answer* — a ~40-file scan
hits case 2 or 3 at least once while a ~3-file scan minutes later does not. Nothing about
server state changed between them, exactly as observed.

What the filing got wrong: it suspected `degraded` itself might be over-firing. It is
not. `degraded: true` for case 2 is correct and deliberate —
`docs/issues/archive/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md` added that
branch because a mid-index server silently costs 60-69 resolutions. The coverage really
was incomplete. Only the word "offline" was false.
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

Fixed on `experiments` in `56fe1dd4`. Both filed parts, without undoing the 2026-08-06 fix.

**Part 1 — the field is renamed to what it measures.** `lsp_languages_offline` →
`lsp_languages_degraded`, in the response, in `ScanMeta`, and in the tracker's
`render_template.j2` (which rendered "⚠ degraded (rust offline)" into a human-facing doc).
A `#[serde(default, alias = "lsp_languages_offline")]` keeps trackers written under the
old key loading — without it the rename would read every pre-existing tracker as
never-degraded, replacing one false claim with another.

**Part 2 — warming is now distinguishable from down.** The cause travels with the
language instead of being dropped at the call site. A new `DegradedCause` enum names the
three, and the response carries a per-language `degraded_causes` map beside the flat list:

- `no_server` — nothing is wired; re-running will not help.
- `lsp_behind_index` — the server is **up** and mid-index; re-running resolves it.
- `no_answer_within_budget` — no answer inside the budget. A cold start and a hung server
  look identical from here, so neither is claimed.

That is the actionable bit the flat list threw away: a caller can now tell "re-run this"
from "this is as good as it gets" without guessing from a word.

Deliberately unchanged: `degraded` still reads `true` for `lsp_behind_index`. The scan's
coverage genuinely was incomplete; that flag is not the defect.

One incidental cleanup the change forced: `build_response` reached clippy's argument
ceiling, so the two values are bundled into a `Degradation` struct — which also stops the
language list and its causes from drifting apart, the exact failure mode being fixed.
## Tests added

Four, two new and two strengthened.

- `scan_meta_reports_degradation_without_calling_a_live_server_offline` (`mod.rs`) — pins
  the response surface: `degraded` still true, the cause surfaced as `lsp_behind_index`,
  the old key **gone** rather than merely joined, and no part of `scan_meta` containing
  the string `offline`.
- `scan_meta_still_loads_a_tracker_written_under_the_old_field_name` (`mod.rs`) — pins the
  serde alias. Mutation-verified: dropping the alias makes an old tracker deserialize to
  `[]` ("never degraded") and fails this test. Nothing else in the suite would have
  noticed.
- `resolver_unknown_when_lsp_offline` — strengthened from "some entry mentions rust" to
  asserting the pair `("rust", NoServer)`, so the one honest case stays distinguishable
  from the two live-server ones.
- `resolver_defers_to_the_ast_when_the_lsp_lags_behind_disk` — strengthened to assert the
  cause is `lsp_behind_index`, i.e. that a server which answered is never reported as
  offline.

Gate: `cargo fmt` + `cargo clippy --all-targets -D warnings` clean, `cargo test --lib`
3748 passed / 0 failed / 7 ignored.
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
