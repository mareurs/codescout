# Diagnostic pilot, frozen before model calls

## Decision

Does fixed medium show a useful correctness/cost tradeoff against fixed xhigh
on a fresh Rust behavioral issue when the repository, prompt, checks and budget
are identical? This is an effort-headroom diagnostic. It neither tunes a router
nor establishes noninferiority. User approved up to $10 on 2026-10-23.

## Task and arms

Source is pinned in external-source.json. Development task: ripgrep-2576, a
word-boundary matching error. The prompt is the issue text prefixed by the same
generic instruction to implement and verify using local development checks.
No solution hints, reference code, hidden tests or grading failures enter agent contexts.

Four runs maximum, order medium-0, xhigh-0, xhigh-1, medium-1. Same model
claude-sonnet-5-5, pinned Claude Code 2.1.309 (SHA256
4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658).
Amendment before any model call: historical CLI 2.1.296 is absent locally;
use 2.1.309 in both arms and compare only these new runs. Conditions:
4 CPUs, 8 GiB, fresh source and conversation, no automatic repair continuation,
no subagents. Existing development tests are available equally in both arms.
No new completion gate is added. This first comparison tests whether there is
effort headroom before testing a controller or additional verification policy.

## Instrument controls before spending

- Unchanged source plus hidden tests must fail the named issue test.
- Reference fix plus the same tests must pass issue and regression tests.
- Agent image must contain base source only, fresh Git history, no reference
  patch or hidden test files, and no accessible original Git object database.
- Use an internal network and a proxy allowing only api.anthropic.com; verify
  direct external requests and proxied GitHub/Hugging Face requests fail.
- Agent runs as an unprivileged user; no Docker socket or host source mounts.
  Only isolated run output and the pinned CLI are mounted. Secrets are passed
  through a private environment file and never printed or committed.
- Grade only after the agent exits, in an independent offline container using
  base source plus the agent's source diff and the original hidden test patch.
  Restore the entire original tests directory before injecting hidden tests.
  Manifest/build-script edits remain allowed for legitimate fixes and are retained
  for review; executable grading is not a sandbox against adversarial benchmark cheating.
  Preserve all test logs and require the named tests to have actually run.

## Budget and abort rules

Each CLI run gets --max-budget-usd 1.25, a ten-minute wall timeout and an output
limit of 8192 tokens per request. Admit the next run only if cumulative recorded
spend plus $3.00 is at most $10. No paid infrastructure retries. The $3 reserve
is conservative admission headroom, not a provider-enforced per-run guarantee;
CLI budget checks occur after requests. Record any in-flight overshoot exactly.
Stop on unknown spend, timeout, auth failure, unexpected effective effort,
failed isolation control, or an invalid/no-test grader result.

## Outcomes

Record pass/fail/invalid, issue and regression outcomes, full client-reported
cost including failures, wall time, turns and observed transcript effort.
Two attempts per level on one development task are descriptive only. Both
levels passing licenses testing a cheaper baseline on more tasks; reproducible
xhigh-only successes motivate an escalation experiment with a same-effort retry
control. Both failing motivates examining context/checks before a router.
No result here licenses a general quality or deployment claim.

## Adaptation ruling

The original S5 runner restricts task repositories to its own authored corpus
and sweeps shared containers carrying its labels. Use a small isolated adapter
for the external image, with unique names and no shared-container cleanup.
Reuse the S5 transcript usage reader; preserve this deviation in the report.
