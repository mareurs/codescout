# Setup A/B eval — Stage 0 instrument and Stage 1 launch — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the instrument that runs `bare`, `cs` and `full` Claude Code arms on identical tasks in isolated containers, grades them independently, and applies the pre-registered decision rule. Then, after operator approval, launch Stage 1 calibration.

**Architecture:** Copy the Codex `reliable-agent-eval` runner and grader into `tools/setup-ab-eval/`, with provenance. Add an arm builder that changes only CLI flags and mounts, a study-owned hybrid retrieval stack (Qdrant, dense, sparse) on the study's internal Docker network with a fresh Qdrant volume per run, a leakage scanner, an arm-integrity checker, and an analysis module. Python standard library only, like the source harness.

**Tech Stack:** Python 3 (stdlib, `unittest`), Docker, Claude Code CLI (pinned at freeze), codescout binary (pinned SHA), rust-analyzer, Node (companion hooks), Qdrant v1.17.0, llama.cpp ROCm (dense), TEI cpu-1.8 (sparse).

**Spec:** `docs/superpowers/specs/2026-10-27-codescout-setup-ab-design.md`

**Scope:** spec § 7 Stages 0 and 1. Stages 2 (freeze) and 3 (main run) get their own plan, once Stage 1's costs are known.

## Global Constraints

- Total budget **$200**, failed, unscored and infrastructure runs included. Stage 1 cap **$25**. Stage 3 sized so that its expected spend plus a **25%** reserve fits what is left.
- Sequential launch, a held reserve for the next run, **no automatic paid retries**. Stop on missing cost, credential error, grader contamination or effort mismatch.
- **No `--effort` flag in any arm.** Read the effective effort from the transcript, and require it to be equal across the arms of a task.
- Models: `claude-sonnet-5-5` and `claude-opus-5-5`, analysed separately.
- Same in every arm: CLI binary, model, task prompt bytes, repository bytes, `--cpus`, `--memory`, network policy, `--max-budget-usd` per call, and the timeout.
- Container and network names start with **`sab-`**. The runner removes only `sab-` objects. Codex runs `pbr27-*` and `rae-*` containers on the same host.
- **No arm reaches the operator's live retrieval stack** (`codescout-qdrant` holds the live repository's index, which contains the fixes) and no arm reaches `~/.codescout` or the global buddy memory.
- The target bug's `docs/issues/` file is removed from every private task's tree (operator ruling, 2026-10-27).
- Retrieval mirrors daily use **as measured at freeze**, not as configured. On 2026-10-27, Qdrant, dense (`:48081`) and sparse (`:48084`) answered. The reranker URL `:48083` had no listener.
- Do not modify the Codex worktrees, the S5 harness in claude-plugins, or the live plugins. Copy them, and record the source commit and SHA-256.
- Raw runtime data (transcripts, patches, logs) goes under `/home/marius/work/claude/setup-ab-data/`, not `/tmp` (the F-31 lesson: tmpfs evidence is lost), and not into Git.

## Review Focus

1. **The codescout MCP server fails to start in the container** (a missing shared library, a wrong path). The run then behaves like `bare` with no error. It must be classed `invalid-arm`, never scored. Pinned in Task 6.
2. **Run N sees what run N−1 wrote** to Qdrant or `.codescout/`. Each run must start from the template state. Pinned in Task 3.
3. **A hook fails without blocking** (Node or `codescout` missing from `PATH`). Claude Code continues without it, so the `cs` arm quietly loses its hooks. Integrity requires hook evidence. Pinned in Tasks 4 and 6.
4. **A plan-limit refusal is graded as a model failure.** It must be a neutral row that stops the batch. Pinned in Task 8.
5. **rust-analyzer runs out of memory** under the container cap on the codescout tasks, and only in the arms that start it. An OOM kill is an infrastructure outcome, recorded and not scored. The memory cap is the same in every arm. Pinned in Task 8.

---

### Task 1: Import the Codex harness with provenance

