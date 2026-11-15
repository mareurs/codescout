---
id: '4e9a786add22cde7'
kind: plan
status: draft
title: Collection Hot View Implementation Plan
---

# Collection Hot View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give a lifecycle collection a hot view: the live entries that need attention, capped, flagged when stale, and rendered by one `codescout hot-view` command that a session-start hook can call.

**Architecture:** Library first (`src/librarian/lifecycle/`): the `status_since` table column, one shared "revisit is due" predicate, two optional readme keys (`hot_cap`, `stale_after`), and a pure `hot_view` module over `LifecycleTree`. Then one read-only CLI command beside `key-gate` and `constitution-check`. The hook that calls the command lives in `../claude-plugins` and ships separately.

**Tech Stack:** Rust, `serde_json`, `chrono::NaiveDate`, `clap`, the existing `lifecycle::{schema, entry, tree, table, checks, collection}` modules.

**Spec:** none. The plan carries its own decisions (§ Decisions). Input: the design note `/home/marius/work/claude/plugins/notes/2026-11-15-memory-system-for-ledgers-design-note.md` (outside this repository, not committed; everything the executor needs is copied below). Parent spec: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (goals G1-G3, anti-goals, D3, D5).

## Decisions

Operator decisions, 2026-11-15:

1. The hot-tier cap is a key in the collection `readme.md` schema.
2. Aliases are a declared `aliases` list field, data only. An alias never resolves inside `[[ ]]`. (This plan adds no reader of `aliases`; Task 1 only proves that the field is filterable.)
3. The dashboard is a read-only board first, writes later and only through the `doc` tools. No dashboard work is in this plan; the board will call the same command (`--json`).
4. The hot-view loader is a session-start hook in the companion plugin, built on a codescout CLI command that renders the view.
5. The lifecycle line writes this plan.

Rulings made in this plan (change them in the plan, not in the code, if you disagree):

- **R1, the open-entry gap.** No check of the eight pinned names questions an `open` entry nobody touches. This plan adds a `stale` flag on the hot view, not a ninth check. The threshold is the readme key `stale_after` (`<n>d`, same grammar as `review_every`). Without the key nothing is flagged: staleness comes from a declared field, never from age alone (parent spec G3). `status_since` is the age source, so re-confirming an entry (rewriting the same status) clears the flag.
- **R2, what is hot.** A live (not archived) entry is hot when its class is `open`, or when its class is `blocked` or `deferred` and its `revisit_by` is strictly before today. Terminal entries and entries whose status the readme does not declare are never hot.
- **R3, order and cap.** Order by `status_since` descending, then key ascending. `hot_cap` (a positive whole number) keeps the first N; the view says how many it hid. No `hot_cap` means no cap.
- **R4, scope.** The view reads one collection folder (`load_collection`, never `load_docs`). It runs no doctor check and its header says so. `check` on a one-folder tree silently drops `cited_target_closed` (`checks.rs`, `tree.closed_dates` holds only that folder), so the view uses no `check` at all.
- **R5, no pinning.** A `pinned` field is not built. A collection can declare one and filter on it with `doc(get, entry_filter)`.

Not in this plan (operator): a Codex adapter, a second manifest, an importer for another tool's memory files, dashboard writes, any work on identity code, the hook itself (`../claude-plugins`, § Contract for the hook).

## Global Constraints

