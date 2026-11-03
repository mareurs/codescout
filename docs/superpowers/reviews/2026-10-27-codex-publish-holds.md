---
id: a553557c61188c83
kind: review
status: done
title: Codex publish holds independent review and closure
---

# Codex publish holds independent review and closure

Independent native gpt-6-astra/high review covered the uncommitted feature snapshot based on 731bfb5e3ff72b75a4a6716af62b5207e26c8e06. The reviewer wrote only to disposable /tmp snapshots. No additional reviewer/subagents were dispatched. Original report retained during this session at /tmp/codex-hold-review-20261027/REVIEW.md, exact mutation records at /tmp/codex-hold-review-20261027/logs/results.json.

## Findings

Critical: none found. Minor: none reported.

1. Important actual defect: corrupt Codex hold refs allowed acknowledged push and made release report no hold. Reviewer observed both failures; implementer independently reproduced two RED pytest cases before fixing Codex lookup statuses with show-ref --exists. Failed release preserves the unreadable ref. Existing legacy Claude policy remains unchanged. Issue: docs/issues/2026-10-27-codex-hold-lookup-mistakes-corrupt-ref-for-absence.md.
2. Important regression gap: stripping codex: during registry lookup borrowed a same-UUID Claude row with the original suite green. A registry-bearing test now pins Claude gone versus Codex unknown, plus qualified Claude alias resolution. Implementer reapplied the exact mutation in a fresh snapshot and observed one failing test; production passes.
3. Important regression gap: routing claude-code:<sid> to Codex could delete the wrong same-UUID hold with the original suite green. The closure creates both holds, explicitly releases the qualified Claude alias, checks both ref outcomes and then implicitly releases Codex. Reapplying the exact mutation in scratch observed one failure; production passes.

## Applied mutations

Baseline and each independent candidate ran python3 -m pytest -q tests/test_codex_publish_hold.py tests/test_pre_commit_publish_hold.py from the review snapshot. Baseline: 49 passed. Files were restored between candidates and checked against an original snapshot afterwards.

| Candidate | Change | Observed outcome |
|---|---|---|
| M01 | Collapse Codex hold into legacy ref | 10 failed, 39 passed |
| M02 | Borrow matching Claude liveness | 49 passed; survived |
| M03 | Discard preserved set-at | 1 failed, 48 passed |
| M04 | Discard preserved reason | 1 failed, 48 passed |
| M05 | Push conflict becomes manual bypass | 2 failed, 47 passed |
| M06 | Use child instead of root | 2 failed, 47 passed |
| M07 | Stamp bare Codex UUID | 8 failed, 41 passed |
| M08 | Conflate same-UUID push ownership | 1 failed, 48 passed |
| M09 | Wildcard acknowledgement defeats hold | 7 failed, 42 passed |
| M10 | Qualified Claude alias targets Codex | 49 passed; survived |

Exactly 10 applied candidates: 8 killed, 2 survived before closure. Both survivors were observed before closure tests were written. Each survivor was subsequently reapplied against its new test and killed with one observed pytest failure. The original eight were not rerun after closure; this record does not claim a final ten-candidate rerun.

## Rulings on review boundaries

- Ruling: leave real remote publication and installed/profile hooks untouched, as explicitly required. Cost: this branch does not establish deployed enforcement or cross-host delivery.
- Ruling: preserve manual/no-identity behavior, unstamped history limits and dependence on explicit recorded withholding. Cost: those intentional bypasses remain possible; the mechanism cannot infer operator intent.
- Ruling: use the existing hold/push shell discrimination suites as evidence for optional liveness/age helper omissions. Cost: the ten-candidate pytest mutation instrument did not separately mutate every optional-helper combination.
- Ruling: the implementer owns the full Rust gate and final closure lanes. The reviewer's verdict on its original snapshot was not ready; the post-fix delivery verdict depends on those actual results, recorded in the implementation plan.

No deferred minors. The one storage ruling changes the initially proposed codex ref directory to _codex: underscore is outside the legacy ID grammar, preventing collision with a legacy ID literally named codex. Both format and explicit-release migration rules are documented in the spec and shared-checkout convention.

Ledger checked: original subject d9d291b44775e50d and active-state ledger; separate coauthor issue5a25de31ae4a8af3 and corrupt-Codex issuef0e6a45fb0eeabf1 were recorded during this work. Phase 0 completed.
