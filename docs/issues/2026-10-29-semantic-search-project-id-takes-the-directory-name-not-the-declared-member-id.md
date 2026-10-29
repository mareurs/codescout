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
unverified: 'No regression test and no fix. Not checked: whether an auto-discovered member (not declared) shows the same mismatch; which other tools take a project_id and look it up in a store; whether the id mismatch also splits a member index between a parent run and a member run.'
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
Run output: scratchpad `kodi-id.out`, 2026-10-29. `git_sync` in the status shows the member's own HEAD `28e79441`. The sequence in Reproduction is the evidence; the two `semantic_search` calls differ only in `project_id`.

## Hypotheses tried
1. **Hypothesis:** activating by the declared id keys the index by that id. **Test:** step 2 and 3 above. **Verdict:** rejected. The status shows the directory name.

## Fix
None yet. Options, not chosen:
- Write the declared id into the member's `project.toml` `[project] name` when onboarding or activating a declared member. The risk is a rename of an existing index.
- Resolve a `project_id` argument through the workspace's project list to the store key before the lookup.
- Make the empty result name the project id it searched and the ids that hold chunks, as the negative-results ADR requires (docs/adrs/2026-08-27-negative-results-name-their-scope.md).

## Tests added
None yet. A red test should call the tool path with a declared id and expect either the file or a hint that names the indexed id.

## Workarounds
- Search with the directory name as `project_id`, or omit `project_id` while the member is the active project.

## Resume
1. Write the red test first (build a replica index in a test, search by declared id).
2. Check an auto-discovered member for the same mismatch.
3. Pick a fix from the options above and ask the user, because it changes what names a store holds.

## References
- docs/issues/2026-10-28-a-nested-git-repo-cannot-join-a-workspace-as-a-project-and-pinned-onboarding-mixes-in-the-home-workspace.md (Evidence 7)
- docs/trackers/issue-clusters/IC-3-declared-not-wired.md
