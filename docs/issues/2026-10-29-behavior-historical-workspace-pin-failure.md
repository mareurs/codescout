---
id: 6061dc022461c0f7
kind: bug
status: fixed
title: Historical behavioral candidate wider server check fails workspace-pin cleanliness
tags:
- cluster/unclassified
closed: 2026-11-04
opened: 2026-10-29
unverified: 'One run of the named test on experiments (Linux). The historical failure was not reproduced here: it needs the historical revision. The archived bug records that no unit test pins `new_blocking` passing the cache directory, so a regression of that line may not be caught by the three model_cache_dir tests.'
---

# Historical workspace-pin test observes embedding-cache files beside the pinned directory

## Summary
The fixed-xhigh historical candidate's wider server test command exits101, reporting348 passed and1 failed. Repeating the exact workspace-pin filter also exits101. This is outside the frozen31-function config acceptance contract.

## Class fit
Unclassified historical test/runtime side effect: a workspace-pinning assertion observes unexpected model-cache files in its subprocess cwd. Unlike the config bounded-read defect being evaluated, this failure involves embedding initialization and directory cleanliness; a causal link to the candidate patch has not been established.

## Evidence
Private behavior-transfer-20261029/broader-check-evidence.json preserves literal Bash requests and tool results, while arms/1-X/checks Cargo receipts establish actual exit101 for both executed commands. Test: server::tests::a_relative_pin_naming_an_existing_directory_still_resolves_against_the_cwd. Assertion at historical src/server/workspace_pin_tests.rs:283 reports .fastembed_cache/models--Xenova--all-MiniLM-L6-v2 and model files where the expected sibling-file list is empty. Do not treat the piped tail wrapper's is_error=false as a passing Cargo result.

## Cause and scope

The cause is a product defect that is already fixed on `experiments`. `LocalEmbedder::new_blocking` set no cache directory, so fastembed used its own default, the RELATIVE path `.fastembed_cache`, under the server's working directory. A test whose cwd is a fresh temp dir therefore saw `.fastembed_cache/models--Xenova--all-MiniLM-L6-v2` appear beside the pinned directory, which is the assertion this record reports at `workspace_pin_tests.rs:283`. The same-test baseline run on unchanged historical source (this record's v3) fails the same way, so the historical revision, not the candidate patch, carries the defect.

Fixed by `640ef2e79` (2026-10-24, five days before this record), under archived bug `docs/issues/archive/2026-10-19-the-first-memory-write-creates-a-fastembed-cache-directory-in-the-servers-cwd.md` (`b753d9ee6f8f9a95`): `model_cache_dir` in `crates/codescout-embed/src/local.rs` returns a non-empty `FASTEMBED_CACHE_DIR`, else `<user cache dir>/codescout/fastembed`, else a temp dir. The historical revision tested for this record predates that commit.

## Verification limits
Registered acceptance remains exactly31 functions and is not enlarged or rewritten after the output. The xhigh candidate is accepted only under that finite contract. Broader baseline probe-v1 failed before Cargo due a wrong module alias; v2 supplied the cargo executable twice and ran no test. Those invalid outcomes are retained and excluded from behavioral evidence. Only completed v3 may qualify the baseline.

## Resume

Nothing open on `experiments`. Measured 2026-11-04 on `experiments`: `server::tests::a_relative_pin_naming_an_existing_directory_still_resolves_against_the_cwd` passes (1 passed, 0 failed, 2.04 s). The research branches still carry the historical revision; the failure there is expected and was kept as evidence.

## Fix provenance

- **SHA:** `640ef2e79` (`experiments`)
- **patch-id:** `71ea06b50d6abac6b501a39a4be911f6dcb4ec1d`

The commit is an ancestor of `HEAD`, and its patch-id, computed today, equals the one in the archived bug. That bug's own `unverified:` says no test checks that `new_blocking` really passes the directory: deleting `.with_cache_dir(...)` keeps its three tests green, so a regression of that line may not be caught by those three tests. Whether the workspace-pin test above loads the model on this machine was not checked.
