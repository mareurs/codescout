---
id: '6afbf9cc4a700020'
kind: plan
status: draft
title: 'IL-3 retirement: run to completion, pipe the capture, return a handle to all of it — Implementation Plan'
---

# IL-3 retirement: run to completion, pipe the capture, return a handle to all of it — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A foreground pipe ending in a terminal filter is never refused; the producer runs to the end, the filter sees the whole stream, and the response carries one handle to a redacted capture of up to 4 MiB.

**Architecture:** Keep the existing tee tap (`inject_tee_in`) and change three things around it: the splice drains the stream so an early-exit filter cannot stop the producer; the stored capture limit rises from 10,000 B to 4 MiB; a timeout runs the filter once on the partial capture. The refusal (`detect_il3_violation`) stays only for `run_in_background`.

**Tech Stack:** Rust (tokio), POSIX `sh -c`, `tempfile`; tests are `#[tokio::test]` in `src/tools/run_command/tests.rs`.

**Spec:** `docs/superpowers/specs/2026-10-21-il3-run-to-completion-pipe-design.md`

> **Superseded in parts (2026-10-22).** This plan was executed and merged as `43d3a25ee`. Review and measurement changed several decisions while it ran (splice at the first pipe, a bounded drain, a bounded capture file, a pure-filter allowlist for the timeout replay, Task 5 dropped then delivered by the final wave). The spec's *Amendments during the build* section is the current record; read it before relying on any text below.

## Global Constraints

- Trigger = the existing splice: last unquoted `|` whose last stage is one of the 13 in `detect_terminal_filter` (`src/tools/command_summary.rs:304`). Other pipes run unchanged.
- Spliced text is exactly `{A} | tee '{cap}' | {{ {B}` + newline + `s=$?; head -c 4194304 >/dev/null; exit $s; }}` (4194304 = `CAPTURE_STORE_LIMIT_BYTES`; format it from the constant). The newline (not `;`) before `s=$?` is required: a trailing `# comment` in `B` must not swallow the brace. The drain stops after the capture limit, so an endless producer into an early-exit filter (`cat /dev/urandom | tr -dc a-z | head -c 20`, `yes | head -1`) ends at once by SIGPIPE with a file of about 4 MiB, instead of running to the timeout and filling the disk. (Ruling R8, 2026-10-21: the unbounded `cat >/dev/null` drain in the first version of this plan was a defect.)
- `CAPTURE_STORE_LIMIT_BYTES = 4 * 1024 * 1024`. The handle is redacted (`redact_credentials`) before it is stored.
- Timeout: `timeout_secs` (default 30) is one total deadline for the producer; then the filter gets `TIMEOUT_FILTER_GRACE_SECS: u64 = 5`.
- New response keys: `capture_incomplete: true` (timeout) and `filter_grace_expired: true`. Reuse `unfiltered_output`, `unfiltered_output_lines`, `unfiltered_truncated`, `unfiltered_buffered_lines`.
- Background pipes keep the refusal, with new text that still starts `IL3 violation`.
- Gate before completion: `./scripts/gate.sh`. Targeted tests: `scripts/with-slot.sh cargo test --lib run_command::tests::<name>`.
- Commits: follow `docs/conventions/shared-checkout-commit-sequence.md`. The checkout holds other sessions' uncommitted files; `git add` named paths only.
- Unix-only tests carry `#[cfg(unix)]`. `/bin/sh` on the dev machine is bash; `dash` is not installed, so the spliced text must use POSIX constructs only.

## Review Focus

- A filter followed by a comment or `;` tail (`A | head -2 # why`): the group must still close — Task 2.
- A filter that matches nothing (`seq 1 10 | grep zzz`): exit code 1 survives the drain and the handle is still offered — Task 2.
- `A`'s stderr: it still reaches the response and is not piped into `B` unless the command says `2>&1` — Task 2.
- Credentials in a capture over the old 10 KB limit are still redacted in the handle — Task 1.
- An endless producer into `head -1` ends at the cap, flagged, with a usable handle — Task 4.

---

### Task 1: Capture store limit 10 KB → 4 MiB, shared store helper

**Files:**
- Modify: `src/tools/run_command/output.rs` (Step 6.5 block in `handle_successful_output_with`, ~lines 1194-1300)
- Test: `src/tools/run_command/tests.rs`

