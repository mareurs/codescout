# Live Rust Screen: Bundles, Grader and Screen Registration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the 9-task Live Rust pool runnable by the existing runner for a bare-arm screen, under a screen registration, with no spending in this plan.

**Architecture:** A new module `multilang_tasks.py` builds Live task bundles (image reset to `base_commit`, agent user, needles, bundle.json) and grades a patch with the control's own reset, rebuild, test and parser code under the relaxed expected-set rule. Two one-branch edits teach `tasks.grader_for` and `run.load_task_from_bundle` the kind `live`. A new `freeze_screen.py` writes the screen-form registration and plan. Nothing in the spend-safety path of `run.py` changes.

**Tech Stack:** Python 3.14 stdlib, `unittest`, Docker CLI through `pilot.docker`.

**Spec:** `docs/superpowers/specs/2026-11-01-setup-ab-live-screen-design.md` (read its decision record and risks first); it extends `2026-10-31-setup-ab-live-tasks-design.md`.

## Global Constraints

- Code in `tools/setup-ab-eval/`; stdlib only; tests `test_multilang_*.py` plus the existing `test_tasks.py` and `test_run.py` for the two branches; run all with `python3 -B -m unittest discover -s tools/setup-ab-eval -p 'test_*.py'`.
- Edits to existing files are limited to ONE new branch in `tasks.grader_for` and ONE in `run.load_task_from_bundle` (plus the bare-only guard in Task 2). Do not touch `stage3.py`, `registration.py`, `freeze_stage1.py`, `freeze_stage2.py`, `analyze*.py`, `arms.py`, `integrity.py`, `frozen.py`, `stage3-registration.json`. `run.run_batch` and the spend-safety path stay as they are.
- Docker objects made here are named `sab-*` (`sab-task-<id>`, `sab-agent-*`, `sab-run-<id>`, `sab-arm-<id>` as in `tasks.py`); never touch `codescout-*`, `pbr27-*`, `rae-*`; never create volumes.
- Evidence under `/home/marius/work/claude/setup-ab-data/` (bundles under `live-tasks/<id>/`, the screen under `live-screen/`), never `/tmp`, never git.
- No model call and no spend in any task of this plan. Never print a token. Spending needs the operator's explicit go after Task 4, per run.
- Pull one image at a time and keep the disk guard: the Live images of the pool total about 40 GB compressed; use `multilang_control.ensure_image`/`registry_gb` rules (cap 12 GB compressed, free space `max(40, 5*size+10)` GB). A pulled image stays after Task 1 for later builds (record which ones the plan pulled so Task 4 can remove them).
- Reuse, do not copy: `multilang_control.reset_to_base`, `run_tests` (step wrapper), `detect_workdir`, `multilang_grade.run_parser`, `verdict`; `tasks.build_agent_image`, `tasks.export_tree`, `tasks.tree_files`, `leak_scan.needles_for`/`baseline_needles`, `tasks.PROMPT_PREFIX`.

## Review Focus

- **The image holds the answer.** After the bundle build, the agent image tree must equal `base_commit` with no gold-patch needle, no `reports/`, no `.git` history of later commits; the leak gate must run on the real built image (not only fakes) and report zero kept-needle hits.
- **The grader and the control must agree.** For each of the 9 pool tasks, grading the empty patch must fail and grading the record's gold patch must pass under the relaxed rule; a grader that passes the empty patch is a defect.
- **Relaxed rule must be exact.** Expected set = `FAIL_TO_PASS` plus the `PASS_TO_PASS` tests that pass in that task's saved control gold run; a test dropped for one arm is dropped for all; the dropped list is recorded in the bundle, not recomputed per run.
- **Bare-only means bare-only.** A non-bare spec on a `live` task must refuse before any container starts; a `live` bundle without a template must not be accepted for cs/full.
- **Spend safety unchanged.** The screen cannot start without a registration whose digest, plan hash and budget match, and a GO.json; `run.py dry-run` must pass; a plan with a task outside the pool must be refused by the screen freeze.

