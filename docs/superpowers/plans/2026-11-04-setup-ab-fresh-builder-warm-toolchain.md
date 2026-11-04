# Fresh builder: warm build with the template's toolchain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the fresh builder's warm `cargo test --no-run` compiles with the same rustup toolchain the template's test command uses, so the committed `target/` matches what the gate and the arms run.

**Architecture:** `build_image` gets one keyword `warm_toolchain`; `_run_locked` derives it from the template's `test_cmds` and passes it; the build evidence records it. No other stage changes.

**Tech Stack:** Python 3 stdlib, `unittest`, existing fakes in `test_fresh_build.py`.

**Spec:** docs/superpowers/specs/2026-11-04-setup-ab-fresh-builder-design.md (sections on the image build and on the control gate). This plan is an amendment found by the first real run.

## Why (measured 2026-11-04)

First real build, rust-analyzer PR 23586, was refused at the warm build: "rustc 1.95.0 is not supported ... requires rustc 1.98". The task image carries a rustup directory override for `/testbed` (1.95.0), and the warm step is hard-coded as `cargo test --no-run` (fresh_build.py, `build_image`, the `warm-build` exec), so it uses the override. The template's test command is `cd /testbed ; RUN_SLOW_TESTS=1 cargo +stable test --no-fail-fast ...` (explicit `+stable`, which beats the override). So the build compiled with a toolchain the gate never uses. The operator ruled (2026-11-04): fresh tasks are built and gated with the image's `stable`; the base image is prepared separately (stable updated, `/testbed` override set to `stable`).

## Global Constraints

- Only `tools/setup-ab-eval/fresh_build.py` and `tools/setup-ab-eval/test_fresh_build.py` change. Do not touch `multilang_*.py`, `freeze_screen.py`, `pilot.py`.
- Template command strings are never executed by the builder; the toolchain name is parsed with the existing `_TOOLCHAIN_USE` regex and validated by the existing `_TOOLCHAIN` pattern.
- `BUILDER_VERSION` is bumped to `"2"` (the warm step changed what an image contains; the cache key includes it).
- Tests first (show RED), mutation-check every new guard, `python3 -B -W error -m unittest test_fresh_build` and `python3 -B -m unittest test_multilang_control test_multilang_grade` stay green.

## Review Focus

- A template whose test commands name two different toolchains (`cargo +stable test` and `cargo +nightly test`): refuse, never pick one silently.
- A template whose test commands name no toolchain: the warm step stays `cargo test --no-run` (default/override), as now.
- A template that names a toolchain only in `rebuild_cmds`/`print_cmds` and not in `test_cmds`: the warm step ignores it (it is evidence only, as today).
- `warm_toolchain` must be a plain rustup name (`stable`, `nightly-2026-01-01`), never flags or shell text.

### Task 1: warm build uses the template's test toolchain

**Files:**
- Modify: `tools/setup-ab-eval/fresh_build.py` (`build_image`, `_run_locked`, a new helper next to `_template_toolchains`, `BUILDER_VERSION`)
- Test: `tools/setup-ab-eval/test_fresh_build.py`

**Interfaces:**
- Consumes: `_TOOLCHAIN_USE`, `_TOOLCHAIN`, `TEMPLATE_LIST_KEYS`, `_template_toolchains` (existing).
- Produces: `_test_toolchain(template) -> str | None` (the single toolchain named by the template's `test_cmds`, `None` if none, `ValueError` naming both if two or more); `build_image(..., warm_toolchain=None)`; build evidence key `warm_toolchain` (str or null).

- [ ] **Step 1: Write the failing tests** (names and assertions)
  - `test_test_toolchain_single`: template with `test_cmds=["cd /testbed ; cargo +stable test"]` gives `"stable"`.
  - `test_test_toolchain_none`: `test_cmds=["cargo test"]` gives `None`.
  - `test_test_toolchain_two_refused`: `["cargo +stable test", "cargo +nightly test"]` raises `ValueError` whose message names `stable` and `nightly`.
  - `test_test_toolchain_ignores_rebuild_and_print`: `+stable` only in `rebuild_cmds` gives `None`.
  - `test_warm_build_argv_uses_toolchain`: with the fake docker recording argv, `build_image(..., warm_toolchain="stable")` runs the warm exec as `cargo +stable test --no-run` (still with `CARGO_NET_OFFLINE=false` on that exec only); with `warm_toolchain=None` it runs `cargo test --no-run`.
  - `test_warm_toolchain_validated`: `warm_toolchain="stable; rm -rf /"`, `"+stable"`, `""` raise `ValueError` before any docker call.
  - `test_evidence_records_warm_toolchain`: `build-evidence.json` has `warm_toolchain` equal to the argument (and `null` for None).
  - `test_run_build_passes_template_toolchain`: an end-to-end `run_build` with a fake build asserts the fake received `warm_toolchain="stable"` for the rust-analyzer-shaped template; and a template with two toolchains in `test_cmds` ends as a refusal (exit 2, stage `build` or `fetch` as the existing argument errors do) with no docker call.
  - `test_builder_version_is_2`.
- [ ] **Step 2: Run them, show RED** (`python3 -B -W error -m unittest test_fresh_build`).
- [ ] **Step 3: Implement** `_test_toolchain`, the `warm_toolchain` keyword (validated with `_TOOLCHAIN`, default `None`), the warm exec argv, the evidence key, the `_run_locked` wiring, `BUILDER_VERSION = "2"`. Update the `build_image` docstring and the module docstring where they say `cargo test --no-run`.
- [ ] **Step 4: Run all three suites, show GREEN**; mutation-check: drop the `+name` from the argv, drop the validation, drop the two-toolchain refusal, drop the evidence key, leave `BUILDER_VERSION` at "1".
- [ ] **Step 5: Commit** `fix(setup-ab): fresh_build warm build uses the template's test toolchain`.
