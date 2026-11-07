---
id: b4c80abc4f6cd956
kind: bug
status: open
title: semantic_search with a sibling member's project_id returns member-relative paths, and the call site of the lift is untested
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- semantic-search
- workspace
opened: 2026-11-08
owner: marius
related:
- docs/issues/archive/2026-11-05-member-semantic-result-paths-miss-workspace-root.md
severity: medium
---

# BUG: semantic_search with a sibling member's project_id returns member-relative paths, and the call site of the lift is untested

## Summary

`semantic_search` re-roots the hits of an explicit `project_id` that names a workspace member to workspace-root-relative `file_path`s, but only when the member directory lies under the FOCUSED root. A session focused on member `alpha` that passes `project_id="beta"` gets `beta`-relative paths (`src/lib.rs`) that every following tool resolves against `alpha`. Separately, the one-line call site that routes hits through the lift is covered by no test, so deleting it leaves the suite green. Both limits were split out of the archived fixed bug `25c50e1b512bce92` (see References), which carried them only in its `unverified` field and its Resume prose.

## Class fit

Class `IC-24` (a value correct in one frame is published under a name that states another), coordinate-space axis. The member-relative `file_path` is exactly right in the frame the store keeps it in (relative to the member directory) and is published in a field and in documentation that say it feeds `read_file` unchanged, i.e. relative to the root every other tool resolves against. It differs from the class's other members in two ways: the repair for the same bug already exists and works for every other session shape (so only one branch is wrong, and it is wrong only for a caller whose focus differs from the member it names), and here the recoverable true value depends on the caller's focus, so no single constant can be added to the response the way the class's body-relative line range could be fixed. The response also carries no field that names which frame the path is in.

## Symptom (Effect)

No live error string was captured for this bug; the behaviour is pinned by an existing test whose assertions (read in `src/tools/semantic/tests.rs`, test `pins_current_limitation_sibling_member_hits_stay_member_relative`, declared at line 1181, doc comment from line 1175) are:

```
assert_eq!(hits[0].file_path, "src/lib.rs");
assert!(
    !focused.join(&hits[0].file_path).is_file(),
    "the unlifted path does not resolve against the focused root"
);
```

The test's doc comment says the hits stay "as the store returned them (`src/lib.rs`, relative to `crates/beta`). Those paths do not resolve against the root `read_file` uses." That test also states it "pins a CURRENT LIMITATION, not a wanted behaviour" (introduced by commit `7dd97e066`, "test(semantic): pin that a sibling member's hits stay member-relative when focused on another member ...").

Inferred, not observed: because `read_file` resolves a relative path against the focused root, the returned `src/lib.rs` would open `alpha`'s own `src/lib.rs` if `alpha` has a file of that name, or fail with file-not-found if it does not (in the test fixture `alpha` has only `src/ser.rs`, so the pinned case is the not-found one; the silent wrong-file case is reasoned from the path rule, not run).

## Reproduction

Not reproduced by running in this session (no cargo allowed). The existing pin test is the reproduction at unit level. Plan:

1. Run the pin test: `scripts/with-slot.sh cargo test pins_current_limitation_sibling_member_hits_stay_member_relative` (test name from `src/tools/semantic/tests.rs:1181`). It builds `two_member_workspace()` (members `crates/alpha` with `src/ser.rs`, `crates/beta` with `src/lib.rs`), calls `activate_within_workspace("alpha", None)`, then `lift_explicit_member_hits(&ctx, "beta", Some(&focused), &mut hits)` and expects the path unchanged.
2. Live variant: in a workspace with two members whose index is built, focus member `alpha` (activate_within_workspace), run `semantic_search(query=..., project_id="beta")`, then `read_file(path=<returned file_path>)`. Expected by the documented contract: it opens `beta`'s file. Expected from the code: not-found, or `alpha`'s same-named file.
3. Gap 2 (call site untested): delete the `lift_explicit_member_hits` line at `src/tools/semantic/semantic_search.rs:838` and run the semantic tests. Predicted: green (see Root cause). Not run.

## Environment

Rust codescout server, branch `experiments` (shared, dirty checkout), read in the working tree on 2026-11-08. MCP transport not relevant (pure path arithmetic). Needs a workspace with at least two discovered members and a focus on one of them.

