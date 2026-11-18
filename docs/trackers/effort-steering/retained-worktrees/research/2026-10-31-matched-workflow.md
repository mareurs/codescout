---
id: '8905299382e2c6cd'
kind: research-note
status: archived
title: 'Historical: 2026-10-31-matched-workflow'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-10-31-matched-workflow.md
source_body_sha256: 21c2a53b27f919ee0534c62bd4a49623bc4c01fd1824341e1397029a3866ffae
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-10-31-matched-workflow.md
  sha256: '21c2a53b27f919ee0534c62bd4a49623bc4c01fd1824341e1397029a3866ffae'
  body_sha256: '21c2a53b27f919ee0534c62bd4a49623bc4c01fd1824341e1397029a3866ffae'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-10-31-matched-workflow.md
  sha256: '21c2a53b27f919ee0534c62bd4a49623bc4c01fd1824341e1397029a3866ffae'
  body_sha256: '21c2a53b27f919ee0534c62bd4a49623bc4c01fd1824341e1397029a3866ffae'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-10-31-matched-workflow.md

---

# Matched low/xhigh checked workflow — stopped pilot

## Result and decision

The prespecified comparison is incomplete and does not support a low-versus-xhigh conclusion. Four model calls ran. Tokio passed its declared target at both efforts. Ruff xhigh initially failed one snapshot case; its sole repair exhausted its per-call allowance before a valid completion. The protocol then stopped payment, so Ruff low was never run. No paid retry or protocol restart occurred.

The saved repair source passes the official nine-case target in a separate unpaid diagnostic. This does not convert the invalid model completion into an accepted workflow endpoint. The phase spent $1.5159117 in client estimates, including the invalid call. Cumulative recorded client estimates are $19.7047145 of the authorized $40, leaving $20.2952855 (rounded); native orchestration, review, CPU and actual billed dollars remain unknown.

## Prospective question and protocol

Can a complete low workflow match accepted quality at lower aggregate client cost when both efforts receive the same independent checker and one optional repair? The two tasks, effort assignments, order, allowances and criterion were fixed before new model outcomes. Both initial prompts used the original issue; a repair used its own exact reviewed failed draft plus the complete actual checker output, at the same assigned effort. Passing initial drafts skipped only their own repair, with a bound zero-charge receipt. The endpoint was the repair whenever it ran; no best-patch selection.

Fixed units: A Tokio low, B Tokio xhigh, C Ruff xhigh, D Ruff low. Order A-L,A-R,B-L,B-R,C-L,C-R,D-L,D-R. Low allowances .60/.40; xhigh1.10/.70. Phase reservation5.60 plus .40 reserve within phase6/global40. Per-call soft limits can overshoot; actual phase/global spend and valid completion remain separate requirements. The new criterion required both low endpoints accepted, valid evidence, and aggregate complete-low cost below aggregate complete-xhigh cost. It did not require both xhigh endpoints to pass. This prospective change does not revise the preceding pilot's unchanged false GO.

[Design](../superpowers/specs/2026-10-31-matched-workflow-design.md) and [execution plan](../superpowers/plans/2026-10-31-matched-workflow.md). Registration SHA256: f59bf46ea77467bfda1ad2ad5a7a9de4a470efda5f4b9a27b1e097b2ba6d6cba.

## Complete observed rows

| Slot | Assigned effort | Independent target | Completion | Client USD |
|---|---|---|---|---:|
| A-L Tokio initial | low | 118/118 | Valid, accepted | .1023558 |
| A-R | low | Own initial already passed | Bound skip | 0 |
| B-L Tokio initial | xhigh | 118/118 | Valid, accepted | .3521277 |
| B-R | xhigh | Own initial already passed | Bound skip | 0 |
| C-L Ruff initial | xhigh | 8/9 | Valid, failed target | .3604388 |
| C-R Ruff repair | xhigh | No eligible final grade | Invalid: budget exhausted | .7009894 |
| D-L Ruff initial | low | Unmeasured | Not started after stop | — |
| D-R Ruff repair | low | Unmeasured | Not started after stop | — |

Tokio's118 comprise the independently listed116 full-feature library cases plus two official net/Unix trait cases. Author-added tests outside that population are not added to this denominator. The complete Ruff target contains nine named flake8_pie cases, not the entire repository.

The C-R raw result is error_max_budget_usd, terminal_reason budget_exhausted, CLI exit1, with recorded requested/observed effort xhigh and no model subagents. Its $0.7009894 charge slightly exceeds the .70 soft allowance. The phase and global caps were not breached. The generic validity reason mentioning an agent/effort check must not be read as an observed effort mismatch.

## Saved-source diagnosis and tool friction

The C-L failure was an official PIE800 snapshot mismatch in multiline comment/blank-line preservation. The repair changed token deletion so that comments survive. After the invalid completion, the unchanged saved final source was reconstructed and evaluated offline:9/9. Exact complete prompt and actual checker delivery were observed in its primary session. Validity/outcome records remain unchanged and the diagnostic is explicitly ineligible for support.

