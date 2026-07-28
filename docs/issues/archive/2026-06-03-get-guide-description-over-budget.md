---
status: fixed
opened: 2026-06-03
closed: 2026-06-03
severity: low
owner: marius
related: []
tags: [prompt-surface, tool-description, test-gate]
kind: bug
---

# BUG: `get_guide` tool description exceeds the 300-char budget (302), failing `tool_descriptions_stay_under_budget`

## Summary
The `get_guide` tool description is 302 characters; the per-tool description cap is 300.
`cargo test --lib` fails on `server::tests::tool_descriptions_stay_under_budget`. Pre-existing
on `experiments` (noticed incidentally 2026-06-03 while running the full suite for the
kotlin-lsp analyzer-disk fix — unrelated to that change).

## Symptom (Effect)
```
---- server::tests::tool_descriptions_stay_under_budget stdout ----
thread '...' panicked at src/server.rs:1598:13:
tool `get_guide` description is 302 chars (cap 300): "Deep guidance for a topic. Use when the
system prompt points here. Topics: librarian | librarian-runtime | tracker-conventions |
progressive-disclosure | error-handling | workspace-state | iron-laws-detail |
symbol-navigation. No args = list topics + summaries. Large bodies overflow to @tool_* buffer."
```
`cargo test --lib` → `test result: FAILED. 2603 passed; 1 failed`.

## Reproduction
1. `git rev-parse HEAD` (any recent `experiments` tip with the 8-topic `get_guide` description).
2. `cargo test --lib tool_descriptions_stay_under_budget` → fails (302 > 300).

## Environment
Arch Linux, Rust (codescout v0.14.0), branch `experiments`. Not transport- or project-specific
(pure unit test on a static string).

## Root cause
The `get_guide` description literal at `src/tools/guide.rs:42` enumerates 8 topics
(`librarian | librarian-runtime | tracker-conventions | progressive-disclosure |
error-handling | workspace-state | iron-laws-detail | symbol-navigation`) plus trailer text,
totalling 302 chars. The budget gate `server::tests::tool_descriptions_stay_under_budget`
(`src/server.rs:1598`) caps each tool description at 300. A topic added to the list (or the
trailer) pushed it 2 chars over without the test being re-run.

## Evidence
- `src/tools/guide.rs:42` — the description literal (begins `"Deep guidance for a topic. ..."`).
- `src/server.rs:1598` — the cap assertion (`cap 300`).

## Hypotheses tried
1. **Hypothesis:** caused by the 2026-06-03 kotlin-lsp analyzer-disk change.
   **Test:** that change edits only `src/lsp/servers/mod.rs` + `src/lsp/mux/process.rs`
   (+ docs); tool descriptions are static strings in neither.
   **Verdict:** rejected — pre-existing, independent of the kotlin work.

## Fix

**Implemented 2026-06-03 on `experiments`.** Trimmed `" buffer"` from the description trailer at `src/tools/guide.rs:46` (`@tool_* buffer.` → `@tool_*.`): 302 → 295 chars, no topic dropped. Verified: `cargo test --lib` → 2604 pass, 0 fail (`tool_descriptions_stay_under_budget` now green). On `experiments`; master-side SHA cited on ship to master.
## Tests added
N/A — the regression gate already exists (`tool_descriptions_stay_under_budget`); it is
currently red and will go green when the description is trimmed.

## Workarounds
None needed for runtime (descriptions still function over-budget); the only impact is the
red test gate blocking a clean `cargo test --lib`.

## Resume

Fixed (see ## Fix). On `experiments`, not yet on master — ships via Standard Ship Sequence (alongside or independent of the kotlin-lsp analyzer-disk batch); `git mv` to `docs/issues/archive/` + cite the master-side SHA on ship. N/A otherwise.
## References
- `src/tools/guide.rs:42` (description), `src/server.rs:1598` (budget gate).
- Noticed during the kotlin-lsp analyzer-disk fix session
  (`docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`).
