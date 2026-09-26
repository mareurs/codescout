---
id: a2d62589fc638396
kind: plan
status: draft
title: System 1 base-rate measurement — implementation plan
tags:
- measurement
- system1
- plan
- usage-db
- deep-agent
topic: system1-measurement
---

# System 1 Base-Rate Measurement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Measure, before building any detector, whether a System 1 is worth building: the base rate of addressable misses that the operator caught, read against a pre-registered go/no-go rule.

**Architecture:** Two parts.
- **Part A** adds two record-only fields to codescout's `usage.db`: `tool_use_id`, and `deliveries_json` written through a task-local sink. It ships first and alone, because every day it is not live is a day of prospective data joined heuristically instead of exactly.
- **Part B** is an offline Python pipeline under a new scripts/measure directory. It freezes corpora outside the repo, joins them into an events database, mines corrections, runs a gated Codex judge, and produces the readout.

**Tech Stack:** Rust (rusqlite, tokio task-locals, sha2), and Python 3 stdlib plus `unittest`. Python runs as `~/work/claude/prompt-engineering/.venv/bin/python`. The judge uses `codex exec` through the existing `codex_complete` in `docs/evals/data/2026-09-24-rule-tell/stage2/generate_synthetic.py`.

**Spec:** `docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md` (`dd69959ffc06248a`). Read it before any task; this plan argues from it.

## Global Constraints

- **Record-only:** no tool's returned content bytes change. Only `usage.db` rows gain fields.
- **Corpora live outside the repo** under `~/work/claude/measurement-corpora/<corpus-id>/`. Only manifests, readouts and code are committed. Never commit a transcript, a `usage.db` copy, or anything containing the operator's private `CLAUDE.md`.
- **Subscription only, never the paid API.** Codex runs through `codex exec` with a fresh `CODEX_HOME`, and strips `OPENAI_API_KEY`/`CODEX_API_KEY`/`OPENAI_BASE_URL`, exactly as `codex_complete` does.
- **The go/no-go rule is fixed by the spec and must not be edited during implementation.**
  - **Window (spec Amendment 1, A1.2):** the decision reads only the latest 7 days before the freeze instant T. The retained window (2026-08-26 onward) is reported as a precedent and never decides. The contrast project is descriptive, on its own retained window.
  - GO: bootstrap lower bound ≥ **0.3** addressable operator-caught misses per session.
  - NO-GO: upper bound < **0.3**.
  - INCONCLUSIVE: anything else, or judge-gate failure, or spot-check agreement < **20/25**.
  - The bootstrap uses **10,000** resamples of sessions, with a recorded seed.
- **Judge gate thresholds (spec):**
  - correction mode: detectability agrees on **≥16/21** RTD cases;
  - audit mode: **≥3/4** `peer × yes` cases, **≥6/8** `yes` cases, **≤5/52** controls fired.
- **Sizes (spec):** correction census ≤ **400** (uniform registered sample beyond it); audit **200 + 100** qualifying decision points; pilot **20** items, excluded from all estimates.
- **Scope (spec Amendment 1):** besides the go/no-go, the pipeline baselines three descriptive outcomes, and none of them decides anything:
  - context support (A1.3);
  - lesson transfer (A1.4);
  - the background-worker opportunity share (A1.5).

  The observability map (A1.6) is published before any Codex call.
- **Leakage:** every judge input's latest included timestamp precedes the decision's. An input that fails is refused, never judged.
- **Timestamps are UTC.** Transcripts use ISO-8601 with `Z`. `usage.db` `called_at` is `YYYY-MM-DD HH:MM:SS[.fff]` with no zone. Normalise both before any comparison.
- **Rust gate:** `./scripts/gate.sh` before each Rust task's commit. Targeted runs use `scripts/with-slot.sh cargo test …`, never the path the gate printed. Rust mutations go through `./scripts/mutation-probe.sh`.
- **Commits:**
  - by pathspec only, never `git add -A`;
  - a `Session-Id:` trailer on each;
  - no push unless the operator asks;
  - no commit may leave the suite deliberately red across a task boundary.
- **Python tests** live in `tests/test_measure_*.py`. They are `unittest` classes that load modules by path through an `importlib` `_load` helper, the shape of `tests/test_phase1b_mining.py`. Run them with `~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_<x>.py -v`. **No CI lane runs them**, so the task's own run is the only evidence.

## Review Focus

1. **A `call_content` nested inside another call's `call_content`** (librarian adapter or peer dispatch). `deliveries_json` must describe the **outer, returned** response. Last writer wins, because the outer fan-out runs after the inner one returns. Pinned in Task 2, Step 1.
2. **A tool call that fails before the engine fan-out.** Its row must carry `deliveries_json` **NULL**, not `[]`. Pinned in Task 2, Step 1.
3. **`type: "user"` transcript entries that are not the operator speaking.** In session `3c5b02df` they were 167 tool results and 6 `isMeta` injections, next to 12 real prompts. Slash-command wrappers (`<command-name>`, `<local-command-stdout>`) and compaction summaries also appear. None of these may be classified as an operator correction. Pinned in Task 5 and Task 8.
4. **A transcript whose last line is truncated, or any malformed JSONL line** (a session killed mid-write). The parser skips and counts it; it never raises. Pinned in Task 5.
5. **Mixed timestamp formats across sources**, as in *Global Constraints*. A join or leakage check that compares raw strings silently misorders events. Pinned in Task 6 and Task 9.

