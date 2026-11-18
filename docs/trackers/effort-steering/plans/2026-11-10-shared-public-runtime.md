---
id: '9127a2cd73136cf3'
kind: plan
status: done
title: Shared public fixture runtime and cached preparation alignment
tags:
- effort-steering
- implementation-plan
- common-runtime
topic: effort-steering
---

# Shared Public Runtime Implementation Plan

> **For agentic workers:** use superpowers:executing-plans inline. Standing user delegation opts out of recurring approval rounds. Existing isolated runtime worktree cf66 remains the implementation location. No merge,push,global config change,destructive cleanup or paid model repair. Preserve all current WIP before edits. One fresh strongest final reviewer applies actual guard mutations; one bounded correction pass, no review loop. Parent eight-attempt campaign stays separate and unadmitted.

**Goal:** Give every arm the same declared public source,offline page fixture and native/command runtime while reusing existing source-only onboarding.

**Architecture:** A frozen public bundle contains a generic Python static HTTP service,spec and two immutable launchers. The native launcher starts the loopback service then execs the SAME /bench/bin/codescout; the command launcher starts it then execs the original argv. Docker remains isolated,UID1000,read-only root and source as already registered,with one exact readonly bundle mount and fixture-prefixed reserved hostname→127.0.0.1. Existing profiles/transport/MCP protocol remain unchanged by default; opt-in profiles bind the complete fixture digest and runtime. Public data carries no private tests,gold,results or repair hints.

**Tech Stack:** Python standard library,Docker,existing lifecycle/native executor,receipt/readiness/preparation modules.

**Spec:** [parent protocol plan](../evidence-protocol-plan-20261109.md), [qualified fixed-page/working-tree baseline observations](../reports/2026-11-10-hermetic-page-fixture.md), [source parity issue](../../../issues/2026-11-10-obscura-evaluator-source-byte-binding-mismatch.md).

## Approaches and ruling

A separate network service would expand the already strict embedding-relay topology and command network boundary. A host-side startup supervisor alone leaves a race before the first native command. Choose an immutable launcher with os.exec: service readiness precedes the real command without changing MCP framing or final main-process exit semantics. This is the implementer's choice under standing delegation. Cost if wrong: reject the new opt-in runtime at qualification; old studies/profiles stay unchanged and payment stays closed.

## Global constraints

- Runtime/module must contain no task/repository IDs,function names or gold/expected-result literals. Fixture hostname/port/routes/body are explicit PUBLIC data only.
- Same public fixture version and source baseline for L/R/P/X within each project. Public actor packets never receive private evaluator files or host-control paths.
- Existing source-only onboarding reused; no fresh author/LLM onboarding. Obscura's2existing image-source edits and namespaced public test-input patch are declared public baseline overlays. Existing preparation remains immutable; changed files require index/readiness/source rebinding before admission.
- No source/repo commit under the publication hold. Private synthetic Git seed objects are preparation artifacts,not publication; preserve lineage and exact source/index equality.
- Existing fixture-v1,cargo-v1,cargo-v2 digests and behavior unchanged. New fixture-public-v1/cargo-public-v1 are explicit opt-ins; latter retains cargo-v2's900sbound.
- Fixture failure must prevent actual command execution and be infrastructure/incomplete,never a functional candidate failure or implicit retry. Complete streams,cost unknowns,cleanup and source-capture evidence remain mandatory.
- Preserve historical known64.401093499999998654USD and4USDhold under200USDseriescap. No experimental model/provider repair calls before parent admission.

## Review focus

1. Missing/changed bundle files,symlinked roots/parents,or modified service/spec must fail before actor/command effects; copying a sibling guard without its regression is insufficient.
2. Runtime must reject writable/extra fixture mounts,wrong hostname/port/entrypoint,image/source/bundle digest drift and fixture injection into old profiles.
3. Failed service startup must not execute supplied argv; successful launch must exec exact argv,cwd and exit status,with MCP stdout unpolluted and bounded readiness.
4. Adapter supplies fixture binding from trusted request only; actor submission/public packet cannot select or replace it. Receipt readback must independently enforce the trusted expected binding.
5. All arms get one identical public source/knowledge/rules/runtime version; cached onboarding is preserved but stale changed-file index/readiness cannot count as current. No private oracle/gold leaks into public bundle/preparation.

### Task 1: Frozen generic bundle and runtime launch

**Files:** create tools/effort_protocol/public_fixture.py and test_public_fixture.py; immutable generated launch-native,launch-command,spec.json are artifacts. No task-specific branch.
**Interfaces:** create_fixture_bundle(out:Path,spec:dict)->dict returns exact version/root/files/fingerprint descriptor. read_fixture_bundle(descriptor:dict)->dict verifies identity/regular files/parents and returns spec. fixture_docker_options(descriptor,mode)->list[str] returns exact readonly mount,host mapping and entrypoint. validate_fixture_runtime(row,descriptor,mode)->dict validates actual runtime. launch(mode)->never returns after successful os.exec; serve(spec_path)->runs loopback only.