---

### Task 1: Seam read and Live bundle assembly

**Files:**
- Create: `tools/setup-ab-eval/multilang_tasks.py`, `tools/setup-ab-eval/test_multilang_tasks.py`
- Report: `.superpowers/sdd/<workspace>/seams.md` (written first, see Step 1)

**Interfaces:**
- Consumes: Task 1 of the earlier plan (`multilang_records.load_records`), `multilang_control` helpers, `tasks.build_agent_image(task_image, tag, log)`, `tasks.export_tree`, `tasks.tree_files`, `tasks.short_tag`, `leak_scan.needles_for`/`baseline_needles`.
- Produces:
  - `live_dockerfile(source_ref: str, record: dict, workdir: str) -> str`: `FROM source_ref`; as root: `git reset --hard <base_commit>` and `git clean -fdq -e target` in the workdir, remove `reports/` (both locations), remove `/testbed/.git` and recreate a one-commit repo `Benchmark base source` (as `tasks.foreign_dockerfile` does), keep `target/`, create the `agent` user (uid 1000), `chown`, `ENV HOME=/home/agent CARGO_NET_OFFLINE=true`, `USER 1000:1000`, `WORKDIR <workdir>`, and a final check that the tree is clean at one commit.
  - `assemble_live(record: dict, out: Path, task_image: str, workdir: str) -> tuple[needles, Path, dict]`: builds the task image, exports the baseline tree, computes baselined needles, writes `prompt.txt` (`tasks.PROMPT_PREFIX` + problem statement, CRLF as LF), `gold.patch`, `record.json`, `assembly.json`.
  - `build_live(record: dict, out: Path, control_json: Path) -> tasks.TaskBundle` and a bundle.json with keys `kind: "live"`, `out`, `task_image`, `image`, `record_path`, `workdir`, `expected` (the relaxed expected set: counts and the `dropped` list), `control_sha256`; `template` absent.
  - A CLI `python3 multilang_tasks.py build|prescan|arm <ids...>` writing evidence under `<data>/live-tasks/<id>/`, reusing the `tasks.py` stage helpers for prescan (leak gate on the agent image) and arm (exec-only layer) wherever they are kind-independent.

- [ ] **Step 1: Seam read first.** Read `tasks.py` stages `build_agent_image`, the prescan, `arm` and `control` code paths, `agent-cs.Dockerfile` (how the codescout binary gets into the agent image and whether the build is cached per task image), `leak_scan.baseline_needles`, `integrity.py` construction checks for a bare arm, and `run._run_one` (what it needs from `TaskInfo` for a bare spec). Write `seams.md`: for each stage, whether it is kind-independent, the exact signature, and what a `live` bundle must provide. Stop with NEEDS_CONTEXT if a stage cannot be reused and a design choice is open.
- [ ] **Step 2: Write the failing tests** (fake docker, fakes for the build): `test_live_dockerfile_resets_to_base_and_keeps_target`, `test_live_dockerfile_removes_reports_in_both_locations`, `test_live_dockerfile_creates_agent_user_and_clean_one_commit_repo`, `test_assemble_live_writes_prompt_with_prefix_and_lf`, `test_assemble_live_needles_exclude_strings_already_in_the_base_tree`, `test_build_live_bundle_records_workdir_expected_and_control_sha256`, `test_build_live_refuses_a_control_that_is_not_relaxed_ok`.
- [ ] **Step 3: Implement** `live_dockerfile`, `assemble_live`, `build_live` and the CLI as specified. The expected set comes from the control's saved logs run through `multilang_grade.run_parser` (as in `stage4-followup.json`'s analysis): `FAIL_TO_PASS` must fail at base and pass at gold, the gold repeat must show no mismatch among kept tests, and kept P2P = those passing at gold.
- [ ] **Step 4: Run the tests; all pass; run the whole suite.**
- [ ] **Step 5: Live build on one task** (`h4ckf0r0day__obscura-412`, image already local or 3.3 GB): `python3 multilang_tasks.py build ...` then `prescan`. Expected: the agent image exists, the leak gate reports zero kept-needle hits, the tree at the workdir is clean at one commit `Benchmark base source`, no `reports/`, `target/` present (check size > 1 GB), the file listing contains no gold-patch-only text. Record the output in the report.
- [ ] **Step 6: Commit** `feat(setup-ab): live Rust task bundles (reset image, agent user, needles, bundle.json)`.