---

## Part A — codescout fields

### Task 1: `tool_use_id` recorded on every call

**Files:**
- Modify: `src/tools/session_key.rs`, beside `conversation_from_meta`.
- Modify: `src/usage/db.rs`: a migration probe after `has_effect_class`, a new `MeasurementLinkage`, and `write_record`.
- Modify: `src/usage/mod.rs`: `UsageRecorder` field, `new`, `write_content`, and the 10 `UsageRecorder::new` call sites in `content_tests`.
- Modify: `src/server.rs`, `call_tool_inner`: read beside `asserted_conversation`; pass to `UsageRecorder::new`.

**Interfaces — produces:**
- `pub const TOOL_USE_ID_META_KEY: &str = "claudecode/toolUseId";` in `session_key.rs`.
- `pub fn tool_use_id_from_meta(meta: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<String>`.
  - Same silence rules as `conversation_from_meta`: absent `_meta`, absent key, non-string or whitespace give `None`.
  - Trimmed.
- `pub struct MeasurementLinkage<'a> { pub tool_use_id: Option<&'a str> }` in `db.rs`.
  - Derives `Debug, Default, Clone, Copy, PartialEq, Eq`.
  - It is a named struct for the reason `BufferLinkage`'s doc gives: adjacent `Option<&str>` parameters transpose silently. Task 2 adds a second field.
- `write_record(…, effect_class, measurement: MeasurementLinkage<'_>)`: a new final parameter, with a `tool_use_id` column in the INSERT.
- `UsageRecorder::new(agent, debug, session_id, cc_session_id, agent_id, tool_use_id: Option<String>)`.

- [ ] **Step 1: Write the failing tests**
  - `session_key.rs` tests:
    - `a_tool_use_id_is_read_from_its_meta_key`: `tool_use_id_from_meta(Some(&meta_map(&[("claudecode/toolUseId", json!(" toolu_01ABC "))])))` equals `Some("toolu_01ABC".into())`.
    - `every_shape_of_client_silence_yields_no_tool_use_id`: `None` for `None`, for an empty map, for `json!(7)`, and for `json!("  ")`.
    - `a_tool_use_id_never_becomes_a_conversation_key`: `conversation_from_meta` on a map holding only `claudecode/toolUseId` equals `None`. This pins the `MEASURED_TOO_FINE` intent against the new reader.
  - `usage/mod.rs` `content_tests`:
    - `record_content_records_tool_use_id_with_debug_off`: `debug = false`, `tool_use_id = Some("toolu_01X")`. `SELECT tool_use_id FROM tool_calls` gives `Some("toolu_01X")`, and `input_json` is `None`.
    - `record_content_records_null_tool_use_id_when_the_client_sent_none`: `tool_use_id = None`, and the column is `NULL`.
  - `usage/db.rs`: `open_db_adds_tool_use_id_to_a_pre_migration_table`. Create `tool_calls` without the column, call `open_db`, and `SELECT tool_use_id FROM tool_calls LIMIT 0` succeeds.
  - `server.rs` tests: `call_tool_inner_records_the_meta_tool_use_id`. Built like `a_conversation_asserted_on_the_request_rearms_the_ledger_once`: `params["_meta"] = {"claudecode/toolUseId": "toolu_srv1"}` on a `run_command` `echo hi`, then read the `make_server` project's `usage.db` for that row's `tool_use_id`, which equals `"toolu_srv1"`. **Why at this level:** it drives the production funnel, so it reds if the `server.rs` wiring is deleted, which a recorder-level test cannot see.

- [ ] **Step 2: Run them to verify they fail.** Run `scripts/with-slot.sh cargo test --lib tool_use_id`; expect compile errors or FAIL naming the missing items.

- [ ] **Step 3: Implement.**
  - `tool_use_id_from_meta` mirrors `conversation_from_meta` over the single key.
  - The migration follows the probe-then-`ALTER TABLE tool_calls ADD COLUMN tool_use_id TEXT;` idiom, plus `CREATE INDEX IF NOT EXISTS idx_tool_calls_tool_use_id ON tool_calls(tool_use_id);`.
  - The migration's comment states the NULL ambiguity, "client sent none" versus "row predates the column", separable only by `called_at`, as the `read_output_ids` migration comment does.
  - `write_content` passes `MeasurementLinkage { tool_use_id: self.tool_use_id.as_deref() }`, **unconditionally** and never behind `self.debug`.
  - The 10 test call sites pass `None`.
  - In `call_tool_inner`, compute `tool_use_id_from_meta(req.meta.as_ref().map(|m| &m.0))` on the line after `asserted_conversation`, before `req.arguments` moves, and pass it to `UsageRecorder::new`.

- [ ] **Step 4: Run the tests to verify they pass.** Same command; expect all PASS.

- [ ] **Step 5: Mutate each site.** Run `./scripts/mutation-probe.sh` twice:
  - write `None` in place of `measurement.tool_use_id` in `write_record`'s params: `record_content_records_tool_use_id_with_debug_off` must KILL;
  - pass `None` in place of the computed value in `call_tool_inner`: `call_tool_inner_records_the_meta_tool_use_id` must KILL.

  A `SURVIVED` on either is a defect in the tests; fix before continuing.