## Root cause

Gap 1, mechanism read from code, not measured at runtime:

- `src/tools/semantic/semantic_search.rs:837-839`: `if explicit_project_id { lift_explicit_member_hits(ctx, &project_id, root.as_deref(), &mut hits).await; }`. `root` is the focused root (from `project_root_for(ctx.workspace_override...)`, line 666-667).
- `src/tools/semantic/semantic_search.rs:985-1017`, `lift_explicit_member_hits`: it finds the member by id among the discovered projects, skipping the root project (`p.relative_root != Path::new(".")`, line 999), builds `member_root = workspace_root.join(relative_root)` and `focused_root`, and only `if let Ok(rel) = member_root.strip_prefix(&focused_root)` (line 1013) calls `lift_member_paths` with `rel`. When `beta` is not under the focused root `crates/alpha`, `strip_prefix` fails, the `if let` has no else, and the hits stay as stored. The doc comment states it: "A root project, an id that names no member, and a member outside `focused_root` are left untouched." (line 984).
- `src/tools/semantic/semantic_search.rs:960-973`, `lift_member_paths`: prepends the prefix only; it already keeps `..` segments (it drops only empty and `.` segments), so a prefix such as `../beta` would be emitted verbatim. The missing part is computing such a prefix, which `strip_prefix` cannot do.

The prefix is "the member root expressed relative to the FOCUSED root" (doc comment at lines 977-984); the branch where that expression does not exist (a sibling) is the one left unchanged. Archived fix commit: `41941e75e` ("fix(semantic): an explicit member project_id returns workspace-root-relative file paths ..."; its subject cites bug id `d3a90bec4f8f5ced`, the id the bug carried before it was re-pointed to `25c50e1b512bce92`).

Gap 2, read from code, not measured by deleting the line:

- The call site is the single statement at `semantic_search.rs:838`, inside `SemanticSearch::call`.
- `call` builds its retrieval client at `semantic_search.rs:668` with `crate::retrieval::client::RetrievalClient::from_env(root.as_deref())`, which reads config and the environment and builds the store and embedder (`src/retrieval/client.rs:351-376`). `call` has no parameter or hook through which a test can supply a client. A `RetrievalClient::from_config_only(config)` constructor exists (`client.rs:403`) but `call` does not use it (the only `RetrievalClient` construction in `semantic_search.rs` found by grep is line 668).
- The only test that invokes `SemanticSearch.call` is `concurrent_semantic_search_does_not_deadlock` (`tests.rs:327-328`), with input `{"query": "test"}` and no `project_id`; it discards both results. A grep for `"project_id":` under `src/tools/semantic/` found it only in the input schema (`semantic_search.rs:593`) and in `index.rs` (IndexStatus/IndexVerify), never in a `SemanticSearch` test.
- The lift tests call a helper `lifted_paths` (`tests.rs` ~line 1076-1090) that re-applies `lift_explicit_member_hits` by hand ("after the step `SemanticSearch::call` applies to an explicit `project_id`"). It exercises the function, not the call site, so removing line 838 changes no test result. This is inferred from reading; the deletion was not run.

Documentation gap (doc-vs-code), read verbatim:

- `semantic_search.rs:534-537` (tool long_docs): "`project_id`: filter to a specific workspace sub-project. Result `file_path`s are relative to the workspace root, not the sub-project, so they feed `read_file` unchanged. (When the call is pinned to the member itself with `workspace=<member root>`, the root is the member, so they stay member-relative.)"
- `docs/manual/src/tools/semantic-search.md:57`: "... Result `file_path`s are relative to the workspace root, not the sub-project, so they feed `read_file` unchanged; they stay member-relative when the call is pinned to the member (`workspace=<member root>`) or the session is focused on it."
- Neither names the cross-member exception, so both claim "feed `read_file` unchanged" for a case where the code does not make that true.

## Evidence

- Code and test quotes above, all read on 2026-11-08 with codescout `read_file`/`grep`/`symbols`.
- Commit subjects verified with `git log -n1`: `41941e75e` (the lift), `7dd97e066` (the pin).
- Archived parent bug: `docs/issues/archive/2026-11-05-member-semantic-result-paths-miss-workspace-root.md` records both limits as "NOT fixed" and "Not covered by any test".
- Nothing was executed (no cargo, no live search).

