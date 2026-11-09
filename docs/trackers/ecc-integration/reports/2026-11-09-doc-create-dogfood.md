---
id: '5f9b537a3583bbc7'
kind: report
status: done
title: 'ECC dogfood: doc(create) cleanup diagnostic correction'
tags:
- ecc
- dogfood
- doc-create
topic: ecc-integration
---

# doc(create) dogfood — 2026-11-09

## Current result

The known conditional diagnostic defect was reproduced and corrected in the isolated branch `fix/ecc-doc-create-cleanup-20261109` at `/home/marius/work/claude/codescout/.worktrees/ecc-doc-create-cleanup`. The correction changes the caller-visible error from a guaranteed catalog rollback to a truthful attempted-cleanup warning. The original I/O cause and best-effort cleanup behavior remain intact. The user subsequently authorized commit/integration: the source correction is now on experiments as `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7` (source-worktree commit `561c945d72a3997c972d5ef055e2c9d38e922e67`, patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`). The shared MCP server has not been redeployed.

**Verified in the isolated worktree:** module GREEN, private real-MCP cases, independent applied mutations and the complete standard-environment gate passed. This describes the pre-integration source verification. Post-integration verification is recorded separately; no shared-server deployment occurred.

## ECC contribution and limits

Applied the installed `ecc-spec-miner` and `ecc-silent-failure-review` instructions directly in this session: bounded observed behavior, contract/implementation distinction, propagation to the caller, tests inspected versus executed, and confirmed defect versus permitted fallback. No new comparative model cohort ran. The prior baseline and ECC arms had already found the same defect; this cycle demonstrates a verified product correction while dogfooding the adapted workflow, not exclusive ECC discovery or superiority.

[Plan and execution ledger](../plans/2026-11-09-doc-create-dogfood.md). [Bug record](../../../issues/archive/2026-11-09-doc-create-claims-rollback-after-best-effort-cleanup.md).

## Failure path and correction

`create::call` inserts the artifact and optional augmentation before directory/file writing. A disk failure invokes `undo_failed_create`, which only logs catalog/file cleanup errors. The prior outer context always said the catalog row was taken back. `select_error_render` exposes that outer message for fatal errors; a cleanup warning in server logs cannot correct it for the agent.

The new outer diagnostic is: `could not write <path>; cleanup was attempted, but the catalog row or a partial file may remain`. This intentionally conservative warning also appears when cleanup succeeded; exact per-effect cleanup status is outside this minimal correction. Source scope: one comment, one diagnostic literal and one regression test in `src/librarian/tools/create.rs`.

## Reproduction and regression

A real regular file occupies the intended parent path, so directory creation fails after catalog insertion even under root. A `BEFORE DELETE ON artifact` trigger raises an SQL abort. No real catalog, quota exhaustion, filesystem permissions, mocks or production failure-injection hooks are used.

- Baseline selected module: 47 passed, 0 failed, 0 ignored.
- New test before production fix: 1 executed, 1 failed at the false outer rollback assurance, after the original I/O cause and retained artifact/augmentation had been asserted.
- Module after correction: 48 passed, 0 failed, 0 ignored, including the new test. It executes two subcases: deletion fails (one artifact and one augmentation remain) and deletion succeeds (neither remains). The parent blocker content stays intact.

The new test is `librarian::tools::create::tests::failed_disk_write_reports_best_effort_cleanup_without_promising_rollback`. These module totals are not full-suite totals. Logs and completion records: [baseline](../evidence/2026-11-09-doc-create-dogfood/baseline-record.json), [RED](../evidence/2026-11-09-doc-create-dogfood/red-record.json), [GREEN](../evidence/2026-11-09-doc-create-dogfood/green-record.json), with matching compressed logs beside them.

## Real MCP observations

A locally built worktree binary ran over stdio MCP with a disposable Git project and private LIBRARIAN_DB, workspace config, sqlite-vec backend and XDG directories. The [probe](../evidence/2026-11-09-doc-create-dogfood/probe.py) uses the existing pilot transport helper; its exact snapshot is retained as [transport-helper.py.txt](../evidence/2026-11-09-doc-create-dogfood/transport-helper.py.txt).

| Case | MCP result | Residual artifact rows at target | Residual augmentation rows at target |
|---|---|---:|---:|
| Disk failure + forced SQL delete failure | isError=true, attempted cleanup/residual-state warning | 1 | 1 |
| Disk failure + successful SQL delete | isError=true, same conservative warning | 0 | 0 |
| Normal create | successful id and file | created normally | not requested |

The probe passed its assertions and recorded exact responses in [mcp-result.json](../evidence/2026-11-09-doc-create-dogfood/mcp-result.json). It does not count transport completion as tool success. [Binary hash](../evidence/2026-11-09-doc-create-dogfood/binary.sha256) was captured under the active build lease before probing; the shared running server was not replaced.

## Applied mutations and independent review

Independent fresh-context review (`gpt-6-astra`, high) found no Critical, Important or Minor issues. [Review record](../evidence/2026-11-09-doc-create-dogfood/independent-review.txt).

Two distinct candidates were applied through mutation-probe.sh --strict in an isolated tree; both were observed KILLED, none of the completed candidates survived. Restoring the old diagnostic ran 48 tests: 47 passed, 1 failed at the new outer-message assertion. Removing the production cleanup invocation ran 48 tests: 46 passed, 2 failed (the existing successful-cleanup assertion and new deletion-success subcase). [Diagnostic record](../evidence/2026-11-09-doc-create-dogfood/mutation-diagnostic-record.json), [cleanup record](../evidence/2026-11-09-doc-create-dogfood/mutation-cleanup-record.json), compressed logs alongside.

Quota/timeout attempts provided no usable verdict and are retained as operational history rather than extra mutation candidates. A timeout temporarily left the isolated copy mutated; the later completed runs reset and restored it. The cleanup run carried untracked scratch artifacts with copy warnings; intended tests nevertheless ran and failed at their discriminating assertions. Parent independently checked archived ARMED/failure/count/verdict lines and [final restoration](../evidence/2026-11-09-doc-create-dogfood/restoration.json): reviewed/isolated source hashes match, mutation markers empty, own scratch removed.

Reviewer scope limits stand: parent ran gate/MCP; actual disk exhaustion and failed partial-file unlink were not separately injected. Existing helper tests execute real partial-file removal. The correction promises no stronger cleanup or atomicity, so these are explicit unexecuted paths rather than a new diagnostic contract.

## Full repository verification

`./scripts/gate.sh --check` exited 0 in the standard environment: FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0, PYTHON_LIGHT=0. Python: 1764 passed, 1 skipped, 554 subtests passed. The new regression is explicitly PASS in DEFAULT; lean excludes librarian and is not evidence for it. [Completion record](../evidence/2026-11-09-doc-create-dogfood/gate-record.json) and [full compressed log](../evidence/2026-11-09-doc-create-dogfood/gate.log.gz).

The earlier nested-TMPDIR attempt was red: Python reported 189 failed / 1606 passed / 1 skipped; CLIPPY=101 and LEAN=101 failed to determine package fingerprints while scratch directories inside the workspace changed; DEFAULT=0 with its regression passing. Logs also show nested Python fixture packages incorrectly inheriting the parent Cargo workspace. That environment was an implementer workaround for /tmp quota and was withdrawn. Own scratch was removed; no source changes intervened before the passing standard-environment run. [Failed-attempt record](../evidence/2026-11-09-doc-create-dogfood/mcp-and-nested-tmp-gate-record.json) and log remain beside it. The final pass does not establish an individual cause for every earlier Python failure.

Gate exclusions: heavy Python suites, server-stack and execution of the local ONNX path remain outside coverage; local-embed is compiled by clippy. Two ignored doctests are not executed assertions. Tracker/replay documents finalized after the source gate receive separate file-link/frontmatter/hash validation.

## Provenance and operating friction

[Source-state record](../evidence/2026-11-09-doc-create-dogfood/source-state.json) records the frozen base `fe965888377151110f7fd10ab7c20469d6ea2f1c`, worktree branch, SHA256 of the tested source and transport helper, and an unmodified parent source file. [Source patch](../evidence/2026-11-09-doc-create-dogfood/source.patch) preserves the uncommitted correction and regression.

An initial build succeeded but copying the binary to /tmp failed with a per-user quota error despite free filesystem space/inodes. That batch ran neither MCP probe nor gate. The known partial copy alone was removed; no peer caches were cleaned. A named slot retry was refused because a peer held it. The successful probing batch executes the rebuilt binary in-place while holding its auto-acquired lease. Its test TMPDIR is the worktree's `.codescout/ecc-dogfood-tmp`, which is part of the verification environment. A transient anonymous document-write lock delayed one ledger update. These are observed operational frictions, not additional confirmed code defects.


Final artifact checks: 16 tracker/bug Markdown files checked; 182 inline file-link targets exist, with supported :line suffixes handled. Required kind/status/title/tags and optional owners shape validated. This checks file existence and basic metadata, not citation semantics or heading anchors. [Validation result](../evidence/2026-11-09-doc-create-dogfood/document-validation.json). The initial checker falsely required omitted empty owners and treated :line as filename bytes; its result is retained separately, and no artifact was edited to appease that incorrect instrument. [Evidence manifest](../evidence/2026-11-09-doc-create-dogfood/manifest.json) records SHA256/bytes; all listed evidence hashes were rechecked.

## Future replay capture

At the user's request, the exact baseline commit/tree, validated full source archive, reconstructed neutral task, original user messages/initial issue, instructions, observed model/effort/tool versions, sanitized config, skill snapshots, tool catalog and actual shared-MCP executable were frozen for later replay. [Replay note and condition checklist](../notes/2026-11-09-doc-create-replay.md). No replay ran. Candidate inputs must exclude the solution patch, added test, reports and evaluator evidence; a fresh session is a bounded repair replay, not an identical reproduction of this informed long session.

## Integration boundary

User authorized commit/integration after the initial report. Source committed in the original worktree as `561c945d72a3997c972d5ef055e2c9d38e922e67`, integrated onto experiments as `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, stable patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`. [Integration record](../evidence/2026-11-09-doc-create-dogfood/integration.json) verifies matching patch-ids and preservation of the pre-existing foreign staged path set. No foreign paths were unstaged. Post-integration gate passed on a frozen clean worktree at exactly that commit: FMT/CLIPPY/LEAN/DEFAULT/PYTHON_LIGHT all 0; lean 6306 passed / 48 skipped, default 9195 passed / 62 skipped, Python 1764 passed / 1 skipped / 554 subtests. [Result](../evidence/2026-11-09-doc-create-dogfood/integration-gate-off-result.json), [log](../evidence/2026-11-09-doc-create-dogfood/integration-gate-off.log). The successful run used external task scratch and `CODESCOUT_GATE_TMPFS_BASE=off`, disabling the optional /tmp nextest override. Earlier cut-off and quota-failed attempts are retained; two lean tests failed on OS quota errors while passing in the default lane. This establishes the integrated source under the recorded environment, not every possible runtime configuration. No push, master promotion or shared-server rollout has occurred.

The next [navigation dogfood cycle](../plans/2026-11-09-navigation-dogfood.md) starts at that exact integrated commit/tree; the first replay remains anchored to fe965888. Personal instruction bodies marked private remain outside versioned artifacts.
