---
id: cbc08fd592466f2e
kind: spec
status: draft
title: Isolated public build inputs for native feedback
tags:
- effort-steering
- native-feedback
topic: effort-steering
---

# Isolated public build inputs for native feedback

## Intent and authority

Support ordinary offline builds whose package managers require writable dependency state while retaining read-only original source in public observations. This is preparation infrastructure for the fresh four-repository L/R/P/X feedback screen, not a new repair mechanism or evidence of low/xhigh efficacy. The operator delegated implementation choices and continued execution; the choices below are the controller's, not a new operator-approved design.

Existing source: tools/effort_protocol/{execution_profiles,executor,receipts,adapter,native_executor,campaign_native,native_campaign,feedback_admission,paid_admission}.py and tools/agent_lifecycle/{external,workflow}.py in /home/marius/.codex/worktrees/2aa6/codescout. Existing cargo-v2 observations bind source read-only, /build writable, /mnt_tmp writable and private network disabled. Native actors intentionally edit their source. Their public checks run in that same native environment.

## Measured reason

Ironclaw's original default build script executes pinned Corepack pnpm install --frozen-lockfile and Vite. Canonical Rust and frontend manifests/locks remain unchanged. Source-read-only/cache-read-only and source-writable/cache-read-only both fail with SQLite unable to open database. Complete public metadata and tarball cache preparation solves network access but not SQLite writes.

The observed minimal control E succeeds with source read-only, one nested node_modules mount writable, and only pnpm store/cache/state writable; Corepack stays immutable. Install and Vite return zero; both actual cgroup samples, source and seed parity and owned cleanup are recorded. This is a component control, not a complete Cargo/public/native/private qualification.

Evidence root: /home/marius/.cache/codescout-resident/codescout/native-feedback-preparation-20261112/nearai__ironclaw-6724/pnpm-offline-write-probe-v5.
Prepared immutable image: sha256:28aa0e0f6f9cd069363ec49448bf7e3dd1d19e3fcdf3629a6f92d5d47c542de1.
No provider repair calls or onboarding calls were made by these controls. Embedding/CPU/setup costs are not measured.

## Approaches and chosen direction

1. Cache prefill alone: insufficient; the actual immutable cache fails at SQLite.
2. Writable entire project during public observations: component control works but weakens the source invariant.
3. Declared dependency submounts plus isolated writable cache copies: chosen. It preserves original-source protection and lets the unmodified build script operate. It adds mount validation, cache custody and native/public parity work.

Skipping the default frontend or adding task-specific source exceptions changes what is built and is excluded.

## Goals, non-goals and invariants

Use the same final image, descriptor, environment, immutable seed and fresh-copy policy for all four arms of each task. Other tasks use an explicit empty descriptor.
Do not change historical cargo-v2 receipts or closed artifacts. Add versioned cargo-v3 support.
No task identity, function name, special frontend command or pnpm-specific branch belongs in product runtime code.
No arbitrary actor-supplied mount, global cache mutation, whole-project writable observation, silently skipped frontend, reference-generated cache reuse or changed acceptance labels.
Private test protection, frozen original-label grading and generic feedback policy remain independently required.
A successful component or full public build never counts as a repaired task.

## Descriptor and trust boundary

Add a standalone, standard-library-only public_build_inputs module shared or loaded by absolute frozen path by the protocol and lifecycle worker. It must not import receipts or lifecycle runtime eagerly and create a cycle. Exact module/API names and implementation layout are left to the builder.

A controller-owned versioned JSON descriptor declares:
- immutable image identity and physical immutable host cache seed root plus SHA-256 inventory;
- an array of dependency directory paths relative to project source;
- an array of cache directories with relative seed path and absolute container destination;
- a fingerprint over its canonical schema and bound inventory.

No environment overrides in the descriptor; image/profile environment stays separately frozen and validated. Reject unknown fields/types, duplicate/overlapping paths, links in mount ancestry, absolute or parent-traversing project paths, nonempty project targets, source/seed/output overlap, and destinations shadowing source, toolchain, guidance, private fixture, /build or other mounts. Cache destinations lie under a dedicated registered public-dependency prefix, not arbitrary system paths. Exact key spellings and prefix can be settled by builder; acceptance behavior is fixed here.