- [ ] **Step 6: Run `./scripts/gate.sh`.** Expect all four exit codes to be 0.

- [ ] **Step 7: Commit.** `git add` the four files, then commit `feat(usage): record claudecode/toolUseId per call (record-only)` with the `Session-Id` trailer, by pathspec.

### Task 2: `deliveries_json` through a task-local sink

**Files:**
- Create: `src/usage/deliveries.rs`, declared as `pub(crate) mod deliveries;` in `src/usage/mod.rs`.
- Modify: `src/tools/guide_ledger.rs`: `GuideLedger::key_set`.
- Modify: `src/engines/coordinator.rs`: `Emission.deliveries`, filled by `run_post_in`.
- Modify: `src/tools/core/types.rs`: `call_content` hands `Emission.deliveries` to the sink after `run_post`.
- Modify: `src/usage/mod.rs`: `record_content` scopes the sink and `write_content` writes it.
- Modify: `src/usage/db.rs`: migration probe, `MeasurementLinkage.deliveries_json`, and the INSERT column.

**Interfaces — consumes:** Task 1's `MeasurementLinkage` and `write_record`.

**Interfaces — produces:**
- `#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)] pub(crate) struct DeliveryRecord { pub engine: &'static str, pub ledger_keys: Vec<String>, pub blocks: Vec<BlockDigest>, pub hint: bool }`.
  - `hint` is true only when this engine's hint is the one `run_post_in` kept.
- `#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)] pub(crate) struct BlockDigest { pub sha256: String, pub bytes: usize }`.
  - The hash is lowercase hex over the block's text bytes.
- `tokio::task_local! { pub(crate) static DELIVERY_SINK: std::cell::RefCell<Option<Vec<DeliveryRecord>>>; }`, with a doc comment citing `PEER_SERVE_DISPATCH` in `src/tools/core/types.rs`. It is deliberately not a `ToolContext` field, because `ToolContext { … }` is constructed at 226 sites in 54 files.
- `pub(crate) fn record(records: Vec<DeliveryRecord>)`.
  - It **replaces** the sink's value with `Some(records)`, so the last writer wins, which is the outer call.
  - It does nothing outside a scope.
- `pub(crate) fn to_column(v: &Option<Vec<DeliveryRecord>>) -> Option<String>`: `None` gives NULL; `Some(vec![])` gives `"[]"`.
- `pub(crate) fn key_set(&self) -> std::collections::BTreeSet<String>` on `GuideLedger`: the keys of `emitted`.
- `Emission { hint, blocks, deliveries: Vec<DeliveryRecord> }`.
- `MeasurementLinkage { tool_use_id, deliveries_json: Option<&'a str> }`.
- `write_content(…, result, deliveries_json: Option<&str>)`: a new final parameter, fed into `MeasurementLinkage`.

- [ ] **Step 1: Write the failing tests**
  - `coordinator.rs` tests, over synthetic engines with the existing `run_post_in` idiom:
    - `each_claiming_engine_is_recorded_with_the_keys_it_added_and_its_block_digests`: an engine inserting key `k1` and returning one block `"abc"` gives one record, with `engine == "<its id>"`, `ledger_keys == ["k1"]`, and `blocks[0].sha256 ==` sha256(`"abc"`).
    - `a_declining_engine_leaves_no_record`.
    - `hint_is_marked_only_on_the_engine_whose_hint_survived`: two hinting engines; only the first record has `hint: true`.
  - `usage/deliveries.rs` tests:
    - `record_outside_a_scope_is_a_no_op`;
    - `the_last_record_in_a_scope_wins`, which is Review Focus 1;
    - `to_column_distinguishes_never_ran_from_delivered_nothing`: `None` gives `None`, and `Some(vec![])` gives `Some("[]")`.
  - `usage/mod.rs` `content_tests`, with `debug = false`:
    - `record_content_writes_null_deliveries_when_the_fan_out_never_ran`: the closure returns `Err(anyhow!("boom"))` without calling `record`, and the column is NULL. This is Review Focus 2.
    - `record_content_writes_the_sinks_value`: the closure calls `record(vec![fixture])`, and the column parses as JSON equal to `[fixture]`.
  - `usage/db.rs`: `open_db_adds_deliveries_json_to_a_pre_migration_table`.
  - `server.rs` tests: `call_tool_inner_records_opener_then_empty_deliveries`, through `call_tool_inner` exactly as in Task 1.
    - The first `run_command` row's `deliveries_json` has an entry with `engine == "session-opener"`.
    - The second, deduplicated call's row is exactly `"[]"`.
    - **Positive control:** the first call's returned content still carries the opener block, byte-identical to what a build without this task returns. Compare against `guide_blocks(&out.content)` from the existing test.

- [ ] **Step 2: Run them to verify they fail.** Run `scripts/with-slot.sh cargo test --lib deliver`; expect compile errors or FAIL.

