# Fresh-task builder and control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `tools/setup-ab-eval/fresh_build.py` turns (repository, PR, issue numbers, base image) into a live-schema record, a local image and a strict-gate `control.json`, with no model call.

**Architecture:** Pure helpers (diff split, base pick, set derivation, spec rule, record assembly) are tested without docker. Fetch, image build and gate take an injected command runner, so tests use fakes and the real run only needs docker, git and gh. The gate imports `multilang_control` helpers and `multilang_grade.control_verdict` read-only.

**Tech Stack:** Python 3 stdlib, `unittest`, docker CLI, git, gh.

**Spec:** `docs/superpowers/specs/2026-11-04-setup-ab-fresh-builder-design.md` (the spec is binding; this plan argues from it). Evidence of every fact: memory `research/setup-ab-eval-fresh-builder-pilot`, `/home/marius/work/claude/setup-ab-data/fresh-pilot-2026-11-04/`; the throwaway driver `/home/marius/.claude-kat/jobs/fbe38184/tmp/pilot_control.py` is a working reference for Task 4 (read it, do not import it).

## Global Constraints

- Stdlib only. Do not modify `run.py`, `arms.py`, `multilang_control.py`, `multilang_tasks.py`, `multilang_grade.py`, `pilot.py` or any frozen registration; import from them.
- Run tests with `cd tools/setup-ab-eval && python3 -B -m unittest test_fresh_build -v`. Tests never call docker, git, gh or the network.
- Study objects only: containers `sab-fresh-build-<id>` and `sab-fresh-ctl-<id>`, image tag `sab-fresh-<owner>__<repo>-<pr>:b<BUILDER_VERSION>`. Never touch `codescout-*` objects or `pbr27-*`/`rae-*`; never create docker volumes; never delete an image other than one the builder itself created.
- Gate container flags: `--network none --cpus 4 --memory 8g --memory-swap 8g --pids-limit 4096 --security-opt no-new-privileges`.
- The build step runs WITH network and `-e CARGO_NET_OFFLINE=false` on that `docker exec` only; the committed image config keeps `CARGO_NET_OFFLINE=true`.
- Refuse to start a build under 60 GB free disk (`MIN_FREE_GB = 60`).
- `gh` and `git` are run with `GITHUB_TOKEN` removed from the environment (the shell's value is invalid). Never print or log a token or an auth header.
- Never alter line endings in a diff, a patch or a log.
- Exact test cmd for every step is the record's own `test_cmds`/`rebuild_cmds`/`print_cmds`, copied from a template record.

## Review Focus

- A build whose log has zero `Compiling` lines must fail (stale binaries: archive mtimes are older than the warm `target/`).
- A failed command's real exit code must never read as success (the pilot lost it in a `| tee` wrapper and reported a 0-second "build").
- An F2P test with no spec verdict is an error, never a silent keep; a task with zero kept F2P tests is unusable.
- A merge commit has two parents: the base is the first parent, and the diff base..merge must equal the PR's changed-file list or the builder refuses.
- CRLF blobs: splitting and joining a diff must be byte-faithful.
- The build and control containers are removed on every failure path, and a failed step never leaves a half-committed image tag.

---

### Task 1: Pure helpers

**Files:**
- Create: `tools/setup-ab-eval/fresh_build.py`
- Test: `tools/setup-ab-eval/test_fresh_build.py`

**Interfaces:**
- Produces (all module level in `fresh_build.py`):
  - `BUILDER_VERSION = "1"`, `MIN_FREE_GB = 60`, `CUTOFF = "2026-07-01"`
  - `TEST_PATH_RE` (compiled): `(^|/)(tests?|testdata|fixtures)/|_tests?\.rs$|(^|/)tests?\.rs$`
  - `is_test_path(path: str) -> bool`
  - `diff_files(diff: str) -> list[str]` (the b-side path of every `diff --git a/X b/Y` block, in order)
  - `split_diff(diff: str) -> tuple[str, str]` = `(patch, test_patch)`; blocks split before each line starting `diff --git `; byte-faithful; raises `ValueError` if non-empty text does not start with `diff --git `
  - `pick_base(parents: list[str]) -> str` (first parent; `ValueError` on empty)
  - `derive_sets(base: dict[str, str], gold: dict[str, str]) -> tuple[list[str], list[str]]` = `(f2p, p2p)`, sorted; F2P = gold `"pass"` and base not `"pass"` (failing or absent); P2P = pass in both
  - `spec_key(problem_statement: str, test_patch: str, patch: str) -> str` (sha256 hex of the three joined with a NUL byte)
  - `apply_spec_verdicts(f2p: list[str], verdicts: dict[str, str]) -> tuple[list[str], list[str]]` = `(kept, dropped)`; `KeyError` naming every test without a verdict; `ValueError` for a value other than `"specified"` or `"unspecified"`
  - `cache_key(repo: str, upstream_base: str, base_image_id: str, version: str = BUILDER_VERSION) -> str` (sha256 hex)
  - `build_record(*, repo, pr, issue_numbers, created_at, problem_statement, base_commit, upstream_base_commit, patch, test_patch, f2p, p2p, dropped_f2p, template, docker_image) -> dict`: `instance_id` is `<owner>__<name>-<pr>`; carries the keys of a `multilang-rust.jsonl` record (`FAIL_TO_PASS`, `PASS_TO_PASS`, `all_hints_text`, `base_commit`, `commit_url`, `commit_urls`, `created_at`, `docker_image`, `hints_text`, `instance_id`, `issue_numbers`, `log_parser`, `patch`, `print_cmds`, `problem_statement`, `pull_number`, `rebuild_cmds`, `repo`, `test_cmds`, `test_patch`) with `test_cmds`, `rebuild_cmds`, `print_cmds`, `log_parser` copied from `template` (lists, not strings), hints empty, plus `upstream_base_commit`, `dropped_f2p`, `spec_rule: "f2p kept only if the issue states the asserted behaviour"` and `builder_version`.

- [ ] **Step 1: Write failing tests** in `test_fresh_build.py` (one `unittest.TestCase` per helper): `is_test_path` on `crates/hir-ty/src/tests/simple.rs`, `tests/x.rs`, `src/lib_tests.rs`, `src/lib.rs` (false), `testdata/a.json`; `split_diff` on a three-file diff (one test, two source), on a CRLF diff (`b"\r\n"` content survives and `split + join` equals the input), on a rename block, and on garbage (`ValueError`); `diff_files` order; `pick_base` with one and two parents; `derive_sets` with a test absent at base counted as F2P and a base-pass/gold-fail test in neither; `apply_spec_verdicts` with a missing verdict (`KeyError` names it) and a bad value; `spec_key` changes when any one input changes; `build_record` keys equal the listed set plus the four extra keys and `instance_id == "rust-lang__rust-analyzer-23096"`.
- [ ] **Step 2: Run** the test command; expected: `ImportError`/`AttributeError`, failing.
- [ ] **Step 3: Implement** the interfaces above in `fresh_build.py` with a module docstring in the style of `multilang_control.py` (what it is, what it imports, what it never touches).
- [ ] **Step 4: Run** the test command; expected: all pass, no warnings.
- [ ] **Step 5: Commit** `git add tools/setup-ab-eval/fresh_build.py tools/setup-ab-eval/test_fresh_build.py && git commit -m "feat(setup-ab): fresh_build pure helpers"`.

### Task 2: Upstream fetch

**Files:**
- Modify: `tools/setup-ab-eval/fresh_build.py`
- Test: `tools/setup-ab-eval/test_fresh_build.py`

**Interfaces:**
- Consumes: Task 1 `pick_base`, `diff_files`, `CUTOFF`.
- Produces: `class FetchError(RuntimeError)`; `fetch_pr(repo: str, pr: int, issue_numbers: list[int], dest: Path, *, run=subprocess.run, cutoff: str = CUTOFF) -> dict` returning keys `merge_sha`, `parents`, `upstream_base`, `created_at` (the PR's), `merged_at`, `title`, `problem_statement`, `diff`, `pr_files`, `issue_created_at`.

- [ ] **Step 1: Write failing tests** with a `FakeRun` that maps the argv tuple (after stripping `gh api` / `git`) to canned stdout and records every call and the env it received: happy path (two-parent merge commit: base is `parents[0]`, `problem_statement` is each issue's `title`, a newline, then `body`, issues joined by a blank line); PR not merged (refuse, `FetchError`); merged or issue created before `cutoff` (refuse); diff files differ from the PR's file list (refuse, message names the difference); `GITHUB_TOKEN` absent from the env of every call; no argv contains the string `token`.
- [ ] **Step 2: Run**; expected: failing.
- [ ] **Step 3: Implement `fetch_pr`.** Calls, in order: `gh api repos/{repo}/pulls/{pr}`; `gh api repos/{repo}/commits/{merge_sha}`; `gh api repos/{repo}/pulls/{pr}/files --paginate`; one `gh api repos/{repo}/issues/{n}` per issue; `git clone --filter=blob:none --no-checkout https://github.com/{repo} {dest}/src` (skipped if `{dest}/src/.git` exists, then `git -C {dest}/src fetch origin {merge_sha}` instead); `git -C {dest}/src diff --binary {base} {merge_sha}`. Parse JSON with `json.loads`; non-zero exit raises `FetchError` carrying only the last 300 characters of stderr.
- [ ] **Step 4: Run**; expected: pass.
- [ ] **Step 5: Commit** `feat(setup-ab): fresh_build upstream fetch`.

### Task 3: Image build

**Files:**
- Modify: `tools/setup-ab-eval/fresh_build.py`
- Test: `tools/setup-ab-eval/test_fresh_build.py`

**Interfaces:**
- Consumes: `pilot.docker` (default `docker`), `MIN_FREE_GB`, `BUILDER_VERSION`.
- Produces: `class BuildError(RuntimeError)`; `image_tag(repo: str, pr: int) -> str`; `build_image(*, base_image: str, repo: str, pr: int, upstream_base: str, tree_tar: Path, evidence_dir: Path, docker=pilot.docker, free_gb=<callable returning GB free of the docker data root>, workdir: str = "/testbed") -> dict` returning `{"image": tag, "base_commit": <synthetic SHA>, "image_id", "size_bytes", "laps_seconds": {...}, "compiling_lines": int}`.

Behaviour, in order (the pilot's measured recipe; commands run with `docker exec` and each exit code is read from the `CompletedProcess`, never from a shell wrapper):
1. Refuse (`BuildError`) when `free_gb() < MIN_FREE_GB`, before any docker call.
2. `docker run -d --name sab-fresh-build-<id> --cpus 4 --memory 24g` from `base_image` with network (default bridge) and `sleep infinity`.
3. Delete tracked files with `git ls-files -z | xargs -0 rm -f` (as one `bash -c` inside the container), then every directory that is empty or absent from the new tree except `target` and `.git` (`find … -depth -empty -delete`, repeated until none).
4. `docker cp` the tree tarball in and extract it over `workdir`; `touch` every extracted file (not under `target/`) so cargo sees them as new.
5. `cargo metadata --format-version 1` (offline allowed to fail over to network) must exit 0 (`BuildError` otherwise).
6. Warm build `cargo test --no-run` with `-e CARGO_NET_OFFLINE=false`, output into a log in `evidence_dir`; non-zero exit raises `BuildError` carrying the log tail; fewer than one `Compiling` line raises `BuildError` ("stale binaries").
7. `git add -A` and a synthetic commit (`-c user.name=sab -c user.email=sab@invalid`) with message `synthetic base: <repo> <sha12> (parent of PR <n> merge)`; its SHA is the result's `base_commit`.
8. `docker commit` to the image tag, then in a `finally` `docker rm -f -v` the container; if any step failed, the tag is not created.

- [ ] **Step 1: Write failing tests** with a `FakeDocker` that records argv and returns scripted results: happy path asserts the order of the eight stages, `-e CARGO_NET_OFFLINE=false` only on the warm-build exec, the commit tag, and that the container is removed; low disk raises before any docker call (`docker` never called); non-zero warm build exit raises and removes the container and issues no `docker commit`; zero `Compiling` lines raises; a failing `cargo metadata` raises; a failed removal is recorded in evidence, not raised over the original error.
- [ ] **Step 2: Run**; expected: failing.
- [ ] **Step 3: Implement** `image_tag` and `build_image` as specified; `evidence_dir` receives `build.log` and `build-evidence.json`.
- [ ] **Step 4: Run**; expected: pass.
- [ ] **Step 5: Commit** `feat(setup-ab): fresh_build image build`.

### Task 4: Derive and strict gate

**Files:**
- Modify: `tools/setup-ab-eval/fresh_build.py`
- Test: `tools/setup-ab-eval/test_fresh_build.py`

**Interfaces:**
- Consumes: `multilang_control` (`detect_workdir`, `reset_to_base`, `rebuild`, `run_tests`, `_apply`, `_fresh_reports`, `_remove_container`, `ControlError`), `multilang_grade` (`run_parser`, `control_verdict`, `ParserError`), Task 1 `derive_sets`.
- Produces: `control_fresh(record: dict, *, container: str, evidence_dir: Path, memory: str = "8g", timeout: int = 5400, docker=pilot.docker) -> dict` where `record` carries `docker_image`, `base_commit`, `patch`, `test_patch`, `test_cmds`, `rebuild_cmds`, `print_cmds`, `log_parser` and no F2P/P2P; the result has `f2p`, `p2p`, `verdict` (from `control_verdict`, after the tolerant repeat below), `base`/`gold`/`gold_repeat` status counters, `laps_seconds`, `wall_seconds`, `container_removed`, and `error` when a step raised. Logs go to `evidence_dir` as `base.log`, `gold.log`, `gold-repeat.log` (and `gold-repeat-2.log`).

Behaviour: start the gate container with the Global Constraints flags (`container` must start with `sab-fresh-ctl-`); states base+test, gold+test, gold+test repeat, each from `reset_to_base` and `_fresh_reports`; derive F2P/P2P with `derive_sets(base, gold)`; an empty F2P set is `verdict.ok = False` with reason `no fail-to-pass test`; compute `control_verdict(base, gold, f2p, p2p, gold_repeat=repeat)`; **tolerant repeat**: if the only reason is that tests differ between the two gold runs and at most 3 tests differ, run a second repeat and accept those tests only if each is `"pass"` in `gold` and in the second repeat; the accepted tests and the extra run are written into the evidence; any other failure reason stands. The container is removed in a `finally`; a step error becomes `verdict = {"ok": False, "error": True, "reasons": [...]}`, never an exception.

- [ ] **Step 1: Write failing tests** with fakes for the imported helpers (monkeypatch the module attributes): happy path derives F2P/P2P and returns `ok`; empty F2P is not ok; one mismatch resolved by the second repeat is ok and recorded; a mismatch that persists is not ok; four mismatches skip the tolerant run and fail; a `ControlError` inside any state yields an `error` verdict and the container is still removed; a container name without the prefix raises `ValueError` before docker.
- [ ] **Step 2: Run**; expected: failing.
- [ ] **Step 3: Implement `control_fresh`**, following the structure of `pilot_control.py` and `multilang_control.control` (read both).
- [ ] **Step 4: Run**; expected: pass.
- [ ] **Step 5: Commit** `feat(setup-ab): fresh_build derive and strict gate`.

### Task 5: Orchestration, cache and CLI

**Files:**
- Modify: `tools/setup-ab-eval/fresh_build.py`
- Test: `tools/setup-ab-eval/test_fresh_build.py`

**Interfaces:**
- Consumes: Tasks 1 to 4, `multilang_tasks` constants are NOT imported (the data directory is a parameter).
- Produces: `run_build(*, repo, pr, issue_numbers, base_image, template_path, template_id, data: Path, spec_verdicts_path: Path | None, dry_run: bool, fetch=fetch_pr, build=build_image, control=control_fresh) -> dict` and `main(argv=None) -> int`; CLI `fresh_build.py build --repo R --pr N --issues 1,2 --base-image IMG --template-record PATH --template-id ID --data DIR [--spec-verdicts FILE] [--dry-run]`.

Behaviour: compute the cache key; if `<data>/evidence/<id>/cache-key.txt` matches and `controls/<id>/control.json` exists, return `{"status": "cached"}` without work. `--dry-run` prints the planned stages and the key, calls nothing external. Otherwise: fetch, split, build, control, write `evidence/<id>/` (patches, issue text, logs, `pending.json` with the derived F2P list and `spec_key`), then: with no verdict file, stop with `status: "awaiting-spec-verdicts"` and exit code 3 (no record written); with a verdict file keyed by `spec_key`, apply `apply_spec_verdicts`, require at least one kept F2P (else `status: "unusable"`, exit 4, nothing appended), write `controls/<id>/control.json` (keys `id`, `image`, `workdir`, `steps`, `base`, `gold`, `gold_repeat`, `verdict`, `wall_seconds`) and append the record to `records.jsonl` atomically (temp file and rename). A lock file `<data>/.lock` makes a second concurrent run refuse. Exit code 0 on success, 2 on a refusal or a failed gate.

- [ ] **Step 1: Write failing tests** with injected fakes for `fetch`, `build`, `control`: cached key skips all three; dry run calls none; no verdict file gives exit 3 and no `records.jsonl`; verdicts that drop every F2P give exit 4; verdicts that keep two of three F2P append one record whose `FAIL_TO_PASS` has the two and `dropped_f2p` the third; a failed gate verdict gives exit 2 and no record; a second run while `.lock` exists refuses; `records.jsonl` is never left half-written when the build step raises.
- [ ] **Step 2: Run**; expected: failing.
- [ ] **Step 3: Implement** `run_build` and `main`; log one JSON object per stage to stderr.
- [ ] **Step 4: Run** the test command, then the whole module's existing neighbours (`python3 -B -m unittest test_multilang_control test_multilang_grade`) to confirm nothing imported changed; expected: all pass.
- [ ] **Step 5: Commit** `feat(setup-ab): fresh_build orchestration, cache and CLI`.
