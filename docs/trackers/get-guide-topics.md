---
kind: tracker
status: active
title: Get-guide topic candidates (Surface D)
owners: []
tags:
  - prompts
  - get_guide
  - surface-d
Last updated: 2026-05-19
---

# Get-guide topic candidates (Surface D)

This tracker is **Surface D** of the MCP prompt channel redesign (see
`docs/superpowers/plans/2026-05-19-mcp-prompt-channel-redesign.md`). It
catalogues markdown content that should be retrievable on demand via the
`get_guide(topic)` tool, instead of being baked into the always-loaded
`server_instructions.md`.

**Rule:** anything that is *occasionally* useful — language-specific
navigation tips, deep-dive workflows, edge-case troubleshooting — belongs
here as a `get_guide` topic, not in the global prompt. The global prompt
stays small (Iron Laws + tool inventory + decision tree); guides ship
long-form content only to sessions that ask.

## Candidates

| # | Topic slug | Source | Notes |
|---|---|---|---|
| 1 | `progressive-discoverability` | `docs/PROGRESSIVE_DISCOVERABILITY.md` | Canonical patterns + anti-patterns for tool output sizing. Loaded today by humans + by Claude before adding a tool; promote to `get_guide` so LLMs can pull it on demand. |
| 2 | `prompt-surface-consistency` | `CLAUDE.md § Prompt Surface Consistency` + `src/prompts/README.md` | The 3-surfaces rule and the 7 writing rules. Useful only when *editing* a prompt surface. |
| 3 | `symbol-navigation` | extract from prior `src/prompts/language_nav.rs` (deleted in U4; content lives in git history at commit before the source.md rewrite — `git show HEAD~1:src/prompts/language_nav.rs`) | Per-language tips for `symbols` / `symbol_at` / `references` (Rust trait impls, Python decorators, TS generics, etc.). |
| 4 | `tracker-design` | `librarian(action="tracker_design")` output + `docs/trackers/README.md` | Archetype library + teaching prompt. Today fetched via a librarian action; mirror the content as a `get_guide` topic for clients that don't run librarian queries. |
| 5 | `bug-tracking-workflow` | `CLAUDE.md § Bug Tracking` + `docs/issues/_TEMPLATE.md` | When + how to open a `docs/issues/<date>-<slug>.md`, status vocabulary, archive rules. |
| 6 | `git-workflow` | `CLAUDE.md § Git Workflow` | Branch strategy, ship sequence, concurrent-work rules, cross-repo SHA convention. Long and stable — perfect on-demand content. |
| 7 | `release-checklist` | `CLAUDE.md § Release Cycle` | The 8-step release dance. Only relevant when actually cutting a release. |

## Next steps

1. Implement the `get_guide(topic)` tool (Plan Task 18+).
2. For each candidate above, decide: extract verbatim, rewrite for the
   LLM consumer, or split into multiple sub-topics.
3. Add an integration test that every advertised topic resolves to a
   non-empty body.
4. Once `get_guide` ships, prune the source sections that are pure
   duplicates of the loaded content in `server_instructions.md`.

## Status

- Last updated: 2026-05-19
- Owner: U7 implementer (Surface D delivery)
- Blocking: Plan Task 18 (get_guide tool wiring)