- [ ] **Step 3: Implement.**
  - `run_post_in`:
    - snapshot `ledger.key_set()` before each engine's `emit` and diff it after;
    - digest each returned block's text;
    - push a `DeliveryRecord` only for `Emitted::Claimed`;
    - set `hint` exactly where the existing `if out.hint.is_none()` keeps one.
  - `call_content`: take `e.deliveries` and call `deliveries::record` before building the primary block. The `(e.hint, e.blocks)` destructure becomes a use of all three fields.
  - `record_content` wraps `f()` as `DELIVERY_SINK.scope(RefCell::new(None), async { let r = f().await; (r, DELIVERY_SINK.with(|s| s.borrow_mut().take())) })`, then passes `to_column(&d)` into `write_content`.
  - The migration follows Task 1's idiom, and its comment states the NULL meanings: "never ran" or "predates the column".

- [ ] **Step 4: Run the tests to verify they pass.** Same command; expect PASS. Then `scripts/with-slot.sh cargo test --lib engines` and `… --lib usage`; the existing suites stay green.

- [ ] **Step 5: Mutate each site** with `./scripts/mutation-probe.sh`:
  - skip one engine's `DeliveryRecord` push in `run_post_in`: the coordinator test must KILL;
  - drop the `deliveries::record` call in `call_content`: `call_tool_inner_records_opener_then_empty_deliveries` must KILL;
  - write `None` for `deliveries_json` in `write_record`: `record_content_writes_the_sinks_value` must KILL;
  - make `to_column` return `None` for `Some(vec![])`: `to_column_distinguishes…` **and** the server test's `"[]"` assertion must KILL.

- [ ] **Step 6: Run `./scripts/gate.sh`.** Expect all 0.

- [ ] **Step 7: Commit.** `feat(usage): record engine deliveries per call via a task-local sink (record-only; [] vs NULL)`, by pathspec.

### Task 3: Ship Part A and verify it live; record the spec's two amendments

**Files:**
- Modify: `docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md`, through `doc(action="update", id="dd69959ffc06248a", patch={body_edits:[…]})`.

- [ ] **Step 1:** Run `./scripts/rb.sh`, which refuses if HEAD is behind `origin`. Then ask the operator to run `/mcp` to reconnect. Expect the rebuild to exit 0.

- [ ] **Step 2: Live exact-join check.** Make three codescout calls. Then:
  - `sqlite3 .codescout/usage.db "select tool_use_id, deliveries_json from tool_calls where cc_session_id like '<sid8>%' order by id desc limit 3"`: all three `tool_use_id` are non-NULL and each `deliveries_json` is non-NULL;
  - compare with `jq -r 'select(.type=="assistant") | .message.content[]? | select(.type=="tool_use") | .id'` over this session's transcript: each `tool_use_id` occurs **exactly once** in the transcript.

  This is the spec's live check.

- [ ] **Step 3: Amend the spec** with a dated Amendment 2, appended to the spec's existing `## Amendments` section (Amendment 1 is already there).
  - **(a) V1 answered 2026-09-26:** transcripts record hook executions as `attachment` entries (`hook_success`: hook name, event, exit code, duration, `toolUseID`). They record injected text as `hook_additional_context` (SessionStart carried 5,692 chars in session `3c5b02df`). A Stop hook that emits nothing records no content. So companion deliveries enter `deliveries` from transcripts.
  - **(b) The `deliveries_json` shape is one entry per claiming engine,** `{engine, ledger_keys[], blocks[{sha256, bytes}], hint}`, not one per block. The coordinator can attribute keys to an engine but not to a block. The spec's purpose, which engine delivered what with `[]` versus NULL, is unchanged.
  - **(c) Metric 7 is `next_action_aligned`, not "changed".** Without a counterfactual, a pipeline can only observe whether the next action matched what the delivery said, as `aligned|not-aligned|unknown`. The readout labels it that way, never as effect.

- [ ] **Step 4: Commit the spec amendment** by pathspec.

---

## Part B — offline pipeline

All code goes in scripts/measure. Every module is importable by path and exposes the functions named below. A thin `scripts/measure/run.py` gives a CLI with the subcommands `freeze`, `join`, `mine`, `gate`, `sample`, `judge`, `readout`, `spotcheck`.

### Task 4: Stage 0 — freeze a corpus and its manifest

**Files:** create `scripts/measure/archive.py`; test in `tests/test_measure_archive.py`.

**Interfaces — produces:**
- `freeze(corpus_id: str, sources: dict, out_root: pathlib.Path) -> dict`.
  - `sources` has these keys:
    - `transcript_dirs`: the project dirs across profiles;
    - `usage_dbs`: paths;
    - `repos`: `{name: path}`;
    - `bounds`: `{start_utc, end_utc}`.
  - It copies into `out_root/corpus_id/`. It snapshots SQLite with `sqlite3 … ".backup"`, never `cp`. It writes `manifest.json` and returns the manifest.
  - `bounds` carries both windows, `{retained: {start_utc, end_utc}, decision: {start_utc, end_utc}}`, where `decision.start_utc` is `end_utc` minus 7 days (spec A1.2).
- `verify(corpus_dir: pathlib.Path) -> list[str]`: returns the mismatching relative paths; an empty list means intact.
- **Manifest keys:**
  - `corpus_id`, `created_utc`, `bounds`;
  - `files: {relpath: {sha256, bytes}}`;
  - `counts: {transcripts, subagent_transcripts, usage_rows}`;
  - `versions: {codescout_sha, claude_code, codex}`;
  - `repos: {name: head_sha}`;
  - `exclusions: []`, filled by Task 5.

