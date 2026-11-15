---
id: '4456f8d93eeced1c'
kind: plan
status: parked
title: 'audit_log resume cursor: a forward read over the catalog audit trail'
tags:
- audit-log
- cursor
- managed-agents
- agent-memory
---

# audit_log resume cursor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `librarian(action="audit_log", after_seq=N)` returns the rows this host wrote after audit `seq` N, oldest first, with a next cursor and a completeness flag, so a caller resumes where it stopped without timestamps.

**Architecture:** Extend the existing `audit_log` query mode. The catalog layer gains one filter field (`after_seq`) and one ordering rule. The tool layer gains one argument, one output block (`cursor`) and three refusals. No new tool, no CLI, no new table.

**Tech Stack:** Rust, rusqlite (`catalog_audit`), the `librarian` tool schema.

**Spec:** none. The plan carries its decisions (P1-P8), as the hot-view plan did. Input: memory `research/managed-agents-interfaces-20261115` (Anthropic, "Scaling Managed Agents", 2026-04-08: a session log answers positional reads; the harness shapes the context). That memory first said no read of the logs existed. That was wrong, and the memory now says so: the read exists, only the resumable forward read is missing.

**Where this file lives:** on `experiments` (docs only). Integration receives it when `experiments` is merged into it. Until then an executor reads it from the main checkout.

## Review outcome (2026-11-15): parked until a consumer exists

An independent Opus review (reconnaissance + architecture-snow-lion, reading only, no cargo) found 1 Critical, 5 Important and 11 Minor findings and a verdict of **not ready, and not needed yet**. I re-read the decisive evidence: `src/server.rs:6611-6625` (budget log: "headroom 0", `TOOL_SURFACE_CHAR_BUDGET` = 57_664), `src/server.rs:6663-6666` (raising IS allowed with a harness reason) and `src/prompts/guide_index.rs:273` (`MAX_DECLARED_SECTION_BYTES` = 2500). The review's report was a job-local file; its content is kept here.

**Decision: park.** Do not build until a consumer is named. For now the answer costs zero tool-surface bytes: poll with `audit_log(since=<stored at_ms>, limit=500)` and drop the boundary rows client-side (`since` is `>=`, so it re-reads them) with `seq > <stored seq>`. Read `truncated` and `filtered_total`: they say "capped". Measured by the reviewer: 348 rows machine-wide in 24 hours, below `MAX_LIMIT` 500. The cursor would add only paging past 500 rows in one gap (a reindex burst) and independence from the wall clock.

**Two statements above and below are wrong, and stay corrected here:** (1) a `since` poller CAN tell caught up from capped (`truncated`, `audit_log.rs:323-334`); (2) `TOOL_SURFACE_CHAR_BUDGET` is not strictly lower-only: a raise is allowed with a harness reason, and content pressure is not one (`src/prompts/README.md` § Caps follow the harness). The comment `types.rs:1699` says "lower-only" and contradicts `server.rs:6663`.

