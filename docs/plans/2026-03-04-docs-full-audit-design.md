# Design: Full Documentation Audit & Update

**Date:** 2026-03-04  
**Status:** Approved

## Context

An audit revealed the following gaps between the actual codebase (28 tools registered) and
the documentation (claims 23 tools, missing GitHub tools entirely, stale `code-explorer`
references, and several tools undocumented or in the wrong section).

## Approach: Two Passes

### Pass 1 — Factual Corrections (existing pages only)

Seven targeted fixes, no new files:

1. **Tool count 23 → 28** — README "## Tools (28)", `overview.md` intro, manual `introduction.md`
2. **README tools table** — add GitHub category row with all 5 tool names + one-liners
3. **`remove_symbol`** — add full entry to `docs/manual/src/tools/symbol-navigation.md`
4. **`create_file` + `edit_file`** — add proper entries to `docs/manual/src/tools/file-operations.md` (currently only in `editing.md`; file tools belong there)
5. **`FEATURES.md`** — add `goto_definition` + `hover` section; add brief GitHub tools section linking to future reference page
6. **`overview.md`** — add GitHub tools category row to tools table, fix intro count
7. **`code-explorer` sweep** — replace all remaining stale `code-explorer` references with `codescout` across all docs EXCEPT `history.md` and `CHANGELOG.md` (which use the old name intentionally as historical context)

### Pass 2 — GitHub Tools Reference Page (new content)

One new file + wiring:

1. **`docs/manual/src/tools/github.md`** — full reference for all 5 GitHub tools:
   - `github_identity` — get authenticated user, search users, get teams/members
   - `github_issue` — list/search/get/create/update issues and comments
   - `github_pr` — list/search/get/diff/review/merge pull requests
   - `github_file` — get/create/update/delete files and push multi-file commits
   - `github_repo` — search repos, manage branches, commits, releases, tags, code search
   Source of truth: `src/prompts/server_instructions.md` (lines 164–188)

2. **`SUMMARY.md`** — add `[GitHub](tools/github.md)` under Tool Reference section

3. **README** — add a brief bullet under "What sets it apart" for GitHub integration

## Out of Scope

- README narrative rewrite (keep visitor-friendly and lean)
- Any code changes
- Changing the storytelling voice of existing narrative sections