Seed is produced only from original public manifests/locks in a disposable public-source workspace. Its image/producer commands, versions, manifests/locks and inventory are bound. Any excluded temporary workspace reference is explicitly recorded before freezing; runtime output is never sanitized. Verify seed unchanged before and after every use.

Empty dependency target directories enter the canonical shared public source before the project's single onboarding. Never redo already-completed onboarding for unchanged tasks. For Ironclaw, onboarding has not occurred yet.

## Preparation and execution

Stage an independent writable copy of each seed cache directory and an empty dependency directory under an invocation-owned generated root. Copy links as link-target strings without traversing host targets; seed inventory binds their text. Reject unsafe device/special files and host directory aliases. Outputs never overlap canonical public source or immutable seed. Each observation starts fresh; native continuations within one invocation keep their own state. No cross-attempt/cross-arm warm state is promoted.

Public observation mounts original source read-only plus only the declared empty dependency directories writable and isolated cache copies writable. It records descriptor/staging fingerprints, raw Docker inspect before/after, actual resources, logs, source invariants, seed invariants and cleanup. Existing profile exact-mount validation extends only for cargo-v3's exact descriptor; legacy profiles remain exact as before.

Native actors get the same image/cache/dependency bindings and fresh-copy policy while their original source is writable for coding. Nested dependencies must remain outside the host actor tree through submounts, so candidate capture retains production edits and does not recursively capture generated node_modules. Native readiness, lifecycle preflight and launch/receipt validators independently verify the actual descriptor/mounts; adapter admission compares native and observation fingerprints. Missing/unknown/different descriptors fail closed before provider forwarding.

Public checks performed within native lifecycle use the same mounts; no hidden second dependency setup or network path. The implementation must distinguish one-off public observation isolation from allowed state retention within one native invocation.

## Generated artifact digest

Existing receipts.input_digest rejects links that resolve outside the host root. Keep that strict source predicate unchanged. pnpm legitimately emits a project-reference symlink naming /testbed inside its generated store; on the host the link escapes the build root.

Add a separate versioned generated-artifact inventory/digest: hash relative entry path, type, mode, regular-file bytes and symlink target text without following links. Record all generated dependency/cache outputs, not only /build. Reject devices and racing/missing files. Preserve links and output bytes as emitted. The producer and independent receipt reader reopen the same complete generated inventory under the same declared policy. Calibrate digest agreement with strict input_digest for link-free overlap; test that external referent contents are never read and strict source escaping-link rejection remains intact. Do not redefine a source digest to accept generated data.

## Whole pipeline binding

Propagate the descriptor through native campaign public allowlist, campaign_native, readiness binding, adapter admission, observation requests/receipts, native runtime checks and paid admission.
Shared-preparation evidence binds original public manifests, cache producer/image/inventory and explicitly unknown setup compute cost.
All sixteen exact zero-forward CLI invocations must show matching descriptor/context/resources/capture/cleanup and complete intended public checks. The author cannot select a different descriptor.
Final private image warming must restore canonical public seed and clear reference-generated node_modules/bundles; independently replay original-label baseline/reference/reference-repeat controls afterward.

## Validation and applied review

Inline test-first implementation. Test meaningful failure boundaries before fixing them: descriptor tampering, unsafe path or mount shadow, different native/public fingerprint, seed mutation, shared state across attempts, generated link traversal and accidental source weakening. Test real candidate capture with a nested dependency mount and a retained production edit. Observe independent verifier rejection, not only request construction.

Run one actual read-only-source offline Cargo ordinary/broader baseline/reference control for Ironclaw, then all four final-image public control pairs and protected private controls. Reuse existing onboarding/index only where source/image contracts allow it.
Run the effort/lifecycle suites, required full gate and independent strongest available whole-change review. Reviewer must actually apply load-bearing mutations in isolated copies, run tests and report observed survivors; gaps are fixed after observed survival.
No paid repair launch until shared preparation, original-label protection, all16 zero-forward qualification and admission all pass. Existing proposed112USD reservation remains unadmitted; known73.375358899999998524USD commitment includes historical4USDhold.

## Builder decisions and remaining qualification

Builder settles concrete JSON field names, internal module/API layout, physical staging folder names, copying strategy preserving symlinks/modes, and whether to stream or buffer inventory hashing. These choices must preserve every invariant above.
Not settled by this design: repair efficacy, semantic retrieval quality, invoice totals and generic transfer beyond four fresh repositories. They require the registered experiment and later held-out replication.
