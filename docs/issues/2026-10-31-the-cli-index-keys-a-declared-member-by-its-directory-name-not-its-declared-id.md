---
id: '4b2dacc49345f964'
kind: bug
status: open
title: 'BUG: the CLI index and migrate-memories key a declared member by its directory name, the server keys it by the declared id'
owners:
- marius
tags:
- cluster/declared-not-wired
opened: 2026-10-31
related:
- docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md
severity: low
---

# BUG: the CLI index and migrate-memories key a declared member by its directory name, the server keys it by the declared id

## Summary
`codescout index --project <member root>` and `codescout migrate-memories --project <member root>` open the member with `Agent::new(Some(member_root))`. That path treats the member directory as a root project, so the key is the name on disk. The server keys the same member by its declared `[[project]]` id after `activate_within_workspace`. An index built by the CLI is therefore filed where `semantic_search(project_id=<declared id>)` does not look.

## Class fit
The declared id is shown by `workspace(list_projects)` and keys every path that goes through `activate_within_workspace`, yet the two CLI commands never learn it (the `declared-not-wired` trait). It differs from the class's other members because the id is wired for the server and not for the CLI.

## Symptom (Effect)
Expected: an index built for a member by the CLI is found by `semantic_search(project_id=<declared id>)`. Observed on 2026-10-31: the key the CLI path computes is the directory name.

## Reproduction
Run 2026-10-31 as a temporary unit test, removed afterwards. The fixture is `workspace_declaring_a_manifestless_member` in `src/agent/declared_member_tests.rs`: a root with `[[project]] id = "thecrew-module", root = "remote-fixes/addon"`.
1. `Agent::new(Some(root.join("remote-fixes/addon")))`, as `Commands::Index` does (src/main.rs).
2. `agent.with_project(|p| Ok(p.project_id().to_string()))`.
3. Result: `"addon"`. The server's key for the same member, after `activate_within_workspace("thecrew-module")`, is `"thecrew-module"` (test `a_declared_member_is_keyed_by_its_declared_id_not_its_directory_name`, green).

## Environment
Branch `experiments`, tree at `45ef054e4`. Not run against a real store or a real embedder.

## Root cause
Read 2026-10-31. `Commands::Index` (src/main.rs) and `Commands::MigrateMemories` call `Agent::new(Some(root))` and then `project_id()`. `Agent::new` opens `root` as the workspace root, so it never reads the parent's `workspace.toml`, which is where the declared id lives. The declared id is applied only in `activate_within_workspace`. Not read: whether `Agent::new` could find a parent `workspace.toml` upward, and whether any other caller opens a member by directory.

## Evidence
The probe result above. The sibling issue, `docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md`, fixed the server path only.

## Hypotheses tried
None beyond the probe.

## Fix
Not started. The choice is open: have the CLI take a declared id (`--project-id`), discover the parent workspace upward, or document that the CLI keys by directory. A change here alters which key an existing CLI-built index lives under.

## Tests added
None. The probe was a throwaway.

## Workarounds
Build the member's index through the MCP `index(action="build")` after `workspace(action="activate", path="<declared id>")`.

## Resume
1. Decide the fix (see Fix).
2. Write a red test on the key for a member opened by directory, then fix.

## Also observed, display only
`get_project_info` (src/dashboard/api/project.rs) loads `ProjectConfig` from disk for `state.project_root`. For a member root that name is the on-disk name, while `project_status` and the activation response show the declared id. Not run against a dashboard on a member root.

## References
- docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md