**Files:**
- Create: `tools/setup-ab-eval/{pilot.py,baseline.py,test_pilot.py,test_baseline.py,test_grader_boundary.py}`. These are copies from `/home/marius/.codex/worktrees/3940/codescout/tools/reliable-agent-eval/` at commit `19dccd9bc`.
- Create: `tools/setup-ab-eval/PROVENANCE.json`
- Create: `tools/setup-ab-eval/test_provenance.py`

**Interfaces:**
- Produces: `PROVENANCE.json` = `{"source_repo", "source_commit": "19dccd9bc…" (full SHA), "files": {name: sha256_of_source}}`.
- Produces: the unchanged module names `pilot`, `baseline`. Later tasks import `pilot.docker`, `pilot.extract_patch`, `pilot.transcript_usage`, `baseline.grade_output` and `baseline.restore_paths`.

- [ ] **Step 1:** Copy the five files byte for byte. Write `PROVENANCE.json` with each source file's SHA-256 and the full commit SHA (`git -C <codex worktree> rev-parse 19dccd9bc`).
- [ ] **Step 2: Test.** `test_provenance.py::test_copied_files_match_recorded_source_hashes` hashes each copied file and asserts that it equals `PROVENANCE.json["files"][name]`. It must pass before any edit.
- [ ] **Step 3: Run** `python3 -m unittest discover -s tools/setup-ab-eval -p 'test_*.py'`. Expected: the copied tests and the provenance test pass. If a copied test needs `/tmp/reliable-agent-eval-20261023/` data, mark it `skipUnless(DATA.exists())` in a **separate** commit, and record that edit in `PROVENANCE.json["local_edits"]`.
- [ ] **Step 4: Commit** `test(setup-ab): import the reliable-agent-eval runner and grader from 19dccd9bc with provenance`

### Task 2: Freeze the setup — `freeze_setup.py`

**Files:** Create `tools/setup-ab-eval/freeze_setup.py`, `tools/setup-ab-eval/test_freeze_setup.py`

**Interfaces:**
- Produces: `freeze(profile_dir: Path, claude_json: Path, probe: Callable[[str], bool]) -> dict`. It writes `setup-manifest.json` with these keys:
  - `cli`: `{"path", "version", "sha256"}`
  - `codescout`: `{"binary_sha256", "version_json"}`, the output of `codescout version`
  - `mcp_env`: the codescout server's `env` from `claude_json`
  - `retrieval`: `{"qdrant": bool, "dense": bool, "sparse": bool, "reranker": bool}`, from `probe(url + "/health")`
  - `plugins_cs`: the codescout-companion directory and its version
  - `plugins_full`: every enabled plugin of the profile, with its install path and version
  - `full_config_files`: the `CLAUDE.md` and `settings.json` paths
  - `profile_dir`
- Consumes: nothing from earlier tasks.

- [ ] **Step 1: Failing tests:**
  - `test_reranker_without_listener_is_recorded_absent`: the probe returns False for `:48083`, and the manifest has `retrieval.reranker == False`.
  - `test_full_plugins_resolve_inside_the_profile`: an `installPath` outside `profile_dir` raises `ValueError` naming the plugin. This is the cross-profile drift that `~/.claude-kat` had.
  - `test_memory_and_transcript_dirs_are_never_listed`: `projects/`, `memory/`, `todos/` and `*.jsonl` never appear in `full_config_files`.
  - `test_codescout_dirty_flag_is_recorded_not_gated`: the `codescout_dirty` field has a known always-1 bug (`docs/issues/2026-10-24-codescout-dirty-is-always-one…`), so the field is copied and never used as a refusal.
