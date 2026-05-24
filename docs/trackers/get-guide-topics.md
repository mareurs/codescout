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

## Live topics

Shipped in Commit A (`e68f3a94`):

| Topic | Source file | Hint trigger |
|---|---|---|
| `librarian` | `src/prompts/guides/librarian.md` | Any librarian-adapter call (unconditional) |
| `tracker-conventions` | `src/prompts/guides/tracker-conventions.md` | — (manual `get_guide` only) |
| `progressive-disclosure` | `src/prompts/guides/progressive-disclosure.md` | `run_command` / `symbols` buffer overflow |
| `error-handling` | `src/prompts/guides/error-handling.md` | — (manual `get_guide` only) |

## Mechanism

First-call hint shipped in Commit C (U8+U9). Behavior:

- Each `Tool` may override `relevant_guide_topic() -> Option<&'static str>`.
  Default `None` — no hint emission.
- `Tool::call_content` checks `ctx.guide_hints_emitted` (a session-scoped
  `HashSet<String>` shared across all tools via `ToolContext`). If the topic
  has not yet been emitted, inject a `_guide_hint` field on the response.
- Dedup is **session-wide by topic**: calling a different tool with the same
  topic (e.g. another librarian adapter) does NOT re-emit.
- Reset: `workspace(action="activate")` clears the set as its first
  statement, so a new project starts fresh.
- For `progressive-disclosure`, the hint only fires when the tool returns
  overflow (`exceeds_inline_limit` OR an `output_id` field is present —
  the latter catches `run_command`'s pre-buffered envelope shape).

Tests live in `src/server.rs::guide_hint_tests` (6 cases).

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

1. Implement the `get_guide(topic)` tool (Plan Task 18+). **Done** — Commit A `e68f3a94` + mechanism in Commit C.
2. For each candidate above, decide: extract verbatim, rewrite for the
   LLM consumer, or split into multiple sub-topics.
3. Add an integration test that every advertised topic resolves to a
   non-empty body. **Done** — `every_topic_has_non_empty_body` + `schema_enum_matches_registered_topics` in `src/tools/guide.rs` (code-explorer:4c6f4b03). The schema-drift sibling catches "added topic to map but forgot the schema enum" — a class of silent-invisibility drift not covered by step 3 alone.
4. Once `get_guide` ships, prune the source sections that are pure
   duplicates of the loaded content in `server_instructions.md`.

## Status


- Last updated: 2026-05-19
- Owner: U7 (Surface D delivery) → U10 (mechanism update after U8+U9 landed)
- Status: **shipped** — 4 live topics, first-call hint mechanism active.
  Candidates table remains as the promotion backlog.
