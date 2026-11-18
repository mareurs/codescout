---
id: '11c3c02b145cd17e'
kind: plan
status: active
title: Public build inputs implementation
tags:
- effort-steering
topic: effort-steering
---

# Public Build Inputs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Inline execution preserves the operator's autonomous-work choice; one independent strongest-model whole-change applied-mutation review before completion.

**Goal:** Run unmodified offline package-manager builds with immutable original source and isolated writable dependencies shared fairly across arms.
**Architecture:** Standard-library-only input contract stages fresh generated directories and binds exact mounts. cargo-v3, native readiness and independent receipts consume the same descriptor, while old profiles and strict source digests stay unchanged.
**Tech Stack:** Python unittest, Docker inspect, existing native CLI/CodeRank preparation; no new dependency.
**Spec:** [Design](../public-build-inputs-design-20261112.md), catalog cbc08fd592466f2e. Fresh builder check: nothing important left.

## Global Constraints

- Original source is read-only during public observations; native source remains editable.
- No task-specific branches, changed default frontend behavior, provider forwarding or cross-arm writable cache.
- Corepack/toolchain remain immutable; only declared empty project directories and declared isolated cache copies are writable.
- Strict source input_digest remains unchanged; generated links are never followed or deleted to satisfy a source predicate.
- Canonical source gains empty dependency targets before its single onboarding; already unchanged projects reuse onboarding.
- No paid launch before all16 zero-forward checks, private controls and final admission. No merge/push in this plan.

## Review Focus

1. A symlink to /testbed in generated store is hashed as target text; external referent mutations cannot affect it and source input_digest must still reject it.
2. Descriptor paths can alias ancestors, shadow protected mounts or inject a different image; reject before Docker/provider execution.
3. Two attempts can accidentally share cache writes or native capture can include generated nested dependencies; real fresh-copy/capture controls must distinguish them.
4. A native worker may pass preparation but omit the mounts during public checks; readback must reject actual mismatches independently.
5. Reference warming can leak generated frontend assets/cache into final image; original-seed parity and final-image baseline/reference controls must reject it.

## Task 1: Generated evidence inventory

**Files:** create tools/effort_protocol/public_build_inputs.py and test_public_build_inputs.py.
**Interfaces:** generated_inventory(root: Path) -> dict[str,dict]; generated_digest(root: Path) -> str. Standard library only; no receipts/lifecycle imports.
- [x] Write tests for link-free digest agreement with strict receipts.input_digest, generated escaping directory/file/dangling/cyclic links counted without traversal, referent changes ignored, link target/mode/file/empty-directory changes reflected, FIFO/root-link rejected and Unicode names handled.
- [x] Run unittest and observe missing implementation failure.
- [x] Implement lexical sorted inventory with lstat, no-follow regular reads and pre/post metadata consistency; symlinks are leaves. File/directory entry shape matches source input_digest on link-free overlap; links bind target and mode. Canonical JSON uses sorted keys and compact separators.
- [x] Rerun tests; independently compute a hand-written expected digest for a tiny fixture, and reopen an actual generated pnpm directory without traversing its project-reference links.

Observed proof: build-inputs-inventory-red/green, contract-red/green, profiles-red/green; real public-build-input-staging-v2 component and cargo-v3-executor-green receipt. Latest full effort suite570tests OK with68explicit native/Docker skips. Whole native/admission integration and final independent applied review remain pending.

## Task 2: Validated descriptor and fresh staging

**Same files. Interfaces:** validate_descriptor(raw, source: Path, image: str) -> dict including fingerprint; stage_inputs(raw, source, image, output: Path) -> dict including fingerprint, exact mounts and seed digests; verify_staged(stage, source, image) -> dict. Descriptor schema public-build-inputs-v1; exact keys schema,image,seed,dependency_dirs,cache_mounts. seed is null for empty descriptor, otherwise path/sha256. Each cache mount has seed_relative/destination; destination below /bench/public-dependencies. Sources/seed/output physical, pairwise disjoint. Project dependency dirs are existing empty physical directories.
- [x] RED tests for tampered image/seed, unknown fields, bool/type errors, missing/nonempty targets, traversal/absolute/duplicate/overlap paths, directory-symlink ancestors, protected mount shadow and nonphysical/overlapping staging roots.
- [x] Implement validation and fresh staging preserving target strings/modes, manifest bindings and no source/seed mutation. Empty descriptor remains explicit and image-bound.
- [x] GREEN tests: two independent stages start equal; modifying one changes neither other nor seed. Reopened stage rejects mutated seed/manifest/output metadata; generated output changes are recorded rather than treated as seed parity.
- [x] Extend actual component E with descriptor staging and inspect readback; preserve original cache/source control.

Observed proof: build-inputs-inventory-red/green, contract-red/green, profiles-red/green; real public-build-input-staging-v2 component and cargo-v3-executor-green receipt. Latest full effort suite570tests OK with68explicit native/Docker skips. Whole native/admission integration and final independent applied review remain pending.

**Local implementation/component checkpoint:** protocol Ran587/OK(skipped=68), lifecycle Ran110/OK including fresh -I helper load. Real Skim native audit V3 reopens all9registered actual RPC responses, ordinary offline Cargo return0 with complete logs, source/template/guidance parity, two cgroups and fresh cleanup. The original timing summary has one row; it is not used to count RPCs. Source capsule127members/SHA bfa047a191ca169b76e5ca8248380c5890f341824711f56f424a007c7fe3caf6. Final all16author-CLI/context/private-grade admission, full gate and independent applied-mutation review remain Task4, not satisfied by these components.

