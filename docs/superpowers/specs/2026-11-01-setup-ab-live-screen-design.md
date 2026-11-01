# Setup A/B, Stage 4b: bare-arm screen of the Live Rust pool (design)

Date: 2026-11-01. Extends `2026-10-31-setup-ab-live-tasks-design.md` (loader and control gate) and `2026-10-27-codescout-setup-ab-design.md` (arms, integrity, cost, strata), which stay binding. This document registers the pool and the relaxed gate, and decides how Live tasks plug into the runner.

## Registered before any screening run

The control batch ended on 2026-11-01 02:29 (50 tasks, `setup-ab-data/live/controls-batch/`): 10 strict-ok, 39 REJECT, 1 ERROR. Only 4 strict-ok tasks pass the hardness filter. The controls showed that every `obscura` image carries the same 71 environment-broken `PASS_TO_PASS` tests (68 in `obscura-js`, 3 in `mcp_client`) that fail at base AND at gold, so the strict gate rejects 11 otherwise sound `obscura` tasks.

**Relaxed gate (decided after seeing the controls; disclose it in any write-up).** A task is usable when, in its control: every `FAIL_TO_PASS` test fails at base and passes at gold; the gold repeat run shows no mismatch among the tests that remain expected; and the expected set is `FAIL_TO_PASS` plus the `PASS_TO_PASS` tests that pass at gold. `PASS_TO_PASS` tests that fail at both base and gold are dropped from the expected set for every arm alike. The strict rule's other parts (offline, after-cutoff, hardness filter: at least 2 files or at least 40 added lines, at most 600) stay.

**Pool (9 tasks).** `rust-lang__rust-analyzer-22789`, `pola-rs__polars-28484`, `gitbutlerapp__gitbutler-14920`, `modelcontextprotocol__rust-sdk-1021`, `h4ckf0r0day__obscura-412`, `-477`, `-414`, `-447`, `-390`. Five of nine are from one repository: stratum and clustering effects must be disclosed in any analysis. The evidence is `controls-batch/<id>/control.json`; `stage4-followup.json` lists the strict-ok set.

**Screen.** Bare arm only; models `claude-sonnet-5-5` and `claude-opus-5-5`; 2 reps per task and model: 36 runs. Purpose: measure the plain agent's pass rate per task so the comparison can use tasks where it fails 30 to 70% of the time. Screen rows are calibration data in their own data dir (`setup-ab-data/live-screen/`), never rows of the comparison: a later comparison on the selected tasks needs a fresh registration and fresh runs (selecting on the screen's outcomes and reusing them would bias against bare). A screen outcome is not a claim about codescout.

## Decision record

**Decision:** Live tasks enter the runner as a third task kind `live`, with a bare-only screen run under a screen-form registration and its own data dir. No Qdrant template, no cs/full arms in this stage.

**Context:** `run.py` is the spend-safety path: `run_batch` refuses to spend without a registration (`load_registration`: self-digest, registered plan hash, budget) and a `GO.json` that binds cap, per-call cost, plan hash and registration hash; it journals every run, latches stops, and re-hashes the frozen arm inputs (`_frozen_inputs`) at each batch start. The heavy host re-check (`registration.check_committed`, `check`) applies only when `plan_reps(plan) == "stage3"` or `reg["stage"] == 2`. A bare-only plan has blocks `[("bare", n)]`, which equals neither `STAGE3_REPS` nor `STAGE1_REPS`, so `plan_reps` returns `None`. `load_task_from_bundle` knows only the kinds `private` and `foreign` and requires a Qdrant template. `tasks.grader_for` dispatches on `record["kind"]` and falls through to the foreign grader. The foreign grader and `baseline.grade_output` parse libtest text and cannot grade Live logs (nextest JSON, per-record `log_parser`).

**Alternatives considered:**
- A separate screen runner: rejected. It would copy the budget, journal, latch and cap logic that protected the $200 study.
- Disguise Live tasks as kind `foreign`: rejected. The foreign record has `eval_script` and a libtest log; Live has `test_cmds` and a parser. One kind would hide two record shapes behind one name.
- Build cs/full support now (index, Qdrant template, exec-only layer for all arms): rejected as a wall in an empty field. If the screen shows the plain agent already passes, that work is wasted. It comes after the screen shows a 30 to 70% band.
- A kind registry replacing the `if kind ==` sites: rejected for now. There are three touch sites (`tasks.grader_for`, `run.load_task_from_bundle`, `stage3.KINDS`/registration), and a third kind is the first time the sites diverge. A fourth source reopens this.
- Reuse `freeze_stage2.py`: rejected. It is the 12-task study's freeze (selection, Stage 1 journal, pilot split, S5 grader inputs, templates).

**Consequences:**
- Now easier: the screen reuses the runner's spend safety, journal, integrity and cost accounting unchanged; Live tasks get a grader that reuses the control's reset, rebuild, test and parser code, so the screen grades the way the control validated.
- Now harder: `tasks.py` and `run.py` get small edits (one branch each), so the Stage 3 registration's file hashes no longer describe the tree; Stage 3 stays verifiable at its commit `bb8d4b873` and its analysis is already final. The Live bundle needs its own `build`, `prescan` and `arm` stages, because the `tasks.py` stages were written per kind.

**Change scenarios absorbed:** (1) another Live pool or a Multi-SWE-bench source arrives: a new bundle builder and a new kind branch, no runner change beyond that branch. (2) the screen shows a band and cs/full arms are wanted: the missing piece is isolated in `multilang_tasks.prepare` (index and template), the runner already handles templates for the other kinds. (3) the relaxed gate is reversed: the pool file and the grader's expected-set rule are one place.

**Revisit-when:** a fourth task source arrives (extract a kind registry); the screen's pass rate is at a ceiling again (stop, the pool is not hard); cs/full arms are added for Live tasks.

**Confidence:** medium. High on the touch sites (read in the code this session); medium on how much of the `tasks.py` stages (`prescan`, `arm`, `control`, `verify`) is kind-independent: the plan makes the first task a seam read with a written report before code.

## Open risks the plan must test, not assume

- The Live image holds the gold fix, stale `reports/` and extra edits. The task image build must reset to `base_commit`, then remove `.git` and `reports/`, keep `target/`, and the leak gate must prove no needle remains.
- The workdir is `/testbed` or `/testbed/repo` per image; the agent user and the prompt must use it.
- The bare arm container has no network. The agent must be able to build and test offline, as the control did.
- The screen needs a GO.json with a cap. Spending requires the operator's explicit go, per run of this stage.

## Out of scope

cs and full arms for Live tasks; the comparison registration and analysis (`analyze_study` strata assume the 12-task shape); any change to the Stage 3 registration or data.
