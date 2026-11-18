---
id: cd8787619611d87a
kind: research-note
status: archived
title: 'Historical: Matched Cargo task admission and completion feedback pilot'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-03-matched-cargo-task-admission.md
source_body_sha256: 0c2c7fd38c621ef5efe62cf6b795a0ec82d03bc81cffff12caa01235b951f934
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-03-matched-cargo-task-admission.md
  sha256: f8daeb7bf7c03b1b269a26c013395c64c6242e109c0286eb8b1ae03bd7fd9d10
  body_sha256: '0c2c7fd38c621ef5efe62cf6b795a0ec82d03bc81cffff12caa01235b951f934'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-03-matched-cargo-task-admission.md
  sha256: f8daeb7bf7c03b1b269a26c013395c64c6242e109c0286eb8b1ae03bd7fd9d10
  body_sha256: '0c2c7fd38c621ef5efe62cf6b795a0ec82d03bc81cffff12caa01235b951f934'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-03-matched-cargo-task-admission.md

---


# Matched Cargo task admission and completion feedback pilot

## Current state — 2026-11-03

The frozen paid pilot **stopped for a protocol violation**, after four charged calls on rust-analyzer22804. Low initial and its sole repair both failed public, original and hidden checks. Xhigh initial failed too; xhigh repair attempted one blocked Glob in the external Cargo registry, triggering the predeclared zero-permission-denial stop. Bindgen3390 was never started. General promotion remains **HOLD**; this is not a completed two-task comparison.

A separate model-free evaluation of the already-produced invalid xhigh repair passes both public cases, the original package population (1007pass/same3upstream ignored) and all4hidden cases. That diagnostic is useful evidence about the patch, but does not rehabilitate the registered workflow. No retry, replacement or post-hoc permission change.

Plan: [task admission](../superpowers/plans/2026-11-03-matched-cargo-task-admission.md). Specification: [matched pilot v2](../superpowers/specs/2026-11-03-matched-cargo-pilot-v2.md). Evidence B: `/home/marius/.cache/codescout-resident/codescout/matched-cargo-task-admission-20261103`. Execution and independent audit have finished. The result commit and calibrated evidence closure are recorded separately in B/publication.json and B/seal.json; canonical closure command is python3 -B B/close.py verify. Keep its sources, four source/prepared images and V worktree. Earlier Cargo admission A remains sealed and unchanged.

## What changed the selection

F-10 establishes that all five cached candidates had already had S5 model results displayed in this root thread. The old proposal stopped before payment, with zero admitted slots. A narrow literal search had not established exposure. The complete frozen historical prefix and correlated prior tool response are retained; matching JSONL records include duplicated event records and are not a model-run count.

The new population contains exactly six strict-control-supported, unscreened dataset tasks, fixed before new model outcomes. A deterministic hash order and at most one task per repository select rust-analyzer 22804 and bindgen 3390. Their inspected prior history contains infrastructure-control mentions, without model outcomes; their production/test-patch identities differ from the old screen pool. This bounded evidence does not establish globally unseen tasks. Small fixes may produce an acceptance ceiling; no difficulty-based resampling or replacement after paid freeze.

## Offline behavior and original controls

Public and hidden contract cases were fixed from issue text before gold-code inspection. The bindgen base exposed three unsupported C-cast macro variants: no constants were generated. Before any gold control or body inspection, an explicit amendment switched those variants to canonical documented static-const typedef syntax and added genuine alias-newtype compatibility. The original unsupported cases and results remain. The exact public hexadecimal repro was unchanged.

| Task | Public contract control | Hidden compatibility control | Original package population |
|---|---|---|---|
| rust-analyzer 22804 | Base reproduces the closure-in-array-length panic; gold passes both public cases. | Base fails the renamed malformed case; gold passes all four hidden cases, including actual valid closure type assertions. | Same 1,010 named cases: base 1,006 pass / 1 fail / 3 upstream ignored; gold 1,007 pass / 0 fail / same 3 ignored. |
| bindgen 3390 | Base generated bindings fail with the reported E0423 `u32(0)` error; gold passes both public cases. | Four hidden cases preserve primitive values/types and genuine enum/alias newtypes; base and gold pass. | Same 622 named cases: base 621 pass / 1 fail; gold 622 pass. |

Each package's only base-to-gold status change is its original added regression. These are the specified package populations, not whole SWE-bench-Live acceptance. The three ignored rust-analyzer tests establish no coverage. B/task-control-audit.json selects twelve source-bound task/variant/check receipts; B/original-control-audit.json reconciles exact original names and ignored sets.