- [ ] **Step 0: Check existing instruments (spec A1.8).** Read `docs/PROBES.md` and list each instrument that overlaps Stages 0–4, e.g. the claude-traces reader's `message.id` dedupe. For each, record reuse-after-predicate-check or declined-because, in `scripts/measure/README.md`. Tasks 5–11 follow that record.
- [ ] **Step 1: Write the failing tests.**
  - `test_verify_is_empty_on_an_intact_corpus`.
  - `test_decision_window_is_the_last_seven_days_of_the_retained_window`.
  - `test_verify_names_a_changed_file`: flip one byte, and `verify` returns exactly that path.
  - `test_freeze_refuses_an_out_root_inside_the_repo`: `ValueError` naming the Global Constraint.
  - `test_manifest_counts_subagent_transcripts_separately`: a fixture tree with `<sid>.jsonl` plus `<sid>/subagents/agent-x.jsonl`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement `freeze` and `verify`.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Commit** `feat(measure): stage 0 corpus freeze + manifest`.

### Task 5: Stage 1a — sessions, exclusions, forks

**Files:** create `scripts/measure/transcripts.py`; test in `tests/test_measure_transcripts.py`.

**Interfaces — produces:**
- `read_jsonl(path) -> tuple[list[dict], int]`: entries, plus the count of malformed lines skipped (Review Focus 4).
- `operator_messages(entries) -> list[dict]`. Keeps `type == "user"` entries that are:
  - not `isMeta`;
  - not a compaction summary;
  - whose content is a string or leads with a `text` item;
  - not a `tool_result`;
  - not wrapped in `<command-name>` / `<local-command-stdout>` / `<local-command-caveat>`.

  This is Review Focus 3.
- `sessions(corpus_dir) -> list[Session]`, a dataclass: `sid, path, profile, entrypoint, first_uuids: list[str], subagent_paths: list, first_ts, last_ts`.
- `exclusions(sessions, excluded_sids: set[str]) -> dict[str, str]`, mapping sid to reason. Reasons:
  - `sdk-cli`;
  - `scratchpad-project`;
  - `fork-of:<sid>`: SUPERSEDED by spec Amendment 3(c). Forks are labelled by `relations()` and counted through `attribute_entries`, not excluded;
  - `excluded-by-spec`: session `3c5b02df`, plus sessions whose task was this measurement.

- [ ] **Step 1: Write the failing tests.**
  - `test_tool_results_meta_and_command_wrappers_are_not_operator_messages`: a fixture holding one each of `tool_result`, `isMeta`, `<command-name>`, a compaction summary and one real prompt; exactly the real prompt is returned.
  - `test_a_truncated_last_line_is_skipped_and_counted`.
  - the fork-orientation twins and the `attribute_entries` tests of spec Amendment 3(c) (replacing `test_a_fork_is_excluded_and_its_original_kept`).
  - `test_sdk_cli_and_scratchpad_sessions_are_excluded_with_reasons`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: V2, the positive control on real data.** RE-POINTED by spec Amendment 3(a): the named fork runs are absent from every profile, so the controls are `d8a1f024` (fork of `571eb3d6`), the `.claude-sdd` prefix copy of `571eb3d6`, and a synthetic fixture. Original text: freeze a scratch corpus with Task 4, run `exclusions`, and check that every fork run named in `docs/evals/rule-tell-scoring-2026-09-23.md` (DP1, RTD-3 and the e2s forks) is excluded as `fork-of:` or `sdk-cli`. **If any known fork is kept, stop and find another signal before Task 6;** that is what the spec's verification requires. Record the list and the result in the commit message.
- [ ] **Step 6: Commit** `feat(measure): stage 1a sessions, exclusions, fork detection (V2: <n>/<n> known forks excluded)`.

### Task 6: Stage 1b — the events database and joins

**Files:** create `scripts/measure/join.py`; test in `tests/test_measure_join.py`.

**Interfaces — consumes:** Task 5's `sessions`, `read_jsonl`, `operator_messages` and `exclusions`.

**Interfaces — produces:**
- `utc(ts: str) -> datetime`, which parses both source formats (Review Focus 5).
- `build_events(corpus_dir, events_db: pathlib.Path) -> dict`, which returns row counts. Tables:
  - `turns(sid, agent_path, uuid, ts, role, kind, text, message_id, tokens)`, where `kind` is one of `prompt|assistant_text|tool_use|tool_result|meta`, and `tokens` is `input + output + cache_creation` from `message.usage`, **counted once per `message.id`**: one completion spans several JSONL lines carrying the same usage dict, and summing per line inflated `cc.py`'s totals 2.1–2.6× before its fix;
  - `tool_events(sid, tool_use_id, ts, name, input_json, usage_row_id, join_method)`, where `join_method` is `exact` (on `tool_use_id`, Task 1), `heuristic` (session + tool + argument equality + nearest `started_at` within 120 s), or `none`;
  - `deliveries(sid, ts, source, engine_or_hook, key, sha256, bytes, tool_use_id)`, with `source` in `usage_output_json|usage_deliveries_json|transcript_hook`;
  - `commits(repo, sha, ts, session_id, subject, files_json)`.
- Parse `output_json` injections from their markers, `<!-- operator-rule OP-N` and the `auto-injected get_guide('<topic>')` comment. Use `deliveries_json` wherever it is non-NULL.