- **Net-neutral advertised surface.** `TOOL_SURFACE_CHAR_BUDGET` stays `57_664` in `src/server.rs` (65 characters of headroom, measured 2026-11-14). No task edits a tool description or adds a tool argument. Tasks 4 and 6 run `scripts/with-slot.sh cargo test --lib tool_surface_under_budget`.
- **Pinned contracts untouched.** The eight names in `CHECK_NAMES`, the `Finding` serialization and `identity::index::DocInput` do not change. Nothing under `src/librarian/identity/` changes.
- `today` is a parameter of every library function. The CLI passes `chrono::Utc::now().date_naive()` (UTC, as `doc(create)` does).
- "Due" means strictly before `today` (`revisit_by < today`), as `check_entry` has it. "Stale" uses the same strictness (`status_since + n days < today`).
- The library code reads no file. The CLI reads through `load_collection` only, so `no_librarian_production_read_bypasses_the_read_door` stays green; run it in Task 5 anyway.
- Every new output names its scope. A zero is never printed alone (`docs/adrs/2026-08-27-negative-results-name-their-scope.md`).
- Run `./scripts/fmt-mine.sh` before each commit (the hook runs `rustfmt --check`). A commit message cites no 16-hex artifact id. No push anywhere.
- Tests run through `scripts/with-slot.sh cargo test --lib <name>`.

## Review Focus

Failure modes the decisions imply and no existing test covers, most likely first. Each has a test in the task named.

1. **A collection with many hot entries and no `hot_cap`.** Expect every hot entry shown and `hidden` 0 (Task 5).
2. **Two entries with the same `status_since`.** Expect key order, identical on every run (Task 5).
3. **A blocked entry whose `revisit_by` is today.** Expect not due and not hot (Tasks 3 and 5).
4. **An entry file the tree cannot read.** Expect the view to say that problems were found, not to look like a clean empty view (Task 6).
5. **An unknown, badly spelt or archived collection key.** Expect exit 2 and `NOT SHOWN — <reason>`, never an empty view that looks healthy (Task 6).

## File Structure

| File | Responsibility |
|---|---|
| `src/librarian/tools/get_table_tests.rs` | list-field and `status_since` filters through `doc(get, entry_filter)` |
| `src/librarian/lifecycle/table.rs` | `EntryRow.status_since`; `cell` visible to `hot_view` |
| `src/librarian/lifecycle/schema.rs` | `TABLE_COLUMNS`, the keys `hot_cap` and `stale_after`, `parse_days` |
| `src/librarian/lifecycle/entry.rs` | `revisit_due`, the one definition of "due" |
| `src/librarian/lifecycle/checks.rs` | `check_entry` calls `revisit_due` |
| `src/librarian/lifecycle/hot_view.rs` (new) | `hot_view`, `render_markdown`: pure, no I/O |
| `src/cli/hot_view.rs` (new) | `codescout hot-view`: load, render, exit codes |
| `src/cli/mod.rs`, `src/main.rs` | module and `Commands::HotView` wiring |

---

### Task 1: A list-field filter reaches the array branch

**Files:**
- Modify: `src/librarian/tools/get_table_tests.rs` (add `GLOSSARY`, `term`, one test)

**Interfaces:**
- Consumes: the file's `project`, `mk_ctx`, `seed`, `get` helpers (read them first).
- Produces: nothing a later task uses. It proves that `entry_filter` on a collection's flat rows reaches the array branch of `eval_leaf` (`src/librarian/filter.rs`, the `Contains` and `In` arms), which Decision 2 relies on. No test covered it before.

- [ ] **Step 1: Write the test `a_list_field_is_filterable_by_membership`.** Add a `GLOSSARY` readme (`key_style: name`, kind `term`, statuses `{open: open, gone: terminal}`, `initial_status: open`, field `aliases: {type: list}`) at `docs/trackers/ledgers/g/readme.md`, and a helper `term(title, more)` that writes an entry of kind `term` with `status: open`, `status_since` and `created` 2026-11-01. Three entries: `weekly-status` with `aliases: [wsr, status-report]`, `risk-log` with `aliases: [rl]`, `glossary-intro` with no `aliases` line. Assert, through `get(&ctx, json!({"id": id, "entry_filter": ...}))`:
  - `{"aliases": {"contains": "wsr"}}` returns exactly `["weekly-status"]`;
  - `{"aliases": {"contains": "nope"}}` returns `[]` and `entry_total` is 3;
  - `{"aliases": {"in": ["rl", "wsr"]}}` returns `["risk-log", "weekly-status"]` (rows sort by `created` descending, then key ascending);
  - the entry with no `aliases` line is in none of the three results.
