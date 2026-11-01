# Gate and CI Test Lanes on nextest — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run the gate's and CI's Rust test lanes with `cargo nextest run` (plus a `cargo test --doc` step), and optimise `sha2` in the dev profile, without weakening any guard.

**Architecture:** `scripts/gate.sh` keeps its four named lanes and their order; each test lane becomes two commands whose exits combine. A nextest presence check runs before the lease. CI's three Rust test jobs change the same way, and `tests/feature_lanes.rs` learns that `cargo nextest run` runs tests. Every file that pins the exact command text changes in the same commit as the text.

**Tech Stack:** bash, Rust 1.97 tests, Python pytest, GitHub Actions, cargo-nextest 0.9.149.

**Spec:** `docs/superpowers/specs/2026-11-01-gate-nextest-design.md`. Measurements: codescout memory `research/gate-speed-20261101`. Read both before any task.

## Global Constraints

- Lean lane, in this order: `cargo nextest run --workspace --no-default-features --no-fail-fast` then `cargo test --workspace --doc --no-default-features`. Default lane: `cargo nextest run --workspace --no-fail-fast` then `cargo test --workspace --doc`.
- A lane's exit is the nextest exit when that is non-zero, else the doctest exit. Use `;` and `if`, never `&&`, between gate commands (`docs/conventions/gate-ordering.md`, the `&&` exception).
- Missing nextest: `gate.sh` exits 2 BEFORE leasing a slot and before any lane, and prints `cargo install cargo-nextest --locked` on stderr. No fallback to libtest.
- No nextest `retries` anywhere. No `.config/nextest.toml`.
- `CLAUDE.md` must stay at or under `CLAUDE_MD_BYTE_BUDGET` (5600; the file is 5572 bytes now). Never raise the budget.
- CI pins nextest to `0.9.149`.
- Unchanged: targeted runs (`scripts/with-slot.sh cargo test …`), `scripts/mutation-probe.sh`, the `GATE SECONDS` / `GATE EXITS` line formats, lane names `LEAN` and `DEFAULT`.
- No prompt-surface change (`src/prompts/*.md`, `.codescout/system-prompt.md`): `ONBOARDING_VERSION` stays. `src/prompts/mod.rs` changes only in its test module.
- Cargo through the pool: `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo …`. Do not run `./scripts/gate.sh` before Task 4.
- Shared checkout: commit by pathspec with a `Session-Id:` trailer (`docs/conventions/shared-checkout-commit-sequence.md`). Never stash, reset or edit a peer's files.

## Review Focus

1. **A lane whose nextest is green but whose doctests are red must be red** (Task 2 test).
2. **Missing nextest takes no lease and runs no lane**, and the message names the install command (Task 2 tests, both fixtures).
3. **The version check must not count as a lane** in `tests/gate-slot.sh`: its fake `cargo` holds on its FIRST recorded call, so a recorded check would park before the lease (Task 2).
4. **`cargo nextest list --features x` must not count as tested**, while `cargo nextest run --features x` must (Task 3 fixture).
5. **Prefix trap in the order test:** `cargo nextest run --workspace` is a prefix of the lean form; the needles stay backtick-delimited (Task 2).

---

### Task 1: Optimise `sha2` in the dev profile

**Files:** Modify `Cargo.toml` (after `[profile.dev.build-override]`, ~L242).

- [ ] **Step 1: Baseline.** `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo nextest run -p codescout --lib -E 'test(=tools::config::tests::project_status_compact_shape)'`. Record the test's time (expected ≈12 s; memory says 12.0 s).
- [ ] **Step 2: Add** `[profile.dev.package.sha2]` with `opt-level = 3`, and a two-line comment: why (the test binary is ~402 MB and `build_id_of` hashes it once per process, `src/retrieval/index_state.rs`) and the measured before/after.
- [ ] **Step 3: Re-run Step 1.** Expected: well under 12 s. Write both numbers into the comment. If the time does not drop, STOP and report: the cause is then not the hash.
- [ ] **Step 4: Commit** `Cargo.toml`: `perf(build): optimise sha2 in the dev profile, the test binary hash cost ~10 s per process`.

### Task 2: The gate runs nextest, and every pin moves with it

**Files:**
- Modify: `scripts/gate.sh` (lanes ~L177-185; presence check before the `. slot-pool.sh` line ~L107; comments that name the old commands)
- Modify: `tests/test_python_runner.py` (`gate_fixture` cargo stub ~L150-163, `assert_gate_calls` ~L194-204, new tests)
- Modify: `tests/gate-slot.sh` (stub L95-107; counts L156-157)
- Modify: `CLAUDE.md` L15, `src/prompts/mod.rs` (`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` `GATE` needles ~L3238-3243 and the doc comment above it that quotes the old commands ~L3177-3195)
- Modify: `docs/conventions/gate-ordering.md` (every mention of the two old commands; one new section)

**Interfaces:** Produces the four lane command strings in Global Constraints; Task 3 and Task 4 rely on them verbatim.

- [ ] **Step 1: Failing tests (Python fixture).** In the `gate_fixture` cargo stub: answer `cargo nextest --version` first, without recording it: exit 1 when `NO_NEXTEST` is set, else 0. Append `-doc` to `lane` when the args contain `--doc`. Change `assert_gate_calls`' expected list to clippy, then the four lane commands in Global Constraints order, and expect 5 target lines. Add:
  - `test_gate_lane_is_red_when_only_its_doctests_fail`: with `FAILED_LANE=lean-doc`, the gate's output contains `LEAN=` with a non-zero value and `DEFAULT=0`, and the gate exits non-zero.
  - `test_gate_without_nextest_exits_2_before_any_lane`: with `NO_NEXTEST=1`, exit code 2, stderr contains `cargo install cargo-nextest --locked`, and `calls.txt` does not exist.
