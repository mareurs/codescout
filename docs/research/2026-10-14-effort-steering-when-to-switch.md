# Effort steering: when to switch effort inside an agent run (research index)

**Date:** 2026-10-14
**Question:** effort matters (R1, ES-22). The open question is WHEN inside a run effort should change, and how to test a router that decides it.
**Operator's idea:** before each LLM call, a small model (TypeSafe's Jev, or JevK5 run locally) receives the current state and what the agent is about to do, and returns the effort. Examples given: editing a tracker with all context gathered → `low`/`medium`; an architecture decision or brainstorming → `high`/`xhigh`.
**Raw reports (subagent output, not verified line by line):**
- `2026-10-14-effort-steering-when-to-switch-academic.md` (Opus subagent)
- `2026-10-14-effort-steering-when-to-switch-industry.md` (Sonnet subagent)
- `2026-10-14-effort-steering-when-to-switch-forums.md` (Sonnet subagent)

## Summary for the decision (2026-10-14)

**Question.** Effort matters; WHEN inside a run should it change, and how do we test a router that decides it?

**What is established (each item has its evidence further down).**
1. R1's verdict stands: `not_shown_safe` for the static rule (stack trace → `low`). G4 stays held.
2. The `low` loss on `cs-p20` reproduces: 0/5 against 5/5 at `xhigh` (V2 plus a rerun of 6 new runs). The loss on `cs-p12` does not reproduce (C 2/3 in the rerun; the new failure is a wrong fix, not the V2 stop-to-ask).
3. Mechanism on `cs-p20`, from command text: `low` verifies narrowly (`--lib`, name filters, `--no-default-features` from its first command, 8 of 8 runs), `xhigh` verifies broadly (plain `cargo test`, `--tests`, `clippy --all-targets`). The build error text of the rerun rows was not read.
4. Over all 48 V2 runs, "ran a test after the last edit" does not separate the arms (21/24 at `low`, 16/24 at `xhigh`). What separates them is volume: median 11 tool calls at `low`, 31 at `xhigh`.
5. Anthropic's verification paragraph in the prompt did not help (0/5, all `build=failed`). The agents met it with a narrow check and said what they had not run.
6. Thinking tokens at the tool-result step do not separate `low` from `xhigh` on 6 questions, so they cannot serve as the check that a mid-loop effort change takes effect.
7. The literature (all checked at the source) gives: prompted effort selectors lose to fixed effort (ARES), escalation rules must be tested against random at the same rate (Nadir), discordance between effort levels is unstable on rerun (PointFive), and no published A/B of per-step routing exists on tasks where fixed `low` fails.

