---
kind: bug
status: open
tags:
- memory
- workspace
- multi-project
closed: null
last_observed: 2026-08-26
opened: 2026-07-07
owner: marius
related: []
reopened: 2026-08-26
severity: medium
unverified: 'REPRODUCED but the mechanism is only half-established. Confirmed by reading the handlers: `list` and `read` never look at `project_id` at all. NOT established: why a bare-id focus-switch also returns 0, since `with_project_at` does resolve `ws.focused_active()` and `activate_within_workspace` does call `MemoryStore::open` on promotion. Two hypotheses were formed and both refuted by reading the code; do not inherit either.'
---

# BUG: `memory(list/read)` only sees 2 topics for a project that `workspace(activate)` reports has 16

> **Verify-open datapoint 2026-08-06 — still open, NOT reproduced, NOT cleared.**
>
> On the home project (`codescout`) the two surfaces agree: `workspace(action="activate")`
> reported 21 memories and `memory(action="list")` returned the same 21 topics. That is a
> data point, not a verdict — the bug's scenario is a *foreign* project after
> `activate`, which this pass did not exercise (reproducing it means activating another
> workspace, and the workspace gate then owes a return trip home).
>
> Note for whoever picks this up: this cohort's `d668927e`
> (`fix(memory): section filter follows the shallowest heading level`) is a **different**
> defect in the same tool — it does not address topic visibility. Do not read it as a fix
> for this.
## Summary
> **RE-OPENED 2026-08-26 — REPRODUCED on the foreign/multi-project path this file said had
> never been covered.** The trigger below fired verbatim: `memory(action="list")` returned
> **materially fewer topics than `.codescout/memories/` holds on disk**. Full reproduction,
> evidence and what is *not* yet established are in § *Reproduction — 2026-08-26*. The
> historical note below is kept unedited; it was accurate for the home-project path it
> tested, and it is the note that named the untested path.
>
> **STATUS (historical): zombie — not reproducible as of 2026-07-28.** A verify-open pass ran
> `memory(action="list")` against this project: it returned **21 topics**, not 2
> (`architecture`, `cargo-test-lib-skips-integration`, `catalog-sql-hazards`,
> `claude-code-mcp-env`, `conventions`, `development-commands`, `domain-glossary`,
> `fable-tuning`, `gotchas`, `infra/headroom-trial-and-langfuse`,
> `kotlin-lsp-rogue-investigation`, `language-patterns`, `onboarding`,
> `project-overview`, `reconnaissance`, three `research/*`, `system-prompt`,
> `test-design-discipline`, `worktree-merge-catalog-reconciliation`). Nested
> path-like topics resolve too, so the truncation is gone in both the flat and
> nested cases.
>
> Kept open as `zombie` rather than `fixed` because **no root cause was ever
> confirmed** and no fix commit is attributable — the symptom simply stopped. Per
> CLAUDE.md's status vocabulary that is exactly what `zombie` means.
>
> **Re-open trigger:** any session where `memory(action="list")` returns materially
> fewer topics than `.codescout/memories/` holds on disk, or where a topic readable
> by path is absent from `list`. That asymmetry (list vs read) is the shape to watch
> — it is what made the original report credible.
>
> Note the original report was filed after `workspace(activate)` on a FOREIGN
> project. This verification ran on the home project with `post_compact=true`, so it
> does not cover the activate-a-foreign-project path the title names. A future
> verify pass should re-check after a foreign activate before this is downgraded
> further.
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

## Reproduction — 2026-08-26 (re-open, foreign multi-project workspace)

Run against `/home/marius/work/mirela`, a 12-sub-project workspace — structurally the
shape the original report named and the two prior verify passes could not test, because
both ran on the home project.

| # | call | result |
|---|---|---|
| 1 | `workspace(activate, path="/home/marius/work/mirela", read_only=true)` | `memories: []` |
| 2 | `memory(list, include_private=true)` | `0 shared, 0 private` — **correct**, root has none on disk |
| 3 | `memory(list, project_id="eduplanner-ui")` | **0 topics** |
| 4 | `memory(read, topic="architecture", project_id="eduplanner-ui")` | **not found**, `available_topics: []` |
| 5 | `workspace(activate, path="eduplanner-ui")` — **bare id** | reports **12 memories** |
| 6 | `memory(list)` | **0 topics** |
| 7 | `workspace(activate, path="/home/marius/work/mirela/eduplanner-ui")` — **absolute** | reports 12 |
| 8 | `memory(list)` | **12 topics** — correct |

On disk: `eduplanner-ui/.codescout/memories/` holds 22 files = **12 `.md` topics** plus 10
`.anchors.toml` sidecars. Rows 5→6 are the original report's exact shape — one surface says
12, the other says 0, same project, same instant.

**Rows 7→8 are the discriminator.** Same project, same session, same `read_only=true`; the
only variable is whether it was addressed by bare id or by absolute path. That rules out
disk permissions, `read_only`, and the store being absent — all three were candidates.

### What IS established

**`list` and `read` ignore `project_id` entirely.** Their handlers
(`src/tools/memory/mod.rs`, the `list` arm and the `read` arm) call
`agent.with_project_at(ctx.workspace_override, …)` and never read the parameter. Only
`resolve_memory_dir` consults it — and that function's own doc comment promises exactly the
routing the read surfaces do not perform: *"If `project_id` … is provided, route to the
per-project directory … Otherwise use the focused project's memory dir."* Rows 3 and 4 are
that contradiction, and it also explains why the 2026-07-07 reporter got nothing when they
passed `project_id` explicitly.

The error text makes it worse rather than neutral: row 4 answered *"no memory topics exist
yet — create one with `memory(action='write', …)`"* for a project holding twelve. A caller
acting on that hint writes a thirteenth into the wrong store.

### What is NOT established — do not inherit a guess here

Why row 6 returns 0. Two hypotheses were formed and **both refuted by reading the code**:

1. *"The bare-id focus-switch never opens the sub-project's `MemoryStore`."* Refuted:
   `Agent::activate_within_workspace` calls `MemoryStore::open(&abs_root)` on the
   dormant→activated promotion.
2. *"`with_project_at` resolves the workspace default rather than the focused project."*
   Refuted: it ends in `ws.focused_active().and_then(|p| p.as_active())`.

So the store is opened and the resolver does look at `focused` — and the answer is still
empty. The next step is to instrument between those two points rather than to theorise a
third time. Worth checking first: whether the `memories` array in the *activate response*
is read from disk directly, which would let the display be right while the store the tool
reads is a different one.

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
