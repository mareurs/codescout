---
status: fixed
opened: 2026-06-09
closed: 2026-06-09
severity: high
owner: marius
related:
  - docs/issues/2026-06-03-project-languages-from-manifest-not-files.md
  - docs/issues/2026-06-01-librarian-adapter-stale-is-write.md
tags:
  - onboarding
  - memory
  - prompt-builder
  - workspace
kind: bug
---

# BUG: workspace-onboarding prompts tell subagents to write/read memories with `project=` (silently ignored) instead of `project_id=`, misrouting every per-project memory to the active project

## Summary
In workspace-mode onboarding, the generated per-project and synthesis prompts instruct subagents to call `memory(action="write"/"read", project="<id>", ...)`. The `memory` tool's parameter is **`project_id`**, not `project`. The unknown `project=` arg is silently dropped, so every per-project write lands in the **active/focused project** instead of its own namespace. With N parallel subagents they overwrite each other — only the last writer survives — and N-1 projects get **no** `project-overview`/`architecture`/`conventions` memories. Every misrouted write still returns `"ok"`, so the failure is invisible without explicit verification.

## Symptom (Effect)
During an 8-project workspace onboarding (the `headroom` repo), all 8 subagents reported "memories written" using `memory(action="write", project="<id>", ...)`. Afterward, the per-project namespaces were empty except for the pre-created stubs:

```
memory(action="list", project_id="headroom-core")
→ 2 topics
    language-patterns
    onboarding
```

Identical result for `docs`, `headroom-parity`, `headroom-proxy`, `headroom-py`, `openclaw`, `typescript` — all 7 non-root projects held only stubs. Only the root project (`headroom`) had `architecture`/`conventions`/`project-overview`, populated by whichever subagent happened to write last.

The read path has the same trap:

```
memory(action="read", project="headroom-core", topic="conventions")
→ returned the ROOT headroom conventions (wrong project)

memory(action="read", project_id="headroom-core", topic="conventions")
→ "topic 'conventions' not found"   (correct: it was never written there)
```

Every write returned `"ok"`.

## Reproduction
In any multi-project workspace, run `onboarding()` and follow the emitted plan (one subagent per project). The rendered per-project prompt (`.codescout/tmp/onboarding-project-<id>.md`, "Phase 3: Write the Memories") contains literally:

```
Write these memories using `memory(action="write", project="<id>", topic="...", content="...")`.
```

After the subagents finish:

```
memory(action="list", project_id="<non-root-id>")   # → only stubs; the 3 written topics are absent
memory(action="list", project_id="<root-id>")        # → holds the topics, last-writer-wins
```

Observed 2026-06-09 against `/home/marius/work/claude/headroom` (codescout MCP, `~/.claude-kat` profile).

## Environment
- OS: Linux. codescout MCP server, master as of 2026-06-09.
- Driven from a Claude Code session running the `headroom` 8-project workspace onboarding.
- `memory` tool schema: param is `project_id` ("Scope to a workspace project ID. Default: focused project."); `workspace=<abs path>` also pins resolution. There is no `project` alias.

## Root cause
Parameter-name mismatch between the onboarding prompt-builder text and the live `memory` tool schema. The builder emits `project="{id}"`; the tool accepts `project_id`. The unknown `project` key is dropped rather than rejected, so project resolution falls through to the focused/active project, and no error is surfaced.

Buggy emissions (all in `src/prompts/builders.rs`):
- `build_per_project_prompt` (fn at `src/prompts/builders.rs:718`) — `src/prompts/builders.rs:828`: `memory(action="write", project="{id}", topic="...", content="...")`.
- `build_synthesis_prompt` (fn at `src/prompts/builders.rs:864`) — `src/prompts/builders.rs:868-874`: the "Read Per-Project Memories" list, `memory(action="read", project="{id}", topic=...)` (×3).
- `src/prompts/builders.rs:893`: synthesis conventions cross-ref `memory(project="{id}", topic="conventions")`.

Counter-evidence that `project_id` is the correct param (already used elsewhere in onboarding output):
- `docs/issues/2026-06-03-project-languages-from-manifest-not-files.md:130` and `docs/plans/2026-06-04-onboarding-integration.md:88` both instruct `memory(action="write", topic="language-patterns", project_id=<id>, ...)`.

Secondary (verify separately): `src/prompts/builders.rs:836` emits `semantic_search(query, project="{id}")` — confirm whether `semantic_search` also expects `project_id` and fix if so.

## Evidence
### Subagent reports vs. actual store
8 subagents each reported success writing 3 memories with `project=`. Post-hoc `memory(action="list", project_id=...)` showed all 7 non-root projects held only the `language-patterns` + `onboarding` stubs; the root project held the 3 topics (last-writer-wins).

### Read trap
```
memory(read, project="headroom-core", topic="conventions")    → ROOT headroom conventions
memory(read, project_id="headroom-core", topic="conventions") → "topic 'conventions' not found"
```

### Tests currently assert the buggy text
- `src/tools/run_command/tests.rs:286-287`:
  ```
  assert!(prompt.contains("memory(action=\"read\", project=\"backend\""));
  assert!(prompt.contains("memory(action=\"read\", project=\"mcp-server\""));
  ```
  These pin the wrong param and will need updating alongside the fix.