**What is NOT shown.**
- Whether a mid-loop effort change takes effect (Anthropic's doc says a change after a `tool_result`-only turn waits for the next user turn; our ES-17 probes show only that the rewrite reaches the wire, on CLI 2.1.292).
- Anything about a router (Jev, JevK5): no router was run.
- Anything beyond one task for the paragraph test and the `cs-p20` mechanism.
- The operator's own examples (tracker edit at `low`, architecture at `xhigh`): `low` is almost absent from the history (41 of about 375,000 effort-bearing records), and documents are 69% of the committed work but only partly gradable.

**Spend in this segment** (measured where the harness prints it): rerun of the three losses $6.62, paragraph test $1.11. Stage 0 (14 runs) and the calibration (12 runs) used text output and have no recorded cost.

**Decisions that are the operator's.**
1. The R1 ruling: record `not_shown_safe` as a no-ship for the static rule.
2. The branch `effort-routing-r1` (never pushed; it holds a foreign commit `f17afad`): how to integrate (cherry-pick the R1 commits, not a merge).
3. The four `docs/research/2026-10-14-effort-steering-when-to-switch*.md` files and `.codescout/memories/research/effort-steering.md` were committed on 2026-10-16 by pathspec at the operator's request, after the Stage A results (not pushed).
4. Tracker updates ES-22 / ES-23 in `claude-plugins` (not done; that checkout is shared with other sessions).
5. Work directories outside any repo: `s5-work-v2r` (rerun), `s5-work-v2p` (paragraph test), `es-stage0` (instruments and mining output). They hold transcripts as evidence.
6. The personal S5 token was pasted into a chat session earlier: revoke or replace it when the study ends.
7. The termination gate was built and tested on `cs-p20` (section below): `low` with a `Stop` hook that type-checks all targets passed 5 of 5 for $1.26, against 0 of 10 without it in the earlier batches. One task, a compile-class failure, references from other batches. The next step, if any, is the operator's.
8. Stage A (2026-10-16, section below): a grounded critic used as a gate for plans and design docs found 18 of 20 (`low`) and 17 of 20 (`high`) planted defects as blocking and blocked no clean original without a real defect; its effort made no difference (paired −1) at half the cost for `low`. It also blocked 16 to 19 of 20 originals for real defects. Not shown: that a producer fixes what it reports, the cost of that round, or results on defects not planted by me (Stage B).
9. Stage C (2026-10-16, section below): with the critic as a `Stop` hook in a plan-writing loop, verified defects in the final plans were `low` 5, gated `low` 4, `xhigh` 0 (6 plans each); the gate blocked once in 6 runs and passed plans with real defects. No help shown; gated `low` does not match `xhigh` ($0.54 vs $1.92 per run). For plans, producer effort mattered and a cheap critic did not replace it.
10. Stage D (2026-10-16, section below): the same 6 plans written at `medium` and `high` without a gate. Verified defects in the final plans were `low` 5, `medium` 5, `high` 3, `xhigh` 0, at $0.35, $0.47, $0.81 and $1.92 per run. By the pre-registered rules neither new level matches `xhigh`. Open confound: plan length rises with effort, and the audit may look less closely at a long plan.

## Our own evidence (R1 V2, Sonnet 5.5, CLI 2.1.296)

`stage2-analysis.json` (V2 root `s5-work-v2`): A (`xhigh`) and C (`low`) differ in 3 of 24 runs. `cs-p20` C 0/2, A 2/2 (verification depth: A ran plain `cargo test` with the integration targets; the C build failure was proven by replay). `cs-p12` C 1/2 (`cs-p12.C.1` stopped to ask). `cs-p18` 0/2 in both arms. All three losses are at the end of the run (verification, the decision to stop). n=3: a hypothesis.

## Verified at the source by the main session (2026-10-14)

1. **Anthropic, "Prompting Claude Sonnet 5.5"** (platform.claude.com, undated). Verbatim: "At `low`, it keeps its thinking short and can skip verifying a change." and "At `low` and `medium`, on long agentic tasks, it's more likely to stop and check in with the user before it finishes." The verification paragraph is claimed to make skipped checks rare at `low` "with no measurable change in task quality and only a slightly higher cost per task" (vendor claim, no numbers). The "Keep working until everything the user asked for is done" paragraph claims only longer and costlier sessions at `low`/`medium`, no quality claim.
2. **Anthropic, "Effort"** (platform.claude.com). Per-message effort (beta header `mid-conversation-output-config-2026-07-01`, an effort-only `role: system` message) keeps the cache. Verbatim: "A `user` turn that holds only `tool_result` blocks doesn't count as new input, so a change placed after one in a tool-use loop waits for the next `user` turn with new input." A top-level effort change "doesn't preserve cached prefixes". This is in tension with our memory (the `turn.step` rewrite reaches the wire as top-level `output_config.effort` plus an appended system pin, no cache miss measured on 2.1.292). Whether a mid-loop change takes effect is NOT established.
3. **`ifoster01/jev-effort`** (GitHub, results 2026-09-23). Proxy, Jev, per step, can only lower effort. Opus 5.5, 6 small tasks, 48 sessions: tests 12/12 in both arms at `high` and at `max`. Author: "It isn't much better than turning effort down yourself." Its tasks pass at low effort, so it cannot detect a loss.
4. **Thariq Shihipar, "Using Claude Code: Spending your effort"** (claude.dev, 2026-09-25; role not stated on the page). Opus 5.5 and Fable 5.1, Terminal-Bench-style tasks. "effort was a great way of modulating how much verification and edgecase testing Claude did and how much of its own judgement it used." Fable 5.1, 370 attempts per setting, low → max: "a bug its tests missed" 40 → 14, "wrong or incomplete fix" 31 → 10, "picked the wrong reading" 25 → 47 (judge labels, approximate). Recommended loop: "Implement it on low effort" … "Verify and test on high effort". Low for brainstorming and sketching.
5. **Nadir, "When LLM Routing Does Not Help"** (2026-09-28). Offline replay, 500 SWE-bench Verified tasks, Haiku 4.5 → Opus 4.6 after 25 turns: 75.6% resolved with 97.2% escalated; random escalation at the same rate 75.4%, modelled cost within 1%. LLM judge AUROC 0.537/0.597 vs fixed order 0.606. "Always test an escalation rule against a random control that escalates at the same rate."
6. **ARES, arXiv 2603.07915** (2026-03-09). Qwen3-1.7B router picks low/medium/high per step for gpt-oss-20b. Labels: from a fixed prefix, K=3 samples per level, the lowest effort that reproduces the high-effort action in all three. SFT router vs fixed high (accuracy): Retail 54.8 vs 54.8, Airline 36.0 vs 38.0, BrowseComp-Plus 41.3 vs 42.7, WebArena 46.5 vs 45.0, with fewer tokens. Prompted GPT-5 and Gemini 3 Pro as selectors are below fixed high on all four. Fixed medium beat fixed high on Airline (42.0 vs 38.0).
7. **The Replay Gap, arXiv 2608.08239** (2026-08-08). Live forks of SWE-bench trajectories with same-model control forks: a model swap rewrites 61-94% of later actions; static log-stitching "mispredicts every success-relevant outcome call". Model swap, not effort.
8. **Prompt-Induced Waste, arXiv 2608.01347** (PointFive, preregistered; v6 2026-09-10). Claude Code 2.1.220, Sonnet 5 and Opus 5, 25 hard SWE-bench Verified instances, low/high/xhigh. Sonnet 5 xhigh vs low: six rescues, one regression; "three of six xhigh rescues reproduced; all three were classified as cross-module-completeness failures of the low-effort run". Opus 5: no discordant pair reproduced. A simulated escalation rule "activated on 64–68% of tasks and did not improve the observed cost–success frontier". "All six timeouts and all four empty patches occurred at high or xhigh effort."

## What follows for the design

- The moment with the most consistent support is verification and the decision to stop (our 3 losses, the Sonnet 5.5 guide, Thariq's data). "Architecture → `xhigh`" has weak support; more effort raised "picked the wrong reading" in Thariq's data.
- Discordance between effort levels is unstable on rerun (PointFive): rerun discordant tasks before building on them, including our 3.
- A prompted, untrained selector is a weak router (ARES, Nadir). Replay labels are also the training data if JevK5 is to be tuned.
- Step labels from a fixed prefix (ARES) say whether low gives the same next action; they do not predict the outcome (Replay Gap). Outcomes need live forks run to the end, with a same-effort control fork.
- Nobody found has tested per-step effort routing on tasks where fixed `low` fails.

## The four stages agreed with the operator (2026-10-14)

0. **Wire check.** Does an effort change inside a tool loop change the next request's thinking? With a positive control, and the cache-read share per request. If not, the "moment" reduces to prompt boundaries.
1. **Mining** (read-only, no model cost). Transcripts carry `effort` and `perTurnEffort` on every `assistant` record (checked in one session: 4,576 records, 9 changes in 10 days). Corpus: 1,348 transcript files, 7.6 GB, in three profiles (since when the field exists: not checked). Output: a catalog of moments (kind of work, effort used, traces in git and trackers). Correlational only: effort was chosen by the operator (confounding by indication), and a later fix had the error in view.
2. **Live forks on tasks with headroom.** From the same prefix: rest at `low`, rest at `xhigh`, and a control at the original effort; run to the end; grade by tests. Prefix replay by a proxy that serves recorded responses for the first k steps (fidelity to verify; a local wire recorder once changed thinking).
3. **Policies compared on the same labels:** fixed `low` + Anthropic's verification paragraph; `low` + a termination gate; the phase rule (low to implement, high to verify); JevK5; random at the same dose; a per-task router; the oracle. Metric: share of the A−C gap recovered vs share of the A−C cost paid, within task.

## Stage 0 result (2026-10-14): thinking tokens at a tool-result step do NOT separate effort levels

Instrument: `/home/marius/work/claude/es-stage0/` (outside any repo; `run-stage0.sh`, `run-calibrate.sh`, `analyze-stage0.py`, `analyze-calib.py`, `questions/`). Sonnet 5.5, CLI 2.1.296 (pinned binary), headless, tools limited to `Read`, effort fixed per run through the `effort-stepper` mod, personal S5 token. T = thinking tokens over requests with index >= 1 (the request that processes the question's `tool_result`).

- First attempt (q1, one pair): T(xhigh) 3319, T(low) 2094. Positive control failed (rule: T(low) <= T(xhigh)/3); the script stopped after 2 runs.
- Calibration (6 candidate questions, one pair each, 12 runs, all rc=0, every `err.txt` empty, no refusal text; answers verified by computation):

| q | T(xhigh) | T(low) | note |
|---|---|---|---|
| q1 count with constraints | 3295 | 3198 | second pair on q1; low 2094 in the first |
| q2 config lookup | 0 | 0 | floor control |
| q3 trace a program | 152 | 122 | |
| q4 liar puzzle | 718 | 452 | |
| q5 inclusion-exclusion | 119 | 72 | |
| q6 trap arithmetic | 49 | 0 | low's reply was a worked line, not the bare number asked |

No question met the rule (T(xhigh) >= 1000 and T(low) <= T(xhigh)/3). Where the model thinks a lot (q1), low thinks about as much (63% and 97% of xhigh in two pairs); where the task is easy, both think little. n=1 per cell except q1 (n=2); the between-run spread on q1 low (2094 vs 3198) is about 50%.

Reading: at the step that processes a tool result, how much the model thinks follows the problem, not the effort level, on these questions. This matches Anthropic's statement that such follow-up requests "can still skip thinking at any level". So thinking tokens at that step are not a usable positive control for "did a mid-loop effort change take effect". Not shown: whether effort changes other behaviour at that step (reply length, tool calls, verification). Cost was not measured.

## Verification behaviour in the V2 transcripts (2026-10-14, no model call)

Instrument: `/home/marius/work/claude/es-stage0/verify-behaviour2.py` (heuristic: it reads command text, not what ran). Input: the 48 final V2 runs, 24 at `low` (arm C) and 24 at `xhigh` (arm A). Two earlier versions of the script were wrong (they missed that `low` edits through Bash, and they read `| tail` and `2>&1` as test filters); the numbers below are from the third version, spot-checked on cs-p05.C.1 and cs-p17.C.1.

| | C (`low`) | A (`xhigh`) |
|---|---|---|
| runs with a `cargo test` command after the last edit | 21 of 24 | 16 of 24 |
| runs with an unrestricted `cargo test` after the last edit | 7 | 6 |
| tool calls per run, median (min–max) | 11 (3–28) | 31 (10–53) |
| `Edit` tool calls in all 24 runs | 5 | 103 |

Reading: at `low` the agents did run tests after their last edit, as often as or more often than at `xhigh`. The difference between the arms is the volume of work (about three times as many tool calls at `xhigh`), not whether a test ran at the end. So "low skips verification" (Anthropic's guide, our three losses) is not visible as a general pattern in these 48 runs. The cs-p20 loss may rest on WHICH tests ran, which this heuristic cannot see; the replay earlier in the study remains the evidence for that run. Not shown: why cs-p20.C failed in both runs, beyond the replay result.

## Rerun of the three losses (2026-10-14): cs-p20 reproduces, cs-p12 does not

Batch: `run --mode calibrate` on a new work root `/home/marius/work/claude/s5-work-v2r` (same tasks through a symlink to the V2 `tasks/`, `cargo-home` and `hook` linked read-only; config `configs/rerun.json` = the V2 stage 2 config with `A` and `C` at 3 runs and `B` at 0), tasks `cs-p12` and `cs-p20`, Sonnet 5.5, CLI 2.1.296, `--stop-at-cost 8`. 12 of 12 runs planned and run, 0 infrastructure attempts, 0 exhausted, not stopped. Cost **$6.62** (my estimate before the batch was about $4.5: the `A` arm on `cs-p12` ran 38 to 59 turns at $1.03 to $1.48 a run; C total $1.46 for 6 runs, A total $5.16 for 6 runs). Rows: `s5-work-v2r/rows-rerun.jsonl`, summary `rerun-summary.json`.

| task, arm | V2 (stage 1 or 2) | new | pooled |
|---|---|---|---|
| cs-p12 C (`low`) | 1/2 | 2/3 | 3/5 |
| cs-p12 A (`xhigh`) | 2/2 | 3/3 | 5/5 |
| cs-p20 C (`low`) | 0/2 | 0/3 | **0/5** |
| cs-p20 A (`xhigh`) | 2/2 | 3/3 | **5/5** |

Rule fixed before the batch (loss reproduces on a task if C passes at most 1 of 3 and A at least 2 of 3, on the new rows): **cs-p20 reproduces** (0/3 and 3/3). **cs-p12 does not** (C 2/3). The cs-p12 loss in the new rows (`cs-p12.C.1`) is a different failure from the V2 one: its f2p tests fail by assertion (a wrong fix), where `cs-p12.C.1` in V2 stopped to ask. Both are one run each; this is not a stable effect.

Cause on cs-p20 (command text of the 6 new runs; not the build error text, which was not read in these rows): all three `low` runs ran only `cargo test --lib …` or `--no-default-features` filters; none ran a command that compiles all targets. All three `xhigh` runs ran plain `cargo test`, `cargo test --no-fail-fast --tests` or `cargo clippy --all-targets`. All three `low` rows end `build=failed` in the grade (the f2p tests are `notRun`), none of the `xhigh` rows. So the loss is breadth of verification (which targets were compiled), not whether a test ran. This matches the earlier replay for cs-p20 and is a more precise reading than "low skips verification": over all 48 V2 runs the share that ran any test after the last edit did not differ (the section above).

Limits: 6 runs on one task for the cs-p20 pattern; the build error text not read; `steered: 0`, efforts exactly `low` and `xhigh`; 48 V2 runs plus 12 here are not a registered test and do not change the R1 verdict (`not_shown_safe`).

## Mining of the operator's transcripts and commits (2026-10-14, no model call)

Instruments (outside any repo, `/home/marius/work/claude/es-stage0/`): `mine-inventory.py`, `mine-replayable.py`, `mine-attribute.py`, `mine-docs.py`; outputs in `mining/` (ids and counts only, no message text). Corpus: 1,347 main transcripts in the three profiles, 3.12 GB, `effort` on about 99.9% of assistant records, present in files from 2026-08-03.

- **Effort in use:** `xhigh` about 249,000 records, `high` about 91,800, `medium` about 33,500, `low` **41**. Natural `low` data does not exist; "low broke it, high repaired it" cannot be mined at `low`. Only 89 sessions mix two or more efforts (61 with at least 50 records at the second level; 30 of them Sonnet 5.5).
- **Time-window attribution of commits to sessions fails:** 90 of 94 xhigh-dominant codescout sessions have 3 or more commits in their window (the checkout is shared; 4,856 commits since 2026-08-01 on all branches). Attribution by the `git commit` output inside the transcript works: 1,571 commits, 50 shas no longer in the repo.
- **Replay pool (code):** 220 commits with code and tests; 174 of them have one parent, at most 8 files and all non-doc files edited by the session itself (121 with at most 4 files), in 39 sessions. By the dominant model of the session: **Opus 5: 161**, Sonnet 5: 9, Opus 5.5: 4, **Sonnet 5.5: 0**. A replay on Sonnet 5.5 changes the model as well as the effort, so a fresh `xhigh` control on the same model is needed.
- **Replay pool (docs):** 1,088 of the 1,571 commits (69%) change only documents: issue files 534, trackers 358, other docs 68, plans 31, specs 24, memories 18. Median 62 changed lines; 628 are single-file; 830 delete lines (not pure appends).
- **Two examples read in full (`mining/ex1.diff`, `ex2.diff`):** (1) an issue-file commit of 5 added lines records a SHA and a patch-id: the content is derivable from the repo and checkable by `git`. (2) a tracker commit of 6 added paragraphs is an analysis built from the session's own discussion (a measured case, a peer report, an argument about triggers): its content is not in the repo state at the parent, so a replay prompt would have to dictate it, and its quality has no objective grader. Conclusion: a mechanical grader fits the first kind only. The share of each kind among the 1,088 commits was NOT measured.
- **Dirty-state check used:** the parent commit is a clean tree by construction; a candidate counts only when its non-doc files are all files the session edited itself (191 of the 220 code+tests commits).

## Test: Anthropic's verification paragraph at `low` on cs-p20 (2026-10-14): no effect

Root `/home/marius/work/claude/s5-work-v2p` (copy of the cs-p20 task, `cp --reflink=always`; `tree/`, `hidden/`, `gate.json` unchanged; `prompt.txt` = the V2 prompt plus a blank line and the "Verification on coding tasks" paragraph of "Prompting Claude Sonnet 5.5", verbatim; manifest and config `promptHash` updated; builder `es-stage0/build-para-root.mjs`). Config `configs/verify-para.json`: arm C (`low`) 5 runs, A and B 0. Sonnet 5.5, CLI 2.1.296, personal S5 token, `--stop-at-cost 3`. Run, not stopped, 0 infrastructure attempts. Cost **$1.107** for 5 runs (estimate before: about $0.9).

Result: **0 of 5 pass**, all five `build=failed` at grading. Rule fixed before the batch (effect if at least 4 of 5 pass, none if at most 1 of 5): **no effect**. Reference without the paragraph, from other batches and the same prompt otherwise: `low` 0/5, `xhigh` 5/5 (cross-batch, not paired).

What the agents ran (command text of all five transcripts): none ran a command that compiles all targets; every run used `cargo test … --lib` or a name filter, and all five started with `--no-default-features` (the build-hint sentence in the prompt). The two final messages read in full state what was NOT run ("I didn't run the rest of the suite outside `librarian::`"; "I ran only the two progress tests … not the full suite"). The paragraph's last clause ("say which one you did not run and why") was used as the way out; its "a real check that exercises the change" was met by a narrow `--lib` run. In the three `xhigh` rerun runs the first `cargo test` carried no `--no-default-features`.

Limits: one task; the paragraph sat in the user prompt, not the system prompt; a more specific instruction (compile all targets) was NOT tested on purpose, because it would be written for this one known failure. The build-hint sentence of the prompt may interact: `low` followed it from the first command in 8 of 8 cs-p20 runs seen (3 rerun + 5 here), `xhigh` in 0 of 3; this is an observation on cs-p20, not shown to cause the loss.

## Test: a termination gate at `low` on cs-p20 (2026-10-15): closes the gap on this task

Instrument (all outside the claude-plugins checkout): a COPY of the harness, `/home/marius/work/claude/es-stage0/es-gate/effort-steering`, with one patched file (`spikes/s5/hook-copy.mjs`: the `gate` directory is copied into the hook copy and `settings.json` registers a `Stop` hook with `timeout: 1500`) and one new file, `gate/stop-gate.mjs`. When the agent is about to finish, the hook runs `cargo check --offline --all-targets` in `/work`. If it fails and no feedback was sent yet, it blocks the stop ONCE (`{"decision":"block","reason":…}` with the last 6,000 characters of compiler output); a second stop is never blocked, but the check still runs and its result is logged in `$XDG_STATE_HOME/gate/log.jsonl`. A check that cannot run allows the stop.

Checks before any model call: the hook semantics from the Claude Code hooks documentation (`decision: "block"` and `reason` continue the conversation; `stop_hook_active`; a cap of 8 consecutive continuations that resets on each tool call). A local Docker test on the cs-p20 tree and warm target, no model (`es-stage0/test-gate.sh`, results in `es-stage0/gate-test/`): clean tree allowed (131 s on the first compile), a planted type error blocked once with the compiler message, the second stop allowed.

Batch: root `/home/marius/work/claude/s5-work-v2g` (tasks and cargo-home linked read-only from V2; its own `hook/`), config `configs/gate.json` (task cs-p20 as in V2, prompt unchanged; arm C `low`, 5 runs), Sonnet 5.5, CLI 2.1.296, personal S5 token. One test run first, then four more in the same rows file. 0 infrastructure attempts, not stopped. Cost **$1.258** for 5 runs (mean $0.252 per run).

Result: **5 of 5 pass** (`build=ok` in all). Rule fixed before the batch (the gate closes the gap if at least 4 of 5 pass, does not if at most 1 of 5): **closes the gap on this task**. References from other batches: `low` without the gate 0/5 (V2 and rerun) and 0/5 (paragraph), `xhigh` 5/5; mean cost on cs-p20: `xhigh` $0.478 (3 rerun runs), `low` $0.182 (3 rerun runs), gated `low` $0.252.

What the gate did, per run (gate log and transcripts): it blocked in **4 of 5 runs** (C.0, C.2, C.3, C.4) after a check of 50 to 55 s; the transcript of each holds one "Stop hook feedback" message; each of these runs then ran `cargo check --offline --all-targets` itself two or three times and passed. The errors in those four messages are all `E0063 missing field progress in initializer of …` in other test targets (`tests/audit_shards_cross_machine.rs`, `tests/link_scan.rs`, `tests/librarian/audit_doc_refs/*.rs`), the same class as the original loss. In **C.1** the gate did not block: the agent's tree already compiled at the first stop (`check_ok`).

Limits: one task, and a failure that is a compile error a type-check sees; the gate was chosen KNOWING that cause (a generic CI step, but picked after the fact). It does not help a wrong fix (cs-p12.C.1) or a stop-to-ask. The references are from other batches, not paired. C.1 reached a compiling tree unaided, so the unaided rate on cs-p20 is not strictly zero (0 of 10 in the earlier batches, 1 of 1 here without a block). The harness copy differs from the claude-plugins checkout in the patched file; the original `hook-copy.mjs` tests were not run on the copy. The check itself adds about a minute per stop to wall time.

## Generalizing the gate beyond code: a critic for plans and design docs (Stage A, 2026-10-16)

Why: the `cargo check` gate only applies to code. The operator asked for a mechanism that also covers brainstorming, specs, designs and plans. The proposal (recorded in the session, 2026-10-16) keeps the gate's core, an independent check at the moment the agent commits, and changes only the check for each artifact kind: mechanical checks first (`audit_doc_refs`, placeholders, schema), then a grounded critic that reads the document AND its sources at a higher effort, then the human at real decision points. Effort moves from the producer to the check, and the producer's effort rises only after a check fails twice. Claude Code supports this directly: a `Stop` hook of `type: "agent"` runs a subagent with Read/Grep/Glob for up to 50 turns and returns `{"ok": false, "reason": …}`, which is fed back to the agent (hooks docs, read 2026-10-16; the docs show a `model` field for it but no effort field).

Stage A asks whether such a critic can act as a gate at all, before any agent loop: does it find a known defect planted in a document, how often does it block the original, and does its own effort matter. Material and rules are pre-registered in `/home/marius/work/claude/es-stage0/critic-a/prereg.md` (outside any repo, with input hashes): 20 codescout documents (12 plans with their specs, 5 specs, 3 ADRs), each clean and with one planted defect whose class was drawn before the document was read; the snapshot is the code the author wrote against; 80 calls (Sonnet 5.5, `low` and `high`), graded blind to effort. Gate rule: usable at an effort if it blocks on the planted defect in at least 14 of 20 and falsely blocks at most 4 of 20 originals.

Found during preparation, before the batch: the first snapshot rule (the parent of the commit that added the document) gave trees that already held the implementation for p09, and the two calibration calls ($0.394) reported "already implemented" blockers that would have appeared on every clean copy. A marker-based rule fixed it (4 snapshots moved); for s03 the first marker was the name the spec proposed, not the one the code used, so the "marker absent" check passed vacuously until the code was read. One planted "nonexistent" helper (p10, `seed_tracker`) turned out to exist with another signature and was replaced. Batch approved by the operator 2026-10-16.

Run: 80 of 80 calls `rc=0`, effort guard clean on every transcript, **$14.55** (plus $0.39 calibration). Mean per call: `low` $0.124, `high` $0.240.

Grading. Planted variants graded by me from a view that showed random codes, not effort (`grading/grades-planted.json`, hashed before unblinding). Clean variants: 88 blocking findings; the pre-registered seeded sample of 60 was verified against the snapshots by three Opus subagents that saw codes only (51 `true`, 6 `false`, 3 `setup`); I re-checked 7 labels (F07 and F33 at the source, five at the level of the verifier's reasoning; F15 and F57 are judgment calls a stricter grader could flip). `setup` is a label added after the runs, a deviation: a finding true only because of my tree (the removed `CLAUDE.md`, or a snapshot that already held part of the work: p04's snapshot commit is the plan's own Task 0, p07's added the deprecated title, p08's plan was first committed with its execution notes). After unblinding, 8 blocked clean calls were still undetermined (their unsampled findings); their 12 findings were verified POST-HOC by a fourth effort-blind verifier (7 `true`, 5 `setup`).

Result (pre-registered rules):

| | `low` | `high` |
|---|---|---|
| planted defect found and rated blocking | 18 of 20 | 17 of 20 |
| planted defect found at any severity | 18 of 20 | 19 of 20 |
| clean original blocked, with a verified real defect | 16 | 19 |
| clean original blocked only by `setup` findings | 1 (p07) | 1 (p07) |
| clean original blocked with no real defect (`false_block`) | **0** | **0** |
| clean original not blocked | 3 | 0 |
| verdict | **usable** | **usable** |

- **The critic can act as a gate on this material, at both efforts.** Missed as blocking: p02 at both (a wrongly quoted existing line, `tracing::debug!(…"tool call start")`), s05 at `low`, p03 and s08 at `high` (found, rated `minor`).
- **The critic's own effort does not matter here:** paired, `high` caught 1 item `low` did not (s05) and `low` caught 2 `high` did not (p03, s08); difference −1, rule needed +4. `high` costs 1.9 times as much.
- **The originals are full of real defects.** 58 of the 72 verified blocking findings on unmodified documents were real (`true`), 6 false, 8 `setup`. A gate would therefore send nearly every document back once; whether each round is worth it depends on how much the defects matter, which this stage did not measure (severity was not judged).
- The one miss at both efforts is a wrong verbatim quote of existing code. That is exactly what a mechanical check catches ("does this quoted line exist in that file"), which supports layer 1 before the critic.

Limits: one call per cell; the defects are mine and the rubric names their classes (an upper bound for recall); one model (Sonnet 5.5); the verifiers are models, not people; the critic was run outside an agent loop, so nothing here shows that a producer FIXES what the gate reports, or what the extra round costs. Files: `critic-a/grading/` (`results.json`, `results-final.json`, the verifier results, the hashes taken before unblinding).

## Stage C (2026-10-16): the critic as a `Stop` hook in a plan-writing loop — no help shown; `xhigh` wins

Pre-registered in `/home/marius/work/claude/es-stage0/critic-c/prereg-c.md` (with two recorded deviations, both before the batch: the first smoke run showed that Claude Code does not pass `CLAUDE_CODE_OAUTH_TOKEN` to hook processes, so the nested critic was "Not logged in" and the hook failed open — fixed by reading the token from a file, with a test that fails on the old code; the second smoke start was killed by my own tool timeout and discarded). An agent (Sonnet 5.5) wrote an implementation plan from a real codescout spec in the code snapshot the original author used, 6 items (p01, p02, p03, p08, p09, p10) × 3 arms: `L` (`low`), `LG` (`low` + the Stage A critic at `low` as a `Stop` hook that blocks once with its blocking findings), `X` (`xhigh`). Grading: an Opus 5.5 `high` audit of each final plan (same rubric, different model, arm unseen), then an Opus verifier on every blocking audit finding (codes only, the tree behind a code-named link). Batch $35.40 (producers, critics and audits).

| arm | verified blocking defects in the final plans (D, 6 plans) | mean cost per run (producer + critic) | plan length |
|---|---|---|---|
| `L` | **5** (p02 2, p09 3) | $0.35 | 140–294 lines |
| `LG` | **4** (p02 1, p03 1, p09 2) | $0.54 | 128–345 lines |
| `X` | **0** | $1.92 | 612–1240 lines |

- Rules: gate helps `low` if D(LG) < D(L) in >= 5 of 6 — it was 2 of 6: **no help shown**. Gated `low` matches `xhigh` if D(LG) <= D(X) in >= 4 of 6 — it was 3 of 6: **not shown**.
- The gate blocked in 1 of 6 runs (p02, 2 findings; the producer revised and the second check passed). In the other 5 it read the plan and the code (about 25 tool calls on p10) and returned no findings, including on p03 and p09, whose final plans hold verified defects. All 9 blocking audit findings were verified `true`.
- So the `low` critic that found 18 of 20 PLANTED defects in Stage A missed the NATURAL defects of these fresh plans (examples: a stated type `dyn Layer<Registry>` that does not compile; an overflow threshold the plan's inputs never reach; a selector the spec names that the plan's gate omits). The auditor was a stronger model at `high`; whether model or effort makes the difference was not separated.
- `X` produced zero verified blocking defects in 6 of 6, at 5.4 times the cost of `L` and 4 to 6 times the plan length.

What this means for "when to change effort": for plan writing on this material, producer effort mattered and a cheap `low` critic did not substitute for it. This is the opposite of the code result (`cargo check` closed the gap), and the difference is the check: a compiler is a complete oracle for its class of defect; a `low` LLM critic is not, on defects nobody planted.

Limits: 6 items, one run per cell; the audit's depth per line is not constant (an Opus `high` audit of a 1,240-line plan may look less closely than of a 140-line one, so `X`'s zero may be optimistic); the auditor used the gate's rubric; nothing here shows whether a plan leads to working code.

## Stage D (2026-10-16): where is the knee — `medium` and `high` plan writers do not match `xhigh`

Pre-registered in `/home/marius/work/claude/es-stage0/critic-d/prereg-d.md`, with input hashes taken before the batch. Phase 1 ran 12 producer runs and 12 audits. It used the same 6 specs, code trees and prompt as Stage C, with no gate. Arms `M` (`medium`) and `H` (`high`) are new. Arms `L` and `X` are the Stage C runs, not rerun. Grading is as in Stage C: an Opus 5.5 `high` audit of each final plan with the arm unseen, then an Opus verifier on every blocking audit finding (codes only).

| arm | verified blocking defects D (6 plans) | per item | mean producer cost | mean plan length |
|---|---|---|---|---|
| `L` | **5** | p02 2, p09 3 | $0.35 | 206 lines |
| `M` | **5** | p02 2, p09 2, p10 1 | $0.47 | 255 lines |
| `H` | **3** | p03 1, p09 1, p10 1 | $0.81 | 475 lines |
| `X` | **0** | none | $1.92 | 984 lines |

- Only p02 and p09 can discriminate. `L` had D = 0 on the other four items, so they show a floor effect. The rules were fixed before the batch: a level "clears" if D = 0 on both p02 and p09, "holds" if D = 0 on at least 3 of the 4 other items, and "matches `xhigh`" if it does both.
- `M` clears neither p02 (2) nor p09 (2). `H` clears p02 but not p09 (1), and it holds on only 2 of 4 (p01, p08). **Verdict: no tested level below `xhigh` matched; `xhigh` remains the only level shown to clear.**
- `medium` equals `low` in total (5 against 5) at 34 percent more producer cost. `high` leaves 3 defects at 42 percent of the cost of `xhigh`.
- Totals fall as effort rises, but single items are noisy: `high` has defects on p03 and p10, where `low` had none.
- The verifier labelled 8 of 9 audit findings `true`. The one `false` (D005, on p02 `M`) does not change D for that run, which stays 2.
- Spend: $18.93 ($7.70 producers, $11.23 audits). The verifier subagent is not in this figure. The conditional phase 2 was not approved and not triggered.

Limits: one run per cell and 6 items. Plan length rises with effort (206 lines at `low`, 984 at `xhigh`), and an audit may look less closely at a long plan. So the zero for `xhigh` may be optimistic, and these data cannot separate better plans from less closely audited plans. `L` and `X` come from the Stage C batch, not from the same randomised batch as `M` and `H`. The auditor used the gate's rubric. Nothing here shows that a plan leads to working code.

What this adds to "when to change effort": for plan writing on this material, no cheaper level reached the `xhigh` result. The cheapest working design so far is `low` plus a mechanical gate for code, and `xhigh` for plans.

## Defects filed from Stage A (2026-10-16)

The 58 verified findings on the ORIGINAL documents (about 35 distinct defects) were triaged at HEAD `a16f7d42` (operator's choice: file only those that still matter now). About 20 were historical only: execution details of plans whose implementers worked around them (wrong test helpers and counts, clippy failures, contradictory steps). Fourteen candidates were re-checked at HEAD by two Opus agents, and every `live` verdict was re-checked by me at the source:

- **Filed, commit `9cef019f`:**
  - a project with no `[ignored_paths]` section gets no default ignore patterns in the code index (IC-3, medium; 5 of 7 real `project.toml` files on this machine also carry an explicit `patterns = []`);
  - the research index README names the retired `artifact_augment` and a gather pass the tracker cannot run (IC-11);
  - the write-path ADR's "every unpinned write" (IC-24);
  - the artifact-id ADR's "full fidelity on a developer machine", with its 8-versus-9 count noted in the same file (IC-24).
- **Refuted at HEAD after a `live` verdict:** reqwest built without `system-proxy`. In the default build, hyper-util's `client-proxy-system` is still enabled through reqwest 0.12.28 (pulled in by `hf-hub` under `local-embed`), and reqwest 0.13 always installs the system matcher. So the macOS system proxy is honoured by an accident of feature unification. The file I had already created was removed (copy kept in `critic-a/filing/`).
- **Not live:** the rest were fixed later (TLS swap `6c19cc7d`; research README frontmatter and params, `952ad795` / `69d09851`) or never reached the code (heartbeat instance id, `resolve_sids` order, dirty-set drain, CLI ignore plumbing, `/v1/v1`). The one-measure ADR's six-versus-nine census is a dated snapshot that no longer drives work, so it was not filed.

So the critic's findings on old documents led to one medium code defect that matters now, and the triage itself needed a source check: one of its `live` verdicts was wrong.
