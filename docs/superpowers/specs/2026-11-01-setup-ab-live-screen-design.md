# Setup A/B, Stage 4b: bare-arm screen of the Live Rust pool (design)

Date: 2026-11-01. Extends `2026-10-31-setup-ab-live-tasks-design.md` (loader and control gate) and `2026-10-27-codescout-setup-ab-design.md` (arms, integrity, cost, strata), which stay binding. This document registers the pool and the relaxed gate, and decides how Live tasks plug into the runner.

## Registered before any screening run

The control batch ended on 2026-11-01 02:29 (50 tasks, `setup-ab-data/live/controls-batch/`): 10 strict-ok, 39 REJECT, 1 ERROR. Only 4 strict-ok tasks pass the hardness filter. The controls showed that every `obscura` image carries the same 71 environment-broken `PASS_TO_PASS` tests (68 in `obscura-js`, 3 in `mcp_client`) that fail at base AND at gold, so the strict gate rejects 11 otherwise sound `obscura` tasks.

**Relaxed gate (decided after seeing the controls; disclose it in any write-up).** A task is usable when, in its control: every `FAIL_TO_PASS` test fails at base and passes at gold; the gold repeat run shows no mismatch among the tests that remain expected; and the expected set is `FAIL_TO_PASS` plus the `PASS_TO_PASS` tests that pass at gold. `PASS_TO_PASS` tests that fail at both base and gold are dropped from the expected set for every arm alike. The strict rule's other parts (offline, after-cutoff, hardness filter: at least 2 files or at least 40 added lines, at most 600) stay.

**Pool (9 tasks).** `rust-lang__rust-analyzer-22789`, `pola-rs__polars-28484`, `gitbutlerapp__gitbutler-14920`, `modelcontextprotocol__rust-sdk-1021`, `h4ckf0r0day__obscura-412`, `-477`, `-414`, `-447`, `-390`. Five of nine are from one repository: stratum and clustering effects must be disclosed in any analysis. The evidence is `controls-batch/<id>/control.json`; `stage4-followup.json` lists the strict-ok set.

> Superseded: the amendments at the end of this document (pool v2, pool v3) replace this pool; the current pool is seven tasks.

**Screen.** Bare arm only; models `claude-sonnet-5-5` and `claude-opus-5-5`; 2 reps per task and model: 36 runs. Purpose: measure the plain agent's pass rate per task so the comparison can use tasks where it fails 30 to 70% of the time. Screen rows are calibration data in their own data dir (`setup-ab-data/live-screen/`), never rows of the comparison: a later comparison on the selected tasks needs a fresh registration and fresh runs (selecting on the screen's outcomes and reusing them would bias against bare). A screen outcome is not a claim about codescout.

> Superseded: 36 runs became 32 (pool v2) and then 28 (pool v3); see the amendments at the end of this document.

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

## Amendment 2026-11-01 (pool v2)

Decided after the build and grade attempts and before any model run; recorded in `tools/setup-ab-eval/multilang-pool-v2.json` (v1 stays in git as the registered history).

- **Pool.** Eight tasks: `h4ckf0r0day__obscura-412`, `-477`, `-414`, `-447`, `-390`, `modelcontextprotocol__rust-sdk-1021`, `gitbutlerapp__gitbutler-14920`, `rust-lang__rust-analyzer-22789`. `pola-rs__polars-28484` is dropped: its build and sweep pass, but the empty and the gold grade were both OOM-killed at the frozen 8 GB grading/arm limit (0 of 1555 tests ran; the gold grade took 1595 s; the image is 85.9 GB). Raising the limit would change the frozen arm resources, so the task leaves the pool. The task entries of v2 are those of v1 (same `control_sha256`, `expected_count`, `dropped_count`), re-verified against the real `control.json` files (`freeze_screen.py verify-pool`).
- **Runs.** 8 tasks x 2 models x 2 reps = 32 bare runs. The budget is unchanged (cap 50.0, per-call 5.0, reserve 5.0 USD): all 32 runs start iff the first 31 cost on average at most (50 - 10) / 31 = 1.29 USD.
- **Known limitation, accepted and to be disclosed: `rust-analyzer-22789` and CRLF.** `run.py` captures the agent's patch with universal newlines, and the image keeps `* -text` for three CRLF blobs. An agent edit to `crates/hir-def/src/nameres/tests/imports.rs`, `crates/proc-macro-srv-cli/README.md` or `crates/proc-macro-srv/src/bridge.rs` would grade "patch does not apply", whatever the edit.
- **Provenance.** The relaxed gate and the pool were registered after seeing the controls (as stated above); this amendment was decided from infrastructure results, not from any agent outcome. Five of the eight tasks are still from one repository (obscura).

## Amendment 2026-11-01 (pool v3)

Decided after grade attempts and before any model run; recorded in `tools/setup-ab-eval/multilang-pool-v3.json` (v1 and v2 stay in git as history; `freeze_screen.py` freezes only the current pool and refuses a pool that lists a dropped id).

- **Pool.** Seven tasks: `h4ckf0r0day__obscura-412`, `-477`, `-414`, `-447`, `-390`, `modelcontextprotocol__rust-sdk-1021`, `rust-lang__rust-analyzer-22789`. `gitbutlerapp__gitbutler-14920` is dropped. Its round-3 prescan had 0 hits and `/mnt_tmp` was empty, but the gold grade fails in the grading environment: three expected tests fail identically in the empty and the gold run (the FAIL_TO_PASS test passes). `read_file_from_workspace_security::rejects_dotdot_traversal` and `::rejects_symlink_escape` write to `workdir.parent()` and get Permission denied as uid 1000 (`/` is root-owned, mode 755; reproduced in a throwaway container for `rejects_dotdot_traversal`, inferred for `rejects_symlink_escape`, which fails the same way in the grade log); `command::land::land_no_ff_creates_signed_merge_commit` fails with "ssh-keygen should produce a signing key" (cause not isolated). The control ran as root; the grade runs as the image user. Keeping the task would need a second post-hoc rule plus an unexplained failure, so it leaves the pool; it may return in a later pool version after investigation.
- **Runs.** 7 tasks x 2 models x 2 reps = 28 bare runs. The budget is unchanged (cap 50.0, per-call 5.0, reserve 5.0 USD): all 28 runs start iff the first 27 cost on average at most (50 - 10) / 27 = 1.48 USD.
- **Provenance.** As for v2: decided from infrastructure and grade results, not from any agent outcome. Five of the seven tasks are from one repository (obscura).