- [x] Run current protocol/lifecycle baseline and preserve11owned WIP files and binary diff before edits; read affected helpers and callers. Expected: baseline matches prior qualified suites or any red/skip is explicitly retained.
- [x] Write failing tests for changed body/spec/module,unknown files/keys,writable/symlinked bundle roots/parents and runtime mounts/hosts; assert refusal before container/command/output effects. Observe real failures before implementation.
- [x] Implement minimal standalone public bundle/service/launchers using stdlib only. Exact readonly source and launcher hashes; root route and explicit public routes; bounded static responses and readiness. No network fallback or task literals.
- [x] Verify actual subprocess/Docker launch serves the declared bytes,executes exact argv and preserves a deliberate nonzero exit; unavailable/bad fixture leaves an execution sentinel absent. No stdout pollution before native MCP. Preserve every negative control.

### Task 2: Native,command and receipt integration

**Files:** external.py/native_executor.py; execution_profiles.py/executor.py/receipts.py/adapter.py and focused tests. Read references before adding optional arguments. Existing transport request_guard/MCPstdio remain unchanged.
**Interfaces:** new profiles require a controller-owned public_fixture descriptor; validate_runtime_profile/verify_receipt receive the expected descriptor explicitly. Native prepared.json carries the same declared public_fixture and hashed bundle files; _runtime_binding includes bundle fingerprint and still verifies the SAME readonly codescout binary. Adapter forwards only trusted request.public_fixture to execution and receipt verification.

- [x] Add regression tests for no bundle,actor-injected descriptor,wrong runtime mount/host/entrypoint and old-profile injection; observe RED. Keep old profile digests pinned.
- [x] Implement new opt-in profiles and readonly launcher wiring without changing limits,cwd,exact command argv,source immutability,process/stream/timeout/cleanup semantics. Verify startup separately from command outcome and preserve unknown cost.
- [x] Extend strict native preparation validation only for the registered fixture profile; require exact host mapping,mount,launcher/binary/source/spec bindings. Verify all L/R/P/X paths consume it; no per-arm exception or repo-name branch.
- [x] Run actual command fixture control on frozen small image and actual scripted CLI/native workflow with substituted replies and no credentials/provider access. Verify source/context/effort delivery,fixture bytes,nonzero outcome and fresh owned-container absence. New runtime failure is invalid/incomplete with no extra actor call.

### Task 3: Reused public preparation and whole-change qualification

**Files:** versioned public preparation artifacts and source/knowledge/index/runtime manifests; linked reports under this tracker. Product source only as needed by Tasks1/2.
**Interfaces:** public source overlay applies only declared image working-tree delta and public fixture-input patch; cached knowledge lineage retained with before/after source/index checks. No private test_patch is copied.

- [x] Create new public preparation versions for the fixed pair without touching old source/knowledge. Apply only public overlays,retain exact raw source-document memory hashes,and generate controlled Git seed lineage. No onboarding call; record reuse and any index update separately.
- [x] Qualify exact changed-file/source/index readiness and four native capabilities on both projects,then same packet/rules/plugin/CLI/profile/fixture manifests across all4arms. Any stale index or missing tool blocks readiness.
- [x] Run focused protocol/lifecycle suites and required product gate,record printed exclusions/skips. Preserve WIP/source/diffs and all native controls with checked custody.
- [x] One fresh strongest reviewer checks source/runtime/receipt/privacy bindings,actually applies at least8guard mutations and reports observed kills/survivors/invalids. Read surviving diffs/logs BEFORE one bounded correction pass and exact replay; record every declined scope/minor.
- [x] Bind a new checkpoint with paid_readyfalse. Parent F6,resource floor,actual eight-attempt controller/accounting and whole-admission preflight remain required; this project does not claim low/xhigh efficacy.

## Completion checkpoint — 2026-11-10

[Final report](../reports/2026-11-10-shared-public-runtime.md) records the completed bounded implementation and actual two-project/eight-invocation CLI controls. Final protocol272/three explicit skips, lifecycle100 and post-handoff full gate pass. One source review applied10mutations,9killed/1survived; the survivor is now caught by actual native dispatch regression and exact isolated replay. Source-bound consuming handoff has3observed RED/GREEN tests and actual Lean four-arm success. No second source review cycle.

[Checked custody](../evidence/shared-public-runtime-20261110/final-closure-v2.json) plus independent source reconstruction preserve final18owned WIP files and current native/CLI/review evidence. Ruling: seven EARLY original background-stream paths are unavailable and explicitly declared, with their job records/versioned preparation-failure artifacts retained. Do not claim all historical raw streams survived. This is a retained custody limit, not permission to erase evidence or rerun a model result.

Task3 complete for this local implementation slice; parent efficacy goal, F6, host resource floor, actual eight-attempt controller/accounting and whole-admission preflight remain open. paid_readyfalse. No source commit/merge/push, global config change or destructive old-resource cleanup.

## Self-review

Actual scout: native _runtime_binding requires readonly /bench/bin/codescout entrypoint today; external.validate_prepared allowlist rejects add-host. Command executor directly creates the actual argv as PID1 and validates final exit/resource/source/cleanup,so the command launcher must os.exec original argv. Lifecycle public verification uses the SAME live native worker. Existing transport already has request_guard; a new transport hook is unnecessary. The new launcher preserves stdio and final exit semantics while providing ordered readiness. Public preparation is programmatic source-document onboarding,not an LLM author call; reuse its raw memories and qualify changed-file index separately. No task-specific repair prompt or private acceptance information is introduced.
