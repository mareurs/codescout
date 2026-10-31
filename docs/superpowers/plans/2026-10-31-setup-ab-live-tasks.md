# Live Rust Tasks: Loader and Control Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Load SWE-bench-Live MultiLang Rust records, select the post-cutoff candidates by a metadata-only rule, and run a no-model control gate that says which tasks are usable.

**Architecture:** Three small modules with a pure core. `multilang_records.py` loads and selects. `multilang_grade.py` parses test logs (the dataset's parser runs in a subprocess) and decides verdicts. `multilang_control.py` drives Docker: reset the image tree to `base_commit`, apply patches, run the tests, and write evidence. Docker is faked in unit tests.

**Tech Stack:** Python 3.14 stdlib only, `unittest`, the Docker CLI through `pilot.docker`.

**Spec:** `docs/superpowers/specs/2026-10-31-setup-ab-live-tasks-design.md` (extends `2026-10-27-codescout-setup-ab-design.md`).

## Global Constraints

- Code in `tools/setup-ab-eval/`; stdlib only; tests are `test_multilang_*.py`, run with `python3 -B -m unittest discover -s tools/setup-ab-eval -p 'test_*.py'`.
- Do not edit `run.py`, `arms.py`, `freeze_stage2.py`, `registration.py`, `tasks.py` or any frozen registration.
- Docker objects made here are named `sab-ml-*`. Never touch `codescout-*`, `pbr27-*` or `rae-*` containers or volumes. Never create Docker volumes.
- Raw evidence goes under `/home/marius/work/claude/setup-ab-data/live/`, never `/tmp` and never git.
- No model call, no spend. Never print a token or credential.
- Selection reads metadata only (repo, files, added lines, creation date, counts). It never reads an outcome.
- Eligible records have `created_at >= 2026-07-01` (the models' cutoff is June 2026).
- Reuse: `pilot.docker(*args, data=None, timeout=..., check=...)` returns an object with `.stdout` and `.returncode`; `select_tasks.draw_key(rule, task_id)` for the draw order.

## Review Focus

- **Hostile or hanging parser.** `log_parser` is dataset code. A parser that loops, raises, prints garbage or returns non-strings must give a failed grade, never a pass and never a hang.
- **Image holds the answer.** The raw image tree carries the gold fix, old test output and extra edits. After reset the tree must equal `base_commit` and be clean, or the control raises.
- **Unpassable task.** A task whose expected tests do not all pass with the gold patch and test patch (probe: 71 of 330) must be reported `ok: false`, never `true`.
- **Disk.** Images are about 14 GB each. A pull below 40 GB free must refuse. An image the control pulled must be removed afterwards, and an image that was already present must stay.
- **Odd records.** Empty `FAIL_TO_PASS`, an empty `test_cmds`, a working directory that is `/testbed` or `/testbed/repo`, and a huge log must each give a clear refusal or a bounded result, not a crash.

---

### Task 1: Records, rule and selection

**Files:**
- Create: `tools/setup-ab-eval/multilang_records.py`, `tools/setup-ab-eval/multilang-selection-rule.json`, `tools/setup-ab-eval/fixtures/multilang/records-mini.jsonl`
- Test: `tools/setup-ab-eval/test_multilang_records.py`

**Interfaces:**
- Produces:
  - `REQUIRED = ("instance_id","repo","base_commit","created_at","patch","test_patch","FAIL_TO_PASS","PASS_TO_PASS","test_cmds","print_cmds","log_parser","docker_image")`
  - `load_records(path: Path, expected_sha256: str | None = None) -> list[dict]` raises `ValueError` on a hash mismatch, a missing required key, or a duplicate `instance_id`.
  - `candidate(record: dict) -> dict` with keys `id`, `repo`, `gold_files` (list of paths from `patch`), `gold_added_lines` (int), `created_at` (str), `expected_tests` (int, `len(F2P)+len(P2P)`).
  - `eligible(candidate: dict, rule: dict, *, ignore_size: bool = False) -> bool`. With `ignore_size=True` the files/lines hardness test and `max_added_lines` are skipped; the cutoff and the `require` fields are always enforced.
  - `select_eligible(records: list[dict], rule: dict) -> list[str]` returns instance ids in ascending `select_tasks.draw_key` order.
- Rule file `multilang-selection-rule.json` keys: `rule`, `version`, `registered` (text: before any control or outcome was read), `created_at_min` = `"2026-07-01"`, `min_files` = 2, `min_added_lines` = 40 (a task is hard enough when `gold_files >= min_files` or `gold_added_lines >= min_added_lines`), `max_added_lines` = 600, `require` = `["FAIL_TO_PASS","test_cmds","log_parser"]` (each non-empty), `draw` = `{"salt": "setup-ab-live-20261031"}`.

- [ ] **Step 1: Write the failing tests** in `test_multilang_records.py`:
  `test_load_records_checks_sha256`, `test_load_records_rejects_missing_key_and_duplicate_id`, `test_candidate_counts_files_and_added_lines` (a patch with 2 files and 5 `+` lines, `+++` headers not counted), `test_eligible_rejects_before_cutoff` (`2026-06-30T23:59:59Z` false, `2026-07-01T00:00:00Z` true), `test_eligible_size_is_or_of_files_and_lines`, `test_eligible_rejects_over_max_lines`, `test_eligible_requires_nonempty_fields`, `test_eligible_ignore_size_still_enforces_cutoff_and_required_fields`, `test_select_eligible_order_is_draw_key_order_and_ignores_unknown_fields` (adds a `pass` field to records and asserts the result does not change).
- [ ] **Step 2: Run them to see them fail** (`ImportError`).
- [ ] **Step 3: Implement** `multilang_records.py` and the rule file. `candidate` projects to the keys above before `eligible` runs, so no other field can reach the rule. Create `fixtures/multilang/records-mini.jsonl` with 4 small hand-made records covering the cases (use the real obscura-426 record shape, with trimmed patches).
- [ ] **Step 4: Run the tests; all pass.**
- [ ] **Step 5: Commit** `feat(setup-ab): live Rust records loader and metadata-only selection rule`.

---

### Task 2: Parser sandbox and verdicts

**Files:**
- Create: `tools/setup-ab-eval/multilang_grade.py`, `tools/setup-ab-eval/fixtures/multilang/obscura-426.json` (keys `FAIL_TO_PASS`, `PASS_TO_PASS` trimmed to 5 names, `log_parser`), `tools/setup-ab-eval/fixtures/multilang/obscura-426-gold.log` (the first 60 lines of `/home/marius/work/claude/setup-ab-data/live/probe-obscura-426/gold-and-test.log` that include both ok and failed tests)
- Test: `tools/setup-ab-eval/test_multilang_grade.py`

**Interfaces:**
- Produces:
  - `class ParserError(ValueError)`
  - `run_parser(parser_src: str, log: str, timeout: float = 30.0) -> dict[str, str]`: runs `sys.executable -I -c <driver>` in a subprocess; the driver reads `{"src": ..., "log": ...}` JSON from stdin, executes `src`, calls `parser(log)`, prints the dict as JSON. Raises `ParserError` on timeout, a non-zero exit, output that is not JSON, a non-dict, or a non-string key or value.
  - `verdict(statuses: dict[str, str], f2p: list[str], p2p: list[str]) -> dict` returns `{"pass": bool, "expected": int, "not_passing": int, "missing": int, "examples": [up to 10 names]}`. `pass` is true only when every expected name has status exactly `"pass"` and `expected > 0`.
  - `control_verdict(base: dict[str, str], gold: dict[str, str], f2p: list[str], p2p: list[str]) -> dict` returns `{"ok": bool, "reasons": [str], "f2p_passing_at_base": int, "expected_not_passing_at_gold": int, "expected": int}`. `ok` is true only when no `FAIL_TO_PASS` name has status `"pass"` in `base` and every `FAIL_TO_PASS` and `PASS_TO_PASS` name has `"pass"` in `gold`. Reasons are short fixed phrases (`"fail-to-pass test passes at base"`, `"expected tests do not pass with the gold patch"`, `"no fail-to-pass tests"`).

- [ ] **Step 1: Write the failing tests**:
  `test_run_parser_on_real_obscura_fixture` (parses the fixture log; the fixture target test is `pass` or `fail` as in the log), `test_run_parser_times_out_on_infinite_loop` (timeout 1 s, raises `ParserError`), `test_run_parser_rejects_exception_and_garbage_output_and_non_dict_and_non_string_values`, `test_run_parser_cannot_read_parent_environment` (the driver runs with `-I`; a parser that prints `os.environ` secrets is not asserted, but the subprocess is started with a minimal environment `{"PATH": ...}`; assert a parser returning `{os.environ.get("HOME","none"): "pass"}` gives `"none"`), `test_verdict_requires_every_expected_name_to_pass`, `test_verdict_empty_expected_is_not_a_pass`, `test_control_ok_when_f2p_fails_at_base_and_all_pass_at_gold`, `test_control_rejects_f2p_passing_at_base`, `test_control_rejects_71_of_330_not_passing_at_gold` (synthetic: 330 expected, 71 `fail` at gold, `ok` false and `expected_not_passing_at_gold == 71`).
- [ ] **Step 2: Run them to see them fail.**
- [ ] **Step 3: Implement** `multilang_grade.py`. The subprocess gets a minimal environment, `stdin` data, and the timeout; kill it on timeout.
- [ ] **Step 4: Run the tests; all pass.**
- [ ] **Step 5: Commit** `feat(setup-ab): live Rust log parser sandbox and control verdicts`.

---

### Task 3: Container driver

**Files:**
- Create: `tools/setup-ab-eval/multilang_control.py` (driver functions first; the CLI comes in Task 4)
- Test: `tools/setup-ab-eval/test_multilang_control.py` (a fake `pilot.docker` records calls and returns scripted outputs)

**Interfaces:**
- Consumes: `multilang_grade.run_parser`, `multilang_grade.control_verdict`, `pilot.docker`.
- Produces:
  - `class ControlError(RuntimeError)`
  - `CONTAINER_PREFIX = "sab-ml-ctl-"`
  - `detect_workdir(container: str) -> str` returns `/testbed/repo` when `/testbed/repo/.git` exists, else `/testbed` when `/testbed/.git` exists; raises `ControlError` otherwise.
  - `reset_to_base(container: str, workdir: str, base_commit: str) -> dict` records `{"dirty_before": [up to 50 `git status --porcelain` lines]}`, then runs `git reset --hard -q <base_commit>`, `git clean -fdq -e target` and `rm -rf <workdir>/reports`, then checks `git rev-parse HEAD` equals `base_commit` and `git status --porcelain` is empty; raises `ControlError` otherwise.
  - `run_tests(container: str, workdir: str, record: dict, timeout: int = 3600) -> str` runs each command of `record["test_cmds"]` with `bash -o pipefail -c` in `workdir` (exit codes ignored), then runs each command of `record["print_cmds"]` and returns their stdout joined; the log is truncated to 64 MiB.
  - `control(record: dict, *, container: str, timeout: int = 3600) -> tuple[dict, str, str]` returns `(evidence, base_log, gold_log)`. Steps: start the container (`docker run -d --name <container> --network none --cpus 4 --memory 24g --memory-swap 24g <image> sleep infinity`), detect workdir, reset, apply `test_patch` with `git apply -`, run tests, parse; apply `patch`, run, parse; `evidence` holds `id`, `image`, `workdir`, `dirty_before`, `base` and `gold` counts per status, `verdict` (from `control_verdict`), and `wall_seconds`. A `ControlError` or `ParserError` becomes `verdict = {"ok": False, "reasons": ["control error: <message>"]}`. The container is always removed with `docker rm -f` in a `finally`, and the name must start with `CONTAINER_PREFIX` or `control` raises before starting.

- [ ] **Step 1: Write the failing tests** with the fake docker:
  `test_reset_to_base_raises_when_head_differs_after_reset`, `test_reset_to_base_raises_when_tree_still_dirty`, `test_reset_records_dirty_before`, `test_detect_workdir_prefers_testbed_repo`, `test_detect_workdir_raises_without_git`, `test_run_tests_ignores_test_exit_code_and_returns_print_output`, `test_control_ok_path_orders_calls` (reset, then test patch, then run, then gold patch, then run, and `docker rm -f` last), `test_control_always_removes_container_on_error`, `test_control_refuses_container_name_without_prefix`, `test_control_marks_parser_error_as_not_ok`.
- [ ] **Step 2: Run them to see them fail.**
- [ ] **Step 3: Implement** the functions above.
- [ ] **Step 4: Run the tests; all pass.**
- [ ] **Step 5: Commit** `feat(setup-ab): live Rust container control (reset to base, patches, tests)`.

---

### Task 4: Control CLI, disk guard and a live check

**Files:**
- Modify: `tools/setup-ab-eval/multilang_control.py` (add the CLI)
- Test: `tools/setup-ab-eval/test_multilang_control.py` (add CLI tests)

**Interfaces:**
- Consumes: Tasks 1 to 3.
- Produces:
  - `MIN_FREE_GB = 40`
  - `free_gb(path: str = "/") -> float`
  - `ensure_image(image: str) -> bool` returns `True` when this call pulled the image. It returns `False` when the image was already present. It raises `ControlError` when free space is below `MIN_FREE_GB` before a pull.
  - `run_one(record: dict, out_dir: Path, *, timeout: int = 3600) -> dict` writes `<out_dir>/<id>/control.json`, `base.log`, `gold.log`; removes the image with `docker rmi <image>` (no force) only if `ensure_image` returned `True`; returns the evidence dict.
  - `main(argv: list[str] | None = None) -> int` with `--records`, `--rule`, `--out`, `--ids` (optional, space separated), `--limit` (optional). Without `--ids` it selects with `select_eligible`. With `--ids` it takes exactly those ids in the given order, checks each with `eligible(..., ignore_size=True)` (the size filter is bypassed, the cutoff and required fields are not; a refused id prints `REFUSED <id> <reason>` and is skipped), skips ids whose `control.json` exists (resume), prints one line per task (`id ok|REJECT reasons`), and prints a final count. It exits 0 when it ran, whatever the verdicts.

- [ ] **Step 1: Write the failing tests**: `test_ensure_image_refuses_pull_below_min_free`, `test_ensure_image_does_not_pull_or_remove_a_present_image`, `test_run_one_removes_only_an_image_it_pulled`, `test_main_skips_ids_with_existing_control_json`, `test_main_ids_overrides_selection_order_but_not_the_cutoff` (an `--ids` entry created before the cutoff is refused with a message), `test_main_never_imports_run_or_arms` (assert `run`, `arms` and `registration` are not in `sys.modules` after importing `multilang_control`).
- [ ] **Step 2: Run them to see them fail.**
- [ ] **Step 3: Implement** the CLI.
- [ ] **Step 4: Run the whole suite**: `python3 -B -m unittest discover -s tools/setup-ab-eval -p 'test_*.py'`. Expected: all pass, including the 522 earlier tests.
- [ ] **Step 5: Live check on the real image** (needs the image `starryzhang/sweb.eval.x86_64.h4ckf0r0day_1776_obscura-426`, already pulled): run `python3 -B tools/setup-ab-eval/multilang_control.py --records /home/marius/work/claude/setup-ab-data/live/multilang-rust.jsonl --rule tools/setup-ab-eval/multilang-selection-rule.json --out /home/marius/work/claude/setup-ab-data/live/controls --ids h4ckf0r0day__obscura-426`. Expected: the task is accepted through `--ids` (it is below the size filter, so a run without `--ids` would not select it), the line reads `REJECT` with the reason `expected tests do not pass with the gold patch`, `control.json` records 71 not passing, and `docker ps -a` shows no `sab-ml-ctl-*` container left. The pre-existing image is not removed.
- [ ] **Step 6: Commit** `feat(setup-ab): live Rust control CLI with disk guard`.