## Task 3: Observation and native integration

**Files:** tools/effort_protocol/execution_profiles.py, executor.py, receipts.py, adapter.py, readiness.py, native_executor.py, native_campaign.py, campaign_native.py, qualify_native.py; tools/agent_lifecycle/external.py, workflow.py; existing matching test modules.
**Interfaces:** request/public/prepared/readiness field public_build_inputs carries the validated descriptor; receipt and runtime-binding carry its fingerprint and invocation-specific staging evidence. Observation profile cargo-v3 requires descriptor, exact legacy profiles forbid it. Lifecycle freezes the standalone helper by explicit path/SHA and lazy import without registry collision.
- [x] RED meaningful integration tests: wrong native/observation fingerprint; declared mount missing or writable undeclared mount present; changed seed; actor production edit captured but nested dependency mount contents excluded.
- [x] Add cargo-v3 exact descriptor mounts with sourceRO and generated digest in executor/independent receipts. Every actual writable output subtree belongs to generated inventory, including caches and node_modules.
- [x] Add native fresh stage and same mounts to the live worker; preserve state within invocation continuations only. Bind helper/descriptor in readiness, prepared config, raw inspect and cutoff capture; close owned runtime before output custody.
- [x] Add isolated lifecycle entry tests loading only bound helpers under -I; check actual public command uses mounts. No reliance on host sys.path or optional fallback.
- [x] Run full effort and lifecycle suites, plus real component before moving to admission.

**Local implementation/component checkpoint:** protocol Ran587/OK(skipped=68), lifecycle Ran110/OK including fresh -I helper load. Real Skim native audit V3 reopens all9registered actual RPC responses, ordinary offline Cargo return0 with complete logs, source/template/guidance parity, two cgroups and fresh cleanup. The original timing summary has one row; it is not used to count RPCs. Source capsule127members/SHA bfa047a191ca169b76e5ca8248380c5890f341824711f56f424a007c7fe3caf6. Final all16author-CLI/context/private-grade admission, full gate and independent applied-mutation review remain Task4, not satisfied by these components.

## Task 4: Qualification, admission and custody
**Newest2026-11-12:** current report9e73a99bf85b7a23 and README are authoritative. Native/shared/private-input guards implemented; actual Dioxus clone preserves114/114member stores without new indexing/onboarding; protocol Ran596/OK(skipped=68), lifecycle last110/OK. Ironclaw root index completes after one onboarding;74-member readonly V7successor runs@bg_00000160(server212011/token9acd60fe). V6failure after controller comment edit retained(F-43); never edit active producers. Source capsule task5-member-transfer-v1:127members/290452bytes/SHA20b9b67f8e262441e79625f4a575edf7dba1802b553c9fb885f5fcfa231c79b9. ProducerV17:167members/213883bytes/SHAa0d6bccb046a141d024fcff561ce819ef06707c3af7fc1218652d0257c511bc1. Actual final controls/16qualification/gate/review remain open.

**Ruling2026-11-12:** Ironclaw's source-owned .claude rules/skills expose a legacy role collision(e5428f5b6298d043). Preserve the complete project .claude subtree under the new build-input/native contract in digest, clone and capture; legacy defaults remain unchanged. The policy is generic to original project instructions, not Ironclaw paths. Apply RED→GREEN across the consuming clone/runtime/capture seams, then continue the existing single onboarding without calling onboarding again. Cost if wrong: lost or unbound project instructions would invalidate matched context. Full actual CLI delivery/all16 qualification remains the closing check.

**Files:** tools/effort_protocol/{feedback_admission,paid_admission}.py and tests; experiment preparation producers/evidence in this tracker.
**Interfaces:** public descriptor is required for cargo-v3; all preparation/16 native evidence must match image/fingerprint/seed. Empty descriptor for the other projects; historic records remain legacy.
- [x] RED test: paid/shared/native admission rejects absent, mutated or cross-task descriptor, changed helper/seed/output, missing actual mounts or incomplete private test protection. New standalone and consuming tests observed RED→GREEN; protocol Ran595/OK(skipped=68), lifecycle110/OK. Actual16proofs remain below.
- [ ] Bind original public manifest/lock inventories, versions, seed producer/image and unknown setup compute costs; reopen seed before/after every control.
- [ ] Complete Ironclaw canonical source targets then single onboarding/root+member index and LSP/context. Qualify ordinary/broader baseline/reference with default frontend behavior.
- [ ] Restore canonical public seed after private image reference warming; clear generated source/build outputs. Complete all-task original-label baseline/reference/reference-repeat controls and inline assertion audit.
- [ ] Run all16 exact zero-forward native/context/capture/resource/private-grade checks, full gate and original closed-input parity.
- [ ] Independent strongest available reviewer actually applies load-bearing mutations in isolated copies and reports observed survivors. Fix meaningful survivors after observing them, then rerun required suites.
- [ ] Archive selected producers and source WIP with actual inventories/hashes; update README/parent/child/current reports by exact headings. Commit explicit owned paths with Session-Id only after final verification; no push.

## Self-review

Tasks cover isolated copies, independent readback, native public-check parity, original source and seed protection, captured edits, protected original-label grading and paid admission. Review-focus1/2 map to tasks1/2; focus3/4 to task3; focus5 to task4. Internal JSON staging shape is builder-owned; descriptor and consumer field names above are fixed to keep the integration consistent. One-off instrument corrections stay versioned with original failures preserved. Current work is not yet a complete execution profile.
