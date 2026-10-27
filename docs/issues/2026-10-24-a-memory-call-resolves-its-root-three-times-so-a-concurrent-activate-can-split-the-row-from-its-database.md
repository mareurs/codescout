---
status: open
opened: 2026-10-24
closed:
severity: low
owner: marius
related:
- docs/issues/archive/2026-10-19-a-pinned-projects-memory-vectors-go-into-the-default-projects-embeddings-database.md
tags:
- cluster/unclassified
kind: bug
unverified: 'Inferred from the code (three separate root resolutions per memory call); not reproduced, and the window is the time between three async calls.'
---

# BUG: a memory call resolves its root three times, so a concurrent `activate` between them can write project A's row into B's database embedded with B's model

## Summary

A `memory` call that has no `workspace=` pin resolves "the current default root" independently in three places: `Agent::with_project_at` (project id and root), `Agent::memory_embedder_for` and `Agent::semantic_memory_store_for`. Another caller sharing the process that `activate`s a different project between those resolutions makes them disagree.

## Class fit

A target that is validated or read once is read again later for the same decision, so a change of the target between the reads is not seen. It is the same trait as the time-of-check to time-of-use bug filed for the pinned directory: here the reads are of the session default rather than of a path on disk.

## Symptom (Effect)

Not observed. The possible outcome: a row carrying project A's id (from the first resolution) is embedded with the model of project B and written into B's `.codescout/embeddings` database (from the later resolutions), so B's database holds a foreign project's row in a vector space that is not its own.

## Reproduction

Not yet reproducible. Best lead: two callers on one server, one running `memory(write)` unpinned in a loop, the other alternating `workspace(activate)` between two projects with different embedding models; then list each project's `*.memories.db` for rows whose `project_id` is not the database's own project.

## Environment

Linux, branch `fix/b-20261024` from `experiments` 30f3e728d.

## Root cause

Inferred from the code, not measured:

- `src/tools/memory/mod.rs` `cross_embed_memory` and `create_semantic_anchors` read `(project_id, root)` with `ctx.agent.with_project_at(ctx.workspace_override, ..)`, then call `ctx.agent.memory_embedder_for(ctx.workspace_override)` and `ctx.agent.semantic_memory_store_for(ctx.workspace_override)`. With `workspace_override == None` each of the three resolves `project_root()` on its own.
- A pinned call (`Some(root)`) is not affected: all three resolve the same pin. Only an unpinned call can see the default move.
- Resolving once and passing the resolved root down was not done: the `Memory/call` arms do not resolve a root before the embedder and store calls, and resolving it there would change the no-project behaviour (`require_project_root_for` errors where `semantic_memory_store_for(None)` falls back to the home directory), which is more than a small diff.

## Evidence

The call sites named above; nothing else.

## Hypotheses tried

None.

## Fix

Not implemented. Candidate: resolve the root (or `None`) once at the top of each memory action and pass `Some(&root)` as the override to `memory_embedder_for` and `semantic_memory_store_for`, deciding explicitly what a call with no project should do.


**Checked 2026-10-27 before any code: the candidate is not decision-free.** `Memory/call` and its helpers resolve the pin at about 30 sites (`grep -n workspace_override src/tools/memory/mod.rs`). Resolving once and passing `Some(&root)` would route every one of them through `Agent::with_project_at(Some(root))`, which runs `ensure_resident(root)` and then looks up `inner.workspaces` by the canonical root before taking the workspace's focused project. `None` takes `inner.default_workspace()`. The two are the same workspace only when `root` is the WORKSPACE root. A `p.root` read from the focused sub-project (for example `crates/codescout-embed` in this workspace) would instead be treated as a new pinned workspace (inferred from reading `with_project_at`; not run). So the fix needs two things this file does not yet have: an accessor for the default workspace's root, and a decision on what a call with no active project does (today `with_project_at(None)` errors with `No active project`, while `semantic_memory_store_for_backend` takes `self.project_root().await`, an `Option`, and hands `None` to the directory resolver; what directory that gives was not checked). A race test also needs a hook between the resolutions. The code states the per-call resolution is deliberate (`semantic_memory_store_for_backend` resolves the root "on EVERY call, from this call's pin or else the CURRENT default" so a store never outlives an `activate`), so a fix changes a documented choice.

Not started. Classified as a design item, not a mechanical fix.

## Tests added

N/A: not fixed; this file records a finding.

## Workarounds

Pin the project (`workspace=`) on memory calls when several callers share the server.

## Resume

Read `cross_embed_memory`, `create_semantic_anchors` and the `remember`/`recall`/`forget` arms of `Memory::call` in `src/tools/memory/mod.rs`; decide the no-project behaviour, then thread one resolved root through.

## References

- `src/tools/memory/mod.rs`
- `src/agent/mod.rs` (`memory_embedder_for`, `semantic_memory_store_for`)
