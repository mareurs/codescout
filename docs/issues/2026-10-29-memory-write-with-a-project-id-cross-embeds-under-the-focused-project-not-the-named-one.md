---
status: open
opened: 2026-10-29
closed:
severity: medium
owner: marius
related:
- docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md
tags:
- cluster/declared-not-wired
kind: bug
unverified: 'Read from the code on 2026-10-29, not run. Not checked: the effect at recall time (which key recall filters by); whether the focused project or the named member holds the point after a write with project_id; the memory store collection is shared, so a runtime test needs an isolated store.'
---

# BUG: memory(write, project_id=<member>) puts the file in the member but cross-embeds it under the focused project's key

## Summary
The `memory` tool takes a `project_id` that scopes it to a workspace project. The file write honors it. The semantic cross-embed and the semantic anchors do not: they read `config.project.name` of the focused project, whatever `project_id` the call named.

## Class fit
`project_id` is declared in the tool's schema as "Scope to a workspace project ID" (src/tools/memory/mod.rs:907), yet two of the write's three effects never read it, so the declaration holds for the file and not for the embedding. The declaration is not executed in part of the call (the `declared-not-wired` trait). It differs from the class's other members because the same call does honor the parameter for the directory.

## Symptom (Effect)
Expected, from the code only: after `memory(action="write", project_id="thecrew-module", topic=..., content=...)` called with another project focused, the markdown lands in the member's memory directory and the cross-embedded point carries the focused project's id as its `project_id` payload and in its point id.

## Reproduction
Not run. To run it: a workspace with a declared member, the parent focused, an isolated memory store, then a write with `project_id=<member id>` and a read of the stored point's payload.

## Environment
Binary `a398925ea`, read from the source tree.

## Root cause
Read 2026-10-29.
- The write arm resolves the directory from the input: `resolve_memory_dir(&input, ctx)` (src/tools/memory/mod.rs:977), which routes through `resolve_memory_dirs` and `ws.memory_dir_for_project(&project_id)` (:587).
- It then calls `cross_embed_memory(ctx, topic, content)` (:1003) and `create_semantic_anchors(ctx, topic, content, &path_files)` (:1031). Neither receives `input`.
- `cross_embed_memory` reads `p.config.project.name` through `with_project_at(ctx.workspace_override...)` (:238-243), which is the focused project. The delete arm does the same when it derives the point id (:1155-1165).
So the embedding key and the file location can name two different projects.

## Evidence
The line references above. No runtime run.

## Hypotheses tried
None yet.

## Fix
None yet. Direction: pass the resolved project id (the one `resolve_memory_dirs` used) to both helpers, and use it for the delete arm's point id.

## Tests added
None yet. A red test needs a store seam: the helpers should take the project id as a parameter so a test can assert which id they use.

## Workarounds
- Focus the member (`workspace(action="activate", path=<member id>)`) before writing, so the focused project and the named project agree.

## Resume
1. Confirm at runtime on an isolated store (the shared `memories` collection must not be written by a probe).
2. Write the red test on a helper that takes the project id.
3. Check the recall path for the key it filters by.

## References
- docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md (the same id-versus-key theme; its fix made a member's `config.project.name` equal its workspace id, which hides this mismatch only when the member is focused)
- docs/trackers/issue-clusters/IC-3-declared-not-wired.md