**Interfaces:**
- Produces: `pub(super) const CAPTURE_STORE_LIMIT_BYTES: usize`; `pub(super) fn read_capture(tmp: &TmpfileGuard) -> Option<(String, usize)>` (redacted text, redaction count; logs and returns `None` on read error, as today); `pub(super) fn store_capture(content: String, original_command: &str, exit_code: i32, ctx: &ToolContext) -> Option<(String, Option<Truncation>, usize, Vec<String>)>` (the tuple `unfiltered_ref` holds today; `None` for empty content).

- [ ] **Step 1: Write failing tests** — `a_capture_under_the_store_limit_is_kept_whole`: `seq 1 100000 | grep 77777` → `unfiltered_output_lines == 100000`, no `unfiltered_truncated`, and `run_command("wc -l <unfiltered_output>")` stdout starts `100000`. `a_capture_over_the_store_limit_is_cut_and_marked`: `seq 1 1000000 | grep 99999` → `unfiltered_truncated == true`, `unfiltered_output_lines == 1000000`, `500000 < unfiltered_buffered_lines < 1000000`. `a_credential_in_a_large_capture_is_redacted_in_the_handle`: reuse the fixture of the existing test for bug `bc0cb248b224d1dd`, padded past 10 KB.
- [ ] **Step 2: Run them** — `scripts/with-slot.sh cargo test --lib run_command::tests::a_capture_`; expected FAIL (buffered lines ≈ 2222).
- [ ] **Step 3: Extract `read_capture` and `store_capture` from Step 6.5; cut at `CAPTURE_STORE_LIMIT_BYTES`** instead of `body_alone_overflows` / `MAX_INLINE_TOKENS * 4`. Keep the truncation marker, `Truncation`, pool-cut notice and "count on the FULL capture" behavior unchanged.
- [ ] **Step 4: Update every existing test that pins the 10,000 B cut** (`grep -n unfiltered_buffered_lines src/tools/run_command/tests.rs`) to the new limit. Run `scripts/with-slot.sh cargo test --lib run_command`; expected PASS.
- [ ] **Step 5: Commit** — `feat(run_command): the unfiltered capture keeps 4 MiB, not 10 KB`.

### Task 2: Drain layout so an early-exit filter cannot stop the producer

**Files:**
- Modify: `src/tools/run_command/inner.rs` (`TeeInjection`, `inject_tee_in`, the SIGPIPE comment at Step 5)
- Create: `docs/issues/2026-10-21-head-capture-looks-complete-but-is-partial.md` (copy `docs/issues/_TEMPLATE.md`; record `seq 1 300000 | head -3` → 1860 lines, no flag)
- Test: `src/tools/run_command/tests.rs`

**Interfaces:**
- Consumes: Task 1's limit (the 300000-line stream is ~1.9 MiB).
- Produces: `TeeInjection { command, capture, skipped, filter: Option<String> }` — `filter` is the text after the spliced `|` (`B`), `None` when nothing is captured.

- [ ] **Step 1: Write failing tests** — unit `inject_tee_in_wraps_the_filter_in_a_draining_group` (command equals the Global Constraints text for `seq 1 3 | head -1`; `filter == Some("head -1")`). Integration: `a_head_pipe_still_captures_the_whole_stream` (`seq 1 300000 | head -3` → stdout `"1\n2\n3\n"`, `unfiltered_output_lines == 300000`, no `unfiltered_truncated`); `a_filter_exit_status_survives_the_drain` (`seq 1 10 | grep zzz` → `exit_code == 1`, `unfiltered_output` present); `a_comment_after_the_filter_does_not_break_the_drain` (`seq 1 10 | head -2 # why` → `"1\n2\n"`); `a_producers_stderr_is_not_piped_into_the_filter` (`(echo out; echo err >&2) | grep err` → stdout empty, `stderr` contains `err`).
- [ ] **Step 2: Run them**; expected FAIL (capture is 1860 lines; the group text is absent).
- [ ] **Step 3: In `inject_tee_in`, splice the Global Constraints text and fill `filter`.** The existing test asserting `command.contains("| tee '")` stays true. Reword the SIGPIPE comment: the drain, not SIGPIPE, now decides how far the producer runs.
- [ ] **Step 4: Run `scripts/with-slot.sh cargo test --lib run_command`**; expected PASS.
- [ ] **Step 5: Commit** — `fix(run_command): a head pipe no longer cuts its own capture`.