- [ ] **Step 1: Write the failing tests.**
  - `test_utc_orders_mixed_formats_correctly`: `"2026-09-26 05:22:48"` sorts before `"2026-09-26T05:22:48.812Z"`, which sorts before `"2026-09-26 05:22:49.001"`.
  - `test_an_exact_join_wins_over_a_heuristic_candidate`.
  - `test_the_operator_rule_marker_parses_to_a_delivery_row`: the fixture is the literal OP-4 block text.
  - `test_every_session_is_in_events_or_in_exclusions`: the completeness check; build over a fixture corpus and assert that events plus excluded sessions equals all sessions.
  - `test_tokens_are_counted_once_per_message_id`: three JSONL lines sharing one `message.id` and usage dict give one `tokens` value, not three.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Real-data positive control.** On the scratch corpus, check that `deliveries` holds a row with `key` `OP-4` at `2026-09-26 05:22:48` UTC from `edit_file`. Record the count.
- [ ] **Step 6: Commit.**

### Task 7: Lessons frozen at a commit

**Files:** create `scripts/measure/lessons.py`; test in `tests/test_measure_lessons.py`.

**Interfaces — produces:**
- `lessons_at(repo: pathlib.Path, sha: str) -> list[Lesson]`, a dataclass: `id, source, text, dated: bool`.
  - Sources come from `git show <sha>:<path>`:
    - the `CLAUDE.md` laws: one lesson per `- **`-led bullet, and one per `##`/`###` section that has no such bullets, with `id = "CLAUDE.md#<heading-slug>[/<n>]"`;
    - `docs/trackers/operator-rules.md` entries (`OP-N`);
    - promoted `R-N`/`T-N` entries whose status reads promoted;
    - `.codescout/memories/*.md` where tracked.
- `undated_lessons(path: pathlib.Path) -> list[Lesson]` reads the operator's global `CLAUDE.md` with `dated=False`, so it never counts as "existing at origin".

- [ ] **Step 1: Write the failing tests.**
  - `test_a_lesson_added_after_the_commit_is_absent`: use a temp git repo, a lesson added in commit 2, and `lessons_at(commit1)`.
  - `test_global_claude_md_lessons_are_undated`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Commit.**

### Task 8: Stage 2 — correction miner

**Files:** create `scripts/measure/miner.py`; test in `tests/test_measure_miner.py`.

**Interfaces — consumes:** the `events.db` from Task 6, Task 5's `operator_messages`, and `mine_pairs.py`'s `commits`/`change_blocks` loaded by path from `docs/evals/data/2026-09-24-rule-tell/stage2/`.

**Interfaces — produces:**
- `candidates(events_db, repos) -> list[Candidate]`, a dataclass:
  - `cid, sid, detected_ts, source`, where `source` is one of `operator_message|correction_commit|review_commit|retraction`;
  - `corrector`, one of `operator|self|peer-session|review`;
  - `origin_hint: {uuid|sha, ts}`;
  - `text`.
- Recall-oriented: the judge confirms each candidate later, so false candidates cost judge calls and are not a bias.
- Corrector rules:
  - an operator message is `operator`;
  - a commit whose subject or body contains `review` is `review`;
  - a correction commit is `self` if its `Session-Id` equals that of the blamed parent's session, else `peer-session`.
- `group_by_origin(cands) -> list[list[Candidate]]`, so each mistake is counted once.

- [ ] **Step 1: Write the failing tests.**
  - `test_a_meta_or_tool_result_user_entry_never_becomes_an_operator_candidate` (Review Focus 3).
  - `test_blame_at_parent_names_the_origin_session`: a temp repo with two sessions' commits.
  - `test_review_commit_is_classified_review`.
  - `test_two_corrections_of_one_origin_group_once`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Mutate each filter by hand** in an isolated copy, the way `96b52f0c` did. Remove each operator-message exclusion, and swap `self`/`peer-session`. Each mutation must turn at least one test red; record the tally in the commit message.
- [ ] **Step 6: Real-data positive control.** The correction commits named in the rule-injection census (`docs/evals/rule-injection-timing-preregistration.md` § *Corpus*) that fall inside the corpus window are all proposed. Record found/total.
- [ ] **Step 7: Commit.**

### Task 9: Judge harness and gate

**Files:** create `scripts/measure/judge.py` and the prompt file `scripts/measure/judge_prompt.md`; test in `tests/test_measure_judge.py`.

**Interfaces — consumes:** `codex_complete(prompt, home, log_path)` loaded by path from `generate_synthetic.py`; Task 7's `lessons_at`; Task 6's `utc`.

**Interfaces — produces:**
- `build_input(item: dict, events_db, lessons: list, mode: str) -> dict`.
  - `mode` is `"correction"` or `"audit"`.
  - It raises `LeakageError` if the latest included timestamp is not **strictly** before `item["decision_ts"]` (compared with `utc()`).
- `parse_verdict(raw: str) -> Verdict`, a dataclass:
  - `is_correction: bool|None`, correction mode only;
  - `is_decision_point`;
  - `lessons: list[str]` or `"uncovered"`/`"abstain"`;
  - `detectability`, one of `in-trace|obtainable|external`;
  - `quote: str`;
  - `evidence_present_before`, `evidence_used`: each `yes|no|unknown` (spec A1.3), audit mode;
  - `lesson_outcomes: dict[str, str]`, lesson id to `applied|missed`, for each lesson the judge finds applicable (spec A1.4), audit mode.