---

### Task 2: Live grader and the two runner branches

**Files:**
- Modify: `tools/setup-ab-eval/multilang_tasks.py`, `tools/setup-ab-eval/tasks.py` (`grader_for`, one branch), `tools/setup-ab-eval/run.py` (`load_task_from_bundle`, one branch plus the bare-only guard)
- Test: `tools/setup-ab-eval/test_multilang_tasks.py`, `test_tasks.py`, `test_run.py`

**Interfaces:**
- Consumes: Task 1's bundle.json and `multilang_control` step helpers.
- Produces:
  - `live_grader(record: dict, bundle: dict, image: str, gdir: Path) -> Callable[[str, str], dict]`: returns `grade(patch, name)` matching the foreign grader's result dict (`pass`, `error`, `oom_killed`, `wall_seconds`, `log`, `name`, ...): a fresh container from `image` (`--network none`, arm resources, always removed, name `sab-grade-<id>-<name>-<hex>`); apply `patch` (when non-empty); apply the record's `test_patch`; run the rebuild commands; run the test and print commands; parse with the record's `log_parser` through `multilang_grade.run_parser`; `verdict` over the bundle's `expected` set. Any grader failure (parser error, timeout, docker error) is `pass: False` with an `error` field, never an exception, never a pass.
  - `tasks.grader_for`: `if record["kind"] == "live": return multilang_tasks.live_grader_from_record(record, image, gdir)` placed before the foreign fallthrough (lazy import).
  - `run.load_task_from_bundle`: a `live` branch: prompt path `Path(rec["out"]) / "prompt.txt"`; the template requirement applies only to the other kinds; for `live` the `TaskInfo.template` is `""`.
  - Bare-only guard: when a spec with `arm != "bare"` meets a task whose bundle kind is `live`, `run._run_one` (or the earliest point that sees both) refuses with a message naming the task and arm before any container starts.

- [ ] **Step 1: Write the failing tests:** `test_live_grader_empty_patch_fails_and_gold_passes_with_fake_docker`, `test_live_grader_error_is_a_failed_grade_not_an_exception`, `test_live_grader_uses_the_bundles_expected_set_not_the_records_p2p`, `test_grader_for_dispatches_live`, `test_load_task_from_bundle_accepts_live_without_template`, `test_load_task_from_bundle_still_requires_template_for_foreign_and_private`, `test_run_one_refuses_cs_arm_on_a_live_task_before_any_container`.
- [ ] **Step 2: Run them to see them fail. Step 3: Implement. Step 4: Run all tests; all pass.**
- [ ] **Step 5: Real grade check on the built obscura-412 bundle:** grade the empty patch (expect `pass: False`) and the record's gold patch (expect `pass: True`); record both result dicts and wall times in the report. If gold fails, stop and report; do not change the expected-set rule.
- [ ] **Step 6: Commit** `feat(setup-ab): live Rust grader and the live task kind in the runner`.

---

### Task 3: Pool file and screen registration

**Files:**
- Create: `tools/setup-ab-eval/multilang-pool-v1.json`, `tools/setup-ab-eval/freeze_screen.py`, `tools/setup-ab-eval/test_freeze_screen.py`

