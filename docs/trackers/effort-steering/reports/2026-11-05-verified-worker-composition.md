---
id: 0a7166e66450c2f2
kind: research
status: done
title: Verified worker composition — native offline controls
tags:
- effort-steering
- evaluation
---

# Verified worker composition — native offline controls

## Result and scope

The repair worker now has an opt-in verified workflow mode. It retains one live CLI/MCP session and one provider-budget object, waits for observed background jobs, then executes an explicit manifest of public verification commands against the same native candidate. A failed round returns public feedback into the same session; the entire manifest runs again after repair. Unknown results stop the workflow without a success claim. No new paid model calls occurred.

This is infrastructure qualification with scripted responses and short public probe commands. It is not evidence that low matches xhigh or that the Rust repair task passed under the new mode. The actual Rust profile is tools/hint_authority_workflow/public-checks.json, matching the frozen task CLAUDE.md's four commands: fmt, long clippy, lean tests, default tests. Its 1800-second total/900-second per-check limits apply equally to both future arms. Model/effort flags, output8192, max32CLIturns and CLI$0.65 target remain unchanged; continuation messages share one campaign envelope.

## Mechanism

Entry: hint_authority_pilot.run.one(..., workflow=config). Legacy defaults and historical receipts retain their old semantics. The new adapter validates the command manifest before credential access, augments the frozen hint hook with an observation hook, invokes the stream controller and uses a completion-check callback after jobs are settled. Public checks execute sequentially in the same identity-checked native container. Ordinary nonzero exits do not short-circuit the remaining checks. Every round has a receipt and per-command logs; feedback is limited to public command output tails. No private grader output enters this loop.

The receipt separates cli_turn_success, lifecycle, public_verification, cleanup_error, budget_stop and unknown_provider_cost. workflow_completed requires settled lifecycle, a complete passing final public round, known cost within the campaign envelope, no transport/budget error and exact worker cleanup. Latest CLI cost is cumulative and is never summed across terminals. A valid terminal records the number of forwarded generations it covers; if another generation finishes without another terminal, the previous amount remains only a known lower bound and unknown_provider_cost becomes true. Worker name and container ID are both pinned across checks.

## Actual CLI/native controls

Final corrected-source root: /home/marius/.cache/codescout-resident/codescout/verified-worker-20261105-v2. All three cells use the real worker setup, CLI2.1.309, Codescout native Docker server, reused frozen onboarding/global/projectCLAUDE, reconnaissance and delivered hint followed by an edit. Provider forwarding and real credential access raise in the scripted driver. The earlier v1 qualification is preserved as evidence of the reviewed pre-fix source.

| Cell | CLI turns | Continuations | Observed public rounds | Workflow result |
|---|---:|---:|---|---|
| All pass | 2 | 1 background wake | [0,0,0,0] | completed |
| Public failure, scripted repair | 3 | 1 background wake +1 public feedback | [0,1,0,0], then [0,0,0,0] | completed |
| Unknown check timeout | 2 | 1 background wake | format0; compile timeout; incomplete round | not completed |

Independent readback verified source hashes, exact session IDs on raw terminal events, retained failure feedback, the same native job owner/container ID for background reads and all public commands, every round receipt, zero provider cost and absence of all six exact owned containers. These checks prove transport and observation, not unscripted model recall or repair ability.

## Tests and independent review

New public-verifier/callback/worker tests were observed RED before implementation, then GREEN. Current lifecycle/composition suite:34 tests passed. Legacy pilot28, completion9 and resume3 passed separately. A real local proxy with dummy credentials verifies that a continuation can exhaust the remaining campaign envelope before a second generation is forwarded, preserving known cost and budget-stop evidence.

Independent review reproduced and filed two Important defects: stale terminal cost after an additional provider response (0b471a97357e4b0f), and acceptance of a replacement container with the same name/image/label (f0e56c3c59f66b42). Each regression was observed RED, then GREEN after the fix. Applied14 mutations initially yielded5 killed and9 survivors. After direct guard tests, exact reapplication ran all34 tests per mutation:13 killed and1 invariant-equivalent survivor. The survivor removes a public-status predicate redundant with genuine verifier/lifecycle settlement; no mock-only test was added to force it to fail. Coverage issue30fca68660938262 records the probes.

A test-isolation issue aa79e4013575950a allowed the missing-validation mutation to create launch-v1/unused before TOKEN Forbidden stopped it. No real credential, CLI or provider access occurred. The test now scopes roots to TemporaryDirectory. The positively attributed accidental directory was preserved at /tmp/workflow-review-20261105/escaped-unused-attempt; old receipts/source were not rewritten.

Full repository gate @bg_00000040 exited0 with Python light/FMT/clippy/lean/default all0. The normal heavy-Python/server-stack/ONNX exclusions apply; the34 tool tests ran separately because the standard Python inventory covers tests/test_*.py. Archived gate log: docs/research/data/2026-11-05-verified-worker-gate.log.gz.

## Evidence and next experiment

MAIN durable data: docs/research/data/2026-11-05-verified-worker-v2-results.json and -v2-verification.json; initial v1 -results.json/-verification.json remain separate. Mutation data: -mutations-initial.json and -mutations-recheck.json. Raw reviewer logs: /tmp/workflow-review-20261105 and /tmp/workflow-review-recheck-20261105. Independent verifier: /tmp/verified-worker-readback-01a15ca9.py. Plan22ed6d6c4e0c2288, parent issuebf167b83c0166a24.

Next: freeze the new composition, public manifest and runtime in a new paired-calibration driver; qualify both intended effort cells and the acceptance controls, and bind technical review to that full fingerprint before spending. Legacy qualification fingerprints omit the new modules/configuration and must not be reused as proof for this composition. Do not rerun the closed completion pair. Full public verification is now a measurable predicate distinct from patch acceptance; a fresh matched pair can measure the total workflow rather than an early successful turn.


## Source preservation

Committed 0998c3b64f81c13331db26e0091ff25404272c66 on research/fixed-acceptance-population-20261101, worktree /home/marius/.codex/worktrees/615e/codescout; stablepatch-id 971ae969a0c1417a98d387e1c314a1fec4dbf3b2. Ten exact paths, correct canonical Session-Id, clean research worktree. No push or merge. Precommit ownership and committed-byte formatting checks passed; dead-artifact-id lookup was skipped because the hook could not find a runnable binary. MAIN report/data/issues/memories remain local: the read-only provenance preflight returned UNKNOWN coverage, recorded in issue421ed73df25f795d. The foreign staged bug-triage blob f4ad12b9cf7b8b830ac35cc341afcde702d0a56c stayed untouched.

## Budget

New model cost $0. Pilot remains2.57626490000000013/4USD, remaining1.42373509999999987; thread31.797419199999999964/40USD, remaining8.202580800000000036. All historical failed/incomplete attempt costs remain charged. No cap reset, push or merge is part of this stage.