- [ ] **Step 2: Run it.** `scripts/with-slot.sh cargo test --lib a_list_field_is_filterable_by_membership`. Expected: PASS. If it FAILS, stop and report to the operator: the aliases decision depends on this path. (The test cannot pass by matching everything, because of the second assertion, or by matching nothing, because of the first.)
- [ ] **Step 3: Commit.** `test(librarian): a list field is filterable through entry_filter on collection rows`.

---

### Task 2: `status_since` in the row and the column list

**Files:**
- Modify: `src/librarian/lifecycle/table.rs` (`EntryRow`, `rows`, tests)
- Modify: `src/librarian/lifecycle/schema.rs` (`TABLE_COLUMNS`)
- Test: `src/librarian/lifecycle/table.rs` (tests module), `src/librarian/tools/get_table_tests.rs`

**Interfaces:**
- Produces: `EntryRow.status_since: NaiveDate`, serialised under the key `status_since`, placed after `status`; `"status_since"` in `TABLE_COLUMNS` after `"status"`. `get_table::flat_row` lifts it with no change, so `entry_filter` can use it. Task 5 reads it.
- The name is already reserved for declared fields (`entry::RESERVED`), so no schema breaks.

- [ ] **Step 1: Write `a_row_carries_status_since_apart_from_created`** in the `table.rs` tests. Build one entry by hand with `created: 2026-10-01` and `status_since: 2026-10-20`. Assert `row.status_since == d("2026-10-20")`, `row.created == d("2026-10-01")` and `serde_json::to_value(row)["status_since"] == "2026-10-20"`. (A row that copied `created` into it would fail.)
- [ ] **Step 2: Write `entry_filter_filters_on_status_since`** in `get_table_tests.rs` using `two_entry_project()` (its helper writes `status_since` equal to `created`). Assert `{"status_since": {"gte": "2026-11-02"}}` returns `["2026-11-02-b"]` and that entry's `status_since` is `"2026-11-02"`.
- [ ] **Step 3: Run both.** Expected: compile error or FAIL (no such field).
- [ ] **Step 4: Add the field.** `EntryRow.status_since: NaiveDate` from `e.meta.status_since` in `rows`; add `"status_since"` to `TABLE_COLUMNS` in the same position as the struct field.
- [ ] **Step 5: Run the guards.** `scripts/with-slot.sh cargo test --lib table::`, `... get_table_tests`, `... every_name_in_the_three_reserved_sets_is_refused`. Expected: PASS, including the existing `table_columns_match_the_serialised_row` (it fails if the struct and `TABLE_COLUMNS` disagree).
- [ ] **Step 6: Commit.** `feat(librarian): status_since is a column of the entry table`.

---

### Task 3: One definition of "due"

**Files:**
- Modify: `src/librarian/lifecycle/entry.rs` (new function beside `class_of`, test)
- Modify: `src/librarian/lifecycle/checks.rs` (`check_entry`)

**Interfaces:**
- Produces: `pub fn revisit_due(class: StatusClass, revisit_by: Option<NaiveDate>, today: NaiveDate) -> Option<NaiveDate>`. It returns `Some(revisit_by)` only when `class.needs_revisit()` and `revisit_by < today`. `check_entry` and Task 5 both call it.

- [ ] **Step 1: Write `revisit_due_is_strictly_before_today_and_only_for_blocked_and_deferred`** in the `entry.rs` tests, with today `2026-11-11`:
  - `(Blocked, Some(2026-11-10))` gives `Some(2026-11-10)`;
  - `(Blocked, Some(2026-11-11))` gives `None`;
  - `(Deferred, Some(2026-11-01))` gives `Some(2026-11-01)`;
  - `(Open, Some(2026-11-01))` and `(Terminal, Some(2026-11-01))` give `None`;
  - `(Blocked, None)` gives `None`.