- `verify_quote(v: Verdict, pre_evidence: str) -> Verdict`: an `in-trace` verdict whose quote is not verbatim in `pre_evidence` becomes `abstain`.
- `judge(item, votes=3) -> dict`: majority over 3 votes, keeping all three verdicts and the disagreement.
- `run_gate() -> dict` over the RTD cases and the 52 never-corrected controls, which are the 57 passages of `docs/evals/rule-tell-controls.md` minus the five its § *Five passages below are known positives* names.
  - RTD positives come from the pre-correction blob at the corpus SHA `78f7662c…` named in `docs/evals/rule-tell-detection.md`.
  - It returns `passed: bool` against the Global Constraints thresholds.
- The prompt file is frozen by sha256 in the spec's Amendments **before** `run_gate` first runs.

- [ ] **Step 1: Write the failing tests.**
  - `test_an_input_with_a_later_timestamp_is_refused` (leakage; Review Focus 5).
  - `test_a_quote_not_in_the_evidence_downgrades_to_abstain`.
  - `test_majority_keeps_all_votes_and_the_disagreement`.
  - `test_audit_verdict_carries_context_and_transfer_fields`: `parse_verdict` on an audit-mode reply returns `evidence_present_before`, `evidence_used` and `lesson_outcomes`. A reply missing any of them parses to `unknown` / `{}` and is flagged, never defaulted to `yes`/`applied`.
  - `test_gate_thresholds_match_the_spec`: `run_gate`'s constants are 16/21, 3/4, 6/8 and 5/52.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.** The `CODEX_HOME` config pins the model. `codex --version` and the model name are written into every output header.
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Register the prompt.** Add the prompt's sha256 to the spec's Amendments, commit, **then** run `run.py gate`.
- [ ] **Step 6: Run the gate.** Commit its full output under `docs/evals/data/<date>-system1-base-rates/gate.txt`. **If `passed` is false, the measurement is INCONCLUSIVE by rule.** Stop the plan here and report to the operator; do not re-tune the prompt against the gate.

### Task 10: Audit sampler and pilot (V3)

**Files:** create `scripts/measure/sampler.py`; test in `tests/test_measure_sampler.py`.

**Interfaces — produces:**
- `draw_audit(events_db, project: str, target: int, seed: int) -> Iterator[dict]`: assistant messages in a seeded uniform order. The caller keeps drawing until `target` qualify.
- `draw_census(cands, cap=400, seed) -> tuple[list, float]`: the sample and its sampling fraction, which is 1.0 below the cap.

