---
id: 104f4fff5b8448ad
kind: plan
status: done
title: Classify grader infrastructure crashes and recover concatenated test statuses
tags:
- effort-steering
- evaluation
- grader
---

# Grader validity classification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the external grader from reporting a toolchain crash as a model failure, and recover a passing test whose status line is glued to test stdout, without editing any frozen result.

**Architecture:** `live_grader` already returns `pass: False` plus an `infra` or `error` field for grader faults, and `run.grade_outcome` already stops a batch on either. Task 1 and 2 add one more fault class to that existing channel. Tasks 3 and 4 add an audit-only reparse, because the faulty parser is dataset code inside the frozen record and must not change.

**Tech Stack:** Python 3, pytest, `tools/setup-ab-eval`.

**Spec:** `docs/issues/2026-11-06-private-grader-linker-crash-counted-as-model-failure.md` (id `54fe4b112aa4e423`) and `docs/issues/2026-11-06-rust-grader-parser-misses-concatenated-test-status.md` (id `0a2929ec2a4f688f`).

## Global Constraints

- Original results stay immutable: `external-matched-repairs-20261105/results.json`, every `private-grade.json` and every `grade.log`. Corrections go in a new audit artifact.
- No LLM call and no paid run. No candidate edit.
- The external wrapper `/tmp/grade-external-candidate-01a15ca9-20261105.py` reads only `error`, `oom_killed`, `container_removed` and the independent absence of the container. It does not read `infra`. So Task 2 must set a nonzero `error`; `infra` alone is not enough (confirmed by thread `01a15ca9`, 2026-11-06).
- The wrapper also checks the hashes of the frozen sources. Any change to `multilang_grade.py` or `multilang_tasks.py` needs new controls and a new registration before it grades a new cohort. The old cohort is regraded only with its archived source.
- `tools/setup-ab-eval` and the cohort belong to the campaign of Codex thread `01a15ca9-f939-7603-acb5-01c6b02f68e2`. Do not edit those files before that thread agrees (Task 0).
- A genuine candidate compile error stays a model failure. Only a process-fatal toolchain signal with zero parsed statuses is infrastructure.
- Do not claim a cause for the SIGBUS. The bug file says disk, memory and LLVM are all unattributed.
- Do not turn an arbitrary matching substring into a status. Keep the exact binary and test identity.
- Test command: `python3 -m pytest tools/setup-ab-eval/<file> -q`. The tests import siblings by bare name, so run from `tools/setup-ab-eval` if the import fails.


## Handoff and execution ruling — 2026-11-06

User relayed original session: only205 baseline tests, no edits or commit elsewhere; Task1 never started. Codex01a15ca9 now owns implementation in existing clean worktree /home/marius/work/claude/codescout.worktrees/grader-validity, branchfix/grader-validity-20261106, base4c90f8fab. No competing edits. Execute inline under existing autonomy; one fresh final review with applied mutations, one fix pass. No per-task commit before whole validation; commit scoped paths at close if good, no merge/push.

Task5 identical NUlow candidate diagnostic is already CLOSED in private-grade-audit-20261106:2F2Pfail/8620P2Ppass/missing0. Do not regrade it again. Reuse its evidence; record this supersession instead of asking permission to repeat. Task4 reparse remains read-only audit; it cannot qualify prospective live parsing. Existing wrapper needs truthy error as well as infra. New paid controls/policy remain a subsequent source-bound step.


## Implementation evidence — first pass

Fresh baseline205 tests plus69 subtests pass. Classifier tests first failed on missing function. Initial live-grader fixture incorrectly used a parser that rejected diagnostic text; this wrong-reason red was retained, production wiring removed, boundary parser corrected, and intended red reobserved:1 failure/2 passes on missing infra. Wiring reapplied:212 tests/76subtests pass. Audit import-missing red retained, then no-op scaffold4 assertion failures/4passes; implementation8tests/3subtests pass. Fixture contains the verbatim original record parser, never a modified live parser.

