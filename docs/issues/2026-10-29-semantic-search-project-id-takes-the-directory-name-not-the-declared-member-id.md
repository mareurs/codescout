---
status: open
opened: 2026-10-29
closed:
severity: medium
owner: marius
related:
- docs/issues/2026-10-28-a-nested-git-repo-cannot-join-a-workspace-as-a-project-and-pinned-onboarding-mixes-in-the-home-workspace.md
tags:
- cluster/declared-not-wired
kind: bug
unverified: 'The fix is tested through Agent and project_id() only. Gate on b358e9aab, 2026-10-29: lean 5356 passed and 2 failed, default 8020 passed and 2 failed; both failures are OP-2 routing tests (operator_rules::route::tests::a_rule_with_several_selectors_routes_on_any_one_of_them and server::tests::every_triggered_rule_names_a_tool_that_can_deliver_it) while a peer holds uncommitted edits to docs/trackers/operator-rules.md and src/operator_rules/profiles.rs; not re-run on a clean tree. Not run: a member whose own project.toml names a project differently from its id; which other tools key a store by config.project.name for a member (memory, status) and whether any depends on the old name.'
---

# BUG: semantic_search(project_id=<declared member id>) returns no results because an index is keyed by the member's directory name

## Summary
`workspace(list_projects)` shows a declared member under its `[[project]]` id, for example `thecrew-module`. An index of that member is keyed by `config.project.name`, which is the directory name `script.module.thecrew`. `semantic_search(project_id="thecrew-module")` therefore returns `results: []` with no hint, while `project_id="script.module.thecrew"` returns the file.

## Class fit
A `[[project]]` id is declared in workspace.toml and shown by `list_projects`, yet the retrieval store never uses it as a key, so the id reads as the member's name and finds nothing. The declaration is not wired to execution (the `declared-not-wired` trait). It differs from the class's other members because the id does work in other paths: `memory(list, project_id="thecrew-module")` returned the member's memories on a live server.

## Symptom (Effect)
After activating a declared member and building its index, `semantic_search` with the declared id returns `{"results": [], "total": 0, "truncated": false}`. The empty result does not say which project it searched or that the id matched no indexed project.

## Reproduction
Binary built from `e34ff991f`, embedder `http://127.0.0.1:48081` (CodeRankEmbed). Script: `kodi-id.py` in the session scratchpad, on a fresh replica (`kodi-replica2`).
1. Build a parent git repo with a manifest, two nested git repos under `remote-fixes/` hidden by `.git/info/exclude`, and `[[project]]` entries `thecrew-module` and `thecrew-video` in `.codescout/workspace.toml`.
2. `workspace(action="activate", path="thecrew-module", read_only=false)`. The response names the project `script.module.thecrew`.
3. `index(action="build")`, then `index(action="status")`. The status shows `"project_id": "script.module.thecrew"`, `file_count: 1`, `chunk_count: 1`.
4. `semantic_search(query="function that returns one", project_id="thecrew-module")` returns `results: []`.
5. The same query with `project_id="script.module.thecrew"` returns `addon.py` lines 1-2. With no `project_id` it returns the same hit.

## Environment
Linux, codescout over stdio, remote-http embedding backend, `sqlite-vec` store (from the server log).

## Root cause
Read 2026-10-29. `semantic_search` takes an explicit `project_id` verbatim as the store key (src/tools/semantic/semantic_search.rs:646-647). `index` keys a project by `ActiveProject::project_id()`, which returns `config.project.name` (src/agent/mod.rs:478-480). For a member activated by path or by declared id, that name comes from the directory, not from the `[[project]]` id. The two names meet nowhere. Not yet read: where the `[project] name` of a member's `project.toml` is first written, and whether a declared id could set it.

## Evidence
Before the fix, run output in scratchpad `kodi-id.out`, 2026-10-29. `git_sync` in the status shows the member's own HEAD `28e79441`. The sequence in Reproduction is the evidence; the two `semantic_search` calls differ only in `project_id`.