- [ ] **Step 1: Write the failing tests.**
  - `test_draw_is_reproducible_from_the_seed`.
  - `test_census_below_cap_is_complete_with_fraction_one`.
  - `test_non_qualifying_draws_are_counted` (the spec's qualifying-share result).
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Pilot.** 20 items end to end, drawn with a seed distinct from the main draws and excluded from every estimate. Record Codex wall time and calls per item in the spec's Amendments. If the main sizes are unaffordable, re-register them there **before** any main item is judged.
- [ ] **Step 6: Commit.**

### Task 11: Readout, go/no-go and spot-check

**Files:** create `scripts/measure/readout.py`; test in `tests/test_measure_readout.py`.

**Interfaces — produces:**
- `wilson(k: int, n: int, z=1.96) -> tuple[float, float]`.
- `session_bootstrap(per_session: list[float], resamples=10000, seed: int) -> tuple[float, float]`: a 95% percentile interval.
- `decide(lower: float, upper: float, gate_passed: bool, agreement: int) -> str`, returning `GO|NO-GO|INCONCLUSIVE` under the Global Constraints rule.
- `spotcheck_packet(judged, seed) -> list[dict]`: 25 items, stratified 10 / 10 / 5, **with judge labels removed**.
- `agreement(operator_labels, judge_labels) -> dict`: raw k/25 and Cohen's κ on addressable yes/no, with `abstain` counted as no. **Also reported per field** for `evidence_present_before`, `evidence_used` and applied/missed. Only the addressable label feeds `decide` (spec A1.7).
- `miner_recall(audit_misses, candidates) -> tuple[int, int]`: of the audit-found misses that an operator message corrected later in the same session, how many the miner proposed. Returned as `(found, total)` and reported with its Wilson interval. **This is the only check that can see a miner biased toward NO-GO.**
- `cost_between(events_db, sid, start_ts, end_ts) -> dict`: `{tokens, tool_calls, operator_turns, wall_s}` from the origin turn to resolution; `tokens` comes from Task 6's once-per-`message.id` counts.
- `next_action_aligned(delivery, next_tool_event) -> str`: `aligned` when the next tool call's name or path argument appears in the delivered text, `not-aligned` when a next call exists and neither does, `unknown` when there is no next call in the session.
- `context_support(verdicts) -> dict`: over qualifying audit decision points, the count and Wilson interval where `evidence_present_before == "yes"` and `evidence_used == "yes"`, with `unknown` counts shown (spec A1.3).
- `transfer_rate(verdicts) -> dict`: applied / (applied + missed) over lesson-applicable decision points, split by project and by dated/undated lessons (spec A1.4).
- `task_family(tool_name: str, input_json: str | None) -> str` and `delegable_share(events_db, window) -> dict`: the deterministic classifier and per-session shares of calls and recorded latency (spec A1.5). **Latencies are never summed across overlapping calls into a time-saved figure.**
- `render(…) -> str`: the readout markdown. It **begins with the spec's § *What a result cannot establish*, copied verbatim**, then the metrics per corpus and project, weekly breakdowns, the contrast project side by side (never pooled, with a generalisation warning when the two rates differ by more than 2×), and the miner-recall estimate.

- [ ] **Step 1: Write the failing tests.**
  - `test_decide_matches_the_registered_rule`, as a table:
    - (0.31, 0.9, True, 20) gives GO;
    - (0.1, 0.29, True, 25) gives NO-GO;
    - (0.2, 0.5, True, 25) gives INCONCLUSIVE;
    - (0.4, 0.9, False, 25) gives INCONCLUSIVE;
    - (0.4, 0.9, True, 19) gives INCONCLUSIVE.
  - `test_wilson_matches_the_spec_example`: `wilson(20, 25)` is about (0.61, 0.91).
  - `test_spotcheck_packet_carries_no_judge_label`.
  - `test_readout_begins_with_the_limits_section`.
  - `test_miner_recall_counts_only_operator_corrected_audit_misses`: an audit miss with no later operator correction is excluded from the denominator.
  - `test_cost_between_uses_once_per_message_tokens`.
  - `test_next_action_aligned_is_unknown_without_a_next_call`.
  - `test_task_family_classifies_each_family`: one fixture per family (`doc` `append_entry` on a `docs/trackers/` path, `run_command` `./scripts/gate.sh`, `edit_code` `rename`) plus an `other`.
  - `test_delegable_share_never_sums_overlapping_latency_into_elapsed_time`: two overlapping calls give shares of recorded latency, and no field named or described as elapsed or saved time.
  - `test_transfer_rate_denominator_excludes_non_applicable_decision_points`.
  - `test_only_the_addressable_label_feeds_decide`: per-field agreement below 20/25 on a descriptive field leaves `decide` unchanged.
  - `test_decide_reads_only_the_decision_window`: a session outside `decision` bounds does not enter the per-session rates that `decide` receives.
  - `test_a_contrast_rate_over_twice_codescouts_emits_the_warning`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Commit.**

### Task 12: Run it and publish (operator-gated)

- [ ] **Step 1: Freeze the real corpora.** codescout plus the contrast project: MRV-poc, or backend-kotlin if MRV-poc has fewer than 15 sessions after exclusions. The contrast is compared on its retained window, descriptive only (spec A1.2). `bounds` carries the retained window (oldest surviving transcript to the freeze instant T) and the decision window (the last 7 days before T). Commit only the manifests under `docs/evals/data/<date>-system1-base-rates/`.
- [ ] **Step 2: Run** `join`, then `mine`, then `sample`, then `judge`, all under the registered seeds.
- [ ] **Step 3: Generate the spot-check packet** and hand it to the operator. **Wait for their 25 labels;** no readout is final before them.
- [ ] **Step 4: Run `readout`.** Commit the readout doc under `docs/evals/` citing the corpus id, plus the gate, pilot and agreement files. Report to the operator in one line: the go/no-go decision with its interval, and the three descriptive baselines (context support, transfer rate, delegable-work share), each with its unit and window.
- [ ] **Step 5: If the result is INCONCLUSIVE,** record the prospective window's read date in the readout. The window covers `[T − 7 days, T_live + 21 days)`, where T_live is the day after Task 3 Step 2 passes (spec A1.2). It is read under the same rule.

### Task 13: The observability map (runs after Task 6, before Task 7)

**Why here:** spec A1.6 requires the map before any Codex call. It needs Task 6's events database, and nothing from Tasks 7–11.

**Files:** create `scripts/measure/observability.py`; test in `tests/test_measure_observability.py`. The output is `docs/evals/data/<date>-system1-base-rates/observability-map.md`, committed.

**Interfaces — consumes:** Task 6's `events.db` and Task 5's `exclusions`.

**Interfaces — produces:**
- `coverage(events_db, manifest) -> dict`, which counts:
  - `tool_events` by `join_method`;
  - `deliveries` by `source`;
  - sessions per project and window;
  - exclusions by reason;
  - turns by `kind`.
- `render_map(coverage: dict) -> str`: for each outcome (mistakes, context, transfer, background-worker) and each chain link (opportunity, signal or request, delivery or action, observed use, checked outcome), one of `measurable now` / `needs adjudication` / `unobservable`, with the coverage number that justifies the label, or the reason none exists. Rediscovery (A1.4) is always `needs adjudication`.

- [ ] **Step 1: Write the failing tests.**
  - `test_every_outcome_and_chain_link_has_a_label`: the rendered map has 4 × 5 labelled cells and no blank.
  - `test_a_link_with_zero_coverage_is_never_labelled_measurable_now`.
  - `test_rediscovery_is_needs_adjudication`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Produce the map on a real frozen corpus.** Use Task 5's scratch corpus until Task 12 freezes the real one; Task 12 then re-runs this step. Commit the map, then commit the code.

**Not in this plan:** V4, the Langfuse completeness check. The spec excludes Langfuse, and nothing here depends on it.