### Task 3: Refusal only for background; replace the `can_append_stage` oracle

**Files:**
- Modify: `src/tools/run_command/mod.rs:428` (gate call), `src/util/path_security.rs` (`detect_il3_violation` message and doc comment), `src/tools/run_command/output.rs:642` (`can_append_stage`)
- Test: `src/tools/run_command/tests.rs`, `src/tools/run_command/observe.rs`, `src/util/path_security.rs` tests

**Interfaces:**
- `detect_il3_violation(command: &str) -> Option<String>` keeps its signature and detection; only its message changes. `can_append_stage(query: &str, stage: &str) -> bool` becomes `is_one_pipeline(query)` alone (a foreground `query | stage` is always run now).

- [ ] **Step 1: Write failing tests** — `a_foreground_pipe_from_an_unbounded_producer_is_not_refused` (`cargo --version | head -1` → `Ok`, stdout contains `cargo`, `unfiltered_output` present); `a_background_pipe_from_an_unbounded_producer_is_still_refused` (same command with `run_in_background: true` → `Err`, message starts `IL3 violation`, contains `background` and `@bg_`).
- [ ] **Step 2: Run them**; expected FAIL on the first.
- [ ] **Step 3: Gate only when `run_in_background`; rewrite the message** to say a background run keeps only what its pipe passes, and to offer (a) run the producer bare in the background and query `@bg_*`, (b) run the pipe in the foreground, which keeps the full stream as `unfiltered_output`. Replace the `can_append_stage` body.
- [ ] **Step 4: Flip the tests that expect a foreground refusal** — `il3_blocks_cargo_pipe_grep_via_run_command`, `il3_blocks_chained_unbounded_pipe`, `il3_blocks_unbounded_pipe_pre_exec` (make them background calls) and the `observe.rs` test whose premise is "the IL-3 gate must refuse this" (use a background call). In `a_pager_the_il3_gate_or_a_list_would_defeat_is_never_offered` (`tests.rs:9490`) change only assertions that encode "the gate refused". The `path_security.rs` unit tests on `detect_il3_violation` stay.
- [ ] **Step 5: Run `scripts/with-slot.sh cargo test --lib`**; expected PASS.
- [ ] **Step 6: Commit** — `feat(run_command): foreground pipes are not refused; background pipes keep IL-3`.

### Task 4: A timeout runs the filter once on the partial capture

**Files:**
- Modify: `src/tools/run_command/inner.rs` (the `Err(_)` timeout arm, ~lines 677-711; new fn)
- Test: `src/tools/run_command/tests.rs`

**Interfaces:**
- Consumes: Task 1 `read_capture` / `store_capture`; Task 2 `TeeInjection.filter`.
- Produces: `async fn run_filter_on_capture(filter: &str, capture_path: &str, work_dir: &Path, grace: Duration) -> Option<String>` — runs `sh -c <filter>` with the capture as stdin; `None` when the grace expires.

- [ ] **Step 1: Write failing tests** (all `#[cfg(unix)]`, `timeout_secs: 1`) — `a_timed_out_filtered_command_returns_the_filter_of_the_partial_capture`: `(seq 1 1000; sleep 30) | grep 77` → `timed_out == true`, `capture_incomplete == true`, stdout contains `977`, `unfiltered_output_lines == 1000`, `unfiltered_output` starts `@cmd_`. `an_endless_producer_into_head_ends_at_the_cap_flagged_incomplete`: `(while :; do echo x; sleep 0.01; done) | head -1` → timed out, `capture_incomplete`, stdout `"x\n"`. Unit `run_filter_on_capture_returns_none_when_the_grace_expires` (filter `sleep 30`, grace 200 ms). The existing `sleep 10` timeout test (`tests.rs:1067`) must stay green: no filter, no new keys.
- [ ] **Step 2: Run them**; expected FAIL.
- [ ] **Step 3: In the timeout arm, after `killpg`:** if a non-empty capture exists, `read_capture` → `run_filter_on_capture(filter, path, &work_dir, Duration::from_secs(TIMEOUT_FILTER_GRACE_SECS))` → `store_capture` (exit code `-1`). Keep `timed_out`, `stderr`, `exit_code: null`, `hint`; add `capture_incomplete: true`, the `unfiltered_*` keys, and `stdout` (clamped with `truncate_lines_and_bytes`, as `handle_successful_output_with` does) — or `filter_grace_expired: true` instead of `stdout`.
- [ ] **Step 4: Run `scripts/with-slot.sh cargo test --lib run_command`** and the jobs-sweep timeout test (`a_timeout_naming_many_large_jobs_delivers_its_timed_out_status`); expected PASS.
- [ ] **Step 5: Commit** — `feat(run_command): a timed-out filtered command returns the filter of its partial capture`.