- [ ] **Step 2: Run them to see them fail:** `python3 -m pytest tests/test_python_runner.py -q`. Expected: the changed and new tests fail.
- [ ] **Step 3: Implement in `gate.sh`.** The presence check (`cargo nextest --version >/dev/null 2>&1`) before the lease, with the message from Global Constraints. The two lanes per Global Constraints, timing each lane over both of its commands. Update comments that quote the old commands.
- [ ] **Step 4: `tests/gate-slot.sh`.** In the stub, before the `echo … >> seen-$tag` line: `[ "$1 $2" = "nextest --version" ] && exit 0`. Change the `GATE_TEST_HOLD_DEFAULT` match from `"test --workspace"` to `"nextest run --workspace --no-fail-fast"`. Change the expected lane-call count at L156 from 3 to 5 and its message to "once per cargo command". Check every other count or `$*` match in the file against the new calls.
- [ ] **Step 5: Run** `python3 -m pytest tests/test_python_runner.py -q` and `bash tests/gate-slot.sh`. Expected: all pass.
- [ ] **Step 6: Pins.** In `CLAUDE.md` L15 replace `` `cargo test --workspace --no-default-features`, `cargo test --workspace` `` with `` `cargo nextest run --workspace --no-default-features`, `cargo nextest run --workspace` ``. Then `wc -c CLAUDE.md` must print ≤ 5600 (expected 5586). In `src/prompts/mod.rs` set `GATE[2]` and `GATE[3]` to the backtick-delimited new forms and update the doc comment's quoted commands and counts (re-count them in the new file; do not guess).
- [ ] **Step 7: `gate-ordering.md`.** Replace every old lane command. Add a section `## Why the test lanes run nextest, and why each has a doctest step` with: the measured numbers (libtest 405-575 s vs nextest 72-80 s for the default lib tests; libtest ~89% kernel CPU; cause not proven, see memory `research/gate-speed-20261101`), that nextest does not run doctests, `--no-fail-fast` and why, and no retries and why.
- [ ] **Step 8: Run** `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo nextest run -p codescout --lib -E 'test(claude_md)'`. Expected: the budget test and the order test pass.
- [ ] **Step 9: Commit** all files of this task in one commit: `feat(gate): run the test lanes with nextest plus a doctest step`.

### Task 3: CI runs nextest, and the feature-lane guard sees it

**Files:** Modify `.github/workflows/ci.yml` (`test` job ~L494, `test-server-stack` ~L534, `feature-check` ~L564), `tests/feature_lanes.rs` (`features_from_text` ~L95-136, `the_workflow_scan_distinguishes_check_from_test` ~L294-321), `CONTRIBUTING.md` (gate prerequisites).

- [ ] **Step 1: Failing test.** Add to the fixture in `the_workflow_scan_distinguishes_check_from_test`: `- run: cargo nextest run --features via-nextest --workspace` and `- run: cargo nextest list --features only-listed`. Assert `tested` contains `via-nextest` and does not contain `only-listed`, and `named` contains both.
- [ ] **Step 2: Run** `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo nextest run -p codescout --test feature_lanes`. Expected: the fixture test fails on `via-nextest`.
- [ ] **Step 3: Implement.** In `features_from_text`, a line counts for `only_cargo_test` when it contains `cargo test` or `cargo nextest run`. Update its doc comment.
- [ ] **Step 4: CI.** In each of the three jobs, add a `taiki-e/install-action@v2` step installing nextest 0.9.149 (check the action's README for the exact `tool:` syntax; do not guess it). `test`: `cargo nextest run --workspace ${{ matrix.config.flags }}` and `cargo test --workspace --doc ${{ matrix.config.flags }}`. `test-server-stack`: `cargo nextest run --features server-stack` and `cargo test --doc --features server-stack`. `feature-check`: `cargo nextest run --features dashboard --all-targets` (`--all-targets` already excludes doctests). Keep each job's existing comments true.
- [ ] **Step 5: Run** the Step 2 command again (the whole `feature_lanes` binary). Expected: all pass, including `every_declared_feature_has_a_lane_or_a_reason` and `the_verb_split_still_separates_the_live_lanes`.
- [ ] **Step 6: `CONTRIBUTING.md`:** one line saying the gate needs `cargo install cargo-nextest --locked`.
- [ ] **Step 7: Commit** `.github/workflows/ci.yml tests/feature_lanes.rs CONTRIBUTING.md`: `ci: run the test jobs with nextest plus a doctest step; the feature-lane guard counts nextest runs`.

### Task 4: Verify on the real gate

- [ ] **Step 1:** Run `./scripts/gate.sh` once, in the background, and record its `GATE SECONDS` and `GATE EXITS` lines and the load average at start and end.
- [ ] **Step 2:** Compare with TOTAL=2007 / LEAN=272 / DEFAULT=1517 (e090efdfb, heavy load). State that the trees and loads differ.
- [ ] **Step 3:** Append the numbers to codescout memory `research/gate-speed-20261101` (new section, do not replace the file).
- [ ] **Step 4:** Report. The CI run needs a push, which only the operator approves; do not push.

---

## Self-review

**Spec coverage.** Decisions 1-3 → Task 2. Decision 4 → Task 3. Decision 5 → Task 3. Decision 6 → Task 2 Step 6. Decision 7 → Task 1. Decision 8 → Global Constraints. Verification → Task 1 Steps 1-3 and Task 4; the CI run is the operator's push.

**Type consistency.** The four lane strings appear once, in Global Constraints; Tasks 2-4 cite them.

**Proportion.** 4 tasks against a ~70-line spec; no step carries a function body.
