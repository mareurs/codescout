---
id: 12d287737be5e06f
kind: plan
status: done
title: Prepare six isolated semantic-contract runtimes without provider calls
tags:
- effort-steering
- zero-provider
- runtime-isolation
---

# Semantic-contract runtime preparation implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. User already delegated execution and routine choices; do not ask for method confirmation. Checkbox steps track this bounded continuation.

**Goal:** Create six physically isolated runtime input copies and executable prepared configs from the reviewed frozen prompt bundle, retaining launch_admitted=false.

**Architecture:** Consume semantic-contract-pilot-20261106 unchanged and corrected-profile native argv templates from the closed external-exec-baseline-20261106-v3. Add a narrow standard-library preparer in a new isolated worktree from36fec1201; use existing external.validate_prepared. No grader edits, new onboarding or model calls. Runtime preparation is independent of still-pending grader/parser admission.

**Tech Stack:** Python unittest, JSON/SHA256, physical file copies; existing lifecycle validation.

**Spec:** docs/superpowers/specs/2026-11-06-semantic-contract-pilot-design.md (28aab59cb5842630).

## Global Constraints

- Source baseline36fec1201b0f809d6f21cc31082def4d651b6cf8, external4CPU32GiB RAM/private executable4GiBtmpfs uid/gid1000. Source worktree .worktrees/semantic-contract-runtime-01a15ca9, branch research/semantic-contract-runtime-20261106.
- Exact six frozen IDs/order/efforts/prompts/rules, original checks/timeouts1800/3600/8. Preserve immutable input bundle and all prior cohorts. Provider calls/credential reads/onboarding0.
- Writable knowledge/native-home/member-state files physically separate from source and every other arm; global model cache readonly. No sharing SQLite files or WAL through links.
- Runtime data outside repo/tmp; only small actor instruction folders under/tmp. Durable actor instruction backups reproduce them exactly.
-20USD reserve/19USD CLI cap each, prior58.180531399999999104, series200. No paid runner, started markers, or admission=true.
- Peer owns tools/setup-ab-eval. No redundant closed NUlow regrade. MAIN gate red for unrelated companion .test.mjs scanner issue1dfa43ac9834a309; preserve/report that baseline, do not patch sibling code during this continuation.

## Review Focus

- Rewritten mounts must not retain old writable state or admit an unknown/duplicate writable destination.
- Same bytes alone do not prove isolation: apply an actual write to a disposable clone and observe source/sibling unchanged; inspect inodes across the real population.
- Variant ID must remain separate from actual effort; treatment-only prompt delta and all full frozen rules remain byte-bound.
- Copy validation must reject missing/extra/symlink/path-traversing payloads and changed source bytes before output side effects.
- Existing actor validation must consume every generated config; prepared configs must not imply model launch or six-arm CLI wire proof.

### Task 1: Implement and test a narrow preparer

**Files:** Create tools/agent_lifecycle/semantic_prepare.py and test_semantic_prepare.py in the isolated worktree. Private command/log/registration backups under resident scratch semantic-contract-runtime-20261106.

**Interfaces:** checked_snapshot(path: Path, expected_digest: str) -> dict; clone_snapshot(source: Path, destination: Path, manifest: dict) -> dict; rewrite_native(argv: list, state: Path, projects: list, name: str) -> list; prepare(bundle: Path, output: Path, actors: Path, qualification: Path) -> dict. Existing external.validate_prepared(out, effort, allowance, cli_cap) remains unchanged.

- [x] Write unittest controls before implementation: corrupt/missing/unlisted/symlink/traversal payloads reject before clone output; clone bytes/inodes and actual write isolation; exact root/member/home mount remap and readonly preservation; missing/extra/duplicate writable mounts rejected; full tiny six-arm fixture checks frozen variants/real efforts/full rules/checks and absent start markers. Expected preimplementation RED, then GREEN.
- [x] Implement only those interfaces, preserving freeze registration/matrix/verifier-v2 bytes. Each real prepared config must pass external.validate_prepared with its actual effort and fixed20/19 budgets.
- [x] Run new suite and existing lifecycle suite; record any baseline failure by exact name. Fresh independent whole-change review must actually apply focused mutations to mounting/isolation/identity guards and report killed/survived.