- Same assertions appear in the design doc `docs/superpowers/plans/2026-03-30-workspace-parallel-onboarding.md:251-252` (and the buggy emissions at lines 186, 293-295, 314).

## Hypotheses tried
1. **Hypothesis:** subagents simply failed to pin `workspace=` so writes hit the active project. **Test:** checked whether any `project_id`-scoped list surfaced the memories. **Verdict:** contributing but not the root — the `project=` arg is ignored regardless of workspace pinning. **Evidence:** the read trap (project= resolves to active project even though project_id reads work).
2. **Hypothesis:** the writes errored. **Test:** inspected return values. **Verdict:** rejected — every write returned `"ok"`. **Evidence:** subagent reports + manual re-writes with `project_id=` also returned `"ok"` and *did* land.

## Fix

Implemented on `experiments` (uncommitted at time of writing — cite the master-side SHA here after cherry-pick, per CLAUDE.md § "After cherry-pick"):

1. `src/prompts/builders.rs` — all 7 `project=`/`project:` occurrences → `project_id` (write `:828`, `semantic_search` `:836`, synthesis reads `:872-874`, conventions ref `:893`, doc comment `:715`, prose `:882`). `build_system_prompt_draft` (lines 198-394) already used `project_id` and was untouched — the stored system prompt was never affected, so **no `ONBOARDING_VERSION` bump is needed**.
2. `src/tools/run_command/tests.rs` — assertions at `:257`/`:286`/`:287` flipped to `project_id=`, plus a `!contains("project=\"")` negative guard added to both builder tests.
3. **Defense-in-depth (chosen by user):** `resolve_memory_dir` (`src/tools/memory/mod.rs`) now accepts `project` as an alias for `project_id` (`.or_else(|| input.get("project"))`); param schema + doc comment updated. A stray `project=` now routes correctly instead of silently falling through to the focused project. (`semantic_search` was confirmed to share the same `project_id`-only schema, but its emission is fixed at the source in step 1, so no alias was added there.)

**Blast-radius correction vs. the original plan below:** the first draft cited only `tests.rs:286-287` + a fixture refresh. Reality: a THIRD assertion at `tests.rs:257` would have broken the build, and `tests/fixtures/prompt_surfaces/` has ZERO `project=` (these builder prompts are ephemeral `.codescout/tmp/` files, not a snapshot surface) — no fixture change, no version bump. Captured as bug-fix-session-log F-15 / W-10 and recon-pattern R-20.

Verified: `cargo fmt` + `cargo clippy --lib --tests` clean; `cargo test --lib` (4 targeted tests) green. Live-MCP `/mcp` verification pending before cherry-pick to master.
## Tests added

- `memory_write_accepts_project_alias_for_project_id` in `src/tools/memory/tests.rs` — asserts a `project=` (alias) write lands in the per-project memory dir, is **absent** from the workspace-level/root dir (the misroute this bug caused), and reads back via the canonical `project_id=` key. Fails without the `.or_else` alias (write misroutes to root); passes with it.
- Negative guards in `build_per_project_prompt_contains_project_context` and `build_synthesis_prompt_contains_readback_and_claude_md` (`src/tools/run_command/tests.rs`): `assert!(!prompt.contains("project=\""))` so any future emission of the silently-ignored bare `project=` fails the build.
## Workarounds
- When dispatching onboarding subagents, override the template: instruct them to call `memory(action="write", project_id="<id>", ...)` (or pin `workspace=<abs path>` on the memory call).
- After onboarding, verify with `memory(action="list", project_id="<id>")` per project before trusting the result.
- If writes already misrouted and the agents have ended (this harness has no agent-resume/SendMessage), reconstruct each project's memories from the subagents' returned summaries and re-write them with `project_id=`. (This is how the headroom onboarding was recovered.)

## Resume
Apply the `project=` → `project_id=` edits at `src/prompts/builders.rs:828,872-874,893`; verify `:836` semantic_search param against the `semantic_search` tool schema. Update `src/tools/run_command/tests.rs:286-287` and the `tests/fixtures/prompt_surfaces/onboarding_prompt.md` fixture. Then decide whether `memory` should reject/alias unknown params. Add the two regression tests in "Tests added". Re-render an onboarding prompt and grep it for `project="` to confirm zero remaining occurrences in memory/read/write instructions.

## References
- Observed during the `headroom` workspace onboarding, 2026-06-09 (`/home/marius/work/claude/headroom`).
- Source: `src/prompts/builders.rs:828` (write), `:868-874` (synthesis read), `:893` (conventions ref), `:836` (semantic_search, verify).
- Tests pinning the buggy text: `src/tools/run_command/tests.rs:286-287`.
- Correct-param precedent: `docs/issues/2026-06-03-project-languages-from-manifest-not-files.md:130`; `docs/plans/2026-06-04-onboarding-integration.md:88`.
- Design doc with the same buggy emissions: `docs/superpowers/plans/2026-03-30-workspace-parallel-onboarding.md:186,251-252,289-295,314`.