Read-only cohort audit docs/research/data/2026-11-06-grader-validity-audit.json SHA2221fee48b54ee56e0040be7bbd4676f802494747c6ad41b4b72af6bf70657b4. Four original verdicts reproduced exactly. One zero-status crash (NUlow), one exact expected status recovered at NUxhigh line3725; genuine failure remains.17 unique old files (results,logs,grades,candidates,bundles,records) hashed unchanged. No model/provider/native regrade.

Full isolated gate @bg_00000127 launched on worktree; outcome pending. One independent fresh review grader_validity_review active; actual frozen mutations required. No commit yet.


## Review observations — pending one fix pass

Full original-source gate127 CLOSED0 at15:11:27UTC; Python1642passed529subtests, FMT/CLIPPY/LEAN/DEFAULT0. Coverage exclusions printed by gate retained. This green baseline does not establish absence of behavioral defects.

Fresh reviewer primary12 exact applied mutations:8killed/4survived. MAIN b1e1f89b7d587cb8 (compiler source excerpt falsely classified as process crash),73efe0aa746d30f9 (cumulative-prefix recovery misattributes an ordinary status),77828ecdbae3b8c2 (P2P/standalone failure/terminal/ANSI split guard tests missing). Original source/tests remain frozen until review handback, followed by one offline fix pass and actual same-mutation reapplication. No new paid task or historical regrade.


## Verified bounded closure — 2026-11-06

Source commit a2ffc143266c5e828afd7d8ef2ceb665318df49f on fix/grader-validity-20261106, patch-id6d3afca0f9ca24e2ccc9032c5aea79f418b64b7c; cleanworktree, no merge/push. Per-task commits were batched into this validated commit. Final wholechanged227tests/85subtests PASS, fullgate12a CLOSED0 allfive lanes. One final review plus one offlinefix, original12mutants8killed4survived; reapply11exactoriginalliterals+explicitM02-v2resite,12applied11killed1survivedM04 (fourcell counterfactual proves redundancy on tested overlap). No absentliteralcountedkilled. D1–D5 rulings in residentrulings.md.

Task5 superseded by earlier closed f7diagnostic; no repeat or approval question. Auditv1 preserved, finalv2 four rows identical and17protectedfilesunchanged. Data/report/evidence docs/research/2026-11-06-grader-validity.md and data/2026-11-06-grader-validity-evidence.tar.gz. MAIN documentation is separate from source commit. Historicalgluedparser0a remainsmitigated; only future prospectivepolicy+freshcontrols canadmit newliveoracle. No paid/model efficacyresult inthisstage.

## Review Focus

- Crash signal present but some statuses parsed: must stay a normal grade (a test binary may die mid-run).
- Zero statuses and no crash signal (a real compile error): must not become infrastructure.
- A glued `test X ... ok` whose name is not in the expected set: must not be recovered.
- A glued `FAILED` and an earlier `ok` for the same key: `FAILED` must win.
- An ANSI-coded or CR-redrawn status line the record's parser already reads: must not be reported as recovered.

---

### Task 0: Coordinate and isolate

**Files:** none.

- [x] **Step 1 (done 2026-11-06: thread `01a15ca9` has no in-flight edits and no objection; it leaves the implementation to this session):** Send thread `01a15ca9` one message (use `/codescout-companion:reaching-peer-sessions`): this plan edits `multilang_grade.py`, `multilang_tasks.py`, `run.py` tests and adds `audit_reparse.py`. Wait for its answer through the operator.
- [x] **Step 2 (done 2026-11-06: empty in the main checkout, in `/home/marius/.codex/worktrees/615e/codescout` and in `/tmp/codescout-tmp-exec-01a15ca9`):** Run `git status --short tools/setup-ab-eval`. Expected: empty. If not empty, stop and ask the owner of those edits.
- [x] **Step 3:** Create an isolated worktree from `experiments` with `superpowers:using-git-worktrees`.

### Task 1: Toolchain crash classifier

**Files:**
- Modify: `tools/setup-ab-eval/multilang_grade.py`
- Test: `tools/setup-ab-eval/test_multilang_grade.py` (new class `CrashSignalTests`)