F-11 / bug fd5a3151756171e4 records a real measurement defect in the provisional original-control parser: inline-only matching omitted bindgen's diagnostic-producing failure. The replacement tracks split serial statuses, normalizes ANSI and reconciles every named status with the unique complete libtest summary and process exit. Old partial receipts remain; separate verified-parser receipts supersede their interpretation. Rust-analyzer's three omitted names were ignored cases, not additional successes.

## Mechanism being tested

The pilot compares a completion-gated workflow at low and xhigh effort. Both receive the same issue, visible public contracts and file tools. After completion, the controller runs the same public checker. A supported public failure permits at most one repair at the same effort; only public evidence enters that prompt. Original and hidden checks judge endpoint quality independently. The fixed order is A-low, A-xhigh, B-xhigh, B-low.

No callable Cargo or MCP tool is available during a model turn. This removes a separate integration question while directly testing the completion-feedback mechanism. It does not test automatic discovery of unknown invariants or in-turn tool use. Human-authored oracle and native orchestration costs remain unmeasured.

Both actual loopback CLI controls observed the requested five file tools plus the CLI's exact inert DeferredToolPlaceholder schema, with the registered effort on outgoing requests and assistant records. A forced placeholder call returned NoSuchTool and was rejected by the protocol audit. These are dummy-credential, synthetic-usage controls and add no model-client charge. They do not establish efficacy.

## Source and controller boundaries

The fresh rust-analyzer image exceeded the fixed 6 GB compressed-source limit. Its exact canonical Git base was fetched in a bounded shallow checkout and combined with the existing immutable toolchain/cache image. Bindgen's allowed source image was pulled and verified by manifest identity; Docker's containerd image Id here names the manifest, separately from the configuration digest. Both source images had gold changes reset before clean source export.

Grading reconstructs a pure `/candidate` tree from the pinned archive, applies the exact captured patch and verifies the full manifest before fixture injection. The cache stays outside that source tree. Canonical validation test frameworks are restored so candidate edits cannot weaken their assertions. Candidate and post-fixture source manifests must remain stable. Bindgen's original dump-preprocessed-input test creates exactly two declared new scratch outputs; those are archived, removed and followed by the strict source check. The earlier rejected invocation remains invalid.

Each container has exact image binding, no network or host/persistent mount, a fixed bounded `/mnt_tmp` tmpfs, finite subprocess/operation/cleanup deadlines and exact owned cleanup. This is a scoped proof about recorded resources, not a global empty-Docker claim.

## Pilot decision and budget

The four-workflow design targeted at most8USD model-client charges, with0.80per-call admission plus0.40request-overrun reserve. The budget did not stop this pilot: the registered protocol did. Whole cost includes every initial/repair/invalid known charge.

| RA22804 arm | Initial client USD / result | Repair client USD / result | Full charged client USD |
|---|---|---|---:|
| low | 0.0379301 / rejected by all three checks | 0.0934725 / rejected; adds another original regression | 0.1314026 |
| xhigh | 0.5259125 / rejected by all three checks | 0.48464910000000005 / protocol-invalid; separate offline patch passes all three | 1.01056160000000005 |

Three valid completed calls and one invalid call are literal started-session/receipt populations, not four accepted workflows. Four bindgen slots (initial/conditional repair for each effort) remain unstarted. No valid xhigh endpoint and no completed matched pair; no acceptance-rate or cost-superiority estimate. Low's failed terminal endpoint already fails the preregistered both-low-accepted requirement. GO is not earned.

Phase client total **1.14196420000000005**. Prior cumulative25.340899699999999810; new cumulative **26.482863899999999860 of40**, remaining **13.517136100000000140**. These are exact raw client list-basis reported charges, including the invalid call; invoices/native/review/CPU/global provider counts remain unmeasured. The invalid last call has no persisted child-exit receipt: its raw result reports success, while the enclosing campaign exited1.

## Verification and remaining work

One fresh stronger reviewer applied 15 mutations: 12 survived, including a redundant early-parent-check mutation, and 3 were assertion-killed. Two real defects were reproduced: the final settled call could exceed its cap while audit promoted the pilot, and two executable imports were missing from freeze. The author reproduced all 15 outcomes before adding consumer tests, then performed one fix pass. Concrete defect regressions observed RED before fixes; 37 unique methods passed and exact15 replay produced 15 assertion-only kills with no survivors, errors or timeouts. Source bytes were restored exactly. F-12 and the four dedicated bug records retain raw evidence. No rereview.

