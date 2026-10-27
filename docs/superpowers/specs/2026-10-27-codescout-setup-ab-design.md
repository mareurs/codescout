---
kind: spec
status: draft
title: Setup A/B eval — does the codescout setup beat bare Claude Code on real tasks?
tags:
- eval
- measurement
- codescout-value
topic: codescout setup versus bare claude code, pass rate and cost, three arms
---

# Setup A/B eval — design

**Status:** draft, not registered. No model call has run. Opened 2026-10-27.
**Branch:** `research/setup-ab-eval-20261027` (worktree `.worktrees/setup-ab-eval`).
**Budget:** $200 total, operator-approved 2026-10-27. It includes failed, unscored and
infrastructure runs.

## The question

On identical tasks, does the codescout setup change (a) the hidden-test pass rate and
(b) the total client cost of a run, compared with bare Claude Code? And how much of a
difference belongs to codescout itself rather than to the rest of the setup?

The study must be able to lose. "No demonstrated advantage" is a valid, publishable
outcome, and the decision rule below is fixed before the first paid run.

## Why the earlier evals cannot answer it

- **Hidden-information eval** (`2026-08-23-hidden-information-eval-design.md`): the
  prompt named the target, so recall sat at 1.0000 in 11 of 12 round-7 runs. It does
  not discriminate.
- **Blast-radius eval** (`prompt-engineering/scenarios/blast-radius/RESULTS.md`): with a
  plain prompt, `l2x` was 2.00 for native tools and 2.25 for codescout at n=4–6. With a
  hinted prompt it was 6.67 against 7.67. Neither arm had the companion hooks, the
  plugins or the user CLAUDE.md. It measured the tool surface, not the setup.
- **Effort trial** (Codex, `tools/reliable-agent-eval/`, 2026-10-23..27): it is the
  harness this study reuses. It found a pass-rate ceiling: low and xhigh each passed
  5 of 5 tasks. The S5 corpus showed the same ceiling: arm C at `low` passed 16 of 16.

So pass rate alone will probably tie. Cost is the co-primary outcome for that reason.

## 1. Arms

Same model, CLI version, effort, repository bytes, task prompt, turn limit, resource
limits and network policy in every arm. Only the configuration differs.

| Arm | Configuration | Answers |
|---|---|---|
| `bare` | Empty `CLAUDE_CONFIG_DIR`, `--strict-mcp-config` with no servers, native tools only. This is the Codex harness unchanged (`pilot.py:233–237`). | the baseline |
| `cs` | `bare` plus the codescout MCP server and the `codescout-companion` hooks, configured as in daily use. | codescout's contribution |
| `full` | `cs` plus superpowers, buddy, the user-level CLAUDE.md and settings: a frozen copy of `~/.claude` with memories, `projects/` and transcripts removed. | the whole setup |

