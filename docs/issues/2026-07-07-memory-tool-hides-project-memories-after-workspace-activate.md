---
status: open
opened: 2026-07-07
closed:
severity: medium
owner: marius
related: []
tags: [memory, workspace, multi-project]
kind: bug
---

# BUG: `memory(list/read)` only sees 2 topics for a project that `workspace(activate)` reports has 16

## Summary
After `workspace(action="activate", path="codescout")` (from within a multi-project
home-directory workspace), `workspace(activate)`/`workspace(status)` report the
codescout project has 16 memory topics (`architecture`, `conventions`,
`development-commands`, `domain-glossary`, `gotchas`, `project-overview`,
`system-prompt`, etc. + `language-patterns`, `onboarding`). But `memory(action="list")`
and `memory(action="read", topic=...)` — even with `project_id="codescout"` passed
explicitly — only see 2 topics: `language-patterns` and `onboarding`. All 14 other
memories that CLAUDE.md says should auto-load are invisible to the memory tool.

## Symptom (Effect)
```
mcp_rmcp_memory(action="read", topic="architecture") ->
{
  "ok": false,
  "error": "topic 'architecture' not found",
  "hint": "2 topic(s) available — see `available_topics` in this response",
  "available_topics": ["language-patterns", "onboarding"]
}

mcp_rmcp_memory(action="list") ->
2 topics
  language-patterns
  onboarding
```
Same result whether or not `project_id="codescout"` is passed explicitly.
Meanwhile `workspace(action="activate", path="codescout", read_only=false)` returns:
```
"memories": [
  "architecture", "cargo-test-lib-skips-integration", "claude-code-mcp-env",
  "conventions", "development-commands", "domain-glossary", "gotchas",
  "infra/headroom-trial-and-langfuse", "language-patterns", "onboarding",
  "project-overview", "reconnaissance", "research/agent-memory-frameworks",
  "research/loadbearing-mcp-guidance", "research/sakana-fugu-integration",
  "system-prompt"
]
```

## Reproduction
1. Commit: `c5e6aee938503bb7e5109ac5b5acbeb50b4726c3` (branch `experiments`)
2. Start codescout MCP server against a multi-project home-directory workspace
   (project `root` = `C:\Users\MAILINCA.BRN.002`, containing `codescout` as a
   sub-project at `work\claude\codescout`).
3. `workspace(action="activate", path="codescout", read_only=false)` — note the
   16-entry `memories` array in the response.
4. `memory(action="list")` — returns only 2 topics.
5. `memory(action="read", topic="architecture")` — `ok: false`, "not found".
6. Also tried `memory(action="read", topic="architecture", project_id="codescout")`
   — same failure.

## Environment
- OS: Windows, PowerShell 7 (pwsh)
- MCP server: freshly rebuilt via `cargo rb` from `origin/experiments` @ `33eca3e2`
  (local HEAD `c5e6aee9` after rebase)
- Transport: VS Code Copilot Chat MCP client (rmcp tools), not Claude Code
- Project topology: home dir is a multi-project workspace (`root`, `codescout`,
  `researcher`, `m365-mcp`, `servicenow-mcp`, Mercury BOM/MRP, m365-data-agent)
- This was triggered while running `onboarding(refresh_prompt=true)` for codescout,
  which instructed reading each of the 14 missing memory topics

## Root cause
Unknown — under investigation. Hypothesis: the `memory` tool's project resolution
defaults to the outer "root" (home-directory) project's on-disk memory store rather
than the nested `codescout` sub-project's store, even when `workspace(activate)` has
switched the active project to `codescout` and `project_id="codescout"` is passed
explicitly. `workspace(status)`/`workspace(activate)` and `memory(list)`/`memory(read)`
appear to resolve the "current project" through two different code paths that
disagree in this nested-workspace topology.

## Evidence
Tool call transcript from this session (see above symptom block) — three consecutive
calls, same session, same active project, all agreeing the project is "codescout"
except the memory tool.

## Hypotheses tried
1. **Hypothesis**: `project_id` param on `memory` isn't being threaded through to the
   store lookup lookup and silently falls back to the session default.
   **Test**: passed `project_id="codescout"` explicitly on `memory(read)`.
   **Verdict**: rejected as sole cause — result was identical, still only 2 topics,
   so either the param is ignored entirely or the "session default" it falls back
   to isn't the plain root project either.
   **Evidence link**: see Symptom block, 3rd call.
2. **Hypothesis**: nested-workspace (project root = home dir, sub-project =
   `work\claude\codescout`) resolves memory storage paths differently than
   top-level single-project usage, and the 2 visible topics
   (`language-patterns`, `onboarding`) are actually the outer `root` project's
   memories, not codescout's — coincidentally the same 2 names appear in the
   `root` project's `workspace(status)` memory list earlier in this session.
   **Test**: compared `workspace(status)` (active=root) memory list ("language-patterns",
   "onboarding") against `memory(list)` output while active=codescout — identical set.
   **Verdict**: consistent with root-cause hypothesis above but not yet confirmed
   against source (didn't trace the resolution code path).
   **Evidence link**: see Symptom + Reproduction.

## Fix
Not yet investigated. Next step: trace `memory` tool's project-resolution code
(likely `src/memory/` — not yet located precisely) vs. `workspace` tool's
project-resolution code, and diff how each computes the "active project" memory
store path in a nested multi-project workspace.

## Tests added
N/A — root cause not yet identified; no regression test written yet.

## Workarounds
None found this session — attempted explicit `project_id` override, did not help.
If memories are load-bearing for a task, consider activating codescout as a
top-level standalone workspace (not nested under the home-dir multi-project
workspace) to sidestep the resolution mismatch, though this wasn't verified.

## Resume
Locate the `memory` tool's project-resolution implementation (likely under
`src/memory/`) and the `workspace` tool's equivalent (likely `src/workspace.rs`
or `src/mcp_resources/`), diff how each resolves "active project" root path when
the active project is a nested sub-project of a larger workspace root. Reproduce
with a minimal 2-project nested workspace fixture if possible.

## References
- Session that discovered this: 2026-07-07, testing post-`cargo rb` rebuild of
  `experiments` branch.
- CLAUDE.md § "Memories (Claude auto-loads these...)" — lists the 14 memories
  that should be available for codescout (`architecture`, `conventions`,
  `development-commands`, `language-patterns`, `gotchas`, `domain-glossary`,
  `project-overview`, `system-prompt`, `onboarding`).