**Interfaces:**
- Produces: `toolchain_crash(log: str) -> str | None`, the first matching log line, else `None`.

- [x] **Step 1: Write failing tests** in `CrashSignalTests`:
  - `test_toolchain_crash_finds_ld_bus_error`: input contains `collect2: fatal error: ld terminated with signal 7 [Bus error], core dumped`; assert the returned line equals that line.
  - `test_toolchain_crash_finds_rustc_signal_line`: input contains `(signal: 7, SIGBUS: access to undefined memory)`; assert not `None`.
  - `test_toolchain_crash_ignores_an_ordinary_compile_error`: input `error[E0425]: cannot find value` ; assert `None`.
  - `test_toolchain_crash_ignores_a_test_name_that_contains_signal`: input `test signal_handler::terminated_with_signal_is_reported ... ok`; assert `None`.
- [x] **Step 2:** Run `python3 -m pytest tools/setup-ab-eval/test_multilang_grade.py -k CrashSignal -q`. Expected: FAIL with `AttributeError`.
- [x] **Step 3:** Implement `toolchain_crash` with one compiled regex over `terminated with signal \d+`, `\(signal: \d+, SIG[A-Z]+`, `Bus error`, `Segmentation fault`, matched per line and never inside a line that starts with `test `. Confirm the pattern against the real lines 502–526 of `private-grader/low-a6931258/grade.log` under the Nushell low arm.
- [x] **Step 4:** Re-run the command. Expected: PASS.
- [x] **Step 5:** Commit `grader: classify process-fatal toolchain signals`.

### Task 2: Wire the classifier into the grade result

**Files:**
- Modify: `tools/setup-ab-eval/multilang_tasks.py` (`live_grader/grade_one`, after `statuses = grade.run_parser(...)`)
- Test: `tools/setup-ab-eval/test_multilang_tasks.py` (`LiveGraderTests`), `tools/setup-ab-eval/test_run.py` (the `"grader infra"` list near line 1523)

**Interfaces:**
- Consumes: `grade.toolchain_crash(log: str) -> str | None`.
- Produces: when `not statuses` and a crash line exists, the result carries `pass: False`, `infra: "toolchain_crash"` and `error: "toolchain crash before any test ran: <line>"`.

- [x] **Step 1: Write failing tests:**
  - `test_live_grader_zero_statuses_with_crash_signal_is_infra`: use the fake docker already in `LiveGraderTests`, log with the `ld terminated with signal 7` line and no `test ... ok` lines; assert `infra == "toolchain_crash"`, `pass is False`, `"signal 7" in error`.
  - `test_live_grader_crash_signal_with_parsed_statuses_is_a_normal_grade`: same log plus one `test t::f1 ... ok`; assert `"infra" not in result`.
  - `test_live_grader_zero_statuses_without_signal_is_not_infra`: log with `error[E0425]` only; assert `"infra" not in result` and `missing == expected`.
  - In `test_run.py`, add `{"pass": False, "infra": "toolchain_crash"}` to the `"grader infra"` cases; assert `run.grade_outcome` returns `infra-other`.
- [x] **Step 2:** Run the three test files with `-k "toolchain or grader_infra or grade_outcome" -q`. Expected: the new live_grader tests FAIL.
- [x] **Step 3:** In `grade_one`, after the parse, set `infra` and `error` when `not statuses and (line := grade.toolchain_crash(log))`. Keep `grade.verdict` output unchanged so the raw counts stay visible.
- [x] **Step 4:** Re-run. Expected: PASS.
- [x] **Step 5:** Commit `grader: report a toolchain crash with zero statuses as infrastructure`.

### Task 3: Concatenated-status recovery (audit only)

**Files:**
- Create: `tools/setup-ab-eval/audit_reparse.py`
- Test: `tools/setup-ab-eval/test_audit_reparse.py`