- [ ] **Step 2: Run it.** Expected: compile error (function not defined).
- [ ] **Step 3: Implement `revisit_due`** in `entry.rs`, then replace the inline `class.needs_revisit()` guard and `meta.revisit_by.filter(|r| *r < today)` in `check_entry` with one call. The pushed `Finding::RevisitDue` is unchanged.
- [ ] **Step 4: Run.** `... revisit_due_is_strictly`, then `... lifecycle::checks`. Expected: PASS, with no change to any existing check test.
- [ ] **Step 5: Prove the boundary row can fail.** Change `<` to `<=` in `revisit_due`, run Step 1's test, see it FAIL on the `2026-11-11` row, restore `<`.
- [ ] **Step 6: Commit.** `refactor(librarian): one revisit_due predicate for the doctor check and the hot view`.

---

### Task 4: The readme keys `hot_cap` and `stale_after`

**Files:**
- Modify: `src/librarian/lifecycle/schema.rs` (consts, `SCHEMA_KEYS`, `CollectionSchema`, `parse_schema`, `parse_days`, tests)
- Test: `src/librarian/lifecycle/collection/tests.rs`, `src/librarian/tools/create_lifecycle_tests.rs`

**Interfaces:**
- Produces: `KEY_HOT_CAP = "hot_cap"`, `KEY_STALE_AFTER = "stale_after"`; `SCHEMA_KEYS: [&str; 11]` (the two keys appended after `review_every`); `CollectionSchema.hot_cap: Option<u32>` and `CollectionSchema.stale_after_days: Option<u32>`; `fn parse_days(fm: &Frontmatter, key: &'static str, errs: &mut Vec<SchemaError>) -> Option<u32>` replacing `parse_review_every` (same `<n>d` grammar and the same `Malformed` message, now naming `key`). Task 5 reads both fields.
- `hot_cap` is a JSON number that is a whole number from 1 to `u32::MAX`. A string, a float, zero and a negative number are `Malformed { key: "hot_cap", .. }`.
- `collection_readme` and `doc(create)` accept the keys because both test membership in `SCHEMA_KEYS`. A readme that predates the keys parses as before, with both `None`.

- [ ] **Step 1: Write the schema tests** in `schema.rs`:
  - `hot_view_keys_parse`: `hot_cap: 5` and `stale_after: 45d` give `Some(5)` and `Some(45)`.
  - `absent_hot_view_keys_are_none`: the existing `LEDGER` text gives `None` and `None`.
  - `a_bad_hot_cap_is_malformed_naming_the_key`: for `0`, `"5"`, `2.5`, `-1` and `true`, `parse_schema` returns an error list containing `Malformed { key: "hot_cap", .. }`.
  - `a_bad_stale_after_is_malformed_naming_the_key`: the same for `45`, `"45"`, `"0d"`, `"d"`.
- [ ] **Step 2: Write `a_collection_readme_with_the_hot_view_keys_renders_and_parses`** in `collection/tests.rs` (beside `a_full_ledger_readme_renders_and_parses`, reusing its `ledger_extra`): add `"hot_cap": 5` and `"stale_after": "45d"` to `extra`, render with `collection_readme`, assert `Ok`, re-parse, assert both fields. And in `create_lifecycle_tests.rs`, `a_collection_create_accepts_hot_cap`: a `doc(create)` collection call with `extra` holding `hot_cap: 3` succeeds and `load_collection(...).schema().hot_cap == Some(3)`.
- [ ] **Step 3: Run them.** Expected: FAIL (unknown key refused, or no field).
- [ ] **Step 4: Implement.** Constants, array length, struct fields, `parse_days` (generalise `parse_review_every`; `review_every` keeps its behaviour and its `Malformed` key), a `parse_hot_cap`, and the two reads in `parse_schema`. Fix every struct literal the compiler names (`parse_schema` is the only production one).
- [ ] **Step 5: Run.** `... lifecycle::schema`, `... lifecycle::collection`, `... create_lifecycle`, then `scripts/with-slot.sh cargo test --lib tool_surface_under_budget`. Expected: PASS and the budget unchanged.
- [ ] **Step 6: Commit.** `feat(librarian): hot_cap and stale_after are collection readme keys`.