### Task 2: Execute one no-provider six-runtime preparation

**Files:** New private resident root semantic-contract-runtime-20261106 and small reproducible actor contexts /tmp/cs-semantic-contract-01a15ca9-20261106/<index>.

**Interfaces:** Consumes task1 prepare and existing frozen prompt/runtime manifests; produces six prepared.json/native-argv/physical-clone manifests, source/input identity registration and launch_admitted=false summary.

- [x] Check headroom at least80GiB root; verify two qualified template hashes/resources/closed baseline evidence and pristine snapshot manifests; snapshot helper/source/input hashes before execution.
- [x] Execute once against fresh output root, never overwrite a partial/old attempt. No Docker container/model/transport server/credentials. Verify every payload hash and distinct inode across all six and pristine source, exact frozen prompt/rules/checks, new names and read-only global cache.
- [x] Archive small manifests/configs/helper/tests/review/command logs with reopened hash verification; exclude writable DB/model/cache payloads. Preserve old archives unchanged.

### Task 3: Close handoff and remaining admission

- [x] Update report/memory with measured physical preparation, source identity, zero costs and actual test/mutation outcomes. Report MAIN gate failure separately.
- [x] Keep prospective parser policy, reviewed committed grader, fresh empty/gold/repeat controls, actual six-arm synthetic CLI wire proof and source-bound paid controller as explicit remaining gates. Do not start paid runs or label mechanism validated.

## Decisions

Ruling: proceed with zero-provider runtime copying despite the unrelated MAIN gate failure — user authorized autonomous progress, and this work uses the previously qualified immutable lifecycle source plus its clean focused baseline. Cost if wrong: preparation must be discarded if later relevant source/input identity changes. This does not admit paid execution or a shared-branch merge.

Ruling: no automatic commit/merge/push by this plan; source work remains isolated and durable evidence is archived for review. Exact-path source commit can follow independently after verification; no shared index changes are needed.

## Rulings on review scope limits

1. Actual CLI delivery remains deferred: native-configs-only does not prove rules/protocol/effort delivery. Require actual six-arm synthetic wire before spending. Cost if ignored: mismatched treatment comparison.
2. Native execution/permissions/cleanup remain deferred to wire/runtime admission; only exact historical template identities are reused here. Cost if wrong: new infrastructure invalidity must stop admission, with prepared data retained.
3. Grader/parser policy and fresh negative/gold/repeat controls remain mandatory and peer-owned. Do not repeat the already closed NUlow diagnostic. Cost if ignored: semantic/infrastructure misclassification.
4. Paid controller/full plugin binding/fresh resource monitoring/cohort accounting remain unimplemented; launch stays false. Cost if ignored: unbounded or unmatched calls, so no paid entry is supplied here.
5. Mechanism efficacy/generalization/cost are unmeasured; no production skill change follows from preparation. Cost if ignored: publishing a causal claim from setup checks.
6. Whole-project gate remains separate: isolated gate closed exit0 at13:54:35UTC after review; MAIN baseline remains red. No merge-ready claim; report each final outcome and the sibling-surface coverage limit. Cost if ignored: hidden source baseline failure.

## Observed review closure

Fresh reviewer actual mutation population12:7 killed/5 survived, four regression-coverage issues filed immediately. One test-only fix pass:13 new controls/89 lifecycle tests GREEN; identical12 mutations reapplied on copies now12 killed/0 survived. Source preparer bytes and all six real configs unchanged. Original mutation outcomes retained. M10 is validator-argument fidelity, not proof of wrong eventual model effort; M01/M02 include genuine production errors. Independent audit rehashed5850 cloned and1950 pristine selected files, actor rules/backups,294 mounts and unchanged frozen inputs; no current config defect found. Source-v1 backup21files matches actual preparation registration; new tests are versioned separately.


## Archive closure

Runtime archive SHA256 ae333e71ab057e185f62746bbec7de854a5a7a64804e77aa3508eef28fdf3628, 475 payloads plus manifest all reopened/hash verified. Report b29efd60d862d8fc records exact scope and remaining admission. Isolated gate10e closed exit0; MAIN109 remains red. Separate actual CLI wire continuation d25ae1f4beca1f9c records delivery evidence and its unresolved full-plugin provenance; preparation itself admits no launch.