**Interfaces:**
- Consumes: `grade.run_parser(parser_src: str, log: str, timeout)`, `grade.verdict(statuses, f2p, p2p)`.
- Produces:
  - `split_concatenated_status(log: str) -> tuple[str, list[dict]]`: the log with a newline inserted before each glued `test <name> ... (ok|FAILED|ignored)`, plus `{"line", "prefix", "status_line"}` per split.
  - `reparse(record: dict, log: str, f2p: list[str], p2p: list[str]) -> dict` with keys `original`, `adjusted` (both `verdict` dicts), `recovered` (list of `{"key","status","line","raw"}`).

- [x] **Step 1: Write failing tests**, using lines copied verbatim from the real log (extract with `sed -n '3722,3726p'` and line 3259 of the Nushell xhigh `grade.log`):
  - `test_split_finds_the_glued_line_3725_shape`: one split, `prefix == "should_not_overwrite_last"`.
  - `test_reparse_recovers_the_expected_test_and_leaves_the_real_failure`: record from `fresh-tasks/nushell__nushell-18863/build/record.json`, log excerpt = the `Running tests/main.rs (target/debug/deps/tests-b7e6894743b0a1f5)` line, the glued line, and the verbatim `FAILED` line for `mixed_str_length_result_does_replace_last`; expected set = exactly those two full keys (`target/debug/deps/tests-b7e6894743b0a1f5::last_result::...`). Assert `original["missing"] == 1`, `adjusted["missing"] == 0`, and the `FAILED` key still not passing in `adjusted`.
  - `test_reparse_does_not_recover_a_name_outside_the_expected_set`.
  - `test_reparse_failed_beats_an_earlier_ok_for_the_same_key`.
  - `test_reparse_does_not_report_an_ansi_prefixed_status_as_recovered`.
- [x] **Step 2:** Run `python3 -m pytest tools/setup-ab-eval/test_audit_reparse.py -q`. Expected: FAIL, module missing.
- [x] **Step 3:** Implement both functions. `reparse` runs the record's own parser on the original and on the split log, then lists keys whose status differs, restricted to expected names. The parser stays the record's, unmodified.
- [x] **Step 4:** Re-run. Expected: PASS.
- [x] **Step 5:** Run `./scripts/mutation-probe.sh` on the two new functions (read `docs/PROBES.md` first) and record which mutants survive. A surviving mutant needs a new test before the commit.
- [x] **Step 6:** Commit `audit: recover a glued libtest status without changing the record parser`.

### Task 4: Audit artifact for the cohort

**Files:**
- Create: `docs/research/data/2026-11-06-grader-validity-audit.json`

- [x] **Step 1:** Run `reparse` and `toolchain_crash` over the four `private-grader/*/grade.log` files of `external-matched-repairs-20261105`, with the expected sets from each bundle. Write one JSON object per arm: raw counts, adjusted counts, recovered list, crash line, and the sha256 of each log.
- [x] **Step 2:** Check against measurements already taken: one split in total (line 3725, Nushell xhigh); the Nushell low arm shows a crash line and zero statuses; the rust-analyzer arms show neither. Any other result means stop and investigate.
- [x] **Step 3:** Confirm `git status` shows no change to any cohort file.
- [x] **Step 4 (bounded closure ruling):** Preserve the artifact and record the verdict in both MAIN bug files. Source/test code is committed as a2ffc1432. MAIN research documents/artifacts remain uncommitted separately; do not imply they are included in the eight-file source commit or disturb the shared foreign index.

### Task 5: Regrade the Nushell low candidate (operator-gated)

- [x] **Step 1:** Ask the operator to approve a regrade of the identical captured `candidate.patch` of `nushell__nushell-18863--low` in a fresh qualified environment, with no LLM call. Take the command from the cohort's own grade script. This plan does not invent it. Grade with the archived source the cohort was frozen with, not with the new code, because the wrapper checks the frozen hashes.
- [x] **Step 2:** Preserve both attempts. If the second run shows tests, record a Bus error as infrastructure, not as proof of a faulty candidate.

### Task 6: Close out

- [x] **Step 1:** Run `./scripts/gate.sh`. Expected: green.
- [x] **Step 2:** Update both bug files with the commit SHAs and the audit artifact path. Do not set `fixed` until the gate is green and Task 5 has a recorded outcome.
