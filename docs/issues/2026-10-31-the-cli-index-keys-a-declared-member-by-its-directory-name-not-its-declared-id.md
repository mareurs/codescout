---
id: '4b2dacc49345f964'
kind: bug
status: fixed
title: 'BUG: the CLI index and migrate-memories key a declared member by its directory name, the server keys it by the declared id'
owners:
- marius
tags:
- cluster/declared-not-wired
closed: 2026-10-31
opened: 2026-10-31
related:
- docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md
severity: low
unverified: 'The CLI wiring in src/main.rs was not run: no test and no live run of codescout index --project <member>. The dashboard display for a member root is unchanged and unchecked. A member of a parent with no workspace.toml keeps the directory-name key by design. The function, its tests, and the clippy, lean, default, Python and format lanes of the 2026-10-31 gate are verified.'
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

Chosen and done 2026-10-31 in 9e75e5b2f (branch `experiments`). `agent::workspace_member_id(member_root)` walks up to the nearest ancestor with a `.codescout/workspace.toml`, asks `resolve_projects` (the function both loaders use), and returns the id of the member whose root is exactly that directory. `Commands::Index` and `Commands::MigrateMemories` take that id when it exists and keep the old key when it does not.

- **SHA:** `9e75e5b2f` (`experiments`)
- **patch-id:** `524a5cd21fdbcda8296b94bac3067fc07f67b31d`

Why this and not the alternatives: a `--project-id` flag leaves the wrong key as the silent default; discovering the parent inside `Agent::new` would turn any repo inside another workspace into a member for the server too.

Limits, by design: only an ancestor with `workspace.toml` is asked, because `.codescout` alone also marks `$HOME` and every directory a session once opened. A member of a parent that never wrote a `workspace.toml` keeps the directory-name key.

## Tests added

In `src/agent/declared_member_tests.rs`. Red first against a stub that returned `None`: the two positive tests failed with `left: None`.
- `a_member_opened_by_its_directory_resolves_its_declared_id`
- `a_discovered_member_resolves_its_directory_name_as_its_id` (its manifest `name` differs from the directory)
- `a_directory_outside_any_workspace_has_no_member_id` and `a_workspace_root_has_no_member_id` (green on the stub too; they guard the `None` arms). Removing `.skip(1)` in the ancestor walk turned the workspace-root test red; reverted.

No test covers the `.codescout/workspace.toml` marker: it is a cost and safety guard, and a test that omits it still gets the right answer. The wiring in `src/main.rs` has no test and was not run: it needs a real store and an embedder.

## Workarounds
Build the member's index through the MCP `index(action="build")` after `workspace(action="activate", path="<declared id>")`.

## Resume

1. Run `codescout index --project <member>` for a declared member against a real store and embedder, then `semantic_search(project_id=<declared id>)`. Not done.
2. Decide the dashboard display (see Also observed).
3. Then archive with `doc(action="move", ...)`.

## Also observed, display only
`get_project_info` (src/dashboard/api/project.rs) loads `ProjectConfig` from disk for `state.project_root`. For a member root that name is the on-disk name, while `project_status` and the activation response show the declared id. Not run against a dashboard on a member root.

## References
- docs/issues/2026-10-29-semantic-search-project-id-takes-the-directory-name-not-the-declared-member-id.md