---

### Task 5: The hot view

**Files:**
- Create: `src/librarian/lifecycle/hot_view.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (add `pub mod hot_view;`), `src/librarian/lifecycle/table.rs` (`cell` and `key_as_str` become `pub(super)`)

**Interfaces:**
- Consumes: `table::rows(tree, key, false)`, `entry::revisit_due`, `EntryRow.status_since`, `CollectionSchema.{hot_cap, stale_after_days}`.
- Produces (all `Serialize`; enums serialise as snake_case words). `Key` has no `Serialize` impl, so `HotView.collection` uses `#[serde(serialize_with = "super::table::key_as_str")]`, as `EntryRow.key` does:

```rust
pub enum HotReason { Open, RevisitDue { revisit_by: NaiveDate } }
pub struct HotEntry { pub row: EntryRow, pub reason: HotReason, pub stale: bool, pub idle_days: i64 }
pub struct HotView {
    pub collection: Key, pub dir: String,
    pub entries: Vec<HotEntry>,      // at most `hot_cap`
    pub hot: usize,                  // hot entries before the cap
    pub hidden: usize,               // hot - entries.len()
    pub live: usize,                 // live entries of the collection, hot or not
    pub unclassified: usize,         // live entries whose status the readme does not declare
    pub cap: Option<u32>, pub stale_after_days: Option<u32>,
}
pub fn hot_view(tree: &LifecycleTree, collection: &Key, today: NaiveDate) -> Option<HotView>
pub fn render_markdown(view: &HotView) -> String
```

`hot_view` returns `None` when the tree has no such collection. `idle_days` is `(today - status_since).num_days()`. `stale` is true only for `HotReason::Open`, only when `stale_after_days` is `Some(n)`, and only when `status_since + n days < today`.

Rendering contract (the hook and the tests rely on it; the header is one line):

```
hot view: <key> (<dir>): <shown> of <hot> hot, <live> live. Scope: this collection only; revisit and stale flags; no doctor check ran.
- [[<entry key>]] <title>: open since <status_since>
- [[<entry key>]] <title>: open since <status_since>, STALE <idle_days>d
- [[<entry key>]] <title>: <status>, revisit was due <revisit_by>
(<hidden> more hot entries hidden by hot_cap <cap>)
(<unclassified> live entries have a status the readme does not declare)
```

The last two lines appear only when their count is above zero. Titles go through `table::cell` (a pipe, a backslash and a newline stay on one line).

- [ ] **Step 1: Write the tests** in `hot_view.rs`, with `LifecycleTree::build` and the `testkit` helpers (`doc`, `k`, `d`). The readme has statuses `{open: open, waiting: blocked, later: deferred, fixed: terminal}`; today is `2026-11-11`:
  - `open_and_due_entries_are_hot_and_the_rest_are_not`: an open entry, a blocked entry due (`revisit_by` 2026-11-01), a blocked entry not due (`2026-12-01`), a deferred entry due, a terminal entry. Hot keys are the open, the blocked-due and the deferred-due entries.
  - `a_blocked_entry_due_today_is_not_hot` (Review Focus 3): `revisit_by: 2026-11-11`.
  - `order_is_newest_status_since_first_then_key` (Review Focus 2): three open entries, two with the same `status_since`; the pair is in key order.
  - `no_hot_cap_shows_every_hot_entry` (Review Focus 1): five hot entries, `cap` `None`, `hidden` 0.
  - `hot_cap_limits_the_view_and_reports_hidden`: `hot_cap: 2`, three hot entries, `entries.len() == 2`, `hot == 3`, `hidden == 1`; the two kept are the two newest.
  - `stale_flags_only_an_open_entry_past_stale_after`: `stale_after: 30d`; an open entry with `status_since: 2026-10-12` (30 days) is not stale; one with `2026-10-11` is stale with `idle_days == 31`; a blocked-due entry is never stale; with no `stale_after` nothing is stale.
  - `an_archived_entry_is_neither_hot_nor_live`.
  - `a_live_entry_with_an_undeclared_status_is_unclassified_and_not_hot`: counted in `live` and `unclassified`.
  - `an_unknown_collection_is_none`.
  - `render_markdown_follows_the_contract`: exact header line, one `[[key]]` line per shown entry, the `STALE` suffix, and the two optional lines present only when their counts are above zero.
  - `a_title_with_a_pipe_and_a_newline_stays_on_one_line`.
