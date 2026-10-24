# Checked completion on a real codescout issue

The first real-project trial fixed the friction probe's missing recoverable-error population. A fixed-low implementation conversation passed its frozen checks and independent grading for $0.0407358 reported client cost. The useful result is a real correction plus evidence of the workflow's overhead; this is not a demonstrated total-cost saving.

## Result and independent calibration

Issue: 70f4cad102afb762, scripts/friction-probe.py, repository base 32d0c713de272738ae7ccba9e1e4fccb004600fe.

The candidate counts exactly error and recoverable_error throughout Python and SQL reports, updates help text, and preserves full-row sequences and call denominators. It changes only the requested script. There is no automatic effort escalation, new command-line flag or warning threshold.

A frozen read-only backup of the retained usage database contains 80,924 rows: 19 error, 3,053 recoverable_error and 77,852 success. Independent SQL with outcome IN ('error','recoverable_error') counts 3,072 errors. The old probe reports 19; the candidate reports 3,072 on those same rows. The snapshot spans 2026-09-24 06:19:21.173 through 2026-10-24 06:19:20.201. These are retained rows in this backup, not all-time totals.

## Acceptance and review

Before implementation, 13 hand-authored SQLite/subprocess tests covered totals, tool coverage, immediate repeats, recovery, consecutive runs, null detail, calibration, date/session selection and SQL split/clean filtering. Unchanged source failed 11. A separate mechanically repaired control passed all 13. Five partial-fix controls failed: totals-only, Python-only, SQL-only, broad nonsuccess selection and error-only input prefiltering.

The agent received the full frozen repository, the behavior contract and visible trusted tests. Native tools were used in an isolated nonroot container; this does not evaluate codescout MCP navigation. Tests and controller were outside editable source. The author had read the issue/source and constructed the positive control; no blindness or pristine-task claim is made.

One fixed-low Sonnet 5.5 conversation ran on pinned CLI 2.1.309. It passed all 13 checks and the independent offline rerun. Its first Stop check passed, with zero repair requests. This proves operation on this case, not a causal benefit from enforcing the Stop check.

A fresh independent review applied 16 mutations across the eight changed classification sites. The frozen 13 tests assertion-killed 15 and missed one: treating an unknown next call as an error still passed because a different signature masked that predicate. The reviewer preserved the survivor and demonstrated a real false repeat before recommending a test.

One additional matching-signature unknown-next-call test then failed against that saved mutant and passed against unchanged candidate source. The strengthened 14-test suite assertion-kills all 16 saved mutations. Original 13-test outcomes remain frozen and are not retrospectively relabeled. There were no syntax/runtime-error kills. The repository mutation script labels Python runs INCONCLUSIVE because its parser understands Cargo only; the observed unittest summaries are the evidence for these counts.

## Cost and effort

| Meter | Observation |
|---|---:|
| Prior experiment attempts / reported client cost | 31 / $2.9529524 |
| This trial's paid implementation attempts | 1 |
| Implementation reported client cost | $0.0407358 |
| All known experiment attempts / reported client cost | 32 / $2.9936882 |
| Approved aggregate experiment allowance | $10 |
| Paid repair restarts | 0 |
| Stop repair continuations | 0 |
| Preparation before driver launch | 631.52 seconds |
| Attempt, container/extraction and independent grading | 22.615 seconds |
| CLI-reported duration / API duration | 18.034 / 11.494 seconds |
| Independent review elapsed | 269.20 seconds |

Preparation includes issue selection, acceptance design, calibration, image/driver setup and three failed image builds. Those failures spent no model-run money but are part of setup overhead. The real project's uncommitted changes were excluded. Review and repository verification overlap; wall durations are not additive active labor or a latency comparison between models.

The implementer's transcript reports 53,137 input tokens (including 47,398 cache reads and 5,729 cache creations) and 1,306 output tokens. CLI prices are dollar equivalents at its reported list basis, not reconciled invoices. Dollar totals are shown to seven decimal places; the raw source values and their decimal sum are archived.

Orchestrator usage was sampled separately from this session's cumulative counters: through 2026-10-24T06:24:41.451Z, the phase used 5,105,108 input tokens including 5,018,496 cached input, and 31,418 output tokens. Reasoning is included in output, and cached input is included in input. This sample excludes later handoff work. The recorded orchestrator model is gpt-6.1-sol; the reviewer used gpt-6-astra, with token counts unavailable. Their dollar prices, infrastructure cost and human effort are unknown, not zero. Therefore $0.0407358 is not the end-to-end price of this fix, and a complete dollar-cost comparison cannot be made from the exposed meters.

## Verification and delivery

The 14 focused tests and the post-review full Python light lane pass. The required full repository gate exits 0: Python light, formatting, clippy, lean and default lanes pass. Heavy Python suites, server-stack and the local ONNX-weight checks are outside that gate's coverage. The gate's Python lane ran before the added review fixture, so the light lane was rerun after that test-only change. Production source remained identical to the extracted agent patch throughout review and verification.

Evidence is retained under .codescout/resident-private/checked-real-task-20261024: frozen registration, prompt/check/controller hashes, calibration controls, paid journal/spend/transcript/patch, hook verdict, independent grading, original mutation review, strengthened replay, retained-corpus calibration and verification logs. The private archive retains a row-preserving projection of the ten database columns consumed by the probe. Both complete old/candidate reports match the original backup after removing the DB pathname. Raw tool payload fields were excluded after a credential scan matched one; the temporary raw backup was discarded. No merge, push or global installation is part of this trial.

## Decision

The workflow can deliver this real fix. Its cheap implementation call is a small part of the surrounding work, and the independently reviewed acceptance suite itself needed repair. The trial supplies no ordinary-low comparison and no measured quality gain from mandatory checks.

Keep the controller available for tasks with existing trusted checks or checks whose authoring cost can be reused. Reuse the project checks and the required mutation review; avoid rebuilding experimental infrastructure for each issue. The observed mutation survivor demonstrates why check adequacy remains part of the work. A broader adoption decision still needs repeated real tasks, a matched ordinary-low control and priced orchestration/review. This single-task phase ends with a reviewed fix and retained cost evidence.
