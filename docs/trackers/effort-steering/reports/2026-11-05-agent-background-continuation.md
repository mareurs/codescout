---
id: 65895e03575df6ae
kind: research
status: done
title: Background continuation — offline lifecycle qualification
tags:
- effort-steering
- evaluation
---

# Background continuation — offline lifecycle qualification

## Checked result

The new generic stream controller keeps a CLI session and its native MCP job owner alive across a yielded turn. Three actual-client offline controls passed and were independently read back. No provider requests were forwarded and no new model cost was incurred. This qualifies lifecycle transport; it does not establish low/xhigh repair quality or completion of every required public check.

## Mechanism and boundaries

Implementation is isolated in /home/marius/.codex/worktrees/615e/codescout, tools/agent_lifecycle/. session.py uses stream-json input/output with stdin open, captures structured run_command launch receipts via observer.py, reads terminal records from the exact pinned native job owner, and delivers bounded continuation messages into the same process/session. Local stdin/stdout operations use nonblocking I/O; the controller terminates its isolated process group on cleanup. Missing/malformed/reused records fail closed. Failed jobs settle truthfully without becoming successes. CLI costs are cumulative, so only the latest reported total is retained; incomplete totals are labelled nonfinal. The caller still owns provider-budget enforcement and worker cleanup.

The existing paid launcher and closed cohorts are unchanged. Its misleading workflow_completed flag remains historical evidence. The new controller has no paid entry point. Integrating it into the paired repair launcher, qualifying that composition and requiring observed public-check coverage remain separate work before another repair comparison.

## Actual CLI/native controls

Pinned CLI 2.1.309, native Docker Codescout, frozen project onboarding plus global/project CLAUDE.md, scripted HTTP responses. Credential access and provider forwarding raise immediately. New scratch root: /home/marius/.cache/codescout-resident/codescout/agent-lifecycle-20261105-v5.

| Control | Successful CLI terminals | Continuations | Native job | Result |
|---|---:|---:|---|---|
| No continuation allowed | 1 | 0 | Completion marker absent after worker cleanup | Incomplete, never full success |
| Successful job | 2 | 1 | running then exited 0 | Original handle read; subsequent tool executed |
| Failing job | 2 | 1 | running then exited 7 | Original handle read; all_jobs_succeeded=false |

Independent verification checked matching session IDs on raw terminal events, original task history present in continuation provider requests, source digests, zero terminal cost, the same native owner across record reads, and absence of all six exact owned containers. The scripted nonce marker proves subsequent tool execution; it is not evidence of unscripted model recall. Job settlement does not prove all mandated public checks ran.

## Failed qualifications retained

v1 rejected two native connections because the qualifier confused probe inventory with job ownership (ec430b3a1b23cdcb). v2 reached native success but legacy cleanup raised on a Docker --rm race (known 2db5e6fbda0834c2). v3/v4 conservatively stopped during asynchronous container removal; raw receipts remain unchanged. The new qualifier pins the record owner separately, identity-checks cleanup targets, polls exact absence for up to 30 seconds and persists cleanup observations. v5 passed all controls. None of these attempts called a paid provider.

## Review and tests

Initial six behavior tests were observed RED without the controller, then GREEN. Independent review reproduced three Important defects: JSON-text decoding swallowed record errors (a926056cc47be553), inherited stdout defeated deadlines (7e757ef79f426f29), and blocked stdin defeated deadlines (2a758a6f17667628). Each regression was observed RED before its fix, then GREEN. Current targeted lifecycle suite: 18 tests passed. Existing pilot/completion/resume regression suites: 28, 9 and 3 passed respectively.

The reviewer applied 13 actual mutations to the first controller: 4 killed, 8 substantive survivors and 1 equivalent survivor. After direct guard/behavior tests and fixes, exact reapplication against the current controller killed all 12 non-equivalent mutations; the same equivalent mutation survived. Every recheck ran all 18 lifecycle tests. Full repository gate @bg_00000035 exited0: Python light, formatter, clippy, lean and default all0. The gate excludes the three heavy Python suites, server-stack and the two local ONNX weight tests; the eighteen lifecycle tests were run separately because the default Python inventory covers tests/test_*.py.

## Evidence

Durable data: docs/research/data/2026-11-05-agent-lifecycle-results.json, -verification.json, -mutations-initial.json and -mutations-recheck.json. Independent readback script: /tmp/lifecycle-verify-01a15ca9.py. Reviewer raw logs: /tmp/lifecycle-review-20261105/ and /tmp/lifecycle-review-20261105-recheck/. New plan: 6b6f3546c3761800. Original lifecycle issue: bf167b83c0166a24.


Source commit f89af31e371268944b4e5e920f27a591bfe7bf75 on research/fixed-acceptance-population-20261101; stablepatch-id b6635eb237a411a4bc62a3184794683a694a15a2. Seven files committed by exact pathspec with canonical Session-Id; research worktree clean, no push/integration. Gate log archived at docs/research/data/2026-11-05-agent-lifecycle-gate.log.gz. Precommit ownership and committed-byte formatting checks passed; dead-artifact-id lookup was skipped because that hook could not find a runnable binary. MAIN report/data/issues remain local under the existing doc-provenance limitation; foreign staged bug-triage blob f4ad12b9cf7b8b830ac35cc341afcde702d0a56c remained untouched.

## Budget and next experiment

New model cost $0. Original pilot remains 2.57626490000000013/4 USD, remaining 1.42373509999999987; thread remains 31.797419199999999964/40 USD, remaining 8.202580800000000036. Old failed/incomplete attempts remain charged. Next integrate and qualify lifecycle plus explicit required-check observation before buying another low/xhigh repair pair; do not compare a finished turn with fully validated work.