All three source captures taken by the repair's author Cargo checks have the same patch hash,01f73698a1d378eb1b1c179d72e165cf0482d6bcf1f90367da463e0c94026842. An exact intermediate capture also passes9/9 under the independent official target in a separate unpaid diagnostic. Thus a passing source was already present during the author-check sequence; this does not establish how much spending a different stopping policy would save.

The raw CLI recorded11 permission-denial entries. Primary tool results show generated .snap.new paths inside /testbed, followed by missing-file responses in the author workspace and blocked attempts to read/copy from /testbed. A fresh unpaid real-shim probe independently reproduced the boundary: a dependency-free Rust test creates a snapshot file inside the container, but it is absent in the author workspace afterward. Caller INSTA_UPDATE=always is also absent in the actual Cargo process. The shim returns logs and exit status only and disposes of its container.

This confirms an operational tool-contract gap. It does not isolate the fraction of repair cost caused by the gap, prove that all11 denials share that cause, or show that a larger allowance would have produced a valid final completion. The concrete next improvement is constrained transfer of author-generated snapshot artifacts, with independent oracle outputs kept separate, before purchasing another comparison. [Open artifact-visibility issue](../issues/2026-10-31-cargo-shim-hides-generated-snapshots.md).

## Evaluator environment and controls

Tokio's official image lacked both a build target and offline registry dependencies; its copied lock refused a locked public fetch. Every failed unpaid preparation is retained. Before payment, the design was amended to support an empty external build target and to resolve/freeze a public dependency seed in the same immutable image with unchanged Rust/Cargo1.85.1. Both efforts used the identical resolved lock/cache and offline checking. This is an adapted environment, not exact official replay.

Original lock SHA6f7401a1c6c2690bc5b48d54b1be4a87a443252febe89ccc74ac4e9e65f38dba; resolved742db2f6b55e0b675f89dbdd699d0f5ebeec0516156724ec0b2f6815a2e8fb97. The full package (name,version,source) tuple comparison removed22/added0; it does not prove identical dependency edges or lock text. The517672960-byte registry archive SHA is5b953c7f7456ad6140e935428f9037abfbe71eb8b8692660815eda5d811cfca9. Full original/resolved locks and delta are retained.

Unpaid source-bound controls: Tokio baseline library116pass; official trait baseline compile E0277, not two executed failed tests; gold composite118pass. Ruff baseline8pass1fail/gold9pass. The oracle restores official test paths before applying the fixed test patch, so author-written snapshots do not determine acceptance. Actual source inventories, patch/log hashes, commands, exact named populations, cache receipts and oracle flags are checked.

Original selected dataset rows and dataset hash were verified. The scoped local exposure audit scanned every direct resident-private phase/arms/started.json receipt in its declared layout,36 receipts at registration, with zero exact hits for either new task and no unresolved rows after inherited provenance checks. This says nothing about other studies, other historical layouts or pretraining. Only original task metadata/official images were used; other-study solutions/results were not consulted.

## Verification and preserved limits

One fresh strongest-model prepayment review applied exactly six isolated mutations:5killed,1survived,0invalid. The surviving mutation changed real repair reconstruction to another unit's draft. One fix pass added a real bootstrap/Git-apply regression and corrected terminal cap validity. The original accounting failure and survivor were observed before regression closure. Exact replay:6killed,0survived,0invalid in22 non-Docker consuming tests. The reviewer supplemental Docker attempt timed out and is not claimed complete.

The complete private prepayment suite passed28tests, including real containers and the selected Cargo CLI path. Subsequent closure tests cover invalid-call charge retention, unstarted low outcomes remaining unknown, raw-receipt calibration and omitted-charge refusal; the full closing suite passed32tests in46.284seconds, with its separate log retained. The required project gate passed earlier in this active phase, before the documentation checkpoint; production repository source has not changed since. Its printed heavy-Python/server-stack/LEAN-librarian/local-ONNX coverage limits remain. This is not a claim of a fresh repeated project gate.

No model-effort causal effect, general quality equivalence, controlled latency or whole-repository correctness follows from this incomplete two-task pilot. Tokio's two valid endpoints need no external repair, so that comparison does not demonstrate an intervention effect. Ruff low remains unmeasured.

## Evidence and accounting

Private evidence root: /home/marius/work/claude/codescout/.codescout/resident-private/matched-workflow-20261031. Registration, controls, selected inputs, complete raw charges/transcripts, snapshots, exact source reviews, diagnostics, applied mutation diffs/logs, repair-friction.json, artifact-probe/result.json, analysis.json and the final physical-file/committed-blob seal are retained there. The independent closure enumerates every direct started slot and reconciles its raw charge, including C-R, rather than summing accepted rows. Remaining/unstarted tasks are not silently assigned zero cost or failure.

Prior sealed accounting anchor: workflow-transfer-20261030/final-outcome.json SHA903775c124c39ae00dbcd81571a2ddf755894c111d9d51b3029d8dee7e3ea3cf, prior18.188802799999999785. The new seal records precise Decimal sums and publication provenance; displayed cumulative/remaining values above are rounded to seven decimals.