After the fix, 2026-10-29, the same script `kodi-id.py` on a fresh replica (`kodi-replica4`), run against the release binary the user rebuilt (`workspace(status)` showed `git_sha: a398925ea`, which contains `b358e9aab`; `git_dirty: true` from peers' uncommitted files). Output in `kodi-id-fixed.out`:
- Activation by `thecrew-module` named the project `thecrew-module`; the index status showed `project_id: "thecrew-module"`, `file_count: 1`.
- `semantic_search(project_id="thecrew-module")` returned `addon.py`, total 1. With no `project_id` it returned the same hit.
- `semantic_search(project_id="script.module.thecrew")` returned total 0. The directory name is no longer a key for a declared member. That is a behavior change of the fix, and a user who indexed a member under its directory name must rebuild.

## Hypotheses tried
1. **Hypothesis:** activating by the declared id keys the index by that id. **Test:** step 2 and 3 above. **Verdict:** rejected. The status shows the directory name.

## Fix
Chosen 2026-10-29: a non-root member is keyed by its workspace id.
- `activate_within_workspace` sets `config.project.name` to the member's workspace id when the member's root is not `.` (src/agent/mod.rs).
- `ActiveProject` gains `member_id: Option<String>`. `reload_config_from_disk` reapplies it, so a reload of the member's `project.toml` cannot put the directory name back.
- Auto-discovery sets an id to the directory name exactly (src/workspace.rs, `rel_path.file_name()`), so a discovered member keeps its key. Only a declared id that differs from the directory changes. Checked on a replica: an auto-discovered member `pytool` (manifest name `pytool-dist`) was keyed `pytool`, and a search by `pytool` found its files.
- Edge, not checked: a member whose own `project.toml` names a project differently from its id now takes the id as the key, and an index built under the old name needs a rebuild.
- Done 2026-10-29: a search that comes back empty because its `project_id` holds no chunks now carries a `hint`. `empty_result_scope_hint` (src/tools/semantic/semantic_search.rs) builds it. The plain path asks `project_has_chunks` for the searched id on an empty page only, then for each workspace project id plus the active project's name, and names the ones that hold chunks. A failed lookup leaves the empty page as it was. The worktree path and a project that does hold chunks are unchanged.

## Tests added
Both in src/agent/declared_member_tests.rs.
- `a_declared_member_is_keyed_by_its_declared_id_not_its_directory_name`: red first, `left: "addon"`, `right: "thecrew-module"`. Green after the fix.
- `a_reload_keeps_a_declared_member_keyed_by_its_declared_id`: green with the fix. With the reapply in `reload_config_from_disk` replaced by a no-op, it failed with `left: "addon"`, so it pins the reapply.
Both go through `Agent` and the member's `project_id()`. Neither runs `semantic_search` against a real store.

For the hint, in the `empty_result_scope_hint_tests` module of src/tools/semantic/semantic_search.rs (red first against a stub that returned `None`, two failures at `expect`; green after):
- `a_project_without_chunks_is_named_with_the_projects_that_hold_chunks`
- `a_project_without_chunks_in_a_workspace_without_any_says_to_build`
- `a_project_with_chunks_gets_no_note_because_the_empty_page_is_a_real_miss` (passed against the stub too, so it only guards the `None` arm)
These cover the pure function. The wiring in `SemanticSearch::call` has no unit test and no mutation probe: it needs a real store and an embedder. It was checked live on a debug binary built from the tree (replica `kodi-replica5`, output `kodi-hint.out`): a search by `script.module.thecrew` and by `no-such-project` each returned `total: 0` with the hint naming `thecrew-module`, and a search by `thecrew-module` returned `addon.py` with no hint.

## Workarounds
- Search with the directory name as `project_id`, or omit `project_id` while the member is the active project.

## Resume
1. Done 2026-10-29: red test, then the fix (see Fix and Tests added).
2. Done 2026-10-29: an auto-discovered member is keyed by its directory name, equal to its id, so it is not affected.
3. Done 2026-10-29: the replica script on the rebuilt binary (see Evidence, after the fix).
4. Done 2026-10-29: the empty-result hint (see Fix and Tests added). Not covered by a test: the async wiring.
5. Gate run 2026-10-29 on b358e9aab: only the two OP-2 failures, not mine (see the `unverified:` line). Close or archive after 3 and a clean-tree gate, with the fix SHA and its patch-id.

## References
- docs/issues/2026-10-28-a-nested-git-repo-cannot-join-a-workspace-as-a-project-and-pinned-onboarding-mixes-in-the-home-workspace.md (Evidence 7)
- docs/trackers/issue-clusters/IC-3-declared-not-wired.md