**Fix before this plan is unparked (the reviewer's findings):**
- **C1 (Critical).** Task 2 cannot pass `tool_surface_under_budget`: headroom is 0 (the memory said at most 65), and the `librarian` description is 1,765 of 1,800 characters, so at most 35 may be added. Add a Task 0 that measures with `tool_surface_report_lengths`, names the sentence elsewhere that gives the bytes, or records an operator decision that this is not funded. That measurement is the go/no-go.
- **I1.** `after_seq > head_seq` misses a rebuilt catalog whose new head has already passed the old cursor (rows 1..N skipped silently). A new catalog mints a new host id (`host.rs` `mint_host_id`), so use `cursor.host` as the cursor's epoch: accept `after_host`, set `cursor.reset: true` when it differs, and test two catalogs.
- **I2.** Filters (`tbl`, `actor`) hide the prune marker, and the marker arrives after the gap (its `seq` is the newest). Add `oldest_seq` and `gap_before_page` (`after_seq + 1 < oldest_seq`); test with a filter that excludes the marker and a cursor strictly inside the pruned range, not at 0.
- **I3.** The Task 3 guide sentence (about 140 B) will exceed `MAX_DECLARED_SECTION_BYTES` 2500 (the `librarian` reference section is about 2,480 B). Split the section at a `###` or move the sentence to an undeclared section; count the `CEILING` p50 emission budget too.
- **I4.** The premise (above): no consumer; `since` plus `seq` covers it today.
- **I5.** An adapter call runs `gc::maybe_reconcile` first (`adapter.rs:389-392`, `gc.rs:108-122`), which can write audit rows under the reader's actor, at most once per interval. `reading_the_log_does_not_move_the_head` calls `audit_log::call` directly and cannot see it. Reword Review Focus 5 to "a polling loop terminates" and assert it through the adapter, or say the adapter path is out of scope.
- **Minor:** reads are not one snapshot (use one deferred read transaction, or derive `complete = filtered_total == count`); `limit=0` never advances (refuse it with `after_seq`); a `u64` above `i64::MAX` wraps negative (use `i64::try_from`); `seq_is_not_reused_after_a_prune` cannot fail because the marker row keeps the max (delete every row with raw SQL, marker included); assert ascending `seq` order at the tool level with a fixture whose `at_ms` falls as `seq` rises; specify the cursor-mode response (keep `shards.self_host`, `table_total`, `unit`, `note`; the truncation hint must say `after_seq=<next_after_seq>`); the `limit` and description texts say "newest first" and go stale; the param sweep needs only `after_seq` in the unprobed list and the comment/doc key lists (the floor 39 does not change); the existing sweep comment "every librarian call writes an audit row" is already false live; `read_door` test is `no_librarian_production_read_bypasses_the_read_door` (`read_door.rs:1414`); `actor` is one per process under the HTTP transport (`server.rs:3696-3707`); add the `BEGIN IMMEDIATE` two-connection test for P8; split Task 2 into 2a (logic, `audit_log.rs` only) and 2b (surface, needs the Task 0 decision).

## What exists today (read 2026-11-15)

- `librarian(action="audit_log")` (`src/librarian/tools/audit_log.rs:148`, `call`) filters `catalog_audit` by `tbl`, `row_id`, `actor`, `op`, `since`, `until` (epoch ms). Order is `seq DESC`; `limit` default 50, max 500; it returns `truncated`, `filtered_total`, a `scope` block and other hosts' committed shard rows.
- `catalog_audit.seq` is `INTEGER PRIMARY KEY AUTOINCREMENT` (`src/librarian/catalog/audit/mod.rs:213`): strictly increasing, never reused after a delete or a prune.
- `actor` is `codescout:<session-id>` (`resolve_actor`, `mod.rs:346`), so "what did session X write" is already answerable newest-first.
- `events` is an audited table, so one cursor covers event inserts too. `doc(action="event_list")` is per artifact and stays as it is.
- A prune deletes rows and inserts ONE marker row (`tbl='catalog_audit'`, `row_id='prune'`, `prune_before`, `mod.rs:493`), so a seq gap explains itself.
- The table is machine-wide: `filter_where` has no repo predicate (`mod.rs:419`). Verified live today: the newest row belonged to another session's `doc.create`.
- Two consecutive live `audit_log` reads (limit 2) returned the same newest `seq` (315858) and the same `table_total`: a read wrote no audit row.

**The gap:** no `after_seq`, no ascending order, no next cursor. A caller that polls must use `since` (milliseconds, `>=`, so it re-reads the boundary row) and cannot tell "caught up" from "capped".

## Decisions

- **P1 No new tool, no CLI.** Extend `audit_log`. A new tool spends the tool surface (`TOOL_SURFACE_CHAR_BUDGET` is lower-only). A CLI over this catalog would need repo attribution (`shard::attribute`) to avoid printing other repos' rows to a hook; that is parked until a hook needs it. Cost if wrong: a hook cannot call it without MCP.
- **P2 The cursor is the local `seq`.** `seq` is per host and not comparable across hosts (`audit_log.rs` merge comment). With `after_seq` set, the call reads the local table only and does not read shard files; the response says so.
- **P3 Order follows the cursor.** With `after_seq`: ascending by `seq`, and `limit` keeps the first rows after the cursor. Without it: unchanged (newest first). The response carries `order`.
- **P4 The response names the cursor.** `cursor = {host, after_seq, next_after_seq, head_seq, complete, ahead_of_head}`. `next_after_seq` is the largest `seq` returned, or the input cursor when the page is empty. `complete` is true when no row after `next_after_seq` matches the filters; compute it with `count_matching` under the same lock, never from `truncated`.
- **P5 Refusals, by name.** A negative or non-integer `after_seq` is refused (`ArgType::Count`). `after_seq` with `export` or `prune_before_ms` is refused (IC-15: never silently dropped). Add `"after_seq"` to `PRUNE_IGNORED_FILTER_KEYS`; that list already guards both.
- **P6 A stale cursor is said, not hidden.** `head_seq` is the newest local `seq` (0 when the table is empty). `after_seq > head_seq` sets `ahead_of_head: true` and a hint: the catalog was rebuilt or replaced (a new machine or a deleted `catalog.db` restarts the counter); restart from 0. A cursor inside a pruned range meets the prune marker row in order.
- **P7 Scope stays machine-wide, and says so.** `after_seq` rows may belong to other repos. Callers narrow with `actor`, `tbl` or `row_id`. `scope` gains `after_seq`. No `exclude_actor` (parked: a consumer that wants "everyone but me" needs it).
- **P8 Concurrency.** SQLite serialises writers, so a lower `seq` cannot commit after a higher one: a cursor does not skip a row. This is reasoned from that rule, not tested. A row written after the completeness count shows on the next call, because it has a larger `seq`.

## Global Constraints

- `AuditFilter` and `shard::matches` stay in parity: `matches` destructures `AuditFilter` exhaustively (`shard.rs:762`), so a new field fails the build until it is decided there.
- Typed arguments: every `audit_log` argument has an `ARG_TYPES` row (`audit_log.rs:62`); a wrong type is refused by name.
- No new cap constant. Reuse `DEFAULT_LIMIT` and `MAX_LIMIT`. Any new constant named like a cap needs a `// cap-class:` line (`tests/result_caps.rs every_cap_constant_is_classified`).
- Negative results name their scope (`docs/adrs/2026-08-27-negative-results-name-their-scope.md`): an empty page states the filters, the cursor and `head_seq`.
- Tool surface: `tool_surface_under_budget` must pass. Do not raise `TOOL_SURFACE_CHAR_BUDGET`. Record the headroom before and after (Task 2, Step 1).
- Do not touch identity code. The pinned contracts in memory `tracker-lifecycle-parked-items` are not touched.
- Repo rules: no push; path-only commits; `workspace=` pinned when in a worktree; run `./scripts/gate.sh` on the final head. Per-task reviews run no suite, so also run `scripts/with-slot.sh cargo test --test result_caps` and the `read_door` scanner test before the whole-branch review (F-4 and F-6 each missed one).
- Base and landing: branch from `integration/identity-lifecycle-20261114` in a worktree (`git worktree add <path> -b <branch> <integration>`, then `EnterWorktree` with `path`). `experiments` stays untouched. Land by the integration agreement with `codescout-5f`. This work touches `src/librarian/tools/librarian.rs`, so tell `codescout-5f` before and after.

## Review Focus

Each line has its test in the owning task.

1. A cursor ahead of the head (catalog rebuilt) says so, instead of returning an empty page that reads as "caught up". (Task 1, Task 2)
2. A cursor inside a pruned range meets the marker row, not silence. (Task 1)
3. An empty page keeps the input cursor, reports `complete: true` and names its filters. (Task 2)
4. A page of exactly `limit` rows at the end is `complete: true`; one more row makes it `false` (off-by-one on `complete`). (Task 2)
5. Reading the log does not move the head, so a polling loop terminates. A failing test here means the read writes its own row: stop and report it. (Task 2)
6. `after_seq` with `export` or `prune_before_ms` is refused by name. (Task 2)
7. Shard rows never leak into a cursor page, and the response says why. (Task 1, Task 2)

## File Structure

- Modify `src/librarian/catalog/audit/mod.rs`: `AuditFilter.after_seq`, `filter_where`, `query` ordering, `head_seq`.
- Modify `src/librarian/catalog/audit/shard.rs`: `matches` handles `after_seq`.
- Modify `src/librarian/tools/audit_log.rs`: argument, refusals, `cursor` block, cursor-mode branch.
- Modify `src/librarian/tools/librarian.rs`: schema property, description sentence, the unprobed-key list in the param sweep.
- Modify the `librarian` guide text (`audit_log` row) after reading `src/prompts/README.md` for the guide's cap rule.

## Task 1: catalog layer

**Files:** `src/librarian/catalog/audit/mod.rs`, `src/librarian/catalog/audit/shard.rs`.

**Interfaces:**
- Produces: `AuditFilter.after_seq: Option<i64>`; `query()` orders `seq ASC` when `after_seq` is `Some`, else `seq DESC`; `pub(crate) fn head_seq(conn: &Connection) -> Result<i64>`; `matches()` returns false for every shard row when `after_seq` is `Some`.

- [ ] **Step 1: Write the failing tests** in the `mod.rs` test module (names fixed; assertions as written):
  - `after_seq_returns_rows_strictly_after_the_cursor_oldest_first`: insert 5 rows, `after_seq` = seq of row 2, `limit` 10 → rows 3,4,5 in ascending order.
  - `after_seq_composes_with_the_other_filters`: with `actor` set, only that actor's rows after the cursor.
  - `a_cursor_inside_a_pruned_range_meets_the_marker_row`: write rows, `prune_before`, query from `after_seq=0` → the marker row (`tbl='catalog_audit'`, `row_id='prune'`) appears in seq order.
  - `seq_is_not_reused_after_a_prune`: write, prune everything, write one more → its `seq` is greater than every earlier `seq`.
  - `head_seq_is_zero_on_an_empty_table_and_the_newest_seq_otherwise`.
  - In `shard.rs` tests: `shard_rows_never_match_a_cursor_filter`.
- [ ] **Step 2: Run them** (`scripts/with-slot.sh cargo test --lib audit::`); expect compile failure, then red.
- [ ] **Step 3: Implement.** `filter_where` adds `AND seq > ?` for `after_seq`. `query` picks the order from `f.after_seq`. `head_seq` is `SELECT COALESCE(MAX(seq), 0)`. `matches` destructures `after_seq` and returns false when it is `Some`. Comment why: `seq` is per host.
- [ ] **Step 4: Run the tests green; run `cargo test --lib audit::` whole module.**
- [ ] **Step 5: Commit** (path-only).

## Task 2: tool layer

**Files:** `src/librarian/tools/audit_log.rs`, `src/librarian/tools/librarian.rs`.

**Interfaces:**
- Consumes: Task 1.
- Produces: argument `after_seq` (non-negative integer). In cursor mode the response has `order: "asc"`, `cursor: {host, after_seq, next_after_seq, head_seq, complete, ahead_of_head}`, `filtered_total` = local matching rows after the cursor, `truncated = !complete`, `shards: {"read": false, "reason": "after_seq reads this host's local trail only"}`, `scope.after_seq`. Without `after_seq` the response is byte-for-byte as today.

- [ ] **Step 1: Record the tool-surface headroom before any edit:** `scripts/with-slot.sh cargo test --lib tool_surface_report_lengths -- --nocapture`; write the remaining characters into the ledger.
- [ ] **Step 2: Write the failing tests** in `audit_log.rs` tests (use `mk_ctx` and the existing row helpers):
  - `a_cursor_walk_visits_every_row_once_and_ends_complete`: 7 rows, `limit=3` → pages of 3, 3, 1; the union is all 7 rows once; only the last page has `complete: true`; feeding `next_after_seq` back advances.
  - `a_page_of_exactly_limit_rows_at_the_end_is_complete`: 3 rows after the cursor, `limit=3` → `complete: true`; add a 4th row → `complete: false`.
  - `an_empty_page_keeps_the_cursor_and_names_its_scope`: cursor = head → `count: 0`, `next_after_seq` = input, `complete: true`, `scope` names the filters, `head_seq` present.
  - `a_cursor_ahead_of_the_head_says_so`: `after_seq` = head + 100 → `ahead_of_head: true` and a `hint` containing `head_seq`.
  - `reading_the_log_does_not_move_the_head`: `head_seq` equal before and after two cursor calls. If this fails, stop: the read writes a row. Report it; do not weaken the test.
  - `after_seq_with_export_or_prune_is_refused_by_name`: both errors name `after_seq`.
  - `a_cursor_read_ignores_foreign_shard_rows_and_says_so` (use `write_foreign_shard`): no foreign host in `entries`; `shards.read` is false.
  - Extend `a_wrong_typed_argument_is_refused_by_name_and_never_read_as_absent` with `("after_seq", json!("5"), "a non-negative integer")` and a negative-number case.
- [ ] **Step 3: Run them red.**
- [ ] **Step 4: Implement** `audit_log::call`: add the `ARG_TYPES` row and the `PRUNE_IGNORED_FILTER_KEYS` entry; parse `after_seq` into the filter; branch early when it is `Some`: query ascending, compute `head_seq`, `next_after_seq`, `complete` (second `count_matching` with `after_seq = next_after_seq`), build the response above. Leave the non-cursor path untouched.
- [ ] **Step 5: Schema and description in `librarian.rs`:** add the `after_seq` property (one sentence, labelled `audit_log:`), one clause in the description, add `"after_seq"` to the unprobed-key list in the param sweep test with its comment kept true. Read the sweep's own count output and update the pinned numbers it names; do not guess them.
- [ ] **Step 6: Run** the new tests, the `librarian` param-sweep tests (`every_action_labelled_schema_key_is_honored_by_that_action`, `no_action_reads_a_key_its_description_does_not_name`), `tool_surface_under_budget`, and the whole `audit_log` test module. Record the headroom after. If the surface does not fit, shorten the new text; never raise the budget.
- [ ] **Step 7: Commit** (path-only).

## Task 3: guide text and records

**Files:** the `librarian` guide (`audit_log` row), memory `research/managed-agents-interfaces-20261115`, this plan's Outcome.

- [ ] **Step 1: Read `src/prompts/README.md`** for the guide's cap rule and the repo-neutral rule (a test enforces both).
- [ ] **Step 2: Add one sentence** to the `audit_log` row: `after_seq=N` reads this host's rows after N, oldest first, with `cursor.next_after_seq` to resume; machine-wide, so narrow with `actor` or `tbl`.
- [ ] **Step 3: Run the guide tests** (the tests that read the guide text).
- [ ] **Step 4: Update the memory and add `## Outcome`** to this plan (what shipped, the headroom before and after, parked items). Commit (path-only).

## Final steps (controller)

Whole-branch review on an Opus model (brief: `codescout-companion:reconnaissance`, `buddy:architecture-snow-lion`), one fix wave, `./scripts/gate.sh` on the final head, then the integration landing and announcements.

## Parked

- A hook or CLI that reads the cursor (repo attribution first).
- `exclude_actor`.
- A cursor over `event_list` (per artifact; its ids are monotonic per thread only).
- A per-session log of an agent's reads and tool calls: `catalog_audit` records catalog writes only.

## Self-review notes

- Spec coverage: the gap named above is covered by P1-P8 and Tasks 1-3.
- Not verified when written: the headroom of the tool surface (measured in Task 2, Step 1); the pinned numbers in the param sweep; the guide's cap rule; P8 (reasoned, not tested).
- Consumer risk: no consumer exists yet. The first likely one is a session that stores `(host, seq)` before compaction and asks "what did other sessions write since". If no consumer appears, the cost is one argument and about 30 lines.