- [ ] **Step 2: Run them.** Expected: compile error (module not found).
- [ ] **Step 3: Implement `hot_view` and `render_markdown`** to the contract above. Membership, order and cap are rulings R2 and R3. One pass over `rows(tree, collection, false)`; sort with `status_since` descending then key ascending.
- [ ] **Step 4: Run.** `... hot_view`, then `... no_librarian_production_read_bypasses_the_read_door`. Expected: PASS.
- [ ] **Step 5: Prove the cap test can fail.** Change `hot_cap` application to keep `n + 1`, run `hot_cap_limits_the_view_and_reports_hidden`, see it FAIL, restore.
- [ ] **Step 6: Commit.** `feat(librarian): the hot view of a collection`.

---

### Task 6: The `hot-view` command

**Files:**
- Create: `src/cli/hot_view.rs`
- Modify: `src/cli/mod.rs` (`pub mod hot_view;`), `src/main.rs` (`Commands::HotView(codescout::cli::hot_view::HotViewArgs)` and its dispatch arm)

**Interfaces:**
- Consumes: `lifecycle::collection::load_collection(root, key)`, `hot_view::{hot_view, render_markdown}`, `LoadedCollection.tree.problems`. Resolve the repository root as `src/cli/key_gate.rs` does (canonicalize `--project` or the current directory, then `lookup_git_root`).
- Produces:

```rust
pub struct HotViewArgs { pub collection: String /* positional: the collection key */, pub project: Option<PathBuf>, pub json: bool }
pub struct Outcome { pub code: i32, pub stdout: String }
pub fn execute(root: &Path, collection: &str, today: NaiveDate, json: bool) -> Outcome
pub async fn run(args: HotViewArgs) -> anyhow::Result<()>   // prints stdout, exits with code when not 0
```

Behaviour:
- `load_collection` fails (bad key, not found, ambiguous, broken readme): code 2, stdout `NOT SHOWN — <the error>`. No other output.
- It loads: code 0, stdout is `render_markdown(view)`, then one more line when `tree.problems` is not empty: `(<n> problems found while reading this folder; an entry with a problem is left out. Run the doctor for detail.)`.
- `--json`: one object with the keys `collection`, `dir`, `entries`, `hot`, `hidden`, `live`, `unclassified`, `cap`, `stale_after_days`, `problems` (a number) and `scope` (the string `this collection only; revisit and stale flags; no doctor check ran`). Each element of `entries` is the serialised `HotEntry`.

- [ ] **Step 1: Write the tests** in `src/cli/hot_view.rs`, using a temp directory with `docs/trackers/ledgers/<key>/readme.md` and entry files (no git needed for `execute`):
  - `it_shows_the_hot_entries_and_the_scope_line`: code 0, the header and a `[[key]]` line.
  - `an_empty_collection_prints_zero_hot_and_exits_0`: code 0, header says `0 of 0 hot`.
  - `an_unknown_collection_exits_2_not_shown` (Review Focus 5): code 2, stdout starts with `NOT SHOWN — `.
  - `a_badly_spelt_key_exits_2_not_shown`: key `Bad Key`.
  - `an_archived_collection_exits_2_not_shown`: the readme sits under `docs/trackers/ledgers/archive/<key>/` (`load_collection` does not look there).
  - `an_entry_with_a_problem_is_named_in_the_problems_line` (Review Focus 4): one entry file with broken frontmatter; code 0; the problems line is present with `1`; the good entry is still listed.
  - `json_has_exactly_the_documented_keys`: the key set of the parsed object equals the list above.