### Task 5: Bound the capture file on disk (spike, decision gate)

**Files:** Modify `src/tools/run_command/inner.rs` (`inject_tee_in`); test `src/tools/run_command/tests.rs`.

**Why a spike:** the tee file is unbounded within the wait cap. The code shows this; no live run has measured it. The mechanism is not decided.

- [ ] **Step 1: Write the acceptance test** `the_capture_file_stops_growing_near_the_store_limit`: build the command with `inject_tee_in("seq 1 3000000 | head -1", false, dir)`, run it with `sh -c`, then assert the capture file is `<= CAPTURE_STORE_LIMIT_BYTES + 64 * 1024`, stdout is `"1\n"`, the exit status is 0 and it ends within 20 s.
- [ ] **Step 2: Try candidates in order; keep the first that passes**: (a) a FIFO feeding `head -c <limit> > cap; cat >/dev/null` as a background drainer, with a guard that removes the FIFO; (b) `( trap '' XFSZ; ulimit -f <blocks>; tee cap )`, rejected if it prints to stderr. Check each under `bash` and, if the CI image has it, `dash`.
- [ ] **Step 3: STOP and ask marius if** a candidate makes `unfiltered_output_lines` a lower bound (a cut file no longer knows the stream's total) or none passes. The fallback is to ship no bound, delete the test, and record the exposure (same as today's `yes | grep x`) in the spec and in the Task 2 issue file.
- [ ] **Step 4: Commit** — `fix(run_command): bound the unfiltered capture file on disk`, or `docs:` for the fallback.

### Task 6: Prompt surfaces, trackers, spec status

**Files:** `src/prompts/` (and `.codescout/system-prompt.md`), `docs/PROGRESSIVE_DISCOVERABILITY.md`, `docs/trackers/run-command-pipeline.md`, the spec, `docs/architecture/companion-plugin.md`.

- [ ] **Step 1: Find every statement of the old rule** — `grep -rn "unbounded run_command\|IL-3\|IL3" src/prompts .codescout/system-prompt.md docs/PROGRESSIVE_DISCOVERABILITY.md docs/manual docs/architecture/companion-plugin.md`. Rewrite each: foreground pipes ending in a terminal filter are allowed and return `unfiltered_output`; background pipes keep the refusal.
- [ ] **Step 2: Apply `src/prompts/README.md`** — bump `ONBOARDING_VERSION` if its rule says so; run `scripts/with-slot.sh cargo test --lib prompts`; expected PASS.
- [ ] **Step 3: Record it** — add a "Tracker updates" entry in `docs/trackers/run-command-pipeline.md` (one-tap case of R3; gate retired for foreground); set the spec's status to implemented.
- [ ] **Step 4: Commit** — `docs: IL-3 is retired for foreground pipes`.
- **Not in this plan (separate repo, shared-checkout rules apply):** the companion plugin's SessionStart text ("Never pipe unbounded run_command output") and the header of the parked `il3-deny-hook.sh`. Tell marius.

### Task 7: Gate and live check

- [ ] **Step 1: `./scripts/gate.sh`**; expected: every stage green. Report any red with its output.
- [ ] **Step 2: Ask marius to run `./scripts/rb.sh`, then `/mcp`.** Then re-run the three recon commands: `seq 1 300000 | head -3` (expect `unfiltered_output_lines: 300000`), `seq 1 300000 | grep 77777` (expect no `unfiltered_truncated`), `cargo --version | head -1` (expect `Ok`).