**Interfaces:**
- Consumes: Tasks 1 and 2; `run.registration_digest`, `run.plan_sha256`, `run.check_plan`, `frozen` (the existing frozen arm inputs root and trees recorded in `stage3-registration.json["frozen"]`).
- Produces:
  - `multilang-pool-v1.json`: `{"pool": "setup-ab live rust v1", "registered": "...", "gate": "relaxed (spec 2026-11-01)", "tasks": [{"id", "repo", "control_sha256", "expected_count", "dropped_count"}, ...9]}` with the nine ids from the spec and the sha256 of each `control.json`.
  - `screen_plan(pool: dict, models: tuple[str, ...], reps: int, seed: int) -> list[RunSpec]`: bare arm only, deterministic order from the seed.
  - `freeze_screen(*, tool_dir, data, run_data, pool_path, plan_path, out) -> dict`: writes a registration `{"stage": "screen", "head", "files" (tracked tool files and their sha256), "pool_sha256", "tasks" (id, kind live, run_image id, arm_image id, bundle sha256), "plan_sha256", "budget": {"cap_usd": 50.0, "per_call_usd": 5.0, "reserve_usd": 5.0}, "models", "reps": [["bare", 2]], "claude_cli_version", "claude_cli_sha256", "frozen": <same root and trees as the Stage 3 registration>, "registration_sha256"}`; refuses a plan that contains a task outside the pool, a non-bare arm, or a bundle whose control is not relaxed-ok; writes `GO.template.json` (never `GO.json`).
  - `python3 freeze_screen.py freeze|check ...` with `check` re-hashing tool files, bundles, images and frozen trees.

- [ ] **Step 1: Write the failing tests:** `test_screen_plan_is_bare_only_two_models_two_reps_36_runs`, `test_screen_plan_order_is_deterministic_for_a_seed`, `test_freeze_refuses_a_task_outside_the_pool`, `test_freeze_refuses_a_non_bare_arm`, `test_freeze_writes_a_registration_run_py_accepts` (call `run.load_registration` on it and `run.check_registered_budget`), `test_freeze_writes_go_template_never_go_json`, `test_check_detects_a_changed_tool_file_and_a_changed_bundle`.
- [ ] **Step 2: Run them to see them fail. Step 3: Implement. Step 4: Run all tests; all pass.**
- [ ] **Step 5: Commit** `feat(setup-ab): live Rust pool file and screen registration`.

---

### Task 4: Prepare the screen (no spend)

> Pool v2 amendment: eight tasks, 32 runs, `multilang-pool-v2.json` (polars-28484 dropped); see the spec amendment. Every `nine` and `36` below reads eight and 32. Before `freeze`, link each runner path `<data>/tasks/<id>` to `<data>/live-tasks/<id>`.

**Files:** none in git beyond evidence notes; operational.

- [ ] **Step 1:** For each of the nine pool tasks, one at a time (disk guard on), run `multilang_tasks.py build`, `prescan`, `arm`; record per task: image pulled or present, build wall time, leak-gate result. Remove the pulled task images the plan pulled only after the run images are built if disk is below 60 GB free; keep `sab-run-*`, `sab-arm-*`.
- [ ] **Step 2:** For each bundle, grade the empty patch and the gold patch with `live_grader`: expect fail and pass. A task that does not get fail/pass is dropped from the pool file by a new committed pool version with the reason; the screen does not run on it.
- [ ] **Step 3:** `freeze_screen.py freeze` into `setup-ab-data/live-screen/`; then `python3 run.py dry-run` on the plan (the dry run spends nothing) and `freeze_screen.py check`. Expected: both pass.
- [ ] **Step 4:** Stop. Report: tasks ready, per-task image sizes and grade times, the plan's run count and a cost estimate (use the Stage 3 cost-per-run range for bare, scaled by the grade times), the GO.template.json path. The operator decides on the spend; the controller writes GO.json only on the operator's explicit instruction.
