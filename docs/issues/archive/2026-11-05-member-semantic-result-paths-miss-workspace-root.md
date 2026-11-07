---
id: 25c50e1b512bce92
kind: bug
status: fixed
title: Member-scoped semantic search returns member-relative paths that fail under the active workspace root
tags:
- cluster/unclassified
- semantic-search
- workspace
closed: 2026-11-07
opened: 2026-11-05
unverified: 'Two limits remain, and neither has a tracking bug file yet: (1) a session focused on member alpha that passes project_id=beta still gets member-relative paths that resolve against the wrong root (pinned by a test as a known limitation, NOT fixed; the manual sentence does not mention the exception); (2) the one-line call site in SemanticSearch::call that routes hits through lift_explicit_member_hits has no test (no injection seam in RetrievalClient::from_env), so deleting it leaves the suite green. The gate check on experiments (FMT, CLIPPY, LEAN, DEFAULT and PYTHON_LIGHT all 0) does not cover the heavy Python suites, server-stack, librarian code in the lean lane, or the local ONNX weight tests.'
---

# BUG: Member semantic result paths cannot be read from the unchanged workspace root

## Class fit

This is a cross-project result-coordinate gap: the search filter names a member collection, but returned relative paths use that member root while subsequent ordinary tool paths still use the active workspace root. It differs from missing member data: indexing fixes the empty result yet leaves a successful search handing the caller a path that does not resolve in its current context.

## Symptom (Effect)

With /testbed active and a native index for nu-json in the explicitly shared per-workspace SQLite directory, semantic_search(query="hjson parse comments unquoted strings", project_id="nu-json", limit=3) returns src/ser.rs and src/de.rs. read_file(path="src/ser.rs", range at returned lines) searches /testbed/src/ser.rs and reports file not found. The same path/range with workspace="/testbed/crates/nu-json" returns the matching indexed source text.

## Reproduction

Provider-disabled `/tmp/external-member-positive-01a15ca9-20261105.py`, evidence `/home/marius/.cache/codescout-resident/codescout/external-member-positive-20261105/nushell__nushell-18863/`: member-semantic-result.json, root-path-read.json, member-path-read.json, wire.jsonl and launch.json. Native binary reports clean e6f07e9a6, source image sha256:3767f31b9b1a3288ad7132740dcff712737d5e855029079a4171a8065a2040ef. No provider or credential access. The preceding member index command completed0/noOOM and exact container absence was checked.

## Cause boundary

Observed: explicit project_id filters the searched collection without making the member root the subsequent tools' active root; returned file_path lacks that coordinate context. Do not infer absence of the source file from the root read. Corrected 2026-11-07: this sentence used to say the source impact and a repair remained to be checked and that no fix was claimed. A fix now exists and is on experiments (see Fix).

## Containment

For the experiment, use explicit member workspace coordinates for navigation and member queries, with identical documented environment handling for both arms. Keep the direct failed read and successful pinned read as separate witnesses. Repair admission remains false until member scope preparation is complete. Do not rewrite source facts or task outcomes to hide this mismatch.

## Fix

Status: fixed. All commits below are now on experiments at 143c1941f. It is the same SHA as on branch fix/open-issues-batch-20261107-r2, because experiments reached that tip by fast-forward.

Where the change lives: src/tools/semantic/semantic_search.rs. When an explicit project_id names a workspace member, the hits' file_path values are lifted to workspace-root-relative. The work is done by the pure helper lift_member_paths and the async helper lift_explicit_member_hits. SemanticSearch::call routes hits through lift_explicit_member_hits.

These cases are left unchanged: the root project, an unknown id, a member outside the focused root, and a pin on the member itself. The tool long_docs now say that paths are workspace-root-relative, and that they stay member-relative when the session is pinned to or focused on the member. The project_id row in docs/manual/src/tools/semantic-search.md states the same contract.

Deviation from the plan: the prefix is (workspace_root/relative_root) expressed relative to the FOCUSED root. It is not the raw DiscoveredProject.relative_root. After activate_within_workspace(member) the focused root is the member directory, and the raw prefix would double the member directory.

Commits (each label names the branch it was made on; all are ancestors of experiments):

- 41941e75eee1b5febb96d1a1e52115436c61af5d (branch fix/open-issues-batch-20261107-r2) / patch-id 9bee4cb468515e91d03b9e73361ef341eff68b76 (the fix)
- 354a164ea06d3649eee8f45c2a44a4104d869d7c (branch fix/open-issues-batch-20261107-r2) / patch-id e4f7dff3798a708a6f01d9bd9ae3f7e825db6305 (manual sentence)
- 7dd97e066bd44828758a4ca890c56c060d38a6be (branch fix/open-issues-batch-20261107-r2) / patch-id 6114e7983099fa064dd5e57dee3a1672ddc17b4b (pin the sibling-member limitation, correct a test comment)

Known limitation, NOT fixed: a session focused on member alpha that passes project_id=beta still gets member-relative paths (src/lib.rs) that resolve against the wrong root. A test pins this. The manual sentence does not mention this exception.

Probed: a symlinked workspace pin does lift correctly.

## Tests added

Seven tests in src/tools/semantic/tests.rs (line numbers: Unknown). Three were red before the fix.

Red before the fix:

- lift_member_paths_prefixes_the_member_root
- lift_member_paths_normalises_backslash_separators
- an_explicit_member_project_id_yields_paths_that_exist_under_the_workspace_root

Guards:

- lift_member_paths_leaves_the_workspace_root_project_alone
- an_unknown_project_id_keeps_its_paths
- pinning_the_member_workspace_does_not_double_the_prefix
- a_focused_member_does_not_double_the_prefix

Also added in 7dd97e066: a test that pins the sibling-member limitation (a session focused on member alpha that passes project_id=beta). Its name: Unknown.

Not covered by any test: the one-line call site in SemanticSearch::call that routes hits through lift_explicit_member_hits. RetrievalClient::from_env has no injection seam, so deleting that line leaves the suite green.

## Verification

./scripts/gate.sh --check was run on fix/open-issues-batch-20261107-r2 at its tip af3e2a9ec on 2026-11-07. Result: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green).

The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, and the local ONNX weight tests.

Each fix was reviewed adversarially by an Opus reviewer. The reviewer ran the new test red against the reverted production change (or used mutants) and approved.

After the fast-forward, ./scripts/gate.sh --check on the identical tree (experiments tip 143c1941fb735bee0c6199a229795783bb8ab64e) gave FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0, with the same coverage limits as above.

## Fix provenance

- **SHA:** `41941e75eee1b5febb96d1a1e52115436c61af5d` (`experiments`)
- **patch-id:** `9bee4cb468515e91d03b9e73361ef341eff68b76`

## Resume

The fix landed on experiments (fast-forward to 143c1941f), the gate check was green on that tree, and this file was archived into docs/issues/archive/ with doc(action="move"). The fix SHA and patch-id are in Fix provenance above.

Two limits remain open, and neither has a tracking bug file yet: (1) a session focused on member alpha that passes project_id=beta still gets member-relative paths that resolve against the wrong root (pinned by a test, not fixed; the manual sentence does not mention the exception); (2) the one-line call site in SemanticSearch::call that routes hits through lift_explicit_member_hits has no test, because RetrievalClient::from_env has no injection seam.