- [ ] **Step 2:** Run the tests. Expected: they FAIL (module missing).
- [ ] **Step 3: Implement** `freeze`. It reads JSON only, and its one side effect is the manifest write. The profile is a CLI argument. **The operator names the profile** (spec § 9 item 3), with no default.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(setup-ab): freeze the daily setup into a manifest, recording retrieval services as measured`

### Task 3: The study retrieval stack — `stack.py`

**Files:** Create `tools/setup-ab-eval/stack.py`, `tools/setup-ab-eval/test_stack.py`

**Interfaces:**
- Consumes: `setup-manifest.json["retrieval"]` (Task 2) and `pilot.docker` (Task 1).
- Produces:
  - `stack_up(network: str, manifest: dict) -> dict[str, str]`: starts one container per service that the manifest marks `True`, named `sab-qdrant`, `sab-dense` and `sab-sparse`, and returns the service URLs on the study network. The dense container copies `codescout-dense-amd`: same image, same command line, `/dev/kfd` and `/dev/dri`, and `/home/marius/models` mounted read-only. The sparse container copies `codescout-sparse-cpu`: `prithivida/Splade_PP_en_v1`, with a **copy** of the model cache, read-only.
  - `qdrant_for_run(template_volume: str, run: str) -> str`: copies the template volume into `sab-qv-<run>`, starts Qdrant on it, and returns its URL.
  - `stack_down(prefix="sab-")`: removes only objects whose name starts with `prefix`.

- [ ] **Step 1: Failing tests** (fake `docker` recorder):
  - `test_absent_service_is_not_started`: reranker `False` means no reranker container and no reranker URL.
  - `test_each_run_gets_its_own_qdrant_volume_copied_from_template`
  - `test_stack_down_never_touches_foreign_containers`: given `pbr27-x`, `rae-y`, `codescout-qdrant` and `sab-qdrant`, only `sab-qdrant` is removed.
  - `test_no_url_points_at_the_host_stack`: no returned URL contains `127.0.0.1`, `localhost` or `host.docker.internal`.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement the three functions.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Live control (no model call).**
  1. Bring the stack up.
  2. Index a 3-file fixture (one function named `zq_marker_fn`) with `codescout index -p <fixture>`, using the study URLs.
  3. Copy the volume to template T.
  4. Start run A from T and write a memory through `codescout`.
  5. Start run B from T.
  6. Assert: semantic search in B finds `zq_marker_fn`, and B does **not** see A's memory.
  7. Save the output to `setup-ab-data/controls/stack-isolation.json`.
- [ ] **Step 6: Commit** `feat(setup-ab): study-owned retrieval stack with a fresh Qdrant volume per run`

### Task 4: Arm images and arm invocations — `arms.py`, `agent-cs.Dockerfile`

**Files:** Create `tools/setup-ab-eval/arms.py`, `tools/setup-ab-eval/test_arms.py`, `tools/setup-ab-eval/agent-cs.Dockerfile`

**Interfaces:**
- Consumes: the manifest (Task 2), the stack URLs (Task 3), and a task's base image (Task 7).
- Produces: `arm_invocation(arm: Literal["bare","cs","full"], model: str, task_prompt: str, manifest: dict, urls: dict[str,str], run_dir: Path) -> ArmSpec`, where `ArmSpec = NamedTuple(cli_args: list[str], mounts: list[tuple[str,str,str]], env: dict[str,str])`.
  - **bare:** exactly the Codex `pilot.py:233–238` argument list, minus `--effort`, with `--model {model}`.
  - **cs:** bare, plus:
    - `--mcp-config /opt/sab/mcp.json`, holding `codescout start` with `manifest["mcp_env"]` and the URL keys rewritten to the study URLs
    - `--plugin-dir /opt/plugins/codescout-companion`
  - **full:** cs, plus one `--plugin-dir` per `plugins_full` entry, with the frozen `CLAUDE.md` and `settings.json` copied into the run's config dir.
- `agent-cs.Dockerfile`: `FROM` the task image. It adds the pinned codescout binary on `PATH`, rust-analyzer (pinned version, recorded in the manifest), `node` and `python3`, then runs `codescout index -p /testbed` against the stack at **prepare** time. It records index wall time in `index-cost.json`. The `bare` arm uses the **same image**, so the bytes the agent sees are the same. Only the flags differ.

- [ ] **Step 1: Failing tests:**
  - `test_bare_args_equal_codex_args_without_effort`
  - `test_no_arm_passes_effort`
  - `test_cs_is_bare_plus_mcp_and_companion_only`
  - `test_full_is_superset_of_cs`
  - `test_mcp_env_urls_are_rewritten_to_study_services`: no `127.0.0.1` survives.
  - `test_resource_flags_identical_across_arms`
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement `arm_invocation`. Write the Dockerfile.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Live control (no model call).** In a `cs` container, run each companion hook's command with a sample payload. Run `codescout constitution-check` and `node --version`, and check that `rust-analyzer --version` exits 0. **Verify whether `--tools Bash,Read,…` hides MCP tools.** The help text does not settle it. Run `claude -p` with no model call (`--help` alone is not enough), or else carry this into Task 6's smoke run. Record the answer in `controls/tools-flag.json`. If `--tools` hides MCP tools, add the codescout tool names to the `cs` list, and note it in the spec as an arm difference.
- [ ] **Step 6: Commit** `feat(setup-ab): arm invocations and the shared agent image`

### Task 5: Leakage scanner — `leak_scan.py`

**Files:** Create `tools/setup-ab-eval/leak_scan.py`, `tools/setup-ab-eval/test_leak_scan.py`

**Interfaces:**
- Produces: `scan(roots: list[Path], qdrant_dump: Path | None, needles: list[str], n: int = 8) -> list[Hit]`, with `Hit = NamedTuple(path, needle, offset)`. Needles are hidden-test function names (whole-word) and 8-token shingles of the gold patch's added lines.
- Produces: `needles_for(task: dict) -> list[str]`, built from S5 `f2p.names` and `hidden.extra`, or from the SWE-bench `test_patch` and `patch`.

- [ ] **Step 1: Failing tests:**
  - `test_planted_needle_is_found_in_file_bytes`
  - `test_planted_needle_is_found_in_a_qdrant_payload_dump`
  - `test_needle_in_a_file_name_is_found`
  - `test_clean_tree_returns_no_hits`
  - `test_short_common_lines_are_not_needles`: a gold line like `}` or `Ok(())` is never a shingle.
- [ ] **Step 2:** Run. Expected: FAIL. **Step 3:** Implement. **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Commit** `feat(setup-ab): leakage scanner over tree, index payloads and config copy`

### Task 6: Arm-integrity checker — `integrity.py`

**Files:** Create `tools/setup-ab-eval/integrity.py`, `tools/setup-ab-eval/test_integrity.py`, and `tools/setup-ab-eval/fixtures/*.jsonl` (redacted excerpts of real transcripts)

**Interfaces:**
- Produces: `check(arm: str, transcript: Path, stdout_json: dict) -> Verdict`, with `Verdict = NamedTuple(ok: bool, reasons: list[str], codescout_calls: int, native_calls: int, hook_events: int)`.

- [ ] **Step 1: Find the markers.** In one existing local transcript from a session with the companion plugin active, find these records and note their exact `type` and field names in the module docstring:
  - the init tool list
  - an `mcp__codescout__*` tool call
  - a hook output record (`[cs-hint]` or a SessionStart hook)
  
  Cut redacted fixtures from it, plus a `bare`-shaped fixture. Do not guess the shapes.
- [ ] **Step 2: Failing tests:**
  - `test_cs_without_codescout_tools_in_init_is_invalid`
  - `test_cs_with_tools_but_zero_calls_is_invalid`
  - `test_cs_without_hook_evidence_is_invalid`
  - `test_bare_with_any_mcp_tool_is_invalid`
  - `test_full_without_superpowers_skills_is_invalid`
  - `test_valid_cs_counts_calls`
- [ ] **Step 3:** Run. Expected: FAIL. **Step 4:** Implement. **Step 5:** Run. Expected: PASS.
- [ ] **Step 6: Commit** `feat(setup-ab): arm-integrity checker; an arm that lost its setup is invalid, not scored`

### Task 7: Task assembly and selection — `tasks.py`, `select_tasks.py`

**Files:** Create `tools/setup-ab-eval/tasks.py`, `tools/setup-ab-eval/select_tasks.py`, `tools/setup-ab-eval/test_tasks.py`, `tools/setup-ab-eval/selection-rule.json`

**Interfaces:**
- Consumes: S5 `build-task.mjs`, invoked through `node`. The source is not modified. The issue file is added through the task's `scrub.removePaths` in a **copied** task definition. Codex `pilot.prepare` handles the foreign tasks.
- Produces:
  - `build_private(task_dir: Path, out: Path) -> TaskBundle`
  - `build_foreign(record: dict, out: Path) -> TaskBundle`
  - `TaskBundle = NamedTuple(id, image, prompt_path, needles, grade: Callable[[str, str], dict])`
- Produces: `select(candidates: list[dict], rule: dict) -> list[str]`. The rule reads only `repo`, `gold_files` and `gold_added_lines`. **The rule is written and committed before any candidate's outcome is read.**

- [ ] **Step 1: Failing tests:**
  - `test_issue_file_is_absent_from_the_built_tree`
  - `test_selection_ignores_every_outcome_field`: the same candidates with different `pass` or `cost` fields give the same selection.
  - `test_codex_development_tasks_are_never_selected`: the seven ids from the spec § 2.
  - `test_grader_control_base_fails_and_gold_passes`, behind `skipUnless(docker)`.
- [ ] **Step 2:** Run. Expected: FAIL. **Step 3:** Implement. **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Live, no model call.** Build every selected task. Run the leakage scan (Task 5) on each tree, its `.codescout/` and its Qdrant template dump. Every task must report 0 hits, plus 1 hit for the planted control. Run the grader controls. Save to `setup-ab-data/controls/tasks.json`.
- [ ] **Step 6: Commit** `feat(setup-ab): task assembly, metadata-only selection, issue-file removal`

### Task 8: Runner — `run.py`

**Files:** Create `tools/setup-ab-eval/run.py`, `tools/setup-ab-eval/test_run.py`

**Interfaces:**
- Consumes: Tasks 1 to 7.
- Produces: `run_batch(plan: list[RunSpec], budget: Budget, data: Path, dry_run: bool) -> None`.
  - `RunSpec = (task_id, model, arm, rep)`
  - `Budget = (cap_usd, spent_usd, per_call_usd, reserve_usd)`
  - Journal rows go to `data/rows.jsonl`. Each row holds: `runId`, `taskId`, `model`, `arm`, `rep`, `sessionId`, `costUsd`, `wallMs`, `numTurns`, `usage`, `effectiveEffort`, `grade`, `integrity` and `outcome`.
  - `outcome` is one of `scored`, `invalid-arm`, `plan-limit`, `infra-oom`, `infra-other` or `unknown-spend`.
  - `make_order(tasks, models, seed) -> list[RunSpec]` builds the order: per task and model, `bare`×2, `cs`×1 and `full`×1, in a seeded random order.

- [ ] **Step 1: Failing tests** (fake docker and fake CLI):
  - `test_admission_refuses_when_spent_plus_reserve_exceeds_cap`
  - `test_plan_limit_text_is_neutral_and_stops_the_batch`: reuse the prefixes from `claude-plugins/effort-steering/spikes/plan-limit.mjs`, called through `node`.
  - `test_oom_kill_is_infra_not_a_failure`: container `OOMKilled=true`.
  - `test_invalid_arm_is_charged_and_not_scored`
  - `test_effort_mismatch_across_arms_of_a_task_marks_the_task_invalid`
  - `test_no_automatic_retry_after_any_outcome`
  - `test_pending_row_is_written_before_the_cli_call`, so that an interruption leaves `unknown-spend`, never a silent gap.
  - `test_cleanup_removes_only_sab_objects`
- [ ] **Step 2:** Run. Expected: FAIL. **Step 3:** Implement. **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Dry run** of the full Stage 1 order with `dry_run=True`. Expected: 12 run specs, no CLI invocation, and cleanup that leaves `docker ps` with no `sab-` object.
- [ ] **Step 6: Commit** `feat(setup-ab): sequential runner with admission, neutral plan-limit and infra outcomes`

### Task 9: Analysis and decision rule — `analyze.py`

**Files:** Create `tools/setup-ab-eval/analyze.py`, `tools/setup-ab-eval/test_analyze.py`

**Interfaces:**
- Consumes: `rows.jsonl` (Task 8).
- Produces: `decide(rows: list[dict], model: str, comparison: tuple[str,str]=("cs","bare"), registered_tasks: int, seed: int) -> Decision`.
  - `Decision = (outcome, pass_counts, cost_ratio_gmean, ci90, floor, n_valid_tasks)`
  - The outcomes and conditions are copied verbatim from spec § 6.
  - The unit is the task. The two `bare` reps are averaged for cost. The task passes for `bare` when the mean pass rate is ≥ 0.5, and the floor is the spread between the reps.
- Produces: `secondary(rows) -> dict`, reported and never gated. Per arm and model it gives the medians of `inputTokens`, `outputTokens`, `numTurns` and `wallMs`, the share of runs whose Bash commands compiled all targets (`--all-targets`, `--tests` or a plain `cargo test`), the count of edited files outside the gold patch's files, and `codescout_calls / (codescout_calls + native_calls)` from Task 6.

- [ ] **Step 1: Failing tests,** one synthetic row set per outcome:
  - `test_promising`
  - `test_no_demonstrated_advantage`
  - `test_worse_by_pass_rate`
  - `test_worse_by_cost`
  - `test_inconclusive_when_fewer_valid_tasks_than_registered`
  - `test_inconclusive_when_difference_inside_floor`
  - `test_invalid_arm_rows_never_enter_the_statistic`
  - `test_full_vs_bare_is_labelled_setup_not_codescout`
  - `test_bootstrap_is_paired_by_task_and_seeded`
- [ ] **Step 2:** Run. Expected: FAIL. **Step 3:** Implement the paired bootstrap over tasks (10,000 resamples) and the geometric-mean ratio. **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Commit** `feat(setup-ab): pre-registered decision rule with paired bootstrap over tasks`

### Task 10: Registration and the Stage 1 launch (paid; requires an explicit operator go)

**Files:** Create `tools/setup-ab-eval/stage1-registration.json` and `docs/research/2026-10-27-setup-ab-stage1.md`

- [ ] **Step 1:** Resolve spec § 9 items 2 to 4. **Ask the operator** for: Opus 5.5's training cutoff against the private task dates, the profile to freeze, and the per-call `--max-budget-usd` (Codex used `1.25`. Opus is unmeasured).
- [ ] **Step 2: Freeze.** Write `stage1-registration.json` with:
  - SHA-256 of every file in `tools/setup-ab-eval/`, the manifest, the images (IDs), the Stage 1 order (2 development tasks × 3 arms × 2 models, 1 rep), `$25` cap, the seed, and the CLI and codescout versions.
  
  Commit it **before** the first model call.
- [ ] **Step 3: Full gate.** Run `python3 -m unittest discover -s tools/setup-ab-eval -p 'test_*.py'` and `./scripts/gate.sh`. Expected: all pass. Report the output verbatim.
- [ ] **Step 4: Stop and ask the operator for the go.** Spending starts only on an explicit yes.
- [ ] **Step 5:** Run Stage 1. Record per-run cost per arm and model, integrity verdicts, and every non-`scored` outcome. Development-task outcomes are **never** scored or quoted.
- [ ] **Step 6:** Write `docs/research/2026-10-27-setup-ab-stage1.md`:
  - the costs
  - the projected Stage 3 size under $200 minus spend, with a 25% reserve
  - the open choice (fewer tasks or one model) if about 12 tasks per model do not fit
- [ ] **Step 7: Commit** `docs(research): setup A/B Stage 1 calibration — costs, integrity, projected Stage 3 size`
