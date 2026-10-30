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
unverified: 'Fixed in 4c32d4341 and tested through the in-memory store seam, then run once live (write, recall, delete with project_id=codescout-embed, shared store; see Resume 2). Gate 2026-10-30 on the combined tree: clippy clean; lean 5368 passed and 2 failed, default 8032 passed and 2 failed; both failures are the OP-2 routing tests (operator_rules::route::tests::a_rule_with_several_selectors_routes_on_any_one_of_them and server::tests::every_triggered_rule_names_a_tool_that_can_deliver_it) while a peer holds uncommitted edits to docs/trackers/operator-rules.md and src/operator_rules/profiles.rs; not re-run on a clean tree. The format step refused (ownership: peer 76494b3d is a live writer of the same files); the narrow rustfmt on the three files was run on the operator''s word that this session is the sole writer. The committed part was re-run alone in a throwaway worktree: tools::memory 103 passed, 0 failed. Not checked: a member that is dormant when a memory is filed (its key is its id; activation later sets the same name, not observed); the root project named by its workspace id while a member is focused; forget (unchanged, the Qdrant delete ignores its project_id argument).'
---

# BUG: memory(write, project_id=<member>) puts the file in the member but cross-embeds it under the focused project's key

## Summary
The `memory` tool takes a `project_id` that scopes it to a workspace project. The file write honors it. The semantic cross-embed and the semantic anchors do not: they read `config.project.name` of the focused project, whatever `project_id` the call named.

## Class fit
`project_id` is declared in the tool's schema as "Scope to a workspace project ID" (src/tools/memory/mod.rs:907), yet two of the write's three effects never read it, so the declaration holds for the file and not for the embedding. The declaration is not executed in part of the call (the `declared-not-wired` trait). It differs from the class's other members because the same call does honor the parameter for the directory.

## Symptom (Effect)
Expected, from the code only: after `memory(action="write", project_id="thecrew-module", topic=..., content=...)` called with another project focused, the markdown lands in the member's memory directory and the cross-embedded point carries the focused project's id as its `project_id` payload and in its point id.

## Reproduction
Run 2026-10-30 on the in-memory store seam (no shared collection touched). In a gradle workspace with an `mcp-server` member and the root focused, `memory(write, project_id="mcp-server")` left 0 points under `mcp-server`. The new tests failed on `left: 0, right: 1` before the fix.

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
Done 2026-10-30 in 4c32d4341.
- `Agent::semantic_key_for_project` (src/agent/mod.rs) gives the key for a named project: an activated project answers with `config.project.name`, a dormant one with its id, an unknown id is refused by name with the valid ids.
- `semantic_project_key` (src/tools/memory/mod.rs) returns that key when the call carries `project_id`, else the focused project's name, as before.
- `cross_embed_memory` and `create_semantic_anchors` take the call's `input` and use the helper. The anchor pass re-upserts the cross-embed's point id, so both must use one key.
- The delete arm, `remember` and `recall` use the helper too.
- Wider than the filed bug: `remember` and `recall` also ignored `project_id`. With only the write changed, a member's memory would have become unreadable from the parent, because `recall` read the focused key. The write fix needed the read side.
- `forget` is unchanged. The Qdrant `delete` ignores its `project_id` argument (src/memory/semantic_store.rs), so no key reaches it.
- Behavior change: a memory written with `project_id=<member>` while the parent is focused is no longer found by a `recall` with no `project_id`. It is found by `recall(project_id=<member>)`.

## Tests added
Red first, in src/tools/memory/tests.rs, on the new fixture `multi_project_ctx_with_store` (it keeps a handle on the in-memory store):
- `a_write_with_a_project_id_cross_embeds_under_the_named_project`
- `a_delete_with_a_project_id_removes_the_named_projects_point`
- `remember_with_a_project_id_files_under_the_named_project`
- `recall_with_a_project_id_reads_the_named_projects_points` also asserts that a recall without `project_id` does not return the member's memory. Without that half it passed on the old code, because `remember` and `recall` both ignored `project_id` and agreed.

Mutation probe: restoring the old focused-name lookup in the delete arm turned the delete test red (`left: 1, right: 0`). The write, remember and recall tests were red on the unfixed code. No probe was run on the `recall` arm alone.

## Workarounds
- Focus the member (`workspace(action="activate", path=<member id>)`) before writing, so the focused project and the named project agree.

## Resume
1. Run the gate on a clean tree, so the two OP-2 failures can be shown to come from the peer's uncommitted files.
2. Done 2026-10-30 on a live server (binary built after 4c32d4341), on the shared store, not an isolated one: `memory(write, project_id="codescout-embed")` then `recall(project_id="codescout-embed")` found the point (similarity 0.47); the same recall without `project_id` did not return it; after `delete(project_id="codescout-embed")` the named recall returned 0 results and no file was left. The member was activated or dormant as the server held it; the dormant-key case is still not told apart.
3. Then move the issue to `docs/issues/archive/` with `doc(action="move", ...)`, the fix SHA 4c32d4341 and its `git patch-id --stable`.

## References
- docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md (the same id-versus-key theme; its fix made a member's `config.project.name` equal its workspace id, which hides this mismatch only when the member is focused)
- docs/trackers/issue-clusters/IC-3-declared-not-wired.md