A new actual admission failure then exposed permission-list ordering across sorted JSON serialization. Its first rejected frozen configuration remains byte-exact in admission-revisions/1-rejected. The minimal deterministic-order fix observed RED then GREEN; revision2 retains exactly the same registration, tasks, sources and session identities, with the same denial sets. Final 38-method suite and another exact15 assertion-only replay pass; fresh readonly preflight returns exit0. No payment or outcome-based slot replacement occurred. This was a new failed admission control, rather than a second review round.

The required project gate passed all five lanes, exit0, in432seconds, with its heavy Python/server-stack/two ONNX exclusions. Repository runtime source remains unchanged. Private experiment checks are separate. Scoped census:28distinct container IDs in the explicitly enumerated phase receipt roots are absent; no global Docker absence claim.

The preceding paragraphs describe the historical admission checkpoint. The paid execution is now stopped and superseded by the result below; do not run its frozen schedule again. Preserve the peer staged memory and all old seals. No push or merge.


## Paid evidence and next decision

`paid_audit.py record/verify` audits all four actual started slots, complete raw cost/session/hash/model evidence, outgoing/assistant effort records, exact prompts and repair-parent source. It reconciles twelve grade receipts (nine registered and three posthalt-only diagnostics), raw command outputs/log hashes, full named original populations and ignored sets. The bounded wire tail is checked separately from the complete logs (F-14); no test count comes from a truncated view. The denied Glob's exact tool_use_id correlates to a permission-denied result, with duplicate persisted records distinguished from requests. Source-bound grading and exact owned cleanup remain valid. All40distinct container IDs under the declared controller/paid/diagnostic receipt roots are now absent; this is not a global Docker claim. Four source/prepared images retained.

**What this teaches:** detection and feedback delivery worked on this case; one low repair did not correct the defect and introduced a second original failure. Xhigh used public feedback to reach a patch that passes the independent evaluator, but its workflow was protocol-invalid. Neither more tests alone nor higher effort alone is established as sufficient, and no general causal or parity claim follows from one case.

The low patches suppressed a downstream panic and then skipped repeat-count traversal. The reference instead prevents an inappropriate expected-type write in create_body_anon_const; the final xhigh patch resolves the conflict during merge and retains its earlier traversal change. Passing the independent checks does not require matching reference text. This source comparison motivates a **new, untested** mechanism candidate: before editing, explicitly trace the producer of the bad value/type, state the violated invariant and show why the patch restores it. Freeze that generic prompt and a blocked-read classification policy before a fresh prospective comparison with equal check/repair access. Do not tune on this exposed case and call it transfer, continue stopped slots, or build another generic runtime proof without a concrete failure.

Admission publication b2516d9384a3af5685cd7f797e490c66496fb53d, patch-id e000c429c1f584bf88b7cd3dfde0cbfb2847683e; recovery9f62d83259f1c9f3bbe611d80d1fda7b6483c5cc. F-13 retains the stop and diagnostic distinction. No new review pass or runtime source modification after freeze. Required project gate already passed all5lanes (432s) with recorded heavy-Python/server-stack/two-ONNX limitations; paid/private evidence audits are separate. Local publication and calibrated closure have their exact receipts in B/publication.json and B/seal.json. The private paid auditor's Decimal roundtrip defect was observed RED and fixed to the record serializer; actual full-data verify passes (bugcb24648f65b4f641). No push/merge.


## Closure receipt

Result publication2947aab55de050aff7d7f790e8c2d1a3ab023465, stable patch-id c1fbeaaadf5ecf69de6f2a94df0b479efa0a4535. Bseal **364010aa416fa07002493d5290d697f79a59bc492433f1261285559ea6a9e273**, with21636physical regular-file paths and13literal symlinks, excluding only seal.json/repeat-verification.log. Complete nofollow populations and hashes/target bytes agree with GNU find/sha256sum/readlink. An actual temporary-fixture link-target mutation is detected and exact restoration passes. The first regular-only rejection and unsealed publication state remain preserved (F-15); no source link was retargeted.

Fresh canonical `python3 -B B/close.py verify` exits0. It checks the historical Git publication, calibrated complete sealed evidence and the40recorded container identities/fourretained images. The independent live paid audit passed before sealing; five external frozen dependencies are archived byte-exact. Future closure verification does not require temporary actors or current repository runtime bytes. B is immutable except repeat-verification.log: do not run ordinary campaign/CLI/tests/LSP/mutation writers there. This completion metadata does not change the published result snapshot or its seal. End goal remains open; no push/merge.