**Retrieval stack (operator ruling 2026-10-27): faithful hybrid.** The `cs` and `full`
arms run a study-owned copy of the daily retrieval stack on the study network: Qdrant,
the dense embedder (`codescout-dense-amd`'s image and arguments, CodeRankEmbed Q4_K_M)
and the sparse embedder (TEI, `prithivida/Splade_PP_en_v1`). The copy follows the stack
**as measured at freeze**: on 2026-10-27 the reranker URL `:48083` had no listener, so
there is no reranker. The live `codescout-qdrant` is never reached, because it holds
the live repository's index, which contains the fixes. Each run starts from a fresh
copy of its task's template Qdrant volume.

`full − bare` is the bundle effect. `cs − bare` is the part a claim about codescout may
cite. `full − cs` is the rest of the setup. No claim about codescout may rest on
`full − bare` alone.

**Effort.** No `--effort` flag in any arm: the CLI default applies. The effective effort
is read from each transcript, recorded, and must be equal across the arms of a task.
A mismatch invalidates the task's comparison.

**Models.** Sonnet 5.5 and Opus 5.5, analysed separately. Blast-radius found that Opus
made 111 Bash calls and 19 navigation calls when a shell was available, against 30 and
167 for Sonnet. A pooled result could hide opposite effects.

## 2. Tasks

Two sources, reported separately and also pooled.

- **Foreign:** SWE-bench Multilingual, Rust records, pinned at revision
  `846e647b9f33c0b51b739d005d13d85493c9af09` (Codex `external-source.json`). The seven
  tasks the Codex trials used (ripgrep-2576, axum734, axum1119, ripgrep2209, tokio6551,
  ruff15443, ruff15309) are development tasks: plumbing and calibration only, never
  scored here.
- **Private:** the S5 `cs-p*` tasks (`claude-plugins/effort-steering/spikes/s5/tasks/`),
  mined from codescout fixes, graded by hidden tests grafted after the run
  (`task.json` → `hidden.graftLines`, `f2p`, `p2p`).

**Selection rule, fixed before any run.** Tasks are drawn by a rule on task metadata
only: repository, files touched by the gold patch, and gold-patch line count. Never on
any earlier outcome. Earlier outcomes by effort arm are not about codescout, so reuse
of S5 tasks is permitted, but the write-up must disclose it.

**Contamination.** Public tasks may be memorised. A memorised fix needs no navigation,
so contamination pushes the result toward a null, not toward a false win. Private tasks
must post-date each model's training cutoff. Sonnet 5.5's cutoff is June 2026.
**Opus 5.5's cutoff is not verified here and must be before registration.**

## 3. Leakage gates (all must pass before the first paid run)

The `cs` and `full` arms can see more of the repository than `bare` does, through
the index, memories, trackers and the librarian catalog. Each of those must be clean
of the answer.

1. **Issue file removed (operator ruling 2026-10-27).** For a private task, the
   `docs/issues/` file of the target bug is deleted from the snapshot in every arm.
   The task prompt is the only bug report.
2. **Answer overlap is zero.** An n-gram check runs over every byte any arm can reach:
   the repository tree, `.codescout/` (memories, catalog, index text), trackers and the
   `full` arm's config copy. The needles are the hidden-test names and the gold-patch
   added lines. The count must be 0. The check gets its own positive control: a planted
   needle must be found.
3. **No project or future state.** S5's `build-task.mjs` (lines 13–14 and 63) removes
   `CLAUDE.md`, `AGENTS.md`, `.claude/`, `.codescout/` and `.buddy/` at every depth,
   and puts a tool-neutral task CLAUDE.md (`task-claude.md.tmpl`: build command, test
   command, offline note) at the root. That holds in every arm. Global codescout and
   buddy memories, which post-date the tasks, are excluded from every arm too.
   **Consequence:** in this study the `cs` arm has codescout's tools, server
   instructions, hooks and a fresh index, but no project memories, trackers or catalog
   history. Their value is not measured here (§ 9).
4. **Repository CLAUDE.md parity.** Closed by gate 3: no arm sees the codescout
   repository's own CLAUDE.md, which would tell `bare` to use tools it lacks. For
   foreign tasks, the repository CLAUDE.md, if any, is identical in all arms.

## 4. Arm-integrity gate

A failed MCP connection makes `cs` identical to `bare` and pushes the result toward a
null without any visible error. The hidden-information eval had the reverse defect
(F-7): a tool restriction that existed only as a prompt request.

Every run is checked, independently of the outcome metrics:

- `cs` and `full`: the init event lists the codescout tools, at least one codescout tool
  call occurs, and at least one companion hook fired.
- `bare`: no MCP tool is listed and none is called.
- `full`: the init event lists the superpowers and buddy skills.

A run that fails the check is `invalid-arm`. It is reported and charged, never scored,
and never silently re-run.

## 5. Metrics (pre-registered)

Unit of analysis: the task. Repeats are averaged inside a task and never counted as new
tasks.

- **Co-primary 1 — pass rate.** A run passes when every hidden `f2p` test and every
  `p2p` test passes in the independent grader. Non-inferiority: per model, the `cs` arm
  passes at least as many tasks as `bare` minus one.
- **Co-primary 2 — cost.** Total client-reported cost per task, failed runs included.
  The statistic is the paired ratio `cs / bare` per task. Its geometric mean is
  reported with a 90% paired-bootstrap interval over tasks.
- **Secondary:** input and output tokens, turns, wall time, verification breadth (did
  the run compile all targets), edits outside the gold patch's files, and the share of
  tool calls that went to codescout. The last one is reported, never used to exclude a
  run.
- **Variance floor:** `bare` runs twice on every task. The floor is the spread between
  those two runs, on each metric.
- **Index cost.** The codescout index is built in the image before the run. Its build
  time and cost are reported on their own line, and are not in the per-run cost.

## 6. Decision rule

Per model, using the `cs − bare` comparison:

| Outcome | Condition |
|---|---|
| **promising** | pass-rate non-inferiority holds AND the upper bound of the cost ratio's 90% interval is below 1.0 |
| **no demonstrated advantage** | non-inferiority holds AND the interval contains 1.0 |
| **worse** | non-inferiority fails, OR the lower bound of the cost interval is above 1.0 |
| **inconclusive** | fewer than the registered number of valid tasks, OR the cost difference is inside the variance floor |
| **invalid instrument** | any leakage gate, control or arm-integrity check fails at the study level |

`full − bare` is reported under the same rule as "the setup", never as "codescout".
With about 12 tasks per model the intervals will be wide; **inconclusive is a likely
outcome and must be stated as such, not re-run until it changes.**

## 7. Stages and budget

Sequential launch, a reserve held for the next run, no automatic paid retries, and a
stop on missing cost, credential errors, grader contamination or an effort mismatch
(the rules of the Codex spec). Plan-limit refusals are detected and are neither passes
nor failures (memory `research/effort-steering-plan-limit`).

1. **Stage 0 — instrument, no model calls.** Build the `cs` and `full` images (the
   codescout binary, rust-analyzer, an offline embedder, the pre-built index, plugin
   directories, and python3 and node for the hooks). Run the leakage gates and their
   positive controls. Check the grader on each task: the base fails, the gold patch
   passes.
2. **Stage 1 — calibration, cap $25.** Two development tasks × 3 arms × 2 models, one
   run each. It measures per-run cost per arm and model, and runs the arm-integrity
   gate live. Its outcomes are never scored.
3. **Stage 2 — freeze.** From Stage 1's costs, choose the task count so that the
   expected spend of Stage 3 plus a 25% reserve fits the remaining budget. Freeze the
   task list, arm order per task (randomised, to spread prompt-cache effects), hashes of
   every artifact, and this decision rule.
4. **Stage 3 — main run.** Per task and model: `bare` × 2, `cs` × 1, `full` × 1.
   Then grade, apply the decision rule, and write the result.

## 8. Reuse

- Runner, container isolation, proxy, cost accounting and independent grading:
  `tools/reliable-agent-eval/` (Codex worktree `.codex/worktrees/3940`, branch
  `research/reliable-agent-evaluation-20261023`). Do not modify it there. Copy it into
  this branch with its source commit recorded.
- Private task definitions and grader: S5 in claude-plugins, `effort-steering/spikes/s5/`.
- Plan-limit detector: `claude-plugins/effort-steering/spikes/plan-limit.mjs`.

## 9. Open questions (resolve before registration)

1. **Project memory is out of scope.** Gate 3 removes `.codescout/` for leakage safety.
   A later study could rebuild memories and trackers as of each task's parent commit,
   scrubbed against the answer, to measure their value. This study does not claim it.
2. **Opus 5.5 training cutoff** against the private tasks' fix dates.
3. **Which profile `full` copies.** This machine has three Claude Code profiles; the
   `full` arm needs one named source, with its plugin install paths re-rooted inside the
   container.
4. **Per-run cost of Opus** is unknown until Stage 1. If Stage 1 shows the $200 cap
   cannot cover about 12 tasks per model, the choice is fewer tasks or one model, and
   it is the operator's.