## Hypotheses tried

1. Hypothesis: the sibling case is already handled elsewhere (for example the store returns root-relative paths). Test: read `lift_explicit_member_hits` and the pin test. Verdict: rejected by the code read; the pin test asserts the unchanged `src/lib.rs`.
2. Hypothesis: the call site is covered by an end-to-end test. Test: grep for `SemanticSearch.call` and `"project_id":` in tests. Verdict: rejected; the one caller passes no `project_id`.

## Fix

Open design decision, deliberately NOT decided here. Options that exist, with the cost read from the code:

1. Emit a `../beta/...` path. `lift_member_paths` already tolerates `..` segments, so the change is in `lift_explicit_member_hits`: compute a relative path between `member_root` and `focused_root` (today only `strip_prefix` is used, line 1013) instead of skipping. `read_file` can open it: `validate_read_path_with` joins a relative path onto the root, canonicalizes, and checks only a deny-list (`src/util/path_security.rs:501-545`). Cost: the path leaves the focused root, so write-side containment (`validate_write_path`, which reports "is outside the project root", `path_security.rs:931`) will treat it as outside; and the same hit is spelled differently depending on the caller's focus.
2. Add a root field to the response. `search_response` builds `{results, total, truncated}` (`semantic_search.rs` ~lines 935-950); a field naming the root the paths are relative to would let a caller pass it as `workspace=` to `read_file`. Cost: `file_path` stays member-relative, so the documented "feed `read_file` unchanged" sentence stays false for this case unless it is rewritten, and every caller must learn the new field.
3. Refuse the call (a `RecoverableError` whose hint names the member root to pin). Cost: removes a cross-member search that works today; the pinned call (`workspace=<beta root>`) is already the route the manual describes.
4. (Not asked, listed for completeness) Emit an absolute path for this branch; `read_file` accepts absolute paths (the `path.is_absolute()` branch in `validate_read_path_with`). Cost: the machine-specific prefix in results, and a different shape from the rest.

Whichever is chosen, the long_docs (`semantic_search.rs:534-537`) and the manual row (`semantic-search.md:57`) must name the cross-member behaviour, and the pin test at `tests.rs:1181` must change with the code ("If this is ever fixed, this test must change with it").

Gap 2 plan (not designed in detail): give `SemanticSearch::call` a seam to receive a client (for example split the post-search tail, lines 833-839 onward, into a function that takes the hits, or let a test build a `RetrievalClient::from_config_only` over a fake store), then assert end to end that an explicit member id yields workspace-root-relative paths. Verify the test red by deleting line 838.

Status stays `open`; no fix SHA or patch-id exists.

## Tests added

None yet. Justification: the bug is open. The existing `pins_current_limitation_sibling_member_hits_stay_member_relative` (`tests.rs:1181`) pins the defect rather than guarding a wanted behaviour, and there is no test of the call site.

## Workarounds

Pin the call to the member that owns the files: pass `workspace=<member root>` (the manual's described route, `semantic-search.md:57`), so the search and the following `read_file` share the member root. Not run in this session; taken from the documented contract. Or focus the session on the member being searched.

## Resume

Run the pin test (plan step 1 under Reproduction) to confirm it is green on the current tip, then take the design decision among the four options in Fix with the owner. After that: edit `lift_explicit_member_hits` at `src/tools/semantic/semantic_search.rs:1013` (the `strip_prefix` branch), update the pin test at `src/tools/semantic/tests.rs:1181`, and update `src/tools/semantic/semantic_search.rs:534-537` and `docs/manual/src/tools/semantic-search.md:57`. Then close Gap 2 with a seam for the call site.

## References

- Archived parent (fixed): `docs/issues/archive/2026-11-05-member-semantic-result-paths-miss-workspace-root.md` (id `25c50e1b512bce92`); fix commit `41941e75e`, pin commit `7dd97e066`.
- `src/tools/semantic/semantic_search.rs` (lines 534-537, 668, 837-839, 960-973, 985-1017), `src/tools/semantic/tests.rs` (lines 327-328, 1175-1181), `src/retrieval/client.rs` (351-376, 403), `src/util/path_security.rs` (501-545, 931), `docs/manual/src/tools/semantic-search.md:57`.
- Class: `docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md`.