- [ ] **Step 2: Run them.** Expected: compile error.
- [ ] **Step 3: Implement `execute`, `run`, the clap args and the `main.rs` wiring.** The variant's doc comment says: read-only; prints the live hot entries of one lifecycle collection; exit 0 shown, exit 2 prints `NOT SHOWN — <reason>`; used by session-start hooks.
- [ ] **Step 4: Run.** `... cli::hot_view`, then `scripts/with-slot.sh cargo test --lib tool_surface_under_budget`. Expected: PASS.
- [ ] **Step 5: Smoke the binary.** `scripts/with-slot.sh cargo run -- hot-view nonesuch --project <a temp git repository>`. Expected: stdout starts `NOT SHOWN — `, exit code 2.
- [ ] **Step 6: Commit.** `feat(cli): codescout hot-view prints the hot entries of a collection`.

---

### Task 7: Documentation and landing

**Files:**
- Modify (managed docs, through `doc(...)` with `workspace=` pinned to the tree): `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (the readme example near line 312 gains `hot_cap: 12` and `stale_after: 45d`, each marked optional)
- Docs-only, on `experiments` by path-only commit: memory `tracker-lifecycle-parked-items` (a section "Hot view (plan 2026-11-15)" with rulings R1-R5 and the CLI contract); the tracker log entry for this work if one is warranted.

- [ ] **Step 1: Update the spec example** by heading and `old_string`/`new_string` through `doc(update)`.
- [ ] **Step 2: Gate the tree.** Merge `integration/identity-lifecycle-20261114` into this branch, then `./scripts/gate.sh`. Expected: `GATE EXITS -> FMT CLIPPY LEAN DEFAULT` all 0. The gate does not run the heavy Python suites or the local ONNX path (CI).
- [ ] **Step 3: Run a whole-branch review** on the most capable model before landing (operator rule: Opus reviewers; the brief tells the reviewer to load `codescout-companion:reconnaissance` and `buddy:architecture-snow-lion`).
- [ ] **Step 4: Land by the agreement.** Announce to `codescout-5f` by message, `git merge --ff-only` from `.worktrees/integration-identity-lifecycle`, announce again. Docs-only commits go on `experiments`. No push.
- [ ] **Step 5: Write the memory section** (Step 1's sibling) and commit it on `experiments` by pathspec (`-m` before `--`; read `git diff --cached --name-only` first).

## Contract for the hook (not built here)

The change belongs to `../claude-plugins/codescout-companion/hooks/session-start.mjs`, shipped separately. It runs `codescout hot-view <key> --project <root>`, uses stdout only when the exit code is 0, and adds nothing otherwise (the hook must degrade open). It makes no MCP call. Which collections to show is the hook change's decision. The hygiene cadence line (`next-sweep-due`, lines 233-242 of the hook) moves into the hygiene collection's readme under the lifecycle spec and must follow that move whatever this plan does.

## Self-review notes

- Spec coverage: Decision 1 → Task 4; Decision 2 → Task 1; Decision 3 → `--json` in Task 6; Decision 4 → Task 6 and § Contract for the hook; R1 → Tasks 4 and 5; the one-definition-of-due proposal → Task 3; scope naming → Tasks 5 and 6.
- Types: `hot_cap: Option<u32>` and `stale_after_days: Option<u32>` (Task 4) are read as such in Task 5; `EntryRow.status_since` (Task 2) is read in Task 5; `revisit_due` (Task 3) is called in Task 5.
- Not verified when this plan was written: that `doc(create)` on a collection with the new keys round-trips through the tool layer (Task 4 Step 2 proves it), and the exact wording of the Task 6 `NOT SHOWN` error texts (they come from `CollectionLoadError`'s own `Display`, which the tests assert only by prefix).
